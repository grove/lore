#!/usr/bin/env python3
"""Capture offline 0.8 compatibility baselines from the frozen public registry.

This invokes only the selected Lore binary, with generation disabled in the
hash-verified fixture and all ambient inspection/egress grants removed. It does
not run a model or turn contract controls into product-outcome evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

import cross_source as cross
import restore_zoom_fixture as frozen


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(binary: Path, output: Path, source_revision: str) -> dict:
    binary = binary.resolve(strict=True)
    output = cross.selected_path(output)
    if output.exists():
        raise ValueError("Use a fresh output directory")
    if len(source_revision) != 40 or any(c not in "0123456789abcdef" for c in source_revision):
        raise ValueError("Bind the selected binary to a full source commit")
    output.mkdir(parents=True)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("LORE_") and key not in
                   {"OPENAI_API_KEY", "ANTHROPIC_API_KEY", "OLLAMA_API_KEY"}}
    cases = [("schema_2", ["--fast"]), ("schema_3", ["--schema-version", "3"]),
             ("schema_4", ["--schema-version", "4"]),
             ("schema_5", ["--schema-version", "5"]), ("default", [])]
    records = []
    with tempfile.TemporaryDirectory(prefix="lore-contracts-08-") as temp:
        project = Path(temp).resolve() / "fixture"
        frozen.restore(project)
        config = json.loads((project / "lore.yml").read_text(encoding="utf-8"))
        if config["models"]["generative"]["enabled"] is not False:
            raise ValueError("The offline fixture must disable model generation")
        before = cross.fingerprint(project)
        for label, flags in cases:
            argv = ["--json", "context", "Change worker queue capacity", "--max-tokens", "8000", *flags]
            if label != "schema_2":
                argv.extend(["--no-inspect", "--no-cache"])
            started = time.monotonic()
            response = subprocess.run([str(binary), *argv], cwd=project, env=environment,
                                      capture_output=True, timeout=120, check=False)
            elapsed = time.monotonic() - started
            path = output / f"{label}.json"
            path.write_bytes(response.stdout)
            try:
                value = json.loads(response.stdout)
                valid_json = isinstance(value, dict)
            except (ValueError, UnicodeError):
                value, valid_json = {}, False
            after = cross.fingerprint(project)
            records.append({"case_id": label, "arguments": argv,
                "source_revision": source_revision, "exit_code": response.returncode,
                "elapsed_seconds": elapsed, "response": path.name,
                "response_sha256": sha256(path), "response_bytes": path.stat().st_size,
                "stderr_bytes": len(response.stderr),
                "stderr_sha256": hashlib.sha256(response.stderr).hexdigest(),
                "valid_json": valid_json, "schema_version": value.get("schema_version"),
                "source_and_state_unchanged": before == after,
                "status": "passed" if response.returncode == 0 and valid_json
                    and not response.stderr and before == after else "failed"})
        result = {"schema_version": 1, "classification": "offline_contract_controls",
            "source_revision": source_revision, "binary_sha256": sha256(binary),
            "harness_sha256": sha256(Path(__file__)),
            "fixture_manifest_sha256": sha256(frozen.CORPUS / "manifest.json"),
            "source_and_state_sha256": before["sha256"],
            "grants": {"inspection": False, "hosted_egress": False, "checkout_egress": False},
            "models": {"enabled": False, "identity": None},
            "provider_calls": 0, "cases": records,
            "status": "passed" if all(row["status"] == "passed" for row in records) else "failed",
            "qualification": "Frozen public-registry compatibility controls, not a productivity or model-quality study."}
        cross.write_json(output / "contracts.json", result)
    return result


def verify(directory: Path) -> dict:
    report = cross.read_json(directory / "contracts.json")
    if report.get("classification") != "offline_contract_controls":
        raise ValueError("Unknown contract capture")
    for row in report["cases"]:
        if row["response"] != f"{row['case_id']}.json" or "/" in row["case_id"] or "\\" in row["case_id"]:
            raise ValueError("Noncanonical response path")
        path = directory / row["response"]
        cross.reject_symlink_path(path)
        if sha256(path) != row["response_sha256"] or path.stat().st_size != row["response_bytes"]:
            raise ValueError("Contract response bytes changed")
    return {"status": "passed", "verified_responses": len(report["cases"]),
            "capture_status": report["status"],
            "qualification": "Hash verification does not authenticate the operator or rerun the binary."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("capture")
    run.add_argument("--binary", type=Path, required=True)
    run.add_argument("--source-revision", required=True)
    run.add_argument("--output", type=Path, required=True)
    check = commands.add_parser("verify")
    check.add_argument("directory", type=Path)
    args = parser.parse_args()
    result = (capture(args.binary, args.output, args.source_revision)
              if args.command == "capture" else verify(args.directory))
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "passed" else 2


if __name__ == "__main__":
    raise SystemExit(main())
