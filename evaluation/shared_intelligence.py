#!/usr/bin/env python3
"""Collect the real shared human/agent experience and recheck its hard contracts.

Reuses the existing coding snapshots, source fingerprints, evidence resolver,
and exact observation verifier. Preparation and assessment perform no inference.
Mechanical correctness is separate from independently measured user outcomes.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import time

import benchmark as bench
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision

PROTOCOL = "shared-intelligence-v1"
EXPERIENCES = ("fast", "schema3", "schema4", "adaptive", "onboard", "adaptive_no_inspect", "onboard_no_inspect")
HUMAN_CRITERIA = ("purpose_accuracy", "concepts_and_architecture", "workflow_accuracy",
                  "critical_conditions_preserved", "source_attribution", "useful_next_step")


def intelligence(response: dict, experience: str) -> dict:
    if experience.startswith(("adaptive", "onboard")):
        value = response.get("intelligence")
        if not isinstance(value, dict):
            raise ValueError("Shared experience lacks its nested intelligence result")
        return value
    return response


def invocation_calls(response: dict, experience: str) -> int:
    # Human presentation performs its own bounded generation and support check
    # after the shared core; those calls must not disappear from cost records.
    return cross.model_calls(response if experience.startswith("onboard") else intelligence(response, experience))


def arguments(experience: str, task: str, max_tokens: int) -> list[str]:
    if experience not in EXPERIENCES:
        raise ValueError("Unknown shared experience")
    if experience.startswith("onboard"):
        result = ["onboard", "--topic", task, "--max-tokens", str(max_tokens)]
    else:
        result = ["context", task, "--max-tokens", str(max_tokens)]
        if experience == "fast":
            result.append("--fast")
        else:
            schema = "3" if experience == "schema3" else "4" if experience == "schema4" else "5"
            result.extend(["--schema-version", schema])
    if experience.endswith("_no_inspect"):
        result.append("--no-inspect")
    return result


def invocation_environment(project: Path, *, allow_inspection: bool, allow_hosted: bool,
                           allow_checkout_egress: bool) -> dict[str, str]:
    """Only explicit collector options establish this run's standing envelope.

    Never write a grant into project evidence or mutate the parent environment.
    Other inherited variables, including provider credentials, remain private.
    """
    environment = dict(os.environ)
    for name in ("LORE_INSPECTION_ROOT", "LORE_ALLOW_HOSTED_EGRESS", "LORE_ALLOW_CHECKOUT_EGRESS"):
        environment.pop(name, None)
    if allow_inspection:
        environment["LORE_INSPECTION_ROOT"] = str(project.resolve(strict=True))
    if allow_hosted:
        environment["LORE_ALLOW_HOSTED_EGRESS"] = "1"
    if allow_checkout_egress:
        environment["LORE_ALLOW_CHECKOUT_EGRESS"] = "1"
    return environment


def snapshot_manifest(response: dict) -> dict:
    snapshot = response.get("snapshot")
    if not isinstance(snapshot, dict) or set(snapshot) != {"project_id", "registry_revision"}:
        raise ValueError("Shared response needs the versioned project/revision manifest")
    if any(not isinstance(value, str) or not value for value in snapshot.values()):
        raise ValueError("Shared response has an empty project/revision identity")
    return snapshot


def generated_reference_ids(value, prefix: str) -> set[str]:
    """Only generated claim text is a citation; caller goals and excerpts are data."""
    ids = set()
    if isinstance(value, dict):
        if "basis" in value and isinstance(value.get("text"), str):
            ids.update(word for word in re.findall(r"\w+", value["text"]) if word.startswith(prefix))
        for child in value.values():
            ids.update(generated_reference_ids(child, prefix))
    elif isinstance(value, list):
        for child in value:
            ids.update(generated_reference_ids(child, prefix))
    return ids


def verify_shared_references(response: dict, experience: str, source: Path) -> dict:
    core = intelligence(response, experience)
    checked = decision.verify_observations(core, source)
    if experience.startswith("onboard"):
        presentation = {key: value for key, value in response.items() if key != "intelligence"}
        # Human presentation must use the same closed evidence manifest as the
        # agent core. A valid ID elsewhere in the registry is insufficient.
        evidence = cross.cited_ids({"evidence": core.get("evidence", []),
                                    "imported_evidence": core.get("imported_evidence", [])})
        required = cross.cited_ids(presentation)
        required |= generated_reference_ids(presentation, "ev_") | generated_reference_ids(presentation, "ne_")
        if not required <= evidence:
            raise ValueError("Human presentation cites evidence absent from the shared manifest")
        observations = {item["id"] for item in core.get("inspection", {}).get("observations", [])}
        observations |= decision.retained_observation_ids(core)
        required_observations = decision.observation_ids(presentation) | generated_reference_ids(presentation, "co_")
        if not required_observations <= observations:
            raise ValueError("Human presentation cites an unmanifested observation")
    return checked


def budget_valid(response: dict) -> bool:
    budget = response.get("budget", {})
    return (isinstance(budget, dict) and type(budget.get("max_tokens")) is int
            and type(budget.get("used_tokens")) is int
            and 0 <= budget["used_tokens"] <= budget["max_tokens"]
            and isinstance(budget.get("tokenizer"), str) and bool(budget["tokenizer"]))


def response_checks(response: dict, experience: str, task: str, source: Path) -> dict:
    core = intelligence(response, experience)
    shared = experience.startswith(("adaptive", "onboard"))
    human = experience.startswith("onboard")
    schema = 1 if human else 5 if shared else 2 if experience == "fast" else int(experience[-1])
    checks = {
        "explicit_schema": response.get("schema_version") == schema,
        "shared_core_schema": not shared or core.get("schema_version") == 4,
        "task_bound": human or core.get("task") == task,
        "reported_output_budget_valid": budget_valid(response),
        "legacy_fast_model_free": experience != "fast" or cross.model_calls(core) == 0,
        "model_call_accounting": cross.model_calls(core) >= 0,
    }
    if shared:
        snapshot_manifest(response)
        checks["shared_snapshot_manifest"] = True
    if shared:
        capabilities = response.get("capabilities", {})
        checks["no_execution_capability"] = capabilities.get("execution") is False and capabilities.get("source_write") is False
        checks["explicit_inspection_capability"] = capabilities.get("inspection") in (
            "granted", "not_granted", "disabled_by_caller", "outside_grant")
    if human:
        orientation = response.get("orientation")
        checks["human_orientation_present"] = isinstance(orientation, dict) and all(
            key in orientation for key in ("purpose", "concepts", "architecture", "workflow", "constraints", "next_exploration"))
        checks["generation_basis_explicit"] = isinstance(response.get("generation_basis"), str) and bool(response["generation_basis"])
        presentation_calls = response.get("presentation_model_calls")
        checks["complete_human_call_accounting"] = (type(presentation_calls) is int and presentation_calls >= 0
            and invocation_calls(response, experience) == cross.model_calls(core) + presentation_calls)
    if core.get("schema_version") == 4:
        egress = core.get("checkout_egress", {})
        checks["checkout_egress_contract"] = (type(egress.get("allowed")) is bool
            and type(egress.get("model_received_checkout")) is bool
            and (not egress["model_received_checkout"] or egress["allowed"]))
        modes = ("intelligent", "fast_fallback", "reference") if shared else ("intelligent", "fast_fallback")
        checks["honest_generation_mode"] = core.get("mode") in modes
        if core.get("mode") == "reference":
            checks["reference_bypasses_investigation"] = (cross.model_calls(core) == 0
                and core.get("inspection_status") == "not_needed"
                and not core.get("inspection", {}).get("observations"))
    elif core.get("schema_version") == 3:
        checks["honest_generation_mode"] = core.get("mode") in ("intelligent", "fast_fallback")
    verify_shared_references(response, experience, source)
    checks["exact_source_observations_and_closed_human_references"] = True
    if experience.endswith("_no_inspect"):
        inspection = core.get("inspection", {})
        checks["inspection_veto"] = (not inspection.get("observations")
            and inspection.get("budget", {}).get("files_read", 0) == 0
            and not core.get("checkout_egress", {}).get("model_received_checkout", False))
    return checks


def collect(binary: str, project: Path, source: Path, experience: str, task: str,
            timeout: int, max_tokens: int, *, env: dict[str, str] | None = None) -> dict:
    before, sources_before = coding.registry_content(project), decision.source_state(project)
    argv = arguments(experience, task, max_tokens)
    response, elapsed = bench.subprocess_json(binary, project, *argv, timeout=timeout, env=env)
    repeated, repeat_elapsed = bench.subprocess_json(binary, project, *argv, timeout=timeout, env=env)
    core, repeated_core = intelligence(response, experience), intelligence(repeated, experience)
    ids = cross.cited_ids(response) | cross.cited_ids(repeated)
    citations = cross.resolve_citations(binary, project, ids, timeout)
    record = {
        "arguments": argv, "requested_max_tokens": max_tokens,
        "host_grants": {"inspection_root": (env or {}).get("LORE_INSPECTION_ROOT"),
            "hosted_egress": (env or {}).get("LORE_ALLOW_HOSTED_EGRESS") == "1",
            "checkout_egress": (env or {}).get("LORE_ALLOW_CHECKOUT_EGRESS") == "1"},
        "response": response, "response_sha256": cross.digest(response),
        "repeat_response": repeated, "repeat_response_sha256": cross.digest(repeated),
        "elapsed_seconds": elapsed, "repeat_elapsed_seconds": repeat_elapsed,
        "usage": coding.usage(response.get("usage", core.get("usage")), calls=invocation_calls(response, experience)),
        "repeat_usage": coding.usage(repeated.get("usage", repeated_core.get("usage")), calls=invocation_calls(repeated, experience)),
        "registry_before": before, "registry_after": coding.registry_content(project),
        "checkout_before": sources_before, "checkout_after": decision.source_state(project),
        "citation_integrity": citations,
    }
    record["checks"] = context_checks(record, experience, task, source)
    return record


def context_checks(record: dict, experience: str, task: str, source: Path) -> dict:
    answers = (record["response"], record["repeat_response"])
    checks = {}
    for label, response in zip(("first", "repeat"), answers):
        checks.update({f"{label}_{key}": value for key, value in response_checks(response, experience, task, source).items()})
    expected_files = cross.fingerprint(source)["files_sha256"]
    checks.update({
        "invocation_bound": record["arguments"] == arguments(experience, task, record["requested_max_tokens"])
            and all(answer.get("budget", {}).get("max_tokens") == record["requested_max_tokens"] for answer in answers),
        "response_hashes": record["response_sha256"] == cross.digest(answers[0])
            and record["repeat_response_sha256"] == cross.digest(answers[1]),
        "source_registry_preserved": cross.valid_registry(record["registry_before"])
            and cross.valid_registry(record["registry_after"])
            and record["registry_before"] == record["registry_after"],
        "checkout_sources_preserved": record["checkout_before"] == record["checkout_after"],
        "checkout_matches_original": {path: value["sha256"] for path, value in record["checkout_before"].items()} == expected_files,
        "citations_resolve": coding.resolver_complete(record["citation_integrity"], cross.cited_ids(answers[0]) | cross.cited_ids(answers[1])),
        "legacy_fast_repeat_identical": experience != "fast" or answers[0] == answers[1],
    })
    first_core, repeated_core = (intelligence(answer, experience) for answer in answers)
    checks["intelligent_repeat_reuses_guidance"] = (experience == "fast" or first_core.get("mode") != "intelligent"
        or (repeated_core.get("mode") == "intelligent" and repeated_core.get("cache_status") == "hit"
            and cross.model_calls(repeated_core) == 0
            and first_core.get("brief", {}).get("preferred_approach") == repeated_core.get("brief", {}).get("preferred_approach")))
    if experience.startswith(("adaptive", "onboard")):
        checks["repeat_snapshot_identical"] = snapshot_manifest(answers[0]) == snapshot_manifest(answers[1])
        grants = record["host_grants"]
        checks["host_grant_ceiling"] = all(
            (answer["capabilities"].get("inspection") != "granted" or bool(grants.get("inspection_root")))
            and (answer["capabilities"].get("hosted_egress") is not True or grants.get("hosted_egress") is True)
            and (answer["capabilities"].get("checkout_egress") is not True or grants.get("checkout_egress") is True)
            for answer in answers)
    return checks


def review_template(report: dict, sample: dict) -> dict:
    return {
        "schema_version": 1, "protocol": PROTOCOL, "metrics_sha256": cross.digest(report),
        "case_id": sample["case_id"], "source_manifest": sample["source_manifest"],
        "onboard_sha256": sample["experiences"]["onboard"]["response_sha256"],
        "assessments": [{"complete": False, "reviewer": "", "reviewed_at": "",
            "criteria": {name: {"score_0_to_3": None, "notes": ""} for name in HUMAN_CRITERIA},
            "severe_unsupported_claims": None} for _ in range(2)],
        "qualification": "Independent source review of orientation. This is not a learner study, a coding-task result, or proof of transfer.",
    }


def review_status(directory: Path, report: dict, sample: dict) -> dict:
    path = directory / "reviews" / f"{sample['case_id']}.json"
    if not path.is_file():
        return {"complete": False, "passed": False, "issues": ["Source review not supplied"]}
    cross.reject_symlink_path(path)
    review, expected = cross.read_json(path), review_template(report, sample)
    errors, reviewers, scores, severe = [], set(), [], []
    if any(review.get(key) != value for key, value in expected.items() if key != "assessments"):
        errors.append("Source review does not match the collected sources and outputs")
    slots = review.get("assessments")
    if not isinstance(slots, list) or len(slots) != 2:
        return {"complete": False, "passed": False, "issues": errors + ["Two independent source reviews required"]}
    for slot in slots:
        if not isinstance(slot, dict):
            errors.append("Malformed source review")
            continue
        reviewer = slot.get("reviewer")
        if not isinstance(reviewer, str) or not reviewer.strip() or reviewer.strip().casefold() in reviewers:
            errors.append("Distinct nonempty reviewer identities required")
        else:
            reviewers.add(reviewer.strip().casefold())
        if slot.get("complete") is not True or not isinstance(slot.get("reviewed_at"), str) or not slot["reviewed_at"].strip():
            errors.append("Source review is incomplete or undated")
        failures = slot.get("severe_unsupported_claims")
        if not isinstance(failures, list) or any(not isinstance(item, str) or not item.strip() for item in failures):
            errors.append("Explicit unsupported-claim assessment required")
        else:
            severe.extend(failures)
        criteria = slot.get("criteria", {})
        for name in HUMAN_CRITERIA:
            criterion = criteria.get(name, {}) if isinstance(criteria, dict) else {}
            score = criterion.get("score_0_to_3") if isinstance(criterion, dict) else None
            if type(score) is not int or not 0 <= score <= 3:
                errors.append(name + ": score must be 0..3")
            else:
                scores.append(score)
            if not isinstance(criterion, dict) or not isinstance(criterion.get("notes"), str) or not criterion["notes"].strip():
                errors.append(name + ": source-backed notes required")
    return {"complete": not errors, "passed": not errors and not severe and all(score >= 2 for score in scores),
        "issues": errors, "severe_unsupported_claims": severe,
        "qualification": "Reviewer identity and source interpretation are accountable attestations, not authenticated facts or learner outcomes."}


def prepare(output: Path, cases: Path) -> dict:
    report = coding.prepare(output, cases)
    report["shared_protocol"] = PROTOCOL
    report["experiences"] = list(EXPERIENCES)
    cross.write_json(output / "prepared.json", report)
    return report


def run(args: argparse.Namespace) -> dict:
    if not 1 <= args.timeout <= 86400 or not 512 <= args.max_tokens <= 100000:
        raise ValueError("Invalid timeout or output budget")
    binary = args.lore_binary.resolve(strict=True)
    cases_path = args.cases.resolve(strict=True)
    output = cross.selected_path(args.output)
    cases = coding.load_cases(cases_path)
    for case in cases["cases"]:
        coding.config_for_case(args, case, output / "preview")
    prepared = prepare(output, cases_path)
    report = {"schema_version": 1, "protocol": PROTOCOL, "phase": "actual_shared_intelligence",
        "created_at": bench.now_utc(), "lore_binary_sha256": coding.hash_file(binary),
        "fixture_only": prepared["fixture_only"], "held_out": prepared["held_out"],
        "independent_projects": prepared["independent_projects"], "samples": [],
        "provider": args.provider, "model": args.model,
        "allow_hosted": args.allow_hosted, "allow_inspection": args.allow_inspection,
        "allow_checkout_egress": args.allow_checkout_egress,
        "max_tokens": args.max_tokens,
        "qualification": "Real CLI collections; exact source/protocol checks. Model semantics, learner transfer and agent productivity are separately assessed; process/network audit is unmeasured.",
    }
    for entry in prepared["cases"]:
        case = entry["case"]
        source, project = output / entry["source_root"], output / "projects" / case["id"]
        cross.copy_snapshot(source, project)
        config = coding.config_for_case(args, case, project)
        config.setdefault("context", {})["inspection"] = {"enabled": args.allow_inspection, "root": "."}
        config.setdefault("privacy", {})["allow_checkout_egress"] = args.allow_checkout_egress
        cross.write_json(project / "lore.yml", config)
        environment = invocation_environment(project, allow_inspection=args.allow_inspection,
            allow_hosted=args.allow_hosted, allow_checkout_egress=args.allow_checkout_egress)
        start = time.monotonic()
        initialized, _ = bench.subprocess_json(str(binary), project, "init", timeout=args.timeout, env=environment)
        preparation_seconds = round(time.monotonic() - start, 3)
        before = coding.registry_content(project)
        noop, noop_seconds = bench.subprocess_json(str(binary), project, "update", timeout=args.timeout, env=environment)
        after = coding.registry_content(project)
        sample = {"case_id": case["id"], "task": case["task"], "source_root": entry["source_root"],
            "source_manifest": entry["source_manifest"], "project_root": f"projects/{case['id']}",
            "configuration_sha256": cross.digest(config), "preparation_seconds": preparation_seconds,
            "preparation_usage": coding.usage(initialized.get("usage"), calls=cross.model_calls(initialized)),
            "no_op": {"report": noop, "registry_before": before, "registry_after": after, "elapsed_seconds": noop_seconds},
            "experiences": {},
        }
        for experience in EXPERIENCES:
            sample["experiences"][experience] = collect(str(binary), project, source, experience,
                case["task"], args.timeout, args.max_tokens, env=environment)
            cross.write_json(output / "metrics.json", report | {"samples": report["samples"] + [sample]})
        report["samples"].append(sample)
        cross.write_json(output / "metrics.json", report)
    for sample in report["samples"]:
        cross.write_json(output / "reviews" / f"{sample['case_id']}.json", review_template(report, sample))
    assessment = assess(output)
    cross.write_json(output / "assessment.json", assessment)
    return assessment


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    report = cross.read_json(directory / "metrics.json")
    prepared = cross.read_json(directory / "prepared.json")
    if report.get("schema_version") != 1 or report.get("protocol") != PROTOCOL or report.get("phase") != "actual_shared_intelligence":
        raise ValueError("Not a completed shared-intelligence collection")
    expected = {entry["case"]["id"]: entry for entry in prepared["cases"]}
    seen, issues, results, reviews = set(), [], [], []
    for sample in report.get("samples", []):
        case_id = sample["case_id"]
        if case_id not in expected or case_id in seen:
            raise ValueError("Missing, duplicated, or unexpected shared case")
        seen.add(case_id)
        entry = expected[case_id]
        source = directory / coding.relative_file(sample["source_root"])
        project = directory / coding.relative_file(sample["project_root"])
        checks = {"source_snapshot_bound": sample["source_manifest"] == entry["source_manifest"] == cross.fingerprint(source)
            and sample["source_root"] == entry["source_root"],
            "task_bound": sample["task"] == entry["case"]["task"],
            "complete_experiences": set(sample["experiences"]) == set(EXPERIENCES),
            "configuration_bound": cross.digest(cross.read_json(project / "lore.yml")) == sample["configuration_sha256"],
        }
        noop = sample["no_op"]
        checks["true_no_op"] = (noop["report"].get("no_op") is True and cross.model_calls(noop["report"]) == 0
            and cross.valid_registry(noop["registry_before"]) and noop["registry_before"] == noop["registry_after"])
        for name, record in sample["experiences"].items():
            try:
                values = context_checks(record, name, sample["task"], source)
                values["collected_checkout_still_bound"] = decision.source_state(project) == record["checkout_after"]
                values["collected_registry_still_bound"] = coding.registry_content(project) == record["registry_after"]
                values["invocation_grants_bound"] = record["host_grants"] == {
                    "inspection_root": str(project.resolve(strict=True)) if report["allow_inspection"] else None,
                    "hosted_egress": report["allow_hosted"], "checkout_egress": report["allow_checkout_egress"]}
                values["requested_budget_bound"] = record["requested_max_tokens"] == report["max_tokens"]
                checks.update({f"{name}:{key}": value for key, value in values.items()})
            except (ValueError, KeyError, TypeError, OSError, UnicodeError):
                checks[f"{name}:response_integrity"] = False
        try:
            adaptive = snapshot_manifest(sample["experiences"]["adaptive"]["response"])
            human = snapshot_manifest(sample["experiences"]["onboard"]["response"])
            checks["human_and_agent_share_snapshot"] = adaptive == human
        except (ValueError, KeyError, TypeError):
            checks["human_and_agent_share_snapshot"] = False
        failures = [name for name, passed in checks.items() if not passed]
        issues.extend(f"{case_id}: {name}" for name in failures)
        results.append({"case_id": case_id, "mechanical_contracts_passed": not failures, "checks": checks})
        reviews.append({"case_id": case_id, **review_status(directory, report, sample)})
    if seen != set(expected):
        issues.append("Required prepared cases are absent")
    fixture_only = (prepared.get("fixture_only") is not False or report.get("fixture_only") is not False
        or bool({entry["source_manifest"]["sha256"] for entry in expected.values()} & coding.bundled_fingerprints()))
    return {"schema_version": 1, "protocol": PROTOCOL, "results": results, "issues": issues,
        "mechanical_contracts_passed": bool(results) and not issues, "fixture_only": fixture_only,
        "orientation_reviews": reviews,
        "orientation_review_status": "complete" if reviews and all(item["complete"] for item in reviews) else "pending",
        "orientation_review_passed": bool(reviews) and all(item["passed"] for item in reviews),
        "human_learning_outcome": "unmeasured",
        "agent_productivity_outcome": "unmeasured", "independent_integrity_audit": "unmeasured",
        "product_acceptance_passed": False,
        "qualification": "Passing protocol/source fixtures does not establish human learning, agent improvement, semantic correctness, or independent network/process safety.",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "run"):
        command = commands.add_parser(name)
        command.add_argument("--output", type=Path, required=True)
        command.add_argument("--cases", type=Path, default=coding.DEFAULT_CASES)
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
            command.add_argument("--allow-inspection", action="store_true")
            command.add_argument("--allow-checkout-egress", action="store_true")
            command.add_argument("--timeout", type=int, default=3600)
            command.add_argument("--max-tokens", type=int, default=6000)
            bench.add_reasoning_options(command)
    command = commands.add_parser("assess")
    command.add_argument("run", type=Path)
    args = parser.parse_args(argv)
    try:
        result = prepare(args.output, args.cases) if args.command == "prepare" else run(args) if args.command == "run" else assess(args.run)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if args.command == "prepare" or result["mechanical_contracts_passed"] else 2
    except (ValueError, OSError, KeyError, TypeError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"Shared-intelligence evaluation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
