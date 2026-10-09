#!/usr/bin/env python3
"""Four-arm Lore 0.6 coding comparisons and independently bound review gates.

This runs the operator's real agent and selected verification commands through
coding_tasks.py. It never fabricates model, human-review, network-audit, billing,
or time-to-correct-completion measurements.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import time

import benchmark as bench
import coding_tasks as coding
import cross_source as cross

SETUPS = ("baseline", "fast", "lore05", "lore06")
PROTOCOL = "decision-coding-v1"


def source_state(project: Path) -> dict:
    """Hash source bytes and modification times, excluding declared Lore output."""
    result = {}
    for path in sorted(project.rglob("*")):
        relative = path.relative_to(project).as_posix()
        if relative.split("/")[0] in (".lore", "wiki") or relative == "lore.yml":
            continue
        cross.reject_symlink_path(path)
        if path.is_file():
            metadata = path.stat()
            result[relative] = {"sha256": coding.hash_file(path), "bytes": metadata.st_size,
                                "modified_ns": metadata.st_mtime_ns}
        elif not path.is_dir():
            raise ValueError("Checkout contains a nonregular source")
    return result


def observation_ids(value) -> set[str]:
    ids = set()
    if isinstance(value, dict):
        if "observation_ids" in value:
            if not isinstance(value["observation_ids"], list) or any(not isinstance(item, str) for item in value["observation_ids"]):
                raise ValueError("Invalid observation reference list")
            ids.update(value["observation_ids"])
        for child in value.values():
            ids.update(observation_ids(child))
    elif isinstance(value, list):
        for child in value:
            ids.update(observation_ids(child))
    return ids


def retained_observation_ids(value) -> set[str]:
    ids = set()
    if isinstance(value, dict):
        ids.update(item["id"] for item in value.get("imported_observations", [])
                   if isinstance(item, dict) and isinstance(item.get("id"), str))
        for child in value.values():
            ids.update(retained_observation_ids(child))
    elif isinstance(value, list):
        for child in value:
            ids.update(retained_observation_ids(child))
    return ids


def generated_observation_ids(response: dict) -> set[str]:
    """Collect inline citations only from generated advice and investigation.

    Match Rust's alphanumeric/underscore token boundary. Copied facts, source
    excerpts, recorded paths and caller task text are data, not model citations.
    """
    ids = set()
    def add(item, *fields):
        for field in fields:
            text = item.get(field, "")
            if not isinstance(text, str):
                raise ValueError("Invalid generated observation-reference text")
            ids.update(word for word in re.findall(r"\w+", text) if word.startswith("co_"))
    brief = response.get("brief", {})
    for field in ("preferred_approach", "rationale", "main_tradeoff", "next_action"):
        add(brief.get(field, {}), "text")
    for field in ("completion_criteria", "remaining_uncertainty"):
        for item in brief.get(field, []):
            add(item, "text")
    for hypothesis in brief.get("hypotheses", []):
        add(hypothesis, "text", "applicability")
        for alternative in hypothesis.get("alternatives", []):
            add(alternative, "text")
    for collection, fields in (
        ("heuristics", ("principle", "application")),
        ("constraints", ("explanation",)),
        ("checks", ("action", "decision_impact")),
        ("implementation_seams", ("purpose",)),
        ("risks", ("text",)),
        ("material_blockers", ("explanation", "decision_needed")),
        ("counterevidence", ("hypothesis", "explanation")),
    ):
        for item in brief.get(collection, []):
            add(item, *fields)
    investigation = response.get("investigation", {})
    for step in investigation.get("steps", []):
        add(step, "uncertainty", "initial_hypothesis", "discriminating_check",
            "previous_recommendation", "revised_recommendation")
        for counterevidence in step.get("counterevidence", []):
            add(counterevidence, "hypothesis", "explanation")
    for text in investigation.get("remaining_uncertainty", []):
        add({"text": text}, "text")
    return ids


def verify_observations(response: dict, snapshot: Path) -> dict:
    """Verify exact SHA-256 content and observation identities independently.

    Rust uses compact UTF-8 JSON of [path,start,end,excerpt,content_hash] for
    observation identity. Line ranges are 1-based and split only at LF, keeping
    CRLF and the final unterminated line byte-for-byte intact.
    """
    records = response.get("inspection", {}).get("observations", [])
    if not isinstance(records, list):
        raise ValueError("Inspection observations are not a list")
    ids = set()
    files = cross.fingerprint(snapshot)["files_sha256"]
    for observation in records:
        path = coding.relative_file(observation["path"])
        if path not in files:
            raise ValueError("Observation names a file outside the original source snapshot")
        raw = (snapshot / path).read_bytes()
        content_hash = "sha256:" + hashlib.sha256(raw).hexdigest()
        if observation.get("content_hash") != content_hash:
            raise ValueError("Observation full-file content hash does not match source bytes")
        parts = raw.split(b"\n")
        lines = [line + b"\n" for line in parts[:-1]]
        if parts[-1]:
            lines.append(parts[-1])
        start, end = observation.get("start_line"), observation.get("end_line")
        if type(start) is not int or type(end) is not int or not 1 <= start <= end <= len(lines):
            raise ValueError("Observation has an invalid exact line range")
        excerpt = b"".join(lines[start - 1:end]).decode("utf-8")
        if observation.get("excerpt") != excerpt:
            raise ValueError("Observation excerpt does not match its exact source lines")
        identity = json.dumps([path, start, end, excerpt, content_hash], ensure_ascii=False,
                              separators=(",", ":")).encode("utf-8")
        expected_id = "co_" + hashlib.sha256(identity).hexdigest()
        if observation.get("id") != expected_id or expected_id in ids:
            raise ValueError("Observation identity is invalid or duplicated")
        if observation.get("kind") not in ("static_source", "static_test") or not isinstance(observation.get("qualification"), str) or not observation["qualification"].strip():
            raise ValueError("Observation lacks a static-evidence qualification")
        ids.add(expected_id)
    retained = {identity for identity in retained_observation_ids(response) if not identity.startswith("co_")}
    referenced = (observation_ids(response) - retained) | generated_observation_ids(response)
    if not referenced <= ids:
        raise ValueError("A local observation reference has no complete hash-bound manifest")
    return {"checked": len(ids), "referenced": len(referenced), "exact": True}


def context_checks(record: dict, task: str, setup: str, *, snapshot: Path | None = None,
                   checkout: Path | None = None) -> dict:
    if setup in ("fast", "lore05"):
        checks = coding.context_checks(record, task, "fast" if setup == "fast" else "intelligent")
    elif setup == "lore06":
        response, repeated = record["response"], record["repeat_response"]
        before, after = record["registry_before"], record["registry_after"]
        checks = {
            "response_contract": all(answer.get("schema_version") == 4 and answer.get("task") == task
                                     and answer.get("mode") in ("intelligent", "fast_fallback")
                                     for answer in (response, repeated)),
            "response_hashes": record["response_sha256"] == cross.digest(response)
                               and record["repeat_response_sha256"] == cross.digest(repeated),
            "source_registry_preserved": cross.valid_registry(before) and cross.valid_registry(after) and before == after,
            "citations_resolve": coding.resolver_complete(record["citation_integrity"], cross.cited_ids(response) | cross.cited_ids(repeated)),
            "intelligent_repeat_reuses_guidance": response.get("mode") != "intelligent" or (
                repeated.get("mode") == "intelligent" and cross.model_calls(repeated) == 0
                and repeated.get("cache_status") == "hit"
                and response.get("brief", {}).get("preferred_approach") == repeated.get("brief", {}).get("preferred_approach")),
            "checkout_egress_contract": all(isinstance(answer.get("checkout_egress"), dict)
                and type(answer["checkout_egress"].get("allowed")) is bool
                and type(answer["checkout_egress"].get("model_received_checkout")) is bool
                and (not answer["checkout_egress"]["model_received_checkout"] or answer["checkout_egress"]["allowed"])
                for answer in (response, repeated)),
        }
    else:
        raise ValueError("Unknown decision comparison arm")
    checks["checkout_sources_preserved"] = (record.get("checkout_before") == record.get("checkout_after")
                                               and isinstance(record.get("checkout_before"), dict))
    if checkout is not None:
        checks["checkout_sources_preserved"] &= source_state(checkout) == record.get("checkout_after")
    if snapshot is not None:
        sources = cross.fingerprint(snapshot)["files_sha256"]
        checks["checkout_matches_original_sources"] = {
            path: entry.get("sha256") for path, entry in record.get("checkout_before", {}).items()
        } == sources
        try:
            for answer in (record["response"], record["repeat_response"]):
                verify_observations(answer, snapshot)
            checks["exact_local_observations"] = True
        except (ValueError, KeyError, TypeError, OSError, UnicodeError):
            checks["exact_local_observations"] = False
    return checks


def collect_context(binary: str, project: Path, task: str, setup: str, timeout: int,
                    max_tokens: int, *, investigate=False, allow_checkout_egress=False) -> dict:
    before_sources = source_state(project)
    if setup in ("fast", "lore05"):
        record = coding.collect_context(binary, project, task,
                                        "fast" if setup == "fast" else "intelligent", timeout, max_tokens)
    else:
        if setup != "lore06":
            raise ValueError("Unknown context arm")
        before = coding.registry_content(project)
        arguments = ["context", task, "--max-tokens", str(max_tokens), "--schema-version", "4", "--inspect"]
        if investigate:
            arguments.append("--investigate")
        if allow_checkout_egress:
            arguments.append("--allow-checkout-egress")
        response, elapsed = bench.subprocess_json(binary, project, *arguments, timeout=timeout)
        repeated, repeat_seconds = bench.subprocess_json(binary, project, *arguments, timeout=timeout)
        citations = cross.resolve_citations(binary, project, cross.cited_ids(response) | cross.cited_ids(repeated), timeout)
        record = {"response": response, "response_sha256": cross.digest(response),
                  "repeat_response": repeated, "repeat_response_sha256": cross.digest(repeated),
                  "elapsed_seconds": elapsed, "repeat_elapsed_seconds": repeat_seconds,
                  "usage": coding.usage(response.get("usage"), calls=cross.model_calls(response)) if cross.model_calls(response) else coding.no_inference(),
                  "repeat_usage": coding.usage(repeated.get("usage"), calls=cross.model_calls(repeated)) if cross.model_calls(repeated) else coding.no_inference(),
                  "mode": response.get("mode"), "cache_status": response.get("cache_status"),
                  "repeat_cache_status": repeated.get("cache_status"),
                  "registry_before": before, "registry_after": coding.registry_content(project),
                  "citation_integrity": citations, "inspection_sources": sorted(coding.inspection_sources(response))}
    record["checkout_before"] = before_sources
    record["checkout_after"] = source_state(project)
    record["checks"] = context_checks(record, task, setup)
    return record


def review_template(sample: dict, binding: str) -> dict:
    def reviewer_slot():
        return {"complete": False, "reviewer": "", "reviewed_at": "", "blind_confirmed": False,
                "material_decision_mistakes": None, "missed_critical_constraints": None,
                "unnecessary_blocking": None, "incorrect_hypotheses_identified": None,
                "revised_incorrect_hypotheses": None, "implementation_quality_0_to_3": None,
                "severe_unsupported_project_assertions": None, "notes": ""}
    return {"schema_version": 2, "sample_id": sample["sample_id"], "metrics_sha256": binding,
            "answer_sha256": sample["answer_sha256"], "task": sample["task"],
            "critical_constraints": sample["constraints"], "assessments": [reviewer_slot(), reviewer_slot()]}


def prepare(output: Path, cases_path: Path) -> dict:
    report = coding.prepare(output, cases_path)
    report["decision_protocol"] = PROTOCOL
    report["setups"] = list(SETUPS)
    cross.write_json(output / "prepared.json", report)
    return report


def run(args: argparse.Namespace) -> dict:
    if args.allow_checkout_egress and not args.allow_hosted:
        raise ValueError("Hosted checkout inspection needs both --allow-hosted and --allow-checkout-egress")
    output = cross.selected_path(args.output)

    def configure(config):
        config["context"] = {"inspection": {"root": ".", "enabled": False}}

    def collector(binary, project, task, setup, timeout, max_tokens):
        # Start each arm from the same freshly initialized registry, before any
        # other arm has warmed its guidance or embedding cache.
        destination = output / "context-projects" / project.name / setup
        original = cross.fingerprint(project)
        start = time.monotonic()
        shutil.copytree(project, destination)
        if cross.fingerprint(destination) != original or cross.fingerprint(project) != original:
            raise ValueError("Lore project changed during isolated arm preparation")
        isolation_seconds = round(time.monotonic() - start, 3)
        record = collect_context(binary, destination, task, setup, timeout, max_tokens,
                                 investigate=args.investigate, allow_checkout_egress=args.allow_checkout_egress)
        record["checkout_root"] = destination.relative_to(output).as_posix()
        record["isolation_seconds"] = isolation_seconds
        record["elapsed_seconds"] += isolation_seconds
        return record

    metadata = {"protocol": PROTOCOL, "inspection_opt_in": True, "investigate": args.investigate,
                "hosted_checkout_permission": args.allow_checkout_egress,
                "relative_material_mistake_reduction_target": 0.15,
                "time_to_correct_completion": {"status": "unmeasured", "seconds": None,
                    "reason": "This runner measures one implementation attempt and its checks, not an iterative path to a correct completion."},
                "egress_audit": "unmeasured_pending_independent_capture_review"}
    result = coding.run(args, setups=SETUPS, context_collector=collector, assessor=assess,
                        review_template=review_template, report_metadata=metadata, config_transform=configure)
    report = cross.read_json(output / "metrics.json")
    cross.write_json(output / "INTEGRITY_AUDIT.json", {
        "schema_version": 1, "metrics_sha256": cross.digest(report), "complete": False,
        "auditor": "", "reviewed_at": "", "method": "", "captures": [],
        "zero_unauthorized_checkout_egress": None, "zero_unrequested_commands": None,
        "no_accidental_source_modification": None, "notes": ""})
    return result


def validated_reviews(packet: dict, sample: dict, binding: str) -> list[dict]:
    expected = {"schema_version": 2, "sample_id": sample["sample_id"], "metrics_sha256": binding,
                "answer_sha256": sample["answer_sha256"], "task": sample["task"],
                "critical_constraints": sample["constraints"]}
    if any(packet.get(key) != value for key, value in expected.items()):
        raise ValueError("Human review binding mismatch")
    annotations = packet.get("assessments")
    if not isinstance(annotations, list) or len(annotations) < 2:
        raise ValueError("Each answer requires at least two independent human assessments")
    if any(annotation.get("complete") is not True for annotation in annotations):
        return []
    reviewers = set()
    for annotation in annotations:
        for field in ("reviewer", "reviewed_at", "notes"):
            if not isinstance(annotation.get(field), str) or not annotation[field].strip():
                raise ValueError("Human assessment is incomplete")
        if annotation["reviewer"].strip().casefold() in reviewers or annotation.get("blind_confirmed") is not True:
            raise ValueError("Reviews must be independent and blinded")
        reviewers.add(annotation["reviewer"].strip().casefold())
        for field in ("material_decision_mistakes", "missed_critical_constraints", "severe_unsupported_project_assertions"):
            values = annotation.get(field)
            if not isinstance(values, list) or any(not isinstance(item, str) or not item.strip() for item in values) or len(set(values)) != len(values):
                raise ValueError("Human mistakes and counterexamples require explicit unique descriptions")
        if any(item not in sample["constraints"] for item in annotation["missed_critical_constraints"]):
            raise ValueError("Unknown critical constraint in human assessment")
        if type(annotation.get("unnecessary_blocking")) is not bool:
            raise ValueError("Unnecessary blocking must be reviewed explicitly")
        for field in ("incorrect_hypotheses_identified", "revised_incorrect_hypotheses", "implementation_quality_0_to_3"):
            if type(annotation.get(field)) is not int or annotation[field] < 0:
                raise ValueError("Human hypothesis and quality scores are missing or invalid")
        if annotation["implementation_quality_0_to_3"] > 3 or annotation["revised_incorrect_hypotheses"] > annotation["incorrect_hypotheses_identified"]:
            raise ValueError("Invalid quality score or hypothesis revision count")
    return annotations


def audit_status(directory: Path, binding: str) -> dict:
    path = directory / "INTEGRITY_AUDIT.json"
    if not path.is_file():
        return {"status": "unmeasured", "passed": False}
    audit = cross.read_json(path)
    if audit.get("complete") is not True:
        return {"status": "unmeasured", "passed": False}
    if audit.get("schema_version") != 1 or audit.get("metrics_sha256") != binding:
        return {"status": "invalid_binding", "passed": False}
    try:
        if any(not isinstance(audit.get(field), str) or not audit[field].strip()
               for field in ("auditor", "reviewed_at", "method", "notes")):
            raise ValueError("Audit lacks accountable capture review")
        captures = audit.get("captures")
        if not isinstance(captures, list) or not captures:
            raise ValueError("Audit has no bound independent capture files")
        for capture in captures:
            relative = coding.relative_file(capture["path"])
            if coding.hash_file(directory / relative) != capture.get("sha256"):
                raise ValueError("Independent capture bytes changed")
        passed = all(audit.get(field) is True for field in (
            "zero_unauthorized_checkout_egress", "zero_unrequested_commands", "no_accidental_source_modification"))
        return {"status": "independently_reviewed" if passed else "failed", "passed": passed,
                "auditor": audit["auditor"], "captures": len(captures)}
    except (KeyError, TypeError, ValueError, OSError):
        return {"status": "invalid_capture_review", "passed": False}


def relative_reduction(baseline: float | None, current: float | None) -> float | None:
    if baseline is None or current is None or baseline == 0:
        return None
    return (baseline - current) / baseline


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    report = cross.read_json(directory / "metrics.json")
    if report.get("schema_version") != 1 or report.get("study", {}).get("protocol") != PROTOCOL or not report.get("samples"):
        raise ValueError("Expected a completed Lore 0.6 decision comparison")
    binding = cross.digest(report)
    totals = {setup: {"tasks": 0, "passed_tasks": 0, "failed_constraint_checks": 0,
        "intelligent_tasks": 0, "fallback_tasks": 0, "tasks_with_local_observations": 0,
        "total_seconds": 0.0, "context_seconds": 0.0, "reviewed_tasks": 0,
        "material_decision_mistakes": 0.0, "unnecessary_blocking": 0.0,
        "incorrect_hypotheses_identified": 0.0, "revised_incorrect_hypotheses": 0.0,
        "missed_critical_constraints": 0.0, "severe_unsupported_project_assertions": 0,
        "implementation_quality_sum": 0.0, "usage_components": [],
        "time_to_correct_completion_seconds": None} for setup in SETUPS}
    issues, pending, identities, assignments, project_sources = [], [], set(), {}, {}
    declared = {(entry["case"]["project"], entry["case"]["id"]): entry for entry in report["cases"]}
    known = coding.bundled_fingerprints() | cross.bundled_fingerprints()
    recognized = sorted({entry["source_manifest"]["sha256"] for entry in report["cases"]} & known)
    fixture_only = report.get("fixture_only") is not False or bool(recognized) or any("base_project" in entry["case"] for entry in report["cases"])
    for sample in report["samples"]:
        identity, setup = sample.get("sample_id"), sample.get("setup")
        if setup not in SETUPS or not isinstance(identity, str) or not identity.startswith("sample-") or coding.relative_file(identity) != identity or identity in identities:
            raise ValueError("Invalid or repeated comparison sample")
        identities.add(identity)
        key = (sample["project"], sample["case_id"])
        assignments.setdefault(key, []).append(setup)
        try:
            entry = declared[key]
            snapshot = directory / coding.relative_file(entry["source_root"])
            def validate_context(record, task, arm):
                checkout = directory / coding.relative_file(record["checkout_root"])
                return context_checks(record, task, arm, snapshot=snapshot, checkout=checkout)
            context, checks = coding.validate_sample(directory, sample, entry, context_validator=validate_context,
                                                    bind_request=True)
            cited_local = set(re.findall(r"\bco_[A-Za-z0-9_-]+\b", json.dumps(sample["agent_response"])))
            manifest_ids = {item["id"] for item in (context or {}).get("response", {}).get("inspection", {}).get("observations", [])}
            if not cited_local <= manifest_ids:
                raise ValueError("Coding answer cites an unavailable local observation")
            project_sources.setdefault(sample["project"], set()).add(sample["source_manifest"]["sha256"])
            total = totals[setup]
            total["tasks"] += 1
            total["passed_tasks"] += all(check["passed"] for check in checks)
            total["failed_constraint_checks"] += sum(check["kind"] == "constraint" and not check["passed"] for check in checks)
            response = context["response"] if context else {}
            total["intelligent_tasks"] += response.get("mode") == "intelligent"
            total["fallback_tasks"] += response.get("mode") == "fast_fallback"
            total["tasks_with_local_observations"] += bool(response.get("inspection", {}).get("observations"))
            for field in ("total_seconds", "context_seconds"):
                number = sample["costs"][field]
                if type(number) not in (int, float) or not math.isfinite(number) or number < 0:
                    raise ValueError("Invalid measured latency")
                total[field] += number
            total["usage_components"].append(sample["costs"]["total_usage"])
            review_path = directory / "reviews" / f"{identity}.json"
            annotations = validated_reviews(cross.read_json(review_path), sample, binding) if review_path.is_file() else []
            if not annotations:
                pending.append(identity)
                continue
            total["reviewed_tasks"] += 1
            count = len(annotations)
            for field in ("material_decision_mistakes", "missed_critical_constraints"):
                total[field] += sum(len(annotation[field]) for annotation in annotations) / count
            for field in ("unnecessary_blocking", "incorrect_hypotheses_identified", "revised_incorrect_hypotheses"):
                total[field] += sum(annotation[field] for annotation in annotations) / count
            total["severe_unsupported_project_assertions"] += sum(len(annotation["severe_unsupported_project_assertions"]) for annotation in annotations)
            total["implementation_quality_sum"] += sum(annotation["implementation_quality_0_to_3"] for annotation in annotations) / count
        except (ValueError, OSError, KeyError, TypeError) as error:
            issues.append(f"{identity}: {error}")
    complete_design = (set(assignments) == set(declared) and report.get("setups") == list(SETUPS)
                       and all(sorted(arms) == sorted(SETUPS) for arms in assignments.values()))
    automatic = complete_design and not issues
    reviewed = automatic and not pending
    for total in totals.values():
        total["total_usage"] = coding.add_usage(total.pop("usage_components")) if total["tasks"] else None
        quality = total.pop("implementation_quality_sum")
        total["mean_implementation_quality_0_to_3"] = quality / total["reviewed_tasks"] if total["reviewed_tasks"] else None
        if not total["reviewed_tasks"]:
            for field in ("material_decision_mistakes", "unnecessary_blocking", "incorrect_hypotheses_identified",
                          "revised_incorrect_hypotheses", "missed_critical_constraints", "severe_unsupported_project_assertions"):
                total[field] = None
    independent = (not fixture_only and report.get("held_out") is True and report.get("independent_projects") is True
                   and len(project_sources) >= 3 and all(len(sources) == 1 for sources in project_sources.values())
                   and len(set().union(*project_sources.values())) == len(project_sources))
    all_intelligent = bool(assignments) and all(totals[arm]["intelligent_tasks"] == len(assignments) for arm in ("lore05", "lore06"))
    reduction = relative_reduction(totals["lore05"]["material_decision_mistakes"], totals["lore06"]["material_decision_mistakes"]) if reviewed else None
    target_met = reduction is not None and reduction >= 0.15
    baseline_zero = reviewed and totals["lore05"]["material_decision_mistakes"] == 0
    zero_baseline_no_regression = baseline_zero and totals["lore06"]["material_decision_mistakes"] == 0
    audit = audit_status(directory, binding)
    severe_clear = reviewed and totals["lore06"]["severe_unsupported_project_assertions"] == 0
    quality_measured = independent and reviewed and all_intelligent
    release = (automatic and quality_measured and audit["passed"] and severe_clear
               and (target_met or zero_baseline_no_regression)
               and totals["lore06"]["passed_tasks"] == totals["lore06"]["tasks"])
    return {"schema_version": 1, "protocol": PROTOCOL, "mechanical_contracts_passed": automatic,
            "comparison_complete": automatic, "human_review_complete": reviewed,
            "reviews_pending": pending, "issues": issues, "fixture_only": fixture_only,
            "recognized_fixture_fingerprints": recognized, "independent_projects": len(project_sources),
            "independent_validation_complete": quality_measured,
            "all_intelligent_tasks_received_intelligence": all_intelligent,
            "relative_material_decision_mistake_reduction_vs_05": reduction,
            "directional_15_percent_target_met": target_met,
            "zero_baseline_no_regression": zero_baseline_no_regression,
            "integrity_audit": audit, "zero_severe_unsupported_project_assertions": severe_clear,
            "release_gates_passed": release, "default_inspection_benefit_established": False,
            "setups": totals,
            "qualification": "Human scores are averages across independently identified blind reviewers per answer; concrete counterexamples remain in the review packets. Identity, independence, blinding and capture interpretation are researcher attestations. One-shot elapsed time is not time to correct completion. Unknown cost and unmeasured egress remain unknown. Inspection remains opt-in until independent overhead and usefulness review establishes its default benefit."}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "run"):
        command = commands.add_parser(name)
        command.add_argument("--cases", type=Path, default=coding.DEFAULT_CASES)
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
            command.add_argument("--allow-checkout-egress", action="store_true")
            command.add_argument("--investigate", action="store_true")
            command.add_argument("--agent-command", required=True, help="JSON argv array for the real coding-agent adapter")
            command.add_argument("--agent-id", required=True, help="Same pinned model and settings for all four arms")
            command.add_argument("--agent-location", choices=("local", "hosted"), required=True)
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
    except (ValueError, OSError, KeyError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"Decision-task evaluation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
