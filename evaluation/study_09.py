#!/usr/bin/env python3
"""Thin empirical-study preparation/inspection/reporting; execution remains experiment_08."""
import argparse
import json
import os
from pathlib import Path
from urllib.parse import quote

import adaptive_tasks as adaptive
import cross_source as cross
import decision_task_intake_09 as intake
import experiment_08 as experiment
import provider_usage as metering


def prepare(cases: Path, output: Path) -> dict:
    if not output.is_absolute():
        raise ValueError("Use an explicit absolute output path")
    output = cross.selected_path(output)
    if not cases.is_absolute() or not output.is_absolute() or output.is_relative_to(Path(__file__).resolve().parents[1]):
        raise ValueError("Use explicit absolute inputs and fresh private output outside the checkout")
    result = intake.prepare(cases, output)
    return {**status(output), "intake": result["intake"]}


def status(bundle: Path) -> dict:
    checked = experiment.preflight(bundle, fresh=False)
    # Do not publish private paths or validation exception excerpts.
    blockers = [{"gate": item["gate"], "passed": False} for item in checked["blockers"]]
    classification = "unprepared"
    try:
        prepared = experiment.read(bundle / "prepared.json")
        classification = "offline_fixture" if prepared.get("fixture_only") is not False else "candidate_unreviewed"
    except (ValueError, OSError, KeyError, TypeError):
        pass
    intake_ready = any(item["gate"] == "independent_decision_task_intake" and item["passed"]
                       for item in checked["checks"])
    return {"schema_version": 1, "protocol": checked["protocol"], "classification": classification,
            "status": "ready_for_external_execution" if checked["preflight_passed"] else "not_run",
            "pilot": "ready_external" if checked["preflight_passed"] and intake_ready else "blocked_external",
            "tasks": checked["tasks"], "arms": list(adaptive.SETUPS),
            "initial_assignments": checked["initial_assignments"],
            "preflight_passed": checked["preflight_passed"], "blockers": blockers,
            "provider_calls": 0, "subprocess_calls": 0,
            "coding_productivity": "unmeasured", "productivity_benefit_established": False}


