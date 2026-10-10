#!/usr/bin/env python3
"""Run actual coding-task comparisons with sources, --fast, and intelligent Lore.

Extends benchmark.py and cross_source.py. The operator supplies a real coding
agent command; no simulated model results are used by `run`. Commands and
generated implementations run with the operator's privileges, not in a sandbox.
"""
from __future__ import annotations

import argparse
import hashlib
from functools import lru_cache
import json
import math
from pathlib import Path, PurePosixPath
import re
import random
import secrets
import shutil
import sqlite3
import subprocess
import sys
import time

import benchmark as bench
import cross_source as cross

ROOT = Path(__file__).resolve().parent
DEFAULT_CASES = ROOT / "corpora" / "coding-tasks" / "cases.json"
SETUPS = ("baseline", "fast", "intelligent")
MAX_RESPONSE_BYTES = 2_000_000
MAX_ATTEMPTS = 5
DEFAULT_REPAIR_MESSAGE = "Independent checks did not all pass. Reconsider the task and supplied project constraints."


def hash_file(path: Path) -> str:
    cross.reject_symlink_path(path)
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative_file(value: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError("File paths must be nonempty relative POSIX paths")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or str(path) != value or ":" in value:
        raise ValueError("File path escapes or is not canonical")
    return value


def load_cases(path: Path) -> dict:
    data = cross.read_json(path)
    if data.get("schema_version") != 1 or not isinstance(data.get("cases"), list) or not data["cases"]:
        raise ValueError("Expected nonempty schema_version: 1 coding-task cases")
    identities = set()
    for case in data["cases"]:
        identity = case.get("id")
        if not isinstance(identity, str) or not identity or not all(c.isalnum() or c in "-_" for c in identity) or identity in identities:
            raise ValueError("Cases need unique simple id strings")
        identities.add(identity)
        for key in ("task", "project"):
            if not isinstance(case.get(key), str) or not case[key].strip():
                raise ValueError(f"Case lacks {key}")
        files = case.get("editable_files")
        if not isinstance(files, list) or not files or len(set(files)) != len(files):
            raise ValueError("Each case needs unique editable_files")
        for filename in files:
            relative_file(filename)
        constraints = case.get("critical_constraints")
        if not isinstance(constraints, list) or not constraints or any(not isinstance(item, str) or not item.strip() for item in constraints) or len(set(constraints)) != len(constraints):
            raise ValueError("Each case needs reviewed critical_constraints")
        command = case.get("test_command")
        if not isinstance(command, list) or not command or any(not isinstance(item, str) or not item for item in command):
            raise ValueError("test_command must be an argv array, never a shell string")
        if ("base_project" in case) == ("source_root" in case):
            raise ValueError("Specify either base_project plus overlay or source_root")
        if "source_root" in case and (not isinstance(case["source_root"], str) or not case["source_root"]):
            raise ValueError("source_root must identify an input directory")
        for field in ("verification_files", "expected_sources"):
            values = case.get(field, [])
            if not isinstance(values, list) or any(not isinstance(value, str) or not value for value in values) or len(set(values)) != len(values):
                raise ValueError(f"{field} must be a list of unique nonempty strings")
        if "base_project" in case:
            cross.selected_projects([case["base_project"]])
            if not isinstance(case.get("overlay"), str):
                raise ValueError("Bundled coding tasks need an overlay directory")
        feedback = case.get("repair_feedback", {})
        if (not isinstance(feedback, dict) or len(feedback) > 50
                or any(not isinstance(key, str) or not key or not isinstance(value, str)
                       or not value.strip() or len(value.encode("utf-8")) > 512
                       for key, value in feedback.items())):
            raise ValueError("repair_feedback must contain at most 50 predetermined messages of at most 512 bytes")
    return data


def bundled_fingerprints() -> set[str]:
    fingerprints = set()
    manifests = (DEFAULT_CASES, ROOT / "corpora" / "shared-intelligence" / "cases.json")
    for manifest in manifests:
        for case in cross.read_json(manifest)["cases"]:
            base = cross.fingerprint(cross.CORPORA / case["base_project"] / "initial")
            overlay = cross.fingerprint(manifest.parent / case["overlay"])
            fingerprints.add(cross.digest(base["files_sha256"] | overlay["files_sha256"]))
    candidates = ROOT / "corpora" / "coding-real-v1" / "manifest.json"
    if candidates.is_file():
        fingerprints.update(candidate_fingerprints(hash_file(candidates)))
    return fingerprints


@lru_cache(maxsize=4)
def candidate_fingerprints(manifest_sha256: str) -> frozenset[str]:
    # Import lazily: the corpus builder uses this runner's source-copy contract.
    import real_coding_corpus as real
    manifest = real.load_manifest()
    if hash_file(real.CORPUS / "manifest.json") != manifest_sha256:
        raise ValueError("Public candidate manifest changed during fingerprinting")
    known = set()
    for task in manifest["tasks"]:
        files = dict(manifest["projects"][task["project"]]["manifest"]["files_sha256"])
        raw = (real.CORPUS / "upstream" / task["project"] / task["file"]).read_bytes().decode("utf-8")
        files[task["file"]] = hashlib.sha256(real.mutate(raw, task["symbol"]).encode("utf-8")).hexdigest()
        known.add(cross.digest(files))
    return frozenset(known)


def prepare(output: Path, cases_path: Path) -> dict:
    output, cases_path = cross.selected_path(output), cross.selected_path(cases_path)
    data = load_cases(cases_path)
    if output.exists():
        raise ValueError("Use a fresh output directory")
    # Validate every original source before writing anything.
    for case in data["cases"]:
        source = cases_path.parent / case.get("overlay", case.get("source_root", ""))
        cross.fingerprint(source)
    output.mkdir(parents=True)
    cases = []
    for case in data["cases"]:
        destination = output / "snapshots" / case["id"]
        if "base_project" in case:
            cross.prepare_project(case["base_project"], destination)
            cross.copy_snapshot(cases_path.parent / case["overlay"], destination)
        else:
            cross.copy_snapshot(cases_path.parent / case["source_root"], destination)
        for filename in case["editable_files"]:
            if not (destination / filename).is_file():
                raise ValueError(f"Editable file does not exist: {filename}")
        _, checker_files = verification_command(case, destination, cases_path.parent)
        cases.append({"case": case, "source_manifest": cross.fingerprint(destination),
                      "checker_files_sha256": checker_files,
                      "source_root": f"snapshots/{case['id']}"})
    known = bundled_fingerprints()
    recognized = sorted({entry["source_manifest"]["sha256"] for entry in cases} & known)
    fixture_only = data.get("fixture_only") is not False or any("base_project" in case for case in data["cases"]) or bool(recognized)
    report = {"schema_version": 1, "phase": "preparation_only", "inference_calls": 0,
              "fixture_only": fixture_only, "held_out": data.get("held_out") is True and not fixture_only,
              "independent_projects": data.get("independent_projects") is True and not fixture_only,
              "recognized_fixture_fingerprints": recognized,
              "cases_manifest": str(cases_path), "cases_manifest_sha256": hash_file(cases_path), "cases": cases}
    cross.write_json(output / "prepared.json", report)
    return report


def registry_content(project: Path) -> dict:
    # Intelligent caches live outside state.db. Hashing the whole database also
    # detects in-place modifications that leave IDs and row counts unchanged.
    cross.reject_symlink_path(project / ".lore" / "state.db")
    return cross.registry_state(project)


def usage(value: dict | None, *, calls: int | None = None) -> dict:
    value = value if isinstance(value, dict) else {}
    result = {key: value.get(key) for key in ("model_calls", "input_tokens", "output_tokens", "billed_cost_usd", "billing_source")}
    if calls is not None:
        result["model_calls"] = calls
    for key in ("model_calls", "input_tokens", "output_tokens"):
        number = result[key]
        if number is not None and (type(number) is not int or number < 0):
            raise ValueError(f"Invalid measured {key}")
    cost = result["billed_cost_usd"]
    if cost is not None and (type(cost) not in (int, float) or not math.isfinite(cost) or cost < 0 or not isinstance(result["billing_source"], str) or not result["billing_source"].strip()):
        raise ValueError("Measured billing requires a finite nonnegative amount and billing_source")
    if cost is None:
        result["billing_source"] = None
    return result


def add_usage(values: list[dict]) -> dict:
    """A missing component remains unknown; calls never imply dollars or tokens."""
    return {key: sum(value[key] for value in values) if all(value.get(key) is not None for value in values) else None
            for key in ("model_calls", "input_tokens", "output_tokens", "billed_cost_usd")}


def no_inference() -> dict:
    return usage({"model_calls": 0, "input_tokens": 0, "output_tokens": 0,
                  "billed_cost_usd": 0.0, "billing_source": "No inference performed"})


def collect_context(binary: str, project: Path, task: str, setup: str, timeout: int, max_tokens: int) -> dict:
    if setup not in ("fast", "intelligent"):
        raise ValueError("Context collection requires fast or intelligent")
    before = registry_content(project)
    arguments = ["context", task, "--max-tokens", str(max_tokens)]
    if setup == "fast":
        arguments.append("--fast")
    else:
        # Preserve the 0.5 comparison contract as 0.6 defaults to schema 4.
        arguments.extend(["--schema-version", "3"])
    response, elapsed = bench.subprocess_json(binary, project, *arguments, timeout=timeout)
    repeated, repeat_seconds = bench.subprocess_json(binary, project, *arguments, timeout=timeout)
    citations = cross.resolve_citations(binary, project, cross.cited_ids(response) | cross.cited_ids(repeated), timeout)
    after = registry_content(project)
    result = {"response": response, "response_sha256": cross.digest(response),
            "repeat_response": repeated, "repeat_response_sha256": cross.digest(repeated),
            "elapsed_seconds": elapsed, "repeat_elapsed_seconds": repeat_seconds,
            "usage": usage(response.get("usage"), calls=cross.model_calls(response)) if cross.model_calls(response) else no_inference(),
            "repeat_usage": usage(repeated.get("usage"), calls=cross.model_calls(repeated)) if cross.model_calls(repeated) else no_inference(),
            "mode": "fast" if setup == "fast" else response.get("mode"),
            "cache_status": response.get("cache_status"), "repeat_cache_status": repeated.get("cache_status"),
            "registry_before": before, "registry_after": after, "citation_integrity": citations,
            "inspection_sources": sorted(inspection_sources(response))}
    result["checks"] = context_checks(result, task, setup)
    return result


def resolver_complete(record: dict, expected: set[str]) -> bool:
    try:
        results = record["results"]
        if cross.id_set([item["evidence_id"] for item in results]) != expected:
            return False
        if record["checked"] != len(expected) or record["resolvable"] != len(expected) or record["failures"]:
            return False
        return all(isinstance(item.get("response"), dict)
                   and item["response"].get("evidence_id", item["response"].get("id")) == item["evidence_id"]
                   and item.get("response_sha256") == cross.digest(item["response"])
                   and item.get("error") is None for item in results)
    except (KeyError, TypeError, ValueError):
        return False


def context_checks(record: dict, task: str, setup: str) -> dict:
    """Derive gates from complete records again during assessment."""
    response, repeated = record["response"], record["repeat_response"]
    before, after = record["registry_before"], record["registry_after"]
    fast = setup == "fast"
    schema = 2 if fast else 3
    return {
        "response_contract": all(answer.get("schema_version") == schema and answer.get("task") == task
                                 and (fast or answer.get("mode") in ("intelligent", "fast_fallback")) for answer in (response, repeated)),
        "response_hashes": record["response_sha256"] == cross.digest(response) and record["repeat_response_sha256"] == cross.digest(repeated),
        "source_registry_preserved": cross.valid_registry(before) and cross.valid_registry(after) and before == after,
        "citations_resolve": resolver_complete(record["citation_integrity"], cross.cited_ids(response) | cross.cited_ids(repeated)),
        "fast_model_free": not fast or cross.model_calls(response) == cross.model_calls(repeated) == 0,
        "fast_repeat_identical": not fast or response == repeated,
        "intelligent_repeat_reuses_guidance": fast or response.get("mode") != "intelligent" or (
            repeated.get("mode") == "intelligent" and cross.model_calls(repeated) == 0
            and repeated.get("cache_status") == "hit"
            and response.get("brief", {}).get("preferred_approach") == repeated.get("brief", {}).get("preferred_approach")),
    }


def answer_evidence_ids(response: dict) -> set[str]:
    return set(re.findall(r"\b(?:ev|ne)_[A-Za-z0-9_-]+\b", json.dumps({
        "summary": response["summary"], "files": response["files"]})))


def inspection_sources(value) -> set[str]:
    result = set()
    if isinstance(value, dict):
        # Source locators are a retrieval-coverage proxy, never semantic entailment.
        for field in ("source", "source_locator"):
            if isinstance(value.get(field), str):
                result.add(value[field])
        for child in value.values():
            result.update(inspection_sources(child))
    elif isinstance(value, list):
        for child in value:
            result.update(inspection_sources(child))
    return result


def validate_agent_response(response: dict, editable_files: list[str]) -> dict:
    if not isinstance(response, dict) or response.get("schema_version") != 1:
        raise ValueError("Agent must return a schema_version: 1 JSON object")
    files = response.get("files")
    if not isinstance(files, dict) or not files or set(files) - set(editable_files):
        raise ValueError("Agent changes must be a nonempty subset of editable_files")
    for filename, content in files.items():
        relative_file(filename)
        if not isinstance(content, str) or len(content.encode("utf-8")) > 500_000:
            raise ValueError("Agent file content must be UTF-8 text at most 500 KB")
    if not isinstance(response.get("summary"), str) or not response["summary"].strip():
        raise ValueError("Agent must provide a nonempty implementation summary")
    response["usage"] = usage(response.get("usage"))
    return response


def invoke_json(command: list[str], cwd: Path, timeout: int, request: dict | None = None,
                *, env: dict[str, str] | None = None) -> tuple[dict, float]:
    start = time.monotonic()
    process = subprocess.run(command, cwd=cwd, input=json.dumps(request) if request is not None else None,
                             capture_output=True, text=True, encoding="utf-8", timeout=timeout,
                             shell=False, env=env)
    elapsed = round(time.monotonic() - start, 3)
    if process.returncode != 0:
        raise ValueError(f"Execution failed with exit {process.returncode}; provider output is not copied into reports")
    if len(process.stdout.encode("utf-8")) > MAX_RESPONSE_BYTES:
        raise ValueError("Execution JSON response exceeds 2 MB")
    try:
        value = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("Execution did not return the documented JSON response") from error
    if not isinstance(value, dict):
        raise ValueError("Execution must return a JSON object")
    return value, elapsed


def verification_command(case: dict, workspace: Path, manifest_dir: Path) -> tuple[list[str], dict]:
    replacements = {"{python}": sys.executable, "{workspace}": str(workspace), "{manifest}": str(manifest_dir)}
    command = []
    for item in case["test_command"]:
        for key, replacement in replacements.items():
            item = item.replace(key, replacement)
        command.append(item)
    executable = shutil.which(command[0])
    if not executable:
        raise ValueError("Verification executable was not found")
    files = {str(Path(executable).resolve()): hash_file(Path(executable).resolve())}
    for argument in command[1:]:
        candidate = Path(argument)
        if candidate.is_absolute() and candidate.is_file():
            files[str(candidate)] = hash_file(candidate)
    # Additional imported helpers or external test data can be bound explicitly.
    for name in case.get("verification_files", []):
        candidate = (manifest_dir / name).absolute()
        files[str(candidate)] = hash_file(candidate)
    return command, files


def validate_checker_result(result: dict, expected_contract: list | None = None) -> list:
    checks = result.get("checks")
    if result.get("schema_version") != 1 or not isinstance(checks, list) or not 1 <= len(checks) <= 1000:
        raise ValueError("Test command must return nonempty schema_version: 1 checks")
    names = set()
    for check in checks:
        if not isinstance(check, dict) or not isinstance(check.get("id"), str) or not check["id"] or check["id"] in names or type(check.get("passed")) is not bool or check.get("kind") not in ("correctness", "constraint"):
            raise ValueError("Tests need unique ids, boolean passed, and correctness/constraint kind")
        names.add(check["id"])
    contract = sorted((check["id"], check["kind"]) for check in checks)
    if expected_contract is not None and contract != expected_contract:
        raise ValueError("Independent checker coverage changed between attempts")
    return contract


def validate_checker_record(result: dict, case: dict, workspace: Path, manifest_dir: Path,
                            expected_contract: list | None = None) -> list:
    contract = validate_checker_result(result, expected_contract)
    command, files = verification_command(case, workspace, manifest_dir)
    if result.get("provenance") != {"argv": command, "files_sha256": files}:
        raise ValueError("Checker program identities differ from the case's declared external checkers")
    return contract


def validate_prepared_checker(entry: dict, workspace: Path, manifest_dir: Path) -> None:
    expected = entry.get("checker_files_sha256")
    if expected is None:
        return  # Historical artifacts predate preparation-time checker pins.
    _, actual = verification_command(entry["case"], workspace, manifest_dir)
    if not expected or actual != expected:
        raise ValueError("External checker bytes changed after source/task preparation")


def execute_checks(case: dict, workspace: Path, manifest_dir: Path, timeout: int,
                   *, env: dict[str, str] | None = None) -> tuple[dict, float]:
    command, files = verification_command(case, workspace, manifest_dir)
    result, elapsed = invoke_json(command, workspace, timeout,
                                   **({"env": env} if env is not None else {}))
    if any(hash_file(Path(path)) != expected for path, expected in files.items()):
        raise ValueError("Verification program changed while executing candidate code")
    validate_checker_result(result)
    result["provenance"] = {"argv": command, "files_sha256": files}
    return result, elapsed


def config_for_case(args: argparse.Namespace, case: dict, project: Path) -> dict:
    config = bench.config_for(project, case["project"], args.provider, args.model,
                              args.decision_provider, args.decision_model, args.allow_hosted, None, True,
                              generative_base_url=args.generative_base_url,
                              decision_base_url=args.decision_base_url, reasoning=bench.reasoning_from_args(args))
    config["schema_version"] = 2
    if "base_project" in case:
        config["imports"] = [
            {"id": "implementation", "kind": "openwiki", "path": "./openwiki"},
            {"id": "agent-memory", "kind": "engram", "path": "./imports/engram.json", "project": case["base_project"]},
            {"id": "work-history", "kind": "beads", "path": "./imports/beads.jsonl", "include_memories": False}]
    else:
        config["imports"] = case.get("imports", [])
    if args.embedding_model:
        config["models"]["embedding"] = {"provider": args.provider, "model": args.embedding_model, "enabled": True}
    return config


def agent_request(case: dict, source_files: dict, context: dict | None) -> dict:
    """One shared, reconstructible input contract for every comparison arm."""
    return {"schema_version": 1, "task": case["task"], "files": source_files,
            "editable_files": case["editable_files"], "context": context["response"] if context else None,
            "response_contract": {"schema_version": 1, "files": "Map of allowed paths to complete proposed UTF-8 contents",
                                  "summary": "Explain the implementation and material assumptions", "usage": "Actual model_calls/input_tokens/output_tokens/billed_cost_usd/billing_source, or null for unknown fields"}}


def repair_policy(args: argparse.Namespace) -> dict:
    attempts = getattr(args, "max_attempts", 1)
    seconds = getattr(args, "attempt_budget_seconds", None) or min(86400, args.timeout * attempts * 2)
    if type(attempts) is not int or not 1 <= attempts <= MAX_ATTEMPTS:
        raise ValueError(f"max_attempts must be 1..{MAX_ATTEMPTS}")
    if type(seconds) is not int or not 1 <= seconds <= 86400:
        raise ValueError("attempt_budget_seconds must be 1..86400")
    return {"max_attempts": attempts, "attempt_budget_seconds": seconds,
            "feedback_policy": "predetermined-manifest-messages-v1"}


def sanitized_feedback(case: dict, checks: dict) -> dict:
    """Do not pass checker stdout, IDs, exceptions or answer keys to an agent."""
    failed = [item for item in checks["checks"] if not item["passed"]]
    messages = sorted({case.get("repair_feedback", {}).get(item["id"], DEFAULT_REPAIR_MESSAGE)
                       for item in failed})
    return {"independent_checks_passed": not failed, "messages": messages[:8]}


def repair_request(case: dict, files: dict, context: dict | None, previous: dict,
                   number: int, policy: dict) -> dict:
    request = agent_request(case, files, context)
    request["repair"] = {"attempt": number, "max_attempts": policy["max_attempts"],
                         "feedback": sanitized_feedback(case, previous)}
    return request


def summed_coding_usage(attempts: list[dict]) -> dict:
    result = add_usage([attempt["response"]["usage"] for attempt in attempts])
    result["billing_source"] = "Sum of reported coding-attempt billing" if result["billed_cost_usd"] is not None else None
    return usage(result)


def run_attempts(command: list[str], case: dict, source_files: dict, context: dict | None,
                 workspace: Path, output: Path, sample_id: str, manifest_dir: Path,
                 timeout: int, policy: dict, environment) -> dict:
    """Bounded proposal/check loop; every tested revision is retained separately.

    The checker remains outside agent input. A failed process is an incomplete
    experiment, never silently converted to a successful or scored attempt.
    Completed attempts are checkpointed before another subprocess is invoked.
    """
    started = time.monotonic()
    current, combined, attempts = dict(source_files), {}, []
    previous, checker_contract = None, None
    for number in range(1, policy["max_attempts"] + 1):
        remaining = policy["attempt_budget_seconds"] - (time.monotonic() - started)
        if remaining < 1:
            break
        request = (agent_request(case, current, context) if number == 1 else
                   repair_request(case, current, context, previous, number, policy))
        request_sha256 = cross.digest(request)
        agent_cwd = output / "agent-runs" / sample_id / str(number)
        agent_cwd.mkdir(parents=True)
        invocation_path = output / "attempt-invocations" / sample_id / f"{number}.json"
        invocation = {"number": number, "request_sha256": request_sha256, "status": "started",
                      "stage": "agent", "elapsed_seconds": None, "usage": usage(None), "error_type": None}
        invocation_started = time.monotonic()
        cross.write_json(invocation_path, invocation)
        try:
            response, coding_seconds = invoke_json(command, agent_cwd, min(timeout, max(1, int(remaining))),
                                                    request, **environment(workspace, "agent"))
            invocation["stage"] = "validate_agent_response"
            response = validate_agent_response(response, case["editable_files"])
            invocation["usage"] = response["usage"]
            current.update(response["files"])
            combined.update(response["files"])
            for filename, content in combined.items():
                target = workspace / filename
                cross.reject_symlink_path(target)
                target.write_bytes(content.encode("utf-8"))
            remaining = policy["attempt_budget_seconds"] - (time.monotonic() - started)
            if remaining < 1:
                raise ValueError("Attempt wall-time budget exhausted before independent verification")
            invocation["stage"] = "verification"
            checks, verification_seconds = execute_checks(case, workspace, manifest_dir,
                min(timeout, max(1, int(remaining))), **environment(workspace, "verification"))
            checker_contract = validate_checker_record(checks, case, workspace, manifest_dir, checker_contract)
            invocation["stage"] = "source_integrity"
            actual = cross.fingerprint(workspace)
            expected = {name: hashlib.sha256(value.encode("utf-8")).hexdigest() for name, value in current.items()}
            if actual["files_sha256"] != expected:
                raise ValueError("Coding attempt or checker changed protected sources or unproposed bytes")
            revision_root = f"attempts/{sample_id}/{number}"
            cross.copy_snapshot(workspace, output / revision_root)
            attempt = {"number": number, "request_sha256": request_sha256, "response": response,
                       "response_sha256": cross.digest(response), "tests": checks,
                       "implementation_manifest": actual, "implementation_root": revision_root,
                       "coding_seconds": coding_seconds, "verification_seconds": verification_seconds,
                       "feedback": sanitized_feedback(case, checks),
                       "elapsed_seconds": round(time.monotonic() - started, 6)}
        except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
            invocation.update(status="incomplete", elapsed_seconds=time.monotonic() - invocation_started,
                              error_type=type(error).__name__)
            cross.write_json(invocation_path, invocation)
            raise
        invocation.update(status="completed", stage="verified", elapsed_seconds=time.monotonic() - invocation_started,
                          attempt_sha256=cross.digest(attempt))
        cross.write_json(invocation_path, invocation)
        attempts.append(attempt)
        cross.write_json(output / "attempt-records" / sample_id / f"{number}.json", attempt)
        previous = checks
        if all(check["passed"] for check in checks["checks"]):
            break
    if not attempts:
        raise ValueError("No coding attempt completed inside the wall-time budget")
    response = dict(attempts[-1]["response"])
    response["files"] = combined
    response["usage"] = summed_coding_usage(attempts)
    measured = sum(item["coding_seconds"] + item["verification_seconds"] for item in attempts)
    return {"response": response, "tests": attempts[-1]["tests"], "attempts": attempts,
            "coding_seconds": sum(item["coding_seconds"] for item in attempts),
            "verification_seconds": sum(item["verification_seconds"] for item in attempts),
            "iteration_overhead_seconds": max(0.0, time.monotonic() - started - measured),
            "stop_reason": "correct" if all(item["passed"] for item in previous["checks"]) else
                "attempt_limit" if len(attempts) == policy["max_attempts"] else "wall_time_limit"}


def validate_attempts(directory: Path, sample: dict, entry: dict, context: dict | None) -> None:
    attempts, policy = sample.get("attempts"), sample.get("repair_policy")
    if attempts is None and policy is None:
        return  # Historical one-attempt artifacts remain assessable.
    expected_policy = repair_policy(argparse.Namespace(timeout=1, **policy))
    if policy != expected_policy or not isinstance(attempts, list) or not 1 <= len(attempts) <= policy["max_attempts"]:
        raise ValueError("Invalid repair policy or attempt count")
    snapshot = directory / entry["source_root"]
    current = {name: (snapshot / name).read_bytes().decode("utf-8")
               for name in entry["source_manifest"]["files_sha256"]}
    combined, previous, elapsed, checker_contract = {}, None, 0.0, None
    for number, attempt in enumerate(attempts, 1):
        if previous is not None and all(check["passed"] for check in previous["checks"]):
            raise ValueError("Attempt continued after first independently correct patch")
        request = (agent_request(entry["case"], current, context) if number == 1 else
                   repair_request(entry["case"], current, context, previous, number, policy))
        if (attempt["number"] != number or attempt["request_sha256"] != cross.digest(request)
                or attempt != cross.read_json(directory / "attempt-records" / sample["sample_id"] / f"{number}.json")
                or attempt["response_sha256"] != cross.digest(attempt["response"])):
            raise ValueError("Attempt request, response or retained ledger binding changed")
        response = validate_agent_response(attempt["response"], entry["case"]["editable_files"])
        invocation = cross.read_json(directory / "attempt-invocations" / sample["sample_id"] / f"{number}.json")
        if (invocation.get("number") != number or invocation.get("status") != "completed"
                or invocation.get("stage") != "verified" or invocation.get("request_sha256") != attempt["request_sha256"]
                or invocation.get("attempt_sha256") != cross.digest(attempt)
                or invocation.get("usage") != response["usage"] or invocation.get("error_type") is not None):
            raise ValueError("Attempt invocation capture does not match its completed checker outcome")
        seconds = invocation.get("elapsed_seconds")
        if (type(seconds) not in (int, float) or not math.isfinite(seconds)
                or seconds + 0.005 < attempt["coding_seconds"] + attempt["verification_seconds"]):
            raise ValueError("Attempt invocation timing drops an executed stage")
        current.update(response["files"])
        combined.update(response["files"])
        expected_root = f"attempts/{sample['sample_id']}/{number}"
        expected_files = {name: hashlib.sha256(value.encode("utf-8")).hexdigest() for name, value in current.items()}
        if (attempt["implementation_root"] != expected_root
                or cross.fingerprint(directory / expected_root) != attempt["implementation_manifest"]
                or attempt["implementation_manifest"]["files_sha256"] != expected_files):
            raise ValueError("Retained attempt no longer matches the tested proposed files")
        previous = attempt["tests"]
        checker_contract = validate_checker_result(previous, checker_contract)
        if sample.get("checker_manifest_dir"):
            validate_checker_record(previous, entry["case"], directory / "implementations" / sample["sample_id"],
                                    Path(sample["checker_manifest_dir"]), checker_contract)
        files = previous["provenance"]["files_sha256"]
        if not files or any(hash_file(Path(path)) != value for path, value in files.items()):
            raise ValueError("Attempt checker revision changed")
        if attempt["feedback"] != sanitized_feedback(entry["case"], previous):
            raise ValueError("Repair feedback was not derived from predetermined messages")
        for field in ("coding_seconds", "verification_seconds", "elapsed_seconds"):
            value = attempt[field]
            if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
                raise ValueError("Invalid coding-attempt timing")
        if attempt["elapsed_seconds"] < elapsed:
            raise ValueError("Attempt timestamps are not monotonic")
        elapsed = attempt["elapsed_seconds"]
        charged = sum(item["coding_seconds"] + item["verification_seconds"] for item in attempts[:number])
        if elapsed + 0.005 * number < charged or elapsed > policy["attempt_budget_seconds"] + 1:
            raise ValueError("Attempt wall time cannot contain the charged coding/checker phases")
    final = sample["agent_response"]
    if (final["files"] != combined or final["summary"] != attempts[-1]["response"]["summary"]
            or final.get("provider_model") != attempts[-1]["response"].get("provider_model")
            or final["usage"] != summed_coding_usage(attempts) or sample["tests"] != previous
            or sample["costs"]["coding_seconds"] != sum(item["coding_seconds"] for item in attempts)
            or sample["costs"]["verification_seconds"] != sum(item["verification_seconds"] for item in attempts)):
        raise ValueError("Final coding result or costs drop a repair attempt")
    passed = all(check["passed"] for check in previous["checks"])
    expected_stop = "correct" if passed else "attempt_limit" if len(attempts) == policy["max_attempts"] else "wall_time_limit"
    if sample["attempt_stop_reason"] != expected_stop:
        raise ValueError("Repair stop reason does not match independent checks")
    if sample["time_to_first_correct_seconds"] != (sample["costs"]["total_seconds"] if passed else None):
        raise ValueError("Time to first correct patch is fabricated or drops charged phases")


def run(args: argparse.Namespace, *, setups=SETUPS, context_collector=None,
        assessor=None, review_template=None, report_metadata=None, config_transform=None,
        subprocess_environment=None, preparation_hook=None) -> dict:
    """Execute one comparison; newer protocols inject explicit policy helpers.

    Defaults preserve the original 0.5 runner. No module globals or monkeypatch
    are used to switch production evaluation protocols.
    """
    if not setups or setups[0] != "baseline" or len(set(setups)) != len(setups):
        raise ValueError("Comparison setups require baseline followed by unique context arms")
    collector = context_collector or collect_context
    iteration_policy = repair_policy(args)
    order_seed = getattr(args, "seed", None)
    if order_seed is None:
        order_seed = secrets.randbits(64)
    if type(order_seed) is not int or not 0 <= order_seed < 2 ** 64:
        raise ValueError("seed must be an unsigned 64-bit integer")
    order_random = random.Random(order_seed)

    def environment(project, stage):
        # A newer protocol can supply grants to each child without changing
        # process-global state or the original runners' inherited environment.
        return {"env": subprocess_environment(project, stage)} if subprocess_environment else {}

    command = json.loads(args.agent_command)
    if not isinstance(command, list) or not command or any(not isinstance(item, str) or not item for item in command):
        raise ValueError("--agent-command must be a JSON argv array")
    if args.agent_location == "hosted" and not args.allow_hosted:
        raise ValueError("Hosted coding-agent execution requires --allow-hosted")
    if not 1 <= args.timeout <= 86400 or not 256 <= args.max_tokens <= 100000:
        raise ValueError("Invalid timeout or context token budget")
    output = cross.selected_path(args.output)
    binary = str(args.lore_binary.resolve(strict=True))
    cases_path = args.cases.resolve(strict=True)
    cases = load_cases(cases_path)
    for case in cases["cases"]:
        preview = config_for_case(args, case, output / "preview")
        if config_transform:
            config_transform(preview)
    prepared = prepare(output, cases_path)
    if preparation_hook:
        preparation_hook(output, prepared)
    report = {"schema_version": 1, "phase": "actual_coding_tasks", "created_at": bench.now_utc(),
              "fixture_only": prepared["fixture_only"], "held_out": prepared["held_out"],
              "independent_projects": prepared["independent_projects"], "cases": prepared["cases"],
              "recognized_fixture_fingerprints": prepared["recognized_fixture_fingerprints"],
              "lore_binary_sha256": hash_file(Path(binary)), "agent_id": args.agent_id,
              "agent_command_sha256": cross.digest(command), "agent_location": args.agent_location,
              "samples": [], "human_review": "pending",
              "cost_scope": "Coding execution and Lore preparation/context. Existing upstream snapshots are supplied inputs; their creation cost is unmeasured. Each Lore setup is charged the full shared preparation cost, with no amortization. Repeat-context probes are separate from task totals.",
              "execution_qualification": "Agent-reported model identity, usage, billing, and genuine model execution are operator attestations. Subprocesses are real; this harness cannot authenticate the runner's claims or sandbox it."}
    if report_metadata is not None:
        report["study"] = report_metadata
        if "cost_scope" in report_metadata:
            report["cost_scope"] = report_metadata["cost_scope"]
    report["setups"] = list(setups)
    report["repair_policy"] = iteration_policy
    report["order_seed"] = order_seed
    report["execution_order"] = []
    for entry in prepared["cases"]:
        case = entry["case"]
        snapshot = output / entry["source_root"]
        validate_prepared_checker(entry, snapshot, cases_path.parent)
        project = output / "lore" / case["id"]
        preparation_start = time.monotonic()
        cross.copy_snapshot(snapshot, project)
        config = config_for_case(args, case, project)
        if config_transform:
            config_transform(config)
        cross.write_json(project / "lore.yml", config)
        init, _ = bench.subprocess_json(binary, project, "init", timeout=args.timeout,
                                        **environment(project, "preparation"))
        preparation_seconds = round(time.monotonic() - preparation_start, 3)
        preparation_usage = usage(init.get("usage"), calls=cross.model_calls(init))
        contexts = {setup: collector(binary, project, case["task"], setup, args.timeout, args.max_tokens)
                    for setup in setups if setup != "baseline"}
        order = list(setups)
        order_random.shuffle(order)
        report["execution_order"].append({"case_id": case["id"], "setups": order})
        for setup in order:
            sample_id = "sample-" + secrets.token_hex(10)
            task_started = time.monotonic()
            workspace = output / "implementations" / sample_id
            cross.copy_snapshot(snapshot, workspace)
            source_files = {name: (snapshot / name).read_bytes().decode("utf-8")
                            for name in entry["source_manifest"]["files_sha256"]}
            context = contexts.get(setup)
            request = agent_request(case, source_files, context)
            result = run_attempts(command, case, source_files, context, workspace, output,
                                  sample_id, cases_path.parent, args.timeout, iteration_policy, environment)
            response = result["response"]
            coding_seconds, verification_seconds = result["coding_seconds"], result["verification_seconds"]
            checks = result["tests"]
            agent_citations = cross.resolve_citations(binary, project, answer_evidence_ids(response), args.timeout,
                                                       **environment(project, "evidence"))
            if cross.fingerprint(snapshot) != entry["source_manifest"]:
                raise ValueError("Original source snapshot changed during coding execution")
            actual = cross.fingerprint(workspace)
            allowed = set(case["editable_files"])
            protected_sources = (set(actual["files_sha256"]) == set(entry["source_manifest"]["files_sha256"])
                                 and all(actual["files_sha256"].get(name) == value for name, value in entry["source_manifest"]["files_sha256"].items() if name not in allowed))
            expected = case.get("expected_sources", [])
            retrieved = context["inspection_sources"] if context else []
            coverage = {source: any(source in locator for locator in retrieved) for source in expected} if context else None
            costs = {"preparation_seconds": preparation_seconds if context else 0.0,
                     "context_seconds": context["elapsed_seconds"] if context else 0.0,
                     "coding_seconds": coding_seconds, "verification_seconds": verification_seconds,
                     "iteration_overhead_seconds": max(0.0, time.monotonic() - task_started - coding_seconds - verification_seconds),
                     "preparation_usage": preparation_usage if context else no_inference(),
                     "context_usage": context["usage"] if context else no_inference(),
                     "coding_usage": response["usage"]}
            costs["total_seconds"] = sum(costs[field] for field in ("preparation_seconds", "context_seconds", "coding_seconds", "verification_seconds", "iteration_overhead_seconds"))
            costs["total_usage"] = add_usage([costs[field] for field in ("preparation_usage", "context_usage", "coding_usage")])
            answer_path = output / "answers" / f"{sample_id}.json"
            cross.write_json(answer_path, {"summary": response["summary"], "files": response["files"]})
            sample = {"sample_id": sample_id, "setup": setup, "case_id": case["id"], "project": case["project"],
                      "source_manifest": entry["source_manifest"], "source_root": entry["source_root"],
                      "configuration_sha256": cross.digest(config) if context else None,
                      "request_sha256": cross.digest(request), "agent_response": response,
                      "source_input_utf8_bytes": sum(len(value.encode("utf-8")) for value in source_files.values()),
                      "agent_citation_integrity": agent_citations,
                      "answer_sha256": hash_file(answer_path), "implementation_manifest": actual,
                      "context": context, "costs": costs, "tests": checks,
                      "attempts": result["attempts"], "repair_policy": iteration_policy,
                      "checker_manifest_dir": str(cases_path.parent),
                      "attempt_stop_reason": result["stop_reason"],
                      "time_to_first_correct_seconds": costs["total_seconds"] if result["stop_reason"] == "correct" else None,
                      "constraints": case["critical_constraints"], "task": case["task"],
                      "retrieval_source_coverage": coverage, "protected_sources_preserved": protected_sources}
            report["samples"].append(sample)
            cross.write_json(output / "metrics.json", report)
    # Bind reviews to the complete comparison, not editable scalar declarations.
    binding = cross.digest(report)
    for sample in report["samples"]:
        review = {"schema_version": 1, "sample_id": sample["sample_id"], "metrics_sha256": binding,
                  "answer_sha256": sample["answer_sha256"], "task": sample["task"],
                  "critical_constraints": sample["constraints"], "complete": False,
                  "reviewer": "", "reviewed_at": "", "blind_confirmed": False,
                  "missed_critical_constraints": None, "useful_recommendation": None,
                  "implementation_quality_0_to_3": None, "high_severity_unsupported_claims": None,
                  "notes": ""}
        if review_template:
            review = review_template(sample, binding)
        cross.write_json(output / "reviews" / f"{sample['sample_id']}.json", review)
    summary = (assessor or assess)(output)
    cross.write_json(output / "assessment.json", summary)
    return summary


def validate_sample(directory: Path, sample: dict, entry: dict, *, context_validator=context_checks,
                    bind_request=False) -> tuple:
    """Revalidate source, answer, request, implementation and test bindings.

    Used by the 0.6 assessor as well as the original runner's assessment. Saved
    pass booleans cannot replace comparisons against the actual bound bytes.
    """
    identity, setup = sample["sample_id"], sample["setup"]
    if sample.get("checker_manifest_dir"):
        validate_prepared_checker(entry, directory / "implementations" / identity,
                                  Path(sample["checker_manifest_dir"]))
    if (sample["source_manifest"] != entry["source_manifest"] or sample["source_root"] != entry["source_root"]
            or sample["task"] != entry["case"]["task"] or sample["constraints"] != entry["case"]["critical_constraints"]):
        raise ValueError("case task, source, or constraint binding changed")
    relative_file(sample["source_root"])
    if hash_file(directory / "answers" / f"{identity}.json") != sample["answer_sha256"]:
        raise ValueError("reviewed answer bytes changed")
    answer = cross.read_json(directory / "answers" / f"{identity}.json")
    agent_response = validate_agent_response(sample["agent_response"], entry["case"]["editable_files"])
    if answer != {"summary": agent_response["summary"], "files": agent_response["files"]}:
        raise ValueError("agent response and reviewed answer disagree")
    if not resolver_complete(sample["agent_citation_integrity"], answer_evidence_ids(agent_response)):
        raise ValueError("coding answer contains an unresolved evidence reference")
    snapshot = directory / sample["source_root"]
    if cross.fingerprint(snapshot) != sample["source_manifest"]:
        raise ValueError("original input snapshot changed")
    actual = cross.fingerprint(directory / "implementations" / identity)
    if actual != sample["implementation_manifest"]:
        raise ValueError("tested implementation bytes changed")
    expected = dict(sample["source_manifest"]["files_sha256"])
    expected.update({path: hashlib.sha256(content.encode("utf-8")).hexdigest()
                     for path, content in agent_response["files"].items()})
    if not sample.get("protected_sources_preserved") or actual["files_sha256"] != expected:
        raise ValueError("coding execution changed protected source files or the proposed implementation")
    context = sample.get("context")
    if setup != "baseline":
        if not isinstance(context, dict) or not all(context_validator(context, sample["task"], setup).values()):
            raise ValueError("context integrity check failed")
    elif context is not None:
        raise ValueError("baseline unexpectedly contains Lore context")
    source_files = {name: (snapshot / name).read_bytes().decode("utf-8")
                    for name in entry["source_manifest"]["files_sha256"]}
    if bind_request and sample["request_sha256"] != cross.digest(agent_request(entry["case"], source_files, context)):
        raise ValueError("coding-agent input differs from the bound task, context or original sources")
    validate_checker_result(sample["tests"])
    if sample.get("checker_manifest_dir"):
        validate_checker_record(sample["tests"], entry["case"], directory / "implementations" / identity,
                                Path(sample["checker_manifest_dir"]))
    checks = sample["tests"]["checks"]
    verification_files = sample["tests"]["provenance"]["files_sha256"]
    if not verification_files or any(hash_file(Path(path)) != expected for path, expected in verification_files.items()):
        raise ValueError("verification program no longer matches the executed revision")
    if not checks or any(type(check.get("passed")) is not bool or check.get("kind") not in ("correctness", "constraint")
                         for check in checks):
        raise ValueError("verification checks are absent or malformed")
    validate_attempts(directory, sample, entry, context)
    return context, checks


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    report = cross.read_json(directory / "metrics.json")
    if report.get("schema_version") != 1 or report.get("phase") != "actual_coding_tasks" or not report.get("samples"):
        raise ValueError("Expected completed actual coding-task metrics")
    binding = cross.digest(report)
    totals = {setup: {"tasks": 0, "passed_tasks": 0, "failed_constraint_checks": 0,
                      "measured_intelligent_tasks": 0, "fast_fallback_tasks": 0,
                      "total_seconds": 0.0, "reviewed_tasks": 0, "missed_critical_constraints": 0,
                      "useful_recommendations": 0, "high_severity_unsupported_claims": 0,
                      "implementation_quality_sum": 0,
                      "usage_components": []} for setup in SETUPS}
    issues, reviews_pending, reviewers, case_setups, identities = [], [], set(), {}, set()
    declared_cases = {(entry["case"]["project"], entry["case"]["id"]): entry for entry in report["cases"]}
    known_fixtures = bundled_fingerprints()
    recognized = sorted({entry["source_manifest"]["sha256"] for entry in report["cases"]} & known_fixtures)
    fixture_only = report.get("fixture_only") is not False or any("base_project" in entry["case"] for entry in report["cases"]) or bool(recognized)
    project_sources = {}
    for sample in report["samples"]:
        setup, identity = sample.get("setup"), sample.get("sample_id")
        if setup not in SETUPS or not isinstance(identity, str) or not identity.startswith("sample-") or relative_file(identity) != identity or identity in identities:
            raise ValueError("Invalid sample identity or setup")
        identities.add(identity)
        key = (sample["project"], sample["case_id"])
        case_setups.setdefault(key, []).append(setup)
        try:
            entry = declared_cases[key]
            context, checks = validate_sample(directory, sample, entry)
            if sample.get("repair_policy") != report.get("repair_policy"):
                raise ValueError("Sample repair allowance differs from the matched study policy")
            project_sources.setdefault(sample["project"], set()).add(sample["source_manifest"]["sha256"])
            total = totals[setup]
            total["tasks"] += 1
            total["passed_tasks"] += bool(checks) and all(check["passed"] for check in checks)
            total["failed_constraint_checks"] += sum(check["kind"] == "constraint" and not check["passed"] for check in checks)
            mode = context["response"].get("mode") if context else None
            total["measured_intelligent_tasks"] += mode == "intelligent"
            total["fast_fallback_tasks"] += mode == "fast_fallback"
            total["total_seconds"] += sample["costs"]["total_seconds"]
            total["usage_components"].append(sample["costs"]["total_usage"])
            review_path = directory / "reviews" / f"{identity}.json"
            review = cross.read_json(review_path) if review_path.is_file() else {}
            if review.get("complete") is not True:
                reviews_pending.append(identity)
                continue
            if review.get("metrics_sha256") != binding or review.get("answer_sha256") != sample["answer_sha256"] or review.get("sample_id") != identity or review.get("task") != sample["task"] or review.get("critical_constraints") != sample["constraints"]:
                raise ValueError("human review binding mismatch")
            if review.get("blind_confirmed") is not True or any(not isinstance(review.get(field), str) or not review[field].strip() for field in ("reviewer", "reviewed_at", "notes")):
                raise ValueError("human review is incomplete or unblinded")
            missed = review.get("missed_critical_constraints")
            if not isinstance(missed, list) or len(set(missed)) != len(missed) or any(item not in sample["constraints"] for item in missed):
                raise ValueError("invalid missed-constraint annotation")
            quality = review.get("implementation_quality_0_to_3")
            severity = review.get("high_severity_unsupported_claims")
            if type(quality) is not int or not 0 <= quality <= 3 or type(severity) is not int or severity < 0 or type(review.get("useful_recommendation")) is not bool:
                raise ValueError("human scores are missing or invalid")
            reviewers.add(review["reviewer"])
            total["reviewed_tasks"] += 1
            total["missed_critical_constraints"] += len(missed)
            total["useful_recommendations"] += review["useful_recommendation"]
            total["high_severity_unsupported_claims"] += severity
            total["implementation_quality_sum"] += quality
        except (ValueError, OSError, KeyError, TypeError) as error:
            issues.append(f"{identity}: {error}")
    for total in totals.values():
        total["total_usage"] = add_usage(total.pop("usage_components")) if total["tasks"] else None
        quality_sum = total.pop("implementation_quality_sum")
        total["mean_implementation_quality_0_to_3"] = quality_sum / total["reviewed_tasks"] if total["reviewed_tasks"] else None
        if not total["reviewed_tasks"]:
            for field in ("missed_critical_constraints", "useful_recommendations", "high_severity_unsupported_claims"):
                total[field] = None
    complete_design = set(case_setups) == set(declared_cases) and all(sorted(setups) == sorted(SETUPS) for setups in case_setups.values())
    automatic = not issues and complete_design
    reviewed = automatic and not reviews_pending and len(reviewers) >= 2
    fully_intelligent = totals["intelligent"]["measured_intelligent_tasks"] == len(case_setups) and bool(case_setups)
    baseline_misses = totals["fast"]["missed_critical_constraints"]
    reduction = (baseline_misses - totals["intelligent"]["missed_critical_constraints"]) / baseline_misses if reviewed and baseline_misses else None
    independent = (not fixture_only and report.get("held_out") is True and report.get("independent_projects") is True
                   and len(project_sources) >= 3 and all(len(sources) == 1 for sources in project_sources.values())
                   and len(set().union(*project_sources.values())) == len(project_sources))
    return {"schema_version": 1, "mechanical_contracts_passed": automatic,
            "comparison_complete": automatic, "all_intelligent_tasks_received_intelligence": fully_intelligent,
            "human_review_complete": reviewed, "reviews_pending": reviews_pending, "issues": issues,
            "fixture_only": fixture_only,
            "recognized_fixture_fingerprints": recognized,
            "independent_validation_complete": independent and reviewed and fully_intelligent,
            "zero_reviewed_high_severity_unsupported_claims": reviewed and all(total["high_severity_unsupported_claims"] == 0 for total in totals.values()),
            "relative_missed_constraint_reduction_vs_fast": reduction,
            "directional_20_percent_target_met": reduction is not None and reduction >= 0.2,
            "setups": totals,
            "qualification": "Executable tests measure only their stated behavior. Source-locator coverage is a retrieval proxy. Fixture results, absent or unblinded reviews, fallback context, and unknown billing do not establish independent intelligent benefit or cost savings. Review and execution identity remain researcher attestations."}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "run"):
        command = subparsers.add_parser(name)
        command.add_argument("--cases", type=Path, default=DEFAULT_CASES)
        command.add_argument("--output", type=Path, required=True)
        if name == "run":
            command.add_argument("--lore-binary", type=Path, required=True)
            command.add_argument("--provider", choices=("ollama", "openai"), required=True)
            command.add_argument("--model", required=True)
            command.add_argument("--embedding-model")
            command.add_argument("--decision-provider", choices=("ollama", "openai", "typesafe"))
            command.add_argument("--decision-model")
            command.add_argument("--generative-base-url")
            command.add_argument("--decision-base-url")
            command.add_argument("--allow-hosted", action="store_true")
            command.add_argument("--agent-command", required=True, help="JSON argv array for the operator's real coding-agent adapter")
            command.add_argument("--agent-id", required=True, help="Same pinned model/version and settings for all three setups")
            command.add_argument("--agent-location", choices=("local", "hosted"), required=True)
            command.add_argument("--timeout", type=int, default=3600)
            command.add_argument("--max-tokens", type=int, default=6000)
            command.add_argument("--max-attempts", type=int, default=1)
            command.add_argument("--attempt-budget-seconds", type=int)
            command.add_argument("--seed", type=int)
            bench.add_reasoning_options(command)
    assess_parser = subparsers.add_parser("assess")
    assess_parser.add_argument("run", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(args.output, args.cases)
        elif args.command == "run":
            result = run(args)
        else:
            result = assess(args.run)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if args.command == "prepare" or result["mechanical_contracts_passed"] else 2
    except (ValueError, OSError, KeyError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"Coding-task evaluation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
