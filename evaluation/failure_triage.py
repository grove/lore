#!/usr/bin/env python3
"""Bind shared-intelligence changes to independently checked coding failures.

This operator-side tool diagnoses retained attempts; it never sends checker
answers to an agent or changes Lore's runtime policy. No live pilot is invented
when a provider, independent review or execution audit is unavailable.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import sys

import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision
import outcome_protocol as outcome

MODULES = (
    "src/context/adaptive.rs", "src/context/intelligence.rs",
    "src/context/decision/runtime.rs", "src/context/memory.rs",
    "src/context.rs", "src/context/retrieval.rs",
)


def task_content_sha256(case: dict) -> str:
    """Compare the task contract independently of case/project IDs and roots."""
    return cross.digest({
        "task": " ".join(case["task"].split()).casefold(),
        "editable_files": sorted(coding.relative_file(name) for name in case["editable_files"]),
        "critical_constraints": sorted(" ".join(value.split()).casefold()
                                       for value in case["critical_constraints"]),
    })


def source_content_sha256(manifest: dict) -> str:
    """Renaming paths cannot make byte-identical source snapshots new inputs."""
    files = manifest["files_sha256"]
    if (not isinstance(files, dict) or not files or manifest["sha256"] != cross.digest(files)
            or any(not isinstance(value, str) or not outcome.SHA256.fullmatch(value)
                   for value in files.values())):
        raise ValueError("Task identity requires an intact source-file manifest")
    return cross.digest(sorted(files.values()))


def pilot_task_bindings(report: dict) -> dict:
    entries = report.get("cases", [])
    expected = {(sample["project"], sample["case_id"]) for sample in report["samples"]}
    bound = {(entry["case"]["project"], entry["case"]["id"]) for entry in entries}
    return {
        "complete": bool(entries) and bound == expected and len(bound) == len(entries),
        "task_content_sha256": sorted({task_content_sha256(entry["case"]) for entry in entries}),
        "source_sha256": sorted({sample["source_manifest"]["sha256"] for sample in report["samples"]}),
        "source_content_sha256": sorted({source_content_sha256(entry["source_manifest"])
                                         for entry in entries}),
    }


def summarize_validated(report: dict, assessment: dict, directory: Path) -> dict:
    """Called only after the adaptive assessor validates full retained artifacts."""
    if assessment.get("mechanical_contracts_passed") is not True:
        raise ValueError("Repair triage requires an intact, matched attempt ledger")
    binding = cross.digest(report)
    failures, counts = [], Counter()
    registration_path = directory / "PREREGISTRATION.json"
    registration = cross.read_json(registration_path) if registration_path.is_file() else {}
    excluded = [registration.get("operator_id", "")]
    for case in registration.get("cases", []):
        excluded.extend(case.get("task_authors", []))
        excluded.extend(case.get("checker_authors", []))
    annotation_count = independent_review_count = 0
    independent_reviews_complete = bool(report["samples"])
    for sample in report["samples"]:
        checks = sample["tests"]["checks"]
        failed = [check for check in checks if not check["passed"]]
        attempts = sample.get("attempts", [])
        classes = set()
        if any(check["kind"] == "constraint" for check in failed):
            classes.add("independently_checked_constraint_failure")
        if any(check["kind"] != "constraint" for check in failed):
            classes.add("independently_checked_functional_failure")
        if sample.get("attempt_stop_reason") == "wall_time_limit":
            classes.add("bounded_attempt_time_exhausted")
        if len(attempts) > 1:
            classes.add("correction_loop_required")
        context = sample.get("context")
        core = adaptive.core(context["response"], sample["setup"]) if context else {}
        if core.get("mode") == "fast_fallback":
            classes.add("fallback_received_not_intelligent_advice")
        review_path = directory / "reviews" / (sample["sample_id"] + ".json")
        packet = cross.read_json(review_path) if review_path.is_file() else {}
        annotations = decision.validated_reviews(packet, sample, binding) if packet else []
        review_status = (adaptive.answer_review_status(directory, packet, sample, binding, annotations,
                                                      excluded=excluded)
                         if annotations else {"complete": False, "reason": "Answer annotations are pending"})
        reviews = annotations if review_status["complete"] else []
        annotation_count += len(annotations)
        independent_review_count += len(reviews)
        independent_reviews_complete &= review_status["complete"]
        for field in ("material_decision_mistakes", "missed_critical_constraints", "severe_unsupported_project_assertions"):
            if any(review.get(field) for review in reviews):
                classes.add("reviewed_" + field)
        if not classes:
            continue
        counts.update(classes)
        failures.append({
            "case_id": sample["case_id"], "project": sample["project"], "setup": sample["setup"],
            "sample_id": sample["sample_id"], "source_manifest_sha256": sample["source_manifest"]["sha256"],
            "classes": sorted(classes), "failed_check_count": len(failed),
            "correct_final_patch": not failed, "attempt_count": len(attempts) or 1,
            "total_seconds": sample["costs"]["total_seconds"],
            "total_usage": sample["costs"]["total_usage"],
            "annotation_count": len(annotations),
            "independent_review_count": len(reviews),
            "independent_review_status": review_status,
            "cause": "Observed failure category; causation requires matched intervention and held-out checks.",
        })
    live = (assessment.get("independent_validation_complete") is True
            and assessment.get("integrity_audit", {}).get("passed") is True
            and assessment.get("fixture_only") is False and independent_reviews_complete)
    return {
        "schema_version": 1, "metrics_sha256": binding,
        "classification": "reviewed_actual_model_pilot" if live else "diagnostic_only",
        "observed_cases": failures,
        "pilot_case_ids": sorted({sample["case_id"] for sample in report["samples"]}),
        "pilot_task_bindings": pilot_task_bindings(report),
        "annotation_count": annotation_count,
        "independent_review_count": independent_review_count,
        "independent_reviews_complete": independent_reviews_complete,
        "dominant_failure_classes": [{"class": name, "samples": count} for name, count in counts.most_common()],
        "eligible_runtime_modules": list(MODULES),
        "algorithm_tuning_ready": live and bool(failures),
        "product_improvement_established": False,
        "next_step": "Choose one recorded failure and test one bounded change on a separate holdout."
            if live else "Collect the actual model pilot, independent reviews and execution/egress audit using the existing runner.",
        "qualification": "Check failures and repeated loops identify work to inspect, not their cause. Provider fallback, unknown usage and missing reviews stay visible; no speculative runtime step is added.",
    }


def triage(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    assessment = adaptive.assess(directory)
    report = cross.read_json(directory / "metrics.json")
    return summarize_validated(report, assessment, directory)


def validate_holdout(prepared_path: Path, heldout: list[str], diagnostics: dict) -> dict:
    """Validate prepared identities and bytes without reading held-out outcomes."""
    path = cross.selected_path(prepared_path)
    if path.is_dir():
        path /= "prepared.json"
    cross.reject_symlink_path(path)
    if path.stat().st_size > 4_194_304:
        raise ValueError("Prepared holdout manifest exceeds 4 MiB")
    prepared = cross.read_json(path)
    if (prepared.get("schema_version") != 1 or prepared.get("phase") != "preparation_only"
            or prepared.get("inference_calls") != 0):
        raise ValueError("Holdout binding requires a preparation-only task manifest")
    entries = prepared.get("cases")
    if not isinstance(entries, list) or not 1 <= len(entries) <= 1000:
        raise ValueError("Prepared holdout needs 1..1000 source-bound tasks")
    manifest_name = prepared.get("cases_manifest")
    if not isinstance(manifest_name, str) or not Path(manifest_name).is_absolute():
        raise ValueError("Prepared holdout must retain its original cases manifest path")
    manifest_path = cross.selected_path(Path(manifest_name))
    if coding.hash_file(manifest_path) != prepared.get("cases_manifest_sha256"):
        raise ValueError("Holdout case definitions changed after preparation")
    definitions = coding.load_cases(manifest_path)
    declared = {case["id"]: case for case in definitions["cases"]}
    indexed = {entry["case"]["id"]: entry for entry in entries}
    if len(indexed) != len(entries) or set(indexed) != set(declared):
        raise ValueError("Prepared holdout cohort differs from its bound case definitions")
    if not set(heldout) <= set(indexed):
        raise ValueError("Proposed held-out IDs are absent from the prepared cohort")
    pilot = diagnostics.get("pilot_task_bindings", {})
    if pilot.get("complete") is not True:
        raise ValueError("Content-bound pilot tasks are required to check holdout overlap")
    known = coding.bundled_fingerprints() | cross.bundled_fingerprints()
    rows = []
    for identity in heldout:
        entry = indexed[identity]
        case = entry["case"]
        if case != declared[identity]:
            raise ValueError("Prepared holdout task differs from its original definition")
        snapshot = path.parent / coding.relative_file(entry["source_root"])
        actual = cross.fingerprint(snapshot)
        if actual != entry["source_manifest"]:
            raise ValueError("Prepared holdout source bytes changed")
        if (definitions.get("fixture_only") is not False or prepared.get("fixture_only") is not False
                or "base_project" in case or actual["sha256"] in known):
            raise ValueError("Public fixtures cannot establish a new held-out task plan")
        pin = outcome.source_provenance(entry)
        if not pin["complete"]:
            raise ValueError("Holdout requires an exact upstream pin and bound input bytes")
        if not entry.get("checker_files_sha256"):
            raise ValueError("Holdout requires preparation-time checker-file identities")
        coding.validate_prepared_checker(entry, snapshot, manifest_path.parent)
        task_digest = task_content_sha256(case)
        content_digest = source_content_sha256(actual)
        if (actual["sha256"] in pilot["source_sha256"]
                or content_digest in pilot["source_content_sha256"]
                or task_digest in pilot["task_content_sha256"]):
            raise ValueError("Renaming a pilot task or reusing its source bytes cannot create a holdout")
        rows.append({"case_id": identity, "task_content_sha256": task_digest,
                     "source_sha256": actual["sha256"], "source_content_sha256": content_digest,
                     "task_contract_sha256": outcome.task_contract_sha256(entry),
                     "source_provenance": pin})
    return {"complete": True, "status": "content_bound_holdout_plan",
            "prepared_sha256": coding.hash_file(path), "cases_manifest_sha256": coding.hash_file(manifest_path),
            "cases": rows, "independent_review_complete": False, "outcomes_read": False,
            "qualification": "Pinned prepared content and pilot non-overlap are checked; flags and different IDs do not establish independent task authorship or independent holdout review."}


def validate_change(proposal: dict, diagnostics: dict, holdout_prepared: Path | None = None) -> dict:
    if proposal.get("schema_version") != 1 or proposal.get("pilot_metrics_sha256") != diagnostics["metrics_sha256"]:
        raise ValueError("Change proposal must bind the exact assessed pilot")
    modules = proposal.get("modules")
    if not isinstance(modules, list) or not modules or len(set(modules)) != len(modules) or not set(modules) <= set(MODULES):
        raise ValueError("Select the existing shared engine modules affected by this change")
    cases = proposal.get("failing_samples")
    observed = {row["sample_id"]: row for row in diagnostics["observed_cases"]}
    if not isinstance(cases, list) or not cases or len(set(cases)) != len(cases) or not set(cases) <= set(observed):
        raise ValueError("Every proposed change needs recorded failing sample IDs")
    for field in ("expected_corrected_behavior", "bounded_change", "cost_risk", "heldout_plan"):
        value = proposal.get(field)
        if not isinstance(value, str) or not value.strip() or len(value) > 4000:
            raise ValueError("Missing bounded proposal field: " + field)
    heldout = proposal.get("heldout_case_ids")
    pilot = set(diagnostics["pilot_case_ids"])
    if not isinstance(heldout, list) or not heldout or any(not isinstance(item, str) or not item.strip() for item in heldout):
        raise ValueError("Specify distinct held-out task IDs before tuning")
    if len(set(heldout)) != len(heldout) or set(heldout) & pilot:
        raise ValueError("Pilot failures cannot serve as the held-out evaluation")
    holdout = (validate_holdout(holdout_prepared, heldout, diagnostics) if holdout_prepared is not None
               else {"complete": False, "status": "heldout_plan_unverified",
                     "independent_review_complete": False, "outcomes_read": False,
                     "reason": "Distinct IDs alone do not bind new task/source content; supply --holdout-prepared."})
    return {"schema_version": 1, "proposal_sha256": cross.digest(proposal),
            "pilot_metrics_sha256": diagnostics["metrics_sha256"],
            "proposal_complete": True,
            "heldout_plan_unverified": not holdout["complete"], "holdout": holdout,
            "algorithm_tuning_ready": diagnostics["algorithm_tuning_ready"] is True and holdout["complete"],
            "measured_improvement": False,
            "reason": "An intervention needs an actual independently reviewed pilot and a content-bound, nonoverlapping holdout plan. Independent holdout review and measured outcomes are subsequent gates."}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    inspect = commands.add_parser("triage")
    inspect.add_argument("run", type=Path)
    validate = commands.add_parser("validate-change")
    validate.add_argument("run", type=Path)
    validate.add_argument("proposal", type=Path)
    validate.add_argument("--holdout-prepared", type=Path,
                          help="Preparation directory or prepared.json with pinned holdout source/task identities")
    args = parser.parse_args(argv)
    try:
        result = triage(args.run)
        if args.command == "validate-change":
            result = validate_change(cross.read_json(args.proposal), result, args.holdout_prepared)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"Failure triage rejected: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