def report(bundle: Path) -> dict:
    bundle = cross.selected_path(bundle)
    value = status(bundle)
    value.update(conclusion="not_yet_measurable", attempted_assignments=0,
        completed_checked_assignments=0, started_assignments=0, not_started_assignments=value["initial_assignments"],
        attempted_invocations=0, reported_successes=0, observed_study_wall_seconds=None,
        captured_provider_usage=metering.normalized(None), billing_status="budget_unverifiable",
        integrity_qualified=False, blinded_reviews_complete=False, audit_complete=False,
        interpretation="pilot_diagnostic_exploratory", paired_tasks=0, baseline_comparisons={}, usage_coverage={},
        arm_totals={}, identities=None,
        target={"success_gain_percentage_points": 10, "all_in_time_or_cost_reduction_percent": 20,
                "noninferior_correctness": True, "no_increase_in_critical_violations": True},
        private_artifacts={name: name for name in ("SEAL.json", "PREFLIGHT_AT_LAUNCH.json", "EXECUTION.json",
            "RETENTION.json", "RESULT.json", "run/metrics.json", "run/INTEGRITY_AUDIT.json", "run/CODEX_IDENTITIES.json")})
    if not (bundle / "EXECUTION.json").exists():
        return value
    try:
        result = experiment.assess(bundle, persist=False)
        retained, comparison = result["retention"], result["comparison"]
        sealed = experiment.read(bundle / "SEAL.json")["contract"]
        value.update(status="retained_execution", initial_assignments=retained["planned_initial_assignments"],
            attempted_assignments=retained["attempted_assignments"],
            completed_checked_assignments=retained["completed_checked_assignments"],
            started_assignments=sum(row["status"] != "not_started" for row in retained["assignments"]),
            not_started_assignments=sum(row["status"] == "not_started" for row in retained["assignments"]),
            attempted_invocations=retained["attempted_invocations"], reported_successes=retained["reported_successes"],
            observed_study_wall_seconds=retained["observed_study_wall_seconds"],
            captured_provider_usage=retained["captured_provider_usage"],
            invocation_status_counts={state: sum(item["status"] == state for row in retained["assignments"] for item in row["attempts"])
                for state in ("completed", "failed", "refused", "timed_out", "cancelled", "interrupted", "not_started")},
            blinded_reviews_complete=comparison.get("independent_answer_reviews_complete") is True,
            audit_complete=comparison.get("integrity_audit", {}).get("passed") is True,
            integrity_qualified="decision_intake" in sealed and result["mechanical_comparison_complete"]
                and comparison.get("independent_validation_complete") is True)
        spec = experiment.specification(bundle)
        value["interpretation"] = "pilot_diagnostic_exploratory" if spec["mode"] == "pilot" else "heldout_uncertainty_reported"
        # Model names/revisions stay private; exact identities are content digests.
        value["identities"] = {key: sealed[key] for key in ("collector_files_sha256", "lore_binary_sha256",
            "agent_command_sha256", "source_sha256_by_case")}
        value["identities"]["models_sha256"] = cross.digest(sealed["model_configuration"]["providers"])
        outcomes = comparison.get("iteration_outcomes", {})
        value.update(paired_tasks=outcomes.get("paired_tasks", 0), baseline_comparisons=outcomes.get("baseline_comparisons", {}),
                     usage_coverage=outcomes.get("usage_coverage", {}))
        for arm in adaptive.SETUPS:
            total = comparison.get("setups", {}).get(arm, {})
            value["arm_totals"][arm] = {key: total.get(key) for key in ("tasks", "passed_tasks", "failed_constraint_checks",
                "total_seconds", "warmup_context_seconds", "served_context_seconds", "total_usage")}
        if value["integrity_qualified"] and spec["mode"] == "full" and value["paired_tasks"] >= 30:
            value["conclusion"] = "inconclusive"
            value["coding_productivity"] = "independently_reviewed_task_attempts"
            value["qualification"] = "Interpret the preregistered paired intervals with independent reviewers; the tool does not automatically declare victory. Conditional project/task intervals do not prove benefit in everyday interactive workflows."
        if value["integrity_qualified"] and all((total.get("total_usage") or {}).get("billed_cost_usd") is not None
                for total in comparison.get("setups", {}).values()) and len(comparison.get("setups", {})) == 6:
            value["billing_status"] = "observed_billing_requires_external_reconciliation"
        if not value["integrity_qualified"]:
            value["blockers"].append({"gate": "independently_reviewed_complete_comparison", "passed": False})
    except (ValueError, OSError, KeyError, TypeError):
        # A broken source/ledger binding is not a saved success to display.
        value.update(status="invalid_retained_evidence", integrity_qualified=False)
        value["blockers"].append({"gate": "retained_evidence_binding", "passed": False})
        try:
            retained = experiment.retention(bundle, experiment.read(bundle / "EXECUTION.json"))
            value.update(initial_assignments=retained["planned_initial_assignments"],
                attempted_assignments=retained["attempted_assignments"],
                completed_checked_assignments=retained["completed_checked_assignments"],
                started_assignments=sum(row["status"] != "not_started" for row in retained["assignments"]),
            not_started_assignments=sum(row["status"] == "not_started" for row in retained["assignments"]),
                attempted_invocations=retained["attempted_invocations"])
        except (ValueError, OSError, KeyError, TypeError):
            pass
    return value


