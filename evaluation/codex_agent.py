#!/usr/bin/env python3
"""Pinned Codex exec adapter for the existing schema-1 file-proposal contract.

Only an explicitly authorized invocation calls a model. CLI version/help probes
are local. Read-only CLI flags are defense in depth, not proof of OS isolation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid

import coding_tasks as coding
import cross_source as cross
import provider_usage as metering
from coding_agent import INSTRUCTIONS

PROMPT = INSTRUCTIONS + "\n\nUntrusted task data:\n"
SCHEMA = Path(__file__).resolve().with_name("codex_proposal.schema.json")
SUPPORTED_VERSION = "codex-cli 0.162.1"
REQUIRED_FLAGS = ("--json", "--output-schema", "--cd", "--ephemeral", "--ignore-user-config",
                  "--ignore-rules", "--sandbox", "--model", "--config", "--skip-git-repo-check", "--strict-config")
# Fixed grants; no caller-selected config/profile/MCP/resume arguments.
CONFIG = ("project_doc_max_bytes=0", 'web_search="disabled"', "features.shell_tool=false",
          "features.unified_exec=false", "features.apply_patch_freeform=false", "features.remote_plugin=false",
          "features.apps=false",
          "features.skill_mcp_dependency_install=false", "features.shell_snapshot=false",
          "history.persistence=\"none\"", "feedback.enabled=false")
ENV_KEYS = ("PATH", "SYSTEMROOT", "WINDIR", "LANG", "OPENAI_API_KEY", "CODEX_API_KEY")
MAX_EVENT_BYTES = coding.MAX_RESPONSE_BYTES
MAX_STREAM_BYTES = 8_000_000
MAX_REQUEST_BYTES = 8_000_000


class AdapterFailure(ValueError):
    def __init__(self, status="validation_failed"):
        super().__init__("Codex invocation did not satisfy the pinned proposal contract")
        self.status = status


class Arguments(argparse.ArgumentParser):
    def error(self, _message):
        raise AdapterFailure("rejected")


def parser() -> argparse.ArgumentParser:
    result = Arguments(description=__doc__, allow_abbrev=False)
    result.add_argument("--codex-bin", type=Path, required=True)
    result.add_argument("--codex-sha256", required=True)
    result.add_argument("--codex-version", required=True)
    result.add_argument("--model", required=True)
    result.add_argument("--reasoning", choices=("none", "minimal", "low", "medium", "high", "xhigh", "max"), required=True)
    result.add_argument("--timeout", type=int, default=600)
    result.add_argument("--allow-hosted", action="store_true")
    return result


def configuration(args) -> dict:
    path = args.codex_bin
    if (not path.is_absolute() or path.resolve() != path or not path.is_file()
            or not os.access(path, os.X_OK) or coding.hash_file(path) != args.codex_sha256
            or not re.fullmatch(r"[0-9a-f]{64}", args.codex_sha256)
            or args.codex_version != SUPPORTED_VERSION
            or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:/-]{0,127}", args.model)
            or type(args.timeout) is not int or not 1 <= args.timeout <= 86400
            or args.reasoning not in ("none", "minimal", "low", "medium", "high", "xhigh", "max")
            or args.allow_hosted is not True):
        raise AdapterFailure("rejected")
    return {"adapter_sha256": coding.hash_file(Path(__file__).resolve()),
            "codex_binary_sha256": args.codex_sha256, "codex_version": args.codex_version,
            "requested_model": args.model, "reasoning": args.reasoning, "timeout": args.timeout,
            "schema_sha256": coding.hash_file(SCHEMA),
            "prompt_sha256": hashlib.sha256(PROMPT.encode("utf-8")).hexdigest(),
            "flags": list(REQUIRED_FLAGS), "config": list(CONFIG),
            "environment_policy_sha256": cross.digest(ENV_KEYS)}


def visible_request(request: dict) -> dict:
    allowed = {"schema_version", "task", "files", "editable_files", "context", "response_contract", "repair"}
    if (not isinstance(request, dict) or set(request) - allowed or type(request.get("schema_version")) is not int or request.get("schema_version") != 1
            or not isinstance(request.get("task"), str) or not request["task"].strip()
            or not isinstance(request.get("files"), dict) or not request["files"]
            or not isinstance(request.get("editable_files"), list) or not request["editable_files"]
            or len(set(request["editable_files"])) != len(request["editable_files"])
            or set(request["editable_files"]) - set(request["files"])
            or request.get("context") is not None and not isinstance(request["context"], dict)):
        raise AdapterFailure("rejected")
    for name, content in request["files"].items():
        coding.relative_file(name)
        if not isinstance(content, str):
            raise AdapterFailure("rejected")
        content.encode("utf-8")
    for name in request["editable_files"]:
        coding.relative_file(name)
    repair = request.get("repair")
    if repair is not None:
        if (not isinstance(repair, dict) or set(repair) != {"attempt", "max_attempts", "feedback"}
                or type(repair["attempt"]) is not int or type(repair["max_attempts"]) is not int
                or not 2 <= repair["attempt"] <= repair["max_attempts"] <= coding.MAX_ATTEMPTS):
            raise AdapterFailure("rejected")
        feedback = repair["feedback"]
        if (not isinstance(feedback, dict) or set(feedback) != {"independent_checks_passed", "messages"}
                or type(feedback["independent_checks_passed"]) is not bool
                or not isinstance(feedback["messages"], list) or not 1 <= len(feedback["messages"]) <= 8
                or any(not isinstance(item, str) or not item.strip() or len(item.encode("utf-8")) > 512
                       for item in feedback["messages"])):
            raise AdapterFailure("rejected")
    value = {key: request.get(key) for key in ("task", "files", "editable_files", "context", "repair")}
    if len(json.dumps(value).encode("utf-8")) > MAX_REQUEST_BYTES:
        raise AdapterFailure("rejected")
    return value


def child_environment() -> dict:
    return {key: os.environ[key] for key in ENV_KEYS if key in os.environ}


def stop(process) -> None:
    # A separate group allows killing descendants even after the leader exits.
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        elif process.poll() is None:
            process.kill()
    except ProcessLookupError:
        pass
    process.wait()


def probe(args, directory: Path, env: dict) -> None:
    for flags, expected in ((["--version"], args.codex_version), (["exec", "--help"], None)):
        process = subprocess.Popen([str(args.codex_bin), *flags], cwd=directory, env=env,
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            start_new_session=os.name == "posix")
        try:
            output, _ = process.communicate(timeout=min(10, args.timeout))
            if process.returncode or len(output) > 100_000:
                raise AdapterFailure("rejected")
            text = output.decode("utf-8")
            if expected is not None and text.strip() != expected:
                raise AdapterFailure("rejected")
            if expected is None and any(flag not in text for flag in REQUIRED_FLAGS):
                raise AdapterFailure("rejected")
        finally:
            stop(process)
    configuration(args)  # Reject binary substitution during probing.


def execute(args, prompt: bytes, directory: Path, env: dict, event: dict, recorder) -> dict:
    command = [str(args.codex_bin), "exec", "--json", "--strict-config", "--output-schema", str(directory / "proposal.schema.json"),
               "--cd", str(directory), "--ephemeral", "--ignore-user-config", "--ignore-rules",
               "--sandbox", "read-only", "--model", args.model, "--skip-git-repo-check"]
    for value in (*CONFIG, "model_reasoning_effort=" + json.dumps(args.reasoning)):
        command += ["--config", value]
    command.append("-")
    process = subprocess.Popen(command, cwd=directory, env=env, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, start_new_session=os.name == "posix")
    pending = queue.Queue(maxsize=16)

    def reader():
        try:
            while True:
                line = process.stdout.readline(MAX_EVENT_BYTES + 1)
                pending.put(line)
                if not line or len(line) > MAX_EVENT_BYTES:
                    break
        except OSError:
            pending.put(b"")

    def writer():
        try:
            process.stdin.write(prompt)
            process.stdin.close()
        except (OSError, BrokenPipeError):
            pass

    reading = threading.Thread(target=reader, daemon=True)
    writing = threading.Thread(target=writer, daemon=True)
    reading.start()
    writing.start()
    messages, terminal, observed_model = [], [], None
    start = time.monotonic()
    size = 0
    try:
        while True:
            remaining = args.timeout - (time.monotonic() - start)
            if remaining <= 0:
                raise AdapterFailure("timed_out")
            try:
                line = pending.get(timeout=min(0.1, remaining))
            except queue.Empty:
                continue
            if not line:
                break
            size += len(line)
            if len(line) > MAX_EVENT_BYTES or size > MAX_STREAM_BYTES:
                raise AdapterFailure()
            value = json.loads(line)
            if not isinstance(value, dict) or not isinstance(value.get("type"), str):
                raise AdapterFailure()
            model = value.get("model")
            if model is not None:
                if model != args.model:
                    raise AdapterFailure()
                observed_model = model
            kind = value["type"]
            item = value.get("item", {})
            if not isinstance(item, dict):
                raise AdapterFailure()
            if kind == "refusal" or item.get("type") == "refusal":
                raise AdapterFailure("refused")
            if kind.startswith("item.") and item.get("type") not in ("agent_message", "reasoning"):
                raise AdapterFailure("rejected")
            if kind == "item.completed" and item.get("type") == "agent_message":
                messages.append(item.get("text"))
            if kind == "turn.completed":
                terminal.append(value)
                if len(terminal) == 1:
                    event.update(metering.tokens("openai", value))
                    event["provider_request_count"] = None  # A turn may contain several requests.
                else:
                    event.update(input_tokens=None, output_tokens=None, total_tokens=None)
                recorder.save()
            if kind in ("turn.failed", "error"):
                raise AdapterFailure("incomplete")
            if kind not in ("thread.started", "turn.started", "turn.completed", "item.started",
                            "item.updated", "item.completed"):
                raise AdapterFailure()
        process.wait(timeout=max(0.01, args.timeout - (time.monotonic() - start)))
        if process.returncode or not terminal or len(messages) != 1 or not isinstance(messages[0], str):
            raise AdapterFailure("incomplete")
        answer = json.loads(messages[0])
        if not isinstance(answer, dict) or set(answer) != {"schema_version", "files", "summary"}:
            raise AdapterFailure()
        used = terminal[0].get("usage") if len(terminal) == 1 else None
        cached = metering.integer(used.get("cached_input_tokens")) if isinstance(used, dict) else None
        if cached is not None and event["input_tokens"] is not None and cached > event["input_tokens"]:
            cached = None
        return {"answer": answer, "observed_model": observed_model, "cached_input_tokens": cached}
    finally:
        stop(process)
        process.stdout.close()
        if not process.stdin.closed:
            process.stdin.close()
        # Drain a stopped reader rather than leave a blocked producer behind.
        while reading.is_alive():
            try:
                pending.get(timeout=0.1)
            except queue.Empty:
                pass
            reading.join(timeout=0.1)
        writing.join(timeout=1)


def generate(args, request) -> dict:
    if os.name != "posix":
        raise AdapterFailure("rejected")
    pins = configuration(args)
    visible = visible_request(request)
    recorder = metering.Recorder(os.environ.get("LORE_USAGE_LEDGER"))
    env = child_environment()
    with tempfile.TemporaryDirectory(prefix="lore-codex-proposal-") as temporary:
        directory = Path(temporary).resolve()
        probe(args, directory, env)  # No ledger writes for rejected CLI preflight.
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        # shortcut: strict schemas name every editable file, revisit for large sparse proposals.
        schema["properties"]["files"] = {"type": "object", "additionalProperties": False,
            "properties": {name: {"type": "string"} for name in visible["editable_files"]},
            "required": visible["editable_files"]}
        (directory / "proposal.schema.json").write_text(json.dumps(schema), encoding="utf-8")
        resolved_schema_sha256 = coding.hash_file(directory / "proposal.schema.json")
        start = time.monotonic()
        identity = str(uuid.uuid4())
        event = {"id": identity, "call_id": identity, "operation": "coding_proposal", "provider": "openai",
                 "model": args.model, "attempt": 1, "elapsed_ms": 0, "http_status": None,
                 "provider_request_count": None, "input_tokens": None, "output_tokens": None,
                 "total_tokens": None, "billed_cost_usd": None, "billing_source": None,
                 "status": "started", "cache_hit": False}
        recorder.events.append(event)
        recorder.save()
        old_term = signal.getsignal(signal.SIGTERM)

        def cancelled(_signal, _frame):
            raise KeyboardInterrupt

        signal.signal(signal.SIGTERM, cancelled)
        try:
            generated = execute(args, (PROMPT +
                                      json.dumps(visible, ensure_ascii=False)).encode("utf-8"), directory, env, event, recorder)
            answer = coding.validate_agent_response(generated["answer"], request["editable_files"])
            event.update(status="completed", elapsed_ms=int((time.monotonic() - start) * 1000))
            saved = recorder.save("completed")
            answer.update(provider_model=generated["observed_model"], usage=metering.normalized(saved["summary"]),
                usage_ledger=saved, adapter_receipt={**pins, "call_id": identity,
                    "observed_model": generated["observed_model"], "effective_backend_revision": None,
                    "cached_input_tokens": generated["cached_input_tokens"], "adapter_retries": 0,
                    "provider_retries": None,
                    "resolved_schema_sha256": resolved_schema_sha256})
            return answer
        except BaseException as error:
            event.update(status=getattr(error, "status", "cancelled" if isinstance(error, KeyboardInterrupt)
                         else "timed_out" if isinstance(error, subprocess.TimeoutExpired) else "validation_failed"),
                         elapsed_ms=int((time.monotonic() - start) * 1000))
            recorder.save("cancelled" if event["status"] == "cancelled" else "failed")
            raise
        finally:
            signal.signal(signal.SIGTERM, old_term)


def main(argv=None) -> int:
    try:
        args = parser().parse_args(argv)
        raw = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
        if len(raw) > MAX_REQUEST_BYTES:
            raise AdapterFailure("rejected")
        request = json.loads(raw)
        answer = generate(args, request)
        print(json.dumps(answer, ensure_ascii=False))
        return 0
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError, KeyboardInterrupt):
        print('Codex adapter failed; private input and provider output omitted', file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
