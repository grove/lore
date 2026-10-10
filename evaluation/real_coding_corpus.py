#!/usr/bin/env python3
"""Prepare source-pinned public fault-repair candidates; never invent held-out outcomes."""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import benchmark as bench

import coding_tasks as coding
import cross_source as cross

ROOT = Path(__file__).resolve().parent
CORPUS = ROOT / "corpora" / "coding-real-v1"


def function_body(source: str, symbol: str) -> tuple[int, int]:
    nodes = ast.parse(source).body
    node = None
    for name in symbol.split("."):
        node = next((item for item in nodes if isinstance(item, (ast.FunctionDef, ast.ClassDef)) and item.name == name), None)
        if node is None:
            raise ValueError(f"Unknown upstream symbol: {symbol}")
        nodes = node.body
    if not isinstance(node, ast.FunctionDef):
        raise ValueError("Fault target must be a function or method")
    body = node.body
    if isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant) and isinstance(body[0].value.value, str):
        body = body[1:]
    if not body:
        raise ValueError("Fault target lacks implementation")
    return body[0].lineno - 1, node.end_lineno


def mutate(source: str, symbol: str, body='raise NotImplementedError("Study fault injection: restore documented behavior.")') -> str:
    lines = source.splitlines(keepends=True)
    start, end = function_body(source, symbol)
    indent = lines[start][:len(lines[start]) - len(lines[start].lstrip())]
    return "".join(lines[:start] + [indent + body + "\n"] + lines[end:])


def load_manifest() -> dict:
    manifest = cross.read_json(CORPUS / "manifest.json")
    if manifest["schema_version"] != 1 or len(manifest["tasks"]) < 30 or len(manifest["projects"]) < 3:
        raise ValueError("Candidate corpus requires 30 tasks and 3 pinned projects")
    for project, entry in manifest["projects"].items():
        if len(entry["commit"]) != 40 or any(char not in "0123456789abcdef" for char in entry["commit"]):
            raise ValueError("Project commit must be a full SHA")
        actual = cross.fingerprint(CORPUS / "upstream" / project)
        if actual != entry["manifest"]:
            raise ValueError(f"Pinned upstream source drift: {project}")
    return manifest


def prepare(output: Path, *, pilot=False) -> dict:
    output = cross.selected_path(output)
    if output.exists():
        raise ValueError("Use a fresh candidate output directory")
    manifest = load_manifest()
    cases = []
    selected = manifest["tasks"]
    if pilot:
        chosen = {"boltons-ordinalize", "boltons-chunked-iter", "more-itertools-grouper",
                  "more-itertools-unique-everseen", "python-dotenv-parse-binding", "python-dotenv-variable-resolve"}
        selected = [task for task in selected if task["id"] in chosen]
    for task in selected:
        project = manifest["projects"][task["project"]]
        target = output / "tasks" / task["id"]
        cross.copy_snapshot(CORPUS / "upstream" / task["project"], target)
        filename = coding.relative_file(task["file"])
        path = target / filename
        path.write_bytes(mutate(path.read_bytes().decode("utf-8"), task["symbol"]).encode("utf-8"))
        cases.append({"id": task["id"], "project": task["project"],
            "source_root": f"tasks/{task['id']}", "task": task["task"],
            "editable_files": [filename], "critical_constraints": task["critical_constraints"],
            "test_command": ["{python}", str(ROOT / "real_coding_checks.py"), "--case", task["id"], "--workspace", "{workspace}"],
            "verification_files": [str(ROOT / "real_coding_checks.py"), str(CORPUS / "manifest.json")],
            "repair_feedback": task.get("repair_feedback", {}),
            "expected_sources": [filename], "upstream": {"repository": project["repository"],
                "commit": project["commit"], "files_sha256": project["manifest"]["files_sha256"]},
            "input_sha256": cross.fingerprint(target)["sha256"],
            "authorship_status": "agent_authored_candidate_independent_review_pending",
            "fault_injection": {"symbol": task["symbol"], "method": "body_removed_preserving_docstring"}})
    cases_manifest = {"schema_version": 1, "bundle_version": "real-coding-candidates-v1",
        "fixture_only": True, "held_out": False, "independent_projects": False,
        "source_kind": "three real public repositories; constructed fault-repair candidate tasks",
        "independent_review": "pending", "outcomes": "unmeasured", "pilot": pilot, "cases": cases}
    cross.write_json(output / "cases.json", cases_manifest)
    result = {"schema_version": 1, "phase": "preparation_only", "inference_calls": 0,
        "projects": len(manifest["projects"]), "tasks": len(cases), "cases_manifest": str(output / "cases.json"),
        "source_manifest_sha256": coding.hash_file(CORPUS / "manifest.json"),
        "independent_review": "pending", "real_model_outcomes": "unmeasured"}
    cross.write_json(output / "prepared.json", result)
    return result


def validate(output: Path) -> dict:
    """Execute external checkers on original and two deliberately wrong bodies."""
    output = cross.selected_path(output)
    prepared = prepare(output)
    manifest = load_manifest()
    records = []
    for task in manifest["tasks"]:
        source = CORPUS / "upstream" / task["project"]
        base = source / task["file"]
        outcomes = {}
        for control, body in (("original", None), ("missing", 'raise NotImplementedError("Known wrong control")'), ("constant_none", "return None")):
            workspace = output / "controls" / task["id"] / control
            cross.copy_snapshot(source, workspace)
            if body is not None:
                (workspace / task["file"]).write_bytes(mutate(base.read_bytes().decode("utf-8"), task["symbol"], body).encode("utf-8"))
            response, seconds = coding.invoke_json([sys.executable, str(ROOT / "real_coding_checks.py"),
                "--case", task["id"], "--workspace", str(workspace)], workspace, 15)
            coding.validate_checker_result(response)
            outcomes[control] = {"passed": all(check["passed"] for check in response["checks"]),
                "checks": response["checks"], "checker_result_sha256": cross.digest(response),
                "implementation_manifest": cross.fingerprint(workspace), "elapsed_seconds": seconds}
        records.append({"case_id": task["id"], "controls": outcomes,
                        "valid": outcomes["original"]["passed"] and not outcomes["missing"]["passed"] and not outcomes["constant_none"]["passed"]})
    result = {"schema_version": 1, "phase": "checker_controls", "inference_calls": 0,
        "created_at": bench.now_utc(), "python_version": sys.version,
        "python_executable_sha256": coding.hash_file(Path(sys.executable).resolve()),
        "harness_sha256": coding.hash_file(Path(__file__).resolve()),
        "coding_runner_sha256": coding.hash_file(ROOT / "coding_tasks.py"),
        "candidate_manifest_sha256": prepared["source_manifest_sha256"], "checker_sha256": coding.hash_file(ROOT / "real_coding_checks.py"),
        "all_controls_valid": all(item["valid"] for item in records), "records": records,
        "independent_review": "pending", "real_model_outcomes": "unmeasured",
        "qualification": "Original-source and deliberately wrong control execution validates these checkers mechanically, not independent authorship or model usefulness."}
    cross.write_json(output / "validation.json", result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "validate"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pilot", action="store_true", help="Prepare the fixed six-task pilot subset")
    args = parser.parse_args(argv)
    try:
        if args.command == "validate" and args.pilot:
            raise ValueError("Control validation always covers all 30 candidate tasks")
        result = prepare(args.output, pilot=args.pilot) if args.command == "prepare" else validate(args.output)
        print(json.dumps(result, indent=2))
        return 0 if result.get("all_controls_valid", True) else 2
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Candidate preparation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
