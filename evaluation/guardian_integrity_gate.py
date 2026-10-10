#!/usr/bin/env python3
"""Offline full-cohort integrity gate for a dedicated, unsynchronized runner.

All artifacts retain failures. This gate makes no provider call, does not assert
an execution/network audit, and cannot establish alert quality or identify a PID.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
import time
import zipfile

import guardian_longitudinal as guardian


def replay_pass(report: dict, expected_events: int) -> bool:
    commands = report.get("command_integrity", {})
    return (report.get("events") == expected_events and report.get("engineering_capture_pass") is True
            and report.get("per_command_source_integrity_pass") is True
            and report.get("total_model_calls") == 0
            and commands.get("total", 0) > 0
            and commands.get("filesystem_source_observer_complete") == commands["total"])


def collect_artifacts(output: Path) -> dict:
    """Retain only bounded structured synthetic captures, never live workspaces."""
    selected = []
    for directory in sorted(output.iterdir()):
        if not directory.is_dir():
            continue
        for name in ("run.json", "assessment.json", "control.json"):
            path = directory / name
            if path.exists():
                selected.append(path)
        selected.extend(sorted((directory / "events").glob("*/*.json")))
    manifest = {path.relative_to(output).as_posix(): {"sha256": guardian.coding.hash_file(path),
                "bytes": path.stat().st_size} for path in selected}
    archive = output / "guardian-integrity-evidence.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as stream:
        for path in selected:
            stream.write(path, path.relative_to(output).as_posix())
        stream.writestr("artifact-manifest.json", json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return {"archive": archive.name, "sha256": guardian.coding.hash_file(archive),
            "bytes": archive.stat().st_size, "artifacts": manifest,
            "content_scope": "synthetic source hashes, exact fixture responses and diagnostic metadata; no databases, binaries or environment values"}


def run(args: argparse.Namespace) -> dict:
    for value in (args.baseline_revision, args.candidate_revision):
        if not re.fullmatch(r"[0-9a-f]{40}", value):
            raise ValueError("Source revisions must be exact full commit IDs")
    output = guardian.cross.selected_path(args.output)
    if output.exists():
        raise ValueError("The full integrity gate needs a fresh output directory")
    output.mkdir(parents=True, mode=0o700)
    sources = guardian.validate_prepared(args.prepared)
    if not sources.get("fixture_only") or not sources.get("full_size"):
        raise ValueError("The offline integrity gate requires the full explicitly synthetic corpus")
    binaries = {name: Path(value).resolve(strict=True) for name, value in (
                ("baseline", args.baseline_binary), ("candidate", args.candidate_binary))}
    pinned_binaries = {name: guardian.coding.hash_file(path) for name, path in binaries.items()}
    started = time.monotonic()
    result = {"schema_version": 1, "gate": "guardian-source-integrity-v1", "status": "failed",
              "baseline_revision": args.baseline_revision, "candidate_revision": args.candidate_revision,
              "binary_sha256": pinned_binaries, "collector_source_sha256": guardian.collector_identity(),
              "gate_source_sha256": guardian.coding.hash_file(Path(__file__)),
              "prepared_sha256": guardian.coding.hash_file(args.prepared / "prepared.json"),
              "compiled_sha256": guardian.coding.hash_file(args.compiled / "compiled.json"),
              "events_manifest_sha256": sources["events_manifest_sha256"], "runs": {}, "errors": [],
              "source_writer_isolation": "dedicated runner directory; all observed source events must pass",
              "execution_egress_audit": "not_run", "guardian_alert_quality": "unmeasured",
              "hosted_inference_authorized": False}
    try:
        for name, role, expected, project, event_count in (
                ("baseline", "baseline", 60, None, None),
                ("candidate", "candidate", 60, None, None),
                ("candidate-repeat", "candidate", 60, None, None),
                ("focused-payments", "candidate", 10, "payments", 10)):
            if guardian.coding.hash_file(binaries[role]) != pinned_binaries[role]:
                raise ValueError("A frozen binary changed before replay")
            replay = argparse.Namespace(prepared=args.prepared, compiled=args.compiled,
                binary=str(binaries[role]), output=output / name, config_template=None,
                max_tokens=8000, timeout=120, allow_inspection=False, allow_hosted=False,
                allow_checkout_egress=False, audit_strace=False, observe_filesystem=True,
                project=project, first_event=1, event_count=event_count)
            guardian.run(replay)
            report = guardian.assess(output / name)
            result["runs"][name] = {"passed": replay_pass(report, expected), "events": report["events"],
                "failed_checks": report["failed_checks"], "command_integrity": report["command_integrity"],
                "total_model_calls": report["total_model_calls"],
                "run_sha256": guardian.coding.hash_file(output / name / "run.json"),
                "assessment_sha256": guardian.coding.hash_file(output / name / "assessment.json")}
            print(json.dumps({"run": name, **result["runs"][name]}, sort_keys=True), flush=True)
        controls = argparse.Namespace(prepared=args.prepared, output=output / "no-lore-control",
                      observe_filesystem=True, dwell_ms=100, project=None, first_event=1, event_count=None)
        control = guardian.no_lore_control(controls)
        control_commands = [command for entry in control["records"]
            for command in guardian.read_json(controls.output / entry["path"])["commands"]]
        observers_complete = sum(command["integrity"]["observer"].get("source_complete") is True
                                 for command in control_commands)
        result["no_lore_control"] = {"passed": control["events"] == 60 and control["source_integrity_pass"]
                and observers_complete == len(control_commands) == 240,
                "events": control["events"], "failed_events": control["failed_events"],
                "source_observer_complete_intervals": observers_complete,
                "processes_started": control["processes_started"],
                "sha256": guardian.coding.hash_file(output / "no-lore-control" / "control.json")}
        if {name: guardian.coding.hash_file(path) for name, path in binaries.items()} != pinned_binaries:
            raise ValueError("A frozen binary changed during replay")
        result["status"] = "passed" if all(row["passed"] for row in result["runs"].values()) and result["no_lore_control"]["passed"] else "failed"
    except (OSError, ValueError, KeyError, TypeError) as error:
        # Raw exceptions can include private paths or provider text. Keep a class.
        result["errors"].append({"type": type(error).__name__, "stage": "capture_or_assessment"})
    finally:
        result["elapsed_seconds"] = round(time.monotonic() - started, 6)
        result["evidence"] = collect_artifacts(output)
        guardian.cross.write_json(output / "receipt.json", result)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("prepared", "compiled", "baseline-binary", "candidate-binary", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    for name in ("baseline-revision", "candidate-revision"):
        parser.add_argument(f"--{name}", required=True)
    try:
        result = run(parser.parse_args(argv))
    except (OSError, ValueError) as error:
        print(f"Guardian integrity gate failed during preflight: {type(error).__name__}", file=sys.stderr)
        return 1
    print(json.dumps({key: value for key, value in result.items() if key != "evidence"}, indent=2, sort_keys=True))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