def markdown(value: dict) -> str:
    lines = ["# Lore 0.9 empirical study", "", f"Status: {value['status']}. Productivity: {value['coding_productivity']}.",
             f"Planned assignments: {value['initial_assignments']} across six mandatory arms.", "",
             "Current launch blockers (retained assessment uses launch-time evidence):"]
    lines += ["- " + item["gate"] for item in value.get("blockers", [])] or ["- None; readiness is not an outcome."]
    if "attempted_assignments" in value:
        lines += ["", f"Started: {value['started_assignments']}; attempted: {value['attempted_assignments']}/{value['initial_assignments']}; checked: {value['completed_checked_assignments']}; unstarted: {value['not_started_assignments']}.",
                  f"Reported successes: {value['reported_successes']}; conclusion: {value['conclusion']}.",
                  f"Billing: {value['billing_status']}. Unknown amounts remain null.", "",
                  "| Arm vs baseline | Success difference (95% CI) | Critical check difference (95% CI) | All-in time ratio (95% CI) | Billed cost ratio (95% CI) |",
                  "| --- | --- | --- | --- | --- |"]
        for arm in adaptive.SETUPS[1:]:
            row = value["baseline_comparisons"].get(arm, {})
            cells = []
            for field in ("passed", "failed_constraint_checks", "total_seconds", "billed_cost_usd"):
                metric = row.get(field, {})
                cells.append(f"{metric.get('estimate')} ({metric.get('ci95')}); pairs={metric.get('pairs', 0)}, coverage={metric.get('coverage')}")
            lines.append("| " + arm + " | " + " | ".join(cells) + " |")
        lines += ["", "| Arm | Checked tasks | Successes | Critical failed checks | All-in seconds | Known input/output tokens | Observed billed USD |",
                  "| --- | --- | --- | --- | --- | --- | --- |"]
        for arm in adaptive.SETUPS:
            total = value["arm_totals"].get(arm, {})
            usage = total.get("total_usage") or {}
            lines.append("| " + arm + " | " + " | ".join(str(total.get(key)) for key in
                ("tasks", "passed_tasks", "failed_constraint_checks", "total_seconds")) +
                f" | {usage.get('input_tokens')}/{usage.get('output_tokens')} | {usage.get('billed_cost_usd')} |")
        lines += ["", "Private retained artifacts:", ""]
        lines += [f"- [{name}]({quote(value['private_artifacts'][name])})" for name in value["private_artifacts"]]
        lines += ["", "Pilot results are diagnostic. At least 30 untouched tasks with independent review, audit and paired intervals are required for stronger conclusions.",
                  "The file-proposal protocol does not reproduce everyday interactive Codex work."]
    return "\n".join(lines) + "\n"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "status", "doctor", "report"))
    parser.add_argument("bundle", type=Path, nargs="?")
    parser.add_argument("--cases", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare" and args.cases and args.output:
            value = prepare(args.cases, args.output)
        elif args.command in ("status", "doctor", "report") and args.bundle:
            value = report(args.bundle) if args.command == "report" else status(args.bundle)
        else:
            raise ValueError("Select a bundle or explicit preparation inputs/output")
        if args.command == "report" and args.output:
            if not args.output.is_absolute():
                raise ValueError("Use an absolute report directory")
            output = cross.selected_path(args.output)
            if output.is_relative_to(Path(__file__).resolve().parents[1]) or output.exists():
                raise ValueError("Use a fresh private report directory")
            output.mkdir(parents=True, mode=0o700)
            exported = {**value, "private_artifacts": {name: os.path.relpath(args.bundle / path, output).replace(os.sep, "/")
                for name, path in value["private_artifacts"].items()}}
            experiment.private_write(output / "RESULT.json", exported, exclusive=True)
            descriptor = os.open(output / "REPORT.md", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
                handle.write(markdown(exported))
        print(markdown(value) if args.format == "markdown" else json.dumps(value, indent=2, sort_keys=True))
        if args.command == "prepare":
            return 0
        if args.command == "report":
            return 2 if value["status"] == "invalid_retained_evidence" else 0
        return 0 if value["preflight_passed"] else 2
    except (ValueError, OSError, KeyError, TypeError):
        print(json.dumps({"status": "invalid", "coding_productivity": "unmeasured"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
