#!/usr/bin/env python3
"""Verify official synthetic Guardian artifacts; never launch Lore or publish."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import io
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import zipfile

sys.dont_write_bytecode = True


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def atomic(path: Path, data: bytes) -> None:
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
        temporary = stream.name
    os.replace(temporary, path)


def bound(data: bytes) -> dict:
    return {"bytes": len(data), "sha256": sha(data)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--collector", type=Path, required=True)
    parser.add_argument("--retained-inner", action="store_true",
                        help="Verify retained receipt/archive bindings; do not claim to revalidate the expired outer GitHub ZIP")
    args = parser.parse_args()
    metadata = json.loads((args.input / "github-metadata.json").read_bytes())
    artifact = metadata["artifact"]
    workflow = metadata["workflow"]
    assert workflow["conclusion"] == "success" and workflow["status"] == "completed"
    assert artifact["workflow_run"]["id"] == workflow["id"]
    assert artifact["workflow_run"]["head_sha"] == workflow["head_sha"]
    outer = None
    if args.retained_inner:
        receipt_bytes = (args.input / "receipt.json").read_bytes()
        inner = (args.input / "guardian-integrity-evidence.zip").read_bytes()
        bindings = metadata["retained_member_bindings"]
        assert bound(receipt_bytes) == bindings["receipt.json"], "retained_receipt_binding_mismatch"
        assert bound(inner) == bindings["guardian-integrity-evidence.zip"], "retained_inner_binding_mismatch"
        assert metadata["retained_binding_provenance"] == {
            "official_artifact_id": artifact["id"], "official_artifact_digest": artifact["digest"],
            "origin": "members_extracted_after_official_outer_digest_verification"}
    else:
        outer = (args.input / "github-artifact.zip").read_bytes()
        assert len(outer) == artifact["size_in_bytes"]
        assert "sha256:" + sha(outer) == artifact["digest"]
        with zipfile.ZipFile(io.BytesIO(outer)) as archive:
            assert sorted(archive.namelist()) == ["guardian-integrity-evidence.zip", "receipt.json"]
            receipt_bytes = archive.read("receipt.json")
            inner = archive.read("guardian-integrity-evidence.zip")
        if "retained_member_bindings" in metadata:
            assert bound(receipt_bytes) == metadata["retained_member_bindings"]["receipt.json"]
            assert bound(inner) == metadata["retained_member_bindings"]["guardian-integrity-evidence.zip"]
    receipt = json.loads(receipt_bytes)
    assert receipt["status"] == "passed" and not receipt["errors"]
    assert receipt["candidate_revision"] == workflow["head_sha"]
    assert receipt["baseline_revision"] == "0ba8b17f7e63726a8d81def2b60af18573d4640c"
    assert bound(inner) == {key: receipt["evidence"][key] for key in ("bytes", "sha256")}
    for name, expected in receipt["collector_source_sha256"].items():
        assert sha((args.collector / name).read_bytes()) == expected, name
    assert sha((args.collector / "guardian_integrity_gate.py").read_bytes()) == receipt["gate_source_sha256"]
    sys.path.insert(0, str(args.collector))
    import guardian_longitudinal as guardian

    def inventory_valid(snapshot: dict) -> bool:
        inventory = snapshot["inventory"]
        files = {path: value["sha256"] for path, value in inventory["paths"].items()
                 if guardian.integrity.category(path) == "primary_source" and value["kind"] == "file"}
        return (snapshot["errors"] == [] and inventory["sha256"] == guardian.cross.digest(inventory["paths"])
                and files == snapshot["sources"]["files_sha256"]
                and snapshot["sources"]["file_count"] == len(files)
                and snapshot["sources"]["sha256"] == guardian.cross.digest(files))

    cohorts = {}
    resolution_counts = {}
    with zipfile.ZipFile(io.BytesIO(inner)) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names))
        manifest = json.loads(archive.read("artifact-manifest.json"))
        assert manifest == receipt["evidence"]["artifacts"]
        assert set(names) == set(manifest) | {"artifact-manifest.json"}
        for name, expected in manifest.items():
            assert bound(archive.read(name)) == expected, name
        for cohort, expected_count, role in (("baseline", 60, "baseline"), ("candidate", 60, "candidate"),
                                             ("candidate-repeat", 60, "candidate"), ("focused-payments", 10, "candidate")):
            run_bytes = archive.read(cohort + "/run.json")
            assessment_bytes = archive.read(cohort + "/assessment.json")
            run = json.loads(run_bytes)
            assessment = json.loads(assessment_bytes)
            assert run["binary_sha256"] == receipt["binary_sha256"][role]
            assert run["collector_source_sha256"] == receipt["collector_source_sha256"]
            assert run["prepared_sha256"] == receipt["prepared_sha256"]
            assert run["events_manifest_sha256"] == receipt["events_manifest_sha256"]
            assert run["fixture_only"] is True and run["filesystem_observer_requested"] is True
            expected_ids = {f"{project}-{index:02}" for project in (
                ["payments"] if expected_count == 10 else ["payments", "ledger", "releases"])
                for index in range(1, 11 if expected_count == 10 else 21)}
            assert len(run["records"]) == len(expected_ids) == expected_count
            assert {record["event_id"] for record in run["records"]} == expected_ids
            checks = Counter()
            commands = []
            snapshot_intervals = []
            event_compilation_calls = 0
            guard_calls = 0
            resolution_counts[cohort] = {}
            for record in run["records"]:
                data = archive.read(cohort + "/" + record["path"])
                assert sha(data) == record["sha256"]
                capture = json.loads(data)
                assert capture["event_id"] == record["event_id"]
                recomputed = guardian.event_checks(capture, run["max_tokens"])
                assert all(recomputed.values()), (cohort, record["event_id"], recomputed)
                checks.update(recomputed)
                commands.extend(guardian.read_commands(capture))
                resolution_counts[cohort][record["event_id"]] = len(capture.get("resolution", []))
                snapshot_intervals.append(capture["materialization"]["snapshot_integrity"])
                # Recompute each invocation; never infer zero from the saved
                # assessment's aggregate. Missing/malformed call counts raise.
                event_compilation_calls += guardian.compilation_calls(capture["materialization"])
                guard = guardian.require_response(capture["guard"])
                if guard.get("intelligence") is not None:
                    guard_calls += guardian.shared.invocation_calls(guard["intelligence"], "adaptive")
            snapshot_intervals.extend(initial["compilation"]["snapshot_integrity"] for initial in run["initialization"])
            assert all(inventory_valid(interval[side]) for interval in snapshot_intervals for side in ("before", "after"))
            counts = {"total": len(commands), "captured": sum("source_integrity" in command for command in commands),
                      "preserved": sum(guardian.command_integrity_valid(command) for command in commands),
                      "filesystem_observer_captured": sum(command["source_integrity"]["observer"]["status"] == "captured" for command in commands),
                      "filesystem_observer_complete": sum(command["source_integrity"]["observer"]["complete"] is True for command in commands),
                      "filesystem_source_observer_complete": sum(command["source_integrity"]["observer"]["source_complete"] is True for command in commands)}
            assert counts == assessment["command_integrity"] == receipt["runs"][cohort]["command_integrity"]
            assert counts["total"] > 0 and counts["total"] == counts["captured"] == counts["preserved"] == counts["filesystem_source_observer_complete"]
            assert all(value == expected_count for value in checks.values())
            assert assessment["run_sha256"] == sha(run_bytes) == receipt["runs"][cohort]["run_sha256"]
            assert sha(assessment_bytes) == receipt["runs"][cohort]["assessment_sha256"]
            assert assessment["events"] == expected_count and assessment["failed_checks"] == {}
            initialization_calls = sum(guardian.compilation_calls(item["compilation"]) for item in run["initialization"])
            compilation_calls = initialization_calls + event_compilation_calls
            total_calls = compilation_calls + guard_calls
            assert assessment["guard_model_calls"] == guard_calls, "guard_model_call_aggregate_mismatch"
            assert assessment["compilation_model_calls"] == compilation_calls, "compilation_model_call_aggregate_mismatch"
            assert assessment["total_model_calls"] == receipt["runs"][cohort]["total_model_calls"] == total_calls == 0, "total_model_calls_not_verified_zero"
            gaps = Counter(gap for command in commands for gap in command["source_integrity"]["observer"]["gaps"])
            assert set(gaps) <= {"new_directory_watch_gap:lore_state"}
            exits = Counter()
            failures = Counter()
            for command in commands:
                error = command.get("error")
                if "error" in command and error is None and isinstance(command.get("response"), dict):
                    exits["0"] += 1
                elif isinstance(error, str) and re.fullmatch(r"exit_-?\d+", error):
                    exits[error.removeprefix("exit_")] += 1
                elif error in ("timeout", "cancelled", "process_unavailable", "invalid_json_response"):
                    failures[error] += 1
                else:
                    failures["unrecognized_record"] += 1
            outcome = {"captured_commands": len(commands), "exit_code_counts": dict(exits),
                       "nonzero_exit_commands": sum(value for key, value in exits.items() if key != "0"),
                       "timeout_commands": failures["timeout"], "cancelled_commands": failures["cancelled"],
                       "unavailable_process_commands": failures["process_unavailable"],
                       "invalid_json_response_commands": failures["invalid_json_response"],
                       "unknown_outcome_commands": failures["unrecognized_record"],
                       "exit_zero_basis": "The pinned collector records error:null and a JSON response only after process returncode0."}
            assert sum(exits.values()) + sum(failures.values()) == len(commands)
            cohorts[cohort] = {"events": expected_count, "event_checks_recomputed": dict(checks), "command_integrity": counts,
                               "authorized_snapshot_intervals_hash_verified": len(snapshot_intervals), "observer_gaps": dict(gaps),
                               "model_calls": total_calls, "model_call_components_recomputed": {
                                   "initialization": initialization_calls, "event_compilation": event_compilation_calls,
                                   "guard_intelligence": guard_calls, "total": total_calls},
                               "physical_provider_attempts": None,
                               "command_outcomes": outcome,
                               "failed_checks": {}, "run_sha256": sha(run_bytes), "assessment_sha256": sha(assessment_bytes)}

        control_bytes = archive.read("no-lore-control/control.json")
        control = json.loads(control_bytes)
        assert sha(control_bytes) == receipt["no_lore_control"]["sha256"]
        assert len(control["records"]) == control["events"] == 60
        expected_ids = {f"{project}-{index:02}" for project in ("payments", "ledger", "releases") for index in range(1, 21)}
        assert {record["event_id"] for record in control["records"]} == expected_ids
        assert control["processes_started"] == control["model_calls"] == 0
        assert control["source_integrity_pass"] is True and control["failed_events"] == []
        intervals = []
        for record in control["records"]:
            data = archive.read("no-lore-control/" + record["path"])
            assert sha(data) == record["sha256"]
            capture = json.loads(data)
            assert capture["event_id"] == record["event_id"]
            assert capture["source_integrity_pass"] is True
            assert capture["sources_before"] == capture["expected_before_sources"]
            assert capture["sources_query"] == capture["sources_after"] == capture["expected_after_sources"]
            assert len(capture["commands"]) == 4
            intervals.extend(command["integrity"] for command in capture["commands"])
        assert len(intervals) == 240
        assert all(guardian.command_integrity_valid({"source_integrity": interval}) for interval in intervals)
        assert all(interval["observer"]["source_complete"] is True for interval in intervals)

    assert receipt["execution_egress_audit"] == "not_run"
    assert receipt["guardian_alert_quality"] == "unmeasured"
    resolution_differences = {event: {"baseline_resolution_reads": count,
                                     "candidate_resolution_reads": resolution_counts["candidate"][event],
                                     "difference": count - resolution_counts["candidate"][event]}
                              for event, count in resolution_counts["baseline"].items()
                              if count != resolution_counts["candidate"][event]}
    args.output.mkdir(parents=True, exist_ok=True)
    atomic(args.output / "receipt.json", receipt_bytes)
    atomic(args.output / "guardian-integrity-evidence.zip", inner)
    result = {"schema_version": 1, "status": "verified", "kind": "final_candidate_guardian_source_integrity",
              "repository": "grove/lore", "pull_request": 47, "head_sha": workflow["head_sha"],
              "source_tree_sha": workflow["head_tree_sha"], "workflow_run": workflow,
              "runner_environment": metadata.get("runner_environment", {"image": None, "image_version": None,
                                      "runner_version": None, "rustc_version": None, "cargo_version": None,
                                      "python_version": None, "exact_toolchain_versions_observed": False}),
              "official_artifact": {"id": artifact["id"], "name": artifact["name"], "created_at": artifact["created_at"],
                                    "expires_at": artifact["expires_at"], "bytes": artifact["size_in_bytes"],
                                    "sha256": artifact["digest"].removeprefix("sha256:"),
                                    "official_digest_verified": outer is not None,
                                    "outer_digest_revalidation": "verified" if outer is not None else "not_run_retained_inner_only",
                                    "retained_origin": "Previously recorded official outer digest is provenance; it is not revalidated in retained-inner mode."},
              "verification_input_mode": "retained_inner" if args.retained_inner else "official_artifact",
              "original_receipt": bound(receipt_bytes), "synthetic_evidence_archive": bound(inner),
              "manifest_members_verified": len(manifest), "archive_members": len(manifest) + 1,
              "cohorts": cohorts, "no_lore_control": {"events": 60, "intervals": 240, "source_preserved": 240,
                                                     "source_observer_complete": 240, "processes_started": 0, "model_calls": 0, "failed_events": []},
              "baseline_candidate_resolution_read_differences": resolution_differences,
              "baseline_revision": receipt["baseline_revision"], "binary_sha256": receipt["binary_sha256"],
              "collector_source_sha256": receipt["collector_source_sha256"], "gate_source_sha256": receipt["gate_source_sha256"],
              "prepared_sha256": receipt["prepared_sha256"], "compiled_sha256": receipt["compiled_sha256"],
              "events_manifest_sha256": receipt["events_manifest_sha256"], "verifier_sha256": sha(Path(__file__).read_bytes()),
              "publication_scope": {"raw_ci_logs_included": False, "synthetic_capture": True,
                                    "providers_called_by_verifier": 0, "replay_launched_by_verifier": False},
              "limits": ["This receipt is bound to the stated PR head; later revisions need their own CI result.",
                         "Retained-inner mode verifies receipt and archive member bindings, without synthesizing an outer ZIP or claiming to revalidate its recorded origin digest.",
                         "All source observers are complete. Recorded new-directory observer gaps apply only to permitted Lore-owned state.",
                         "Guardian independent alert quality remains unmeasured; execution/egress syscall audit was not run.",
                         "Logical model calls are independently summed from every initialization, event materialization and guard intelligence response. Physical provider-attempt ledgers are not retained in this synthetic replay archive, so that independent field remains null.",
                         "The original historical source-restoration failures are not relabeled or attributed to a writer by this clean run.",
                         "Platform tests and release-mode CLI smoke results are recorded separately; no raw CI log is included."]}
    payload = (json.dumps(result, indent=2, sort_keys=True) + "\n").encode()
    atomic(args.output / "guardian-verification.json", payload)
    print(json.dumps({"head_sha": result["head_sha"], "verification_receipt": bound(payload), "evidence": bound(inner),
                      "counts": {name: {"events": value["events"], "commands": value["command_integrity"]} for name, value in cohorts.items()},
                      "no_lore_control": result["no_lore_control"], "binary_sha256": result["binary_sha256"]}, indent=2))


if __name__ == "__main__":
    main()
