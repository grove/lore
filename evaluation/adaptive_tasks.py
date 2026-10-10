#!/usr/bin/env python3
"""Matched actual coding tasks with explicit schema-5 adaptive reuse policies.

The existing runner supplies source copies, the real operator-selected agent,
independent executable checks and bound blind-review packets. This protocol adds
isolated, equally warmed context arms; it does not simulate model outcomes.
External agent/check commands run with the operator's privileges, not a sandbox.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import re
import random
import statistics
import secrets
import shutil
import sqlite3
import subprocess
import sys
import time

import benchmark as bench
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision
import shared_intelligence as shared
import outcome_protocol as outcome

PROTOCOL = "adaptive-coding-v1"
ADAPTIVE = ("adaptive_no_reuse", "adaptive_reuse")
SETUPS = (*decision.SETUPS, *ADAPTIVE)
REUSE_SCOPE = "combined adaptive response/investigation and optional semantic cache policy; not a memory-only ablation"


def single_review_template(sample: dict, binding: str) -> dict:
    packet = decision.review_template(sample, binding)
    packet["independence_target_sha256"] = cross.digest({
        "answer_sha256": sample["answer_sha256"], "metrics_sha256": binding})
    packet["independence_evidence"] = []
    return packet


def attempt_review_sample(sample: dict, attempt: dict) -> dict:
    return {"sample_id": f"{sample['sample_id']}-attempt-{attempt['number']}",
        "answer_sha256": cross.digest({"response_sha256": attempt["response_sha256"],
            "implementation_manifest": attempt["implementation_manifest"]}),
        "task": sample["task"], "constraints": sample["constraints"]}


def review_template(sample: dict, binding: str) -> dict:
    packet = single_review_template(sample, binding)
    # The final review already covers the last tested implementation. Every
    # earlier failed implementation needs its own retained, blinded review.
    packet["earlier_attempt_reviews"] = [single_review_template(attempt_review_sample(sample, attempt), binding)
                                         for attempt in sample.get("attempts", [])[:-1]]
    return packet


def answer_review_status(directory: Path, packet: dict, sample: dict, binding: str,
                         annotations: list[dict], *, excluded=()) -> dict:
    target = single_review_template(sample, binding)["independence_target_sha256"]
    evidence = packet.get("independence_evidence", [])
    if not evidence:
        return {"complete": False, "reason": "Independent answer-review captures are pending"}
    if packet.get("independence_target_sha256") != target:
        raise ValueError("Independent answer review targets a different answer or study")
    status = outcome.independent_reviews(directory, evidence, excluded=excluded, target_sha256=target)
    if status["complete"] and set(status["reviewers"]) != {
            annotation["reviewer"].strip().casefold() for annotation in annotations}:
        raise ValueError("Independent answer evidence does not identify the annotation reviewers")
    return status


def earlier_attempt_review_status(directory: Path, packet: dict, sample: dict, binding: str,
                                  *, excluded=()) -> dict:
    attempts = sample.get("attempts", [])[:-1]
    reviews = packet.get("earlier_attempt_reviews", [])
    if not isinstance(reviews, list):
        raise ValueError("Earlier attempt reviews must be a list")
    if len(reviews) != len(attempts):
        if reviews:
            raise ValueError("Earlier attempt review count differs from the retained implementation count")
        return {"complete": False, "pending": [f"{sample['sample_id']}-attempt-{attempt['number']}" for attempt in attempts], "outcomes": []}
    pending, rows = [], []
    for attempt, review in zip(attempts, reviews):
        subject = attempt_review_sample(sample, attempt)
        annotations = decision.validated_reviews(review, subject, binding)
        status = (answer_review_status(directory, review, subject, binding, annotations, excluded=excluded)
                  if annotations else {"complete": False})
        if not status["complete"]:
            pending.append(subject["sample_id"])
        rows.append({"sample_id": sample["sample_id"], "attempt": attempt["number"],
            "independent_review_complete": status["complete"],
            "material_decision_mistakes": sum(len(item["material_decision_mistakes"]) for item in annotations) / len(annotations) if annotations else None,
            "missed_critical_constraints": sum(len(item["missed_critical_constraints"]) for item in annotations) / len(annotations) if annotations else None})
    return {"complete": not pending, "pending": pending, "outcomes": rows}


def policy_from_args(args: argparse.Namespace) -> dict:
    if not 1 <= args.timeout <= 86400 or not 512 <= args.max_tokens <= 100000:
        raise ValueError("Invalid timeout or complete context budget")
    if args.allow_checkout_egress and not (args.allow_hosted and args.allow_inspection):
        raise ValueError("Checkout egress requires both --allow-hosted and --allow-inspection")
    return {"allow_inspection": args.allow_inspection, "allow_hosted": args.allow_hosted,
            "allow_checkout_egress": args.allow_checkout_egress,
            "max_tokens": args.max_tokens, "timeout": args.timeout}


def grant_record(project: Path, policy: dict) -> dict:
    return {"inspection_root": str(project.resolve(strict=True)) if policy["allow_inspection"] else None,
            "hosted_egress": policy["allow_hosted"], "checkout_egress": policy["allow_checkout_egress"]}


def environment_factory(policy: dict):
    # Capture inherited credentials privately once, removing ambient Lore
    # grants. Each subprocess gets a separate dictionary and a scoped root.
    base = shared.invocation_environment(Path.cwd(), allow_inspection=False,
                                         allow_hosted=False, allow_checkout_egress=False)

    def environment(project: Path, _stage: str) -> dict[str, str]:
        result = dict(base)
        grants = grant_record(project, policy)
        if grants["inspection_root"] is not None:
            result["LORE_INSPECTION_ROOT"] = grants["inspection_root"]
        if grants["hosted_egress"]:
            result["LORE_ALLOW_HOSTED_EGRESS"] = "1"
        if grants["checkout_egress"]:
            result["LORE_ALLOW_CHECKOUT_EGRESS"] = "1"
        return result
    return environment


def arguments(setup: str, task: str, policy: dict) -> list[str]:
    if setup not in SETUPS or setup == "baseline":
        raise ValueError("Unknown adaptive context arm")
    result = ["context", task, "--max-tokens", str(policy["max_tokens"])]
    if setup == "fast":
        return result + ["--fast"]
    schema = "3" if setup == "lore05" else "4" if setup == "lore06" else "5"
    result.extend(["--schema-version", schema])
    if setup != "lore05":
        if not policy["allow_inspection"]:
            result.append("--no-inspect")
        elif setup == "lore06":
            # Schema 4 retains its opt-in investigation contract. Schema 5
            # chooses that work automatically inside the identical host grant.
            result.extend(["--inspect", "--investigate"])
        if policy["allow_checkout_egress"]:
            result.append("--allow-checkout-egress")
    if setup == "adaptive_no_reuse":
        result.append("--no-cache")
    return result


def experience(setup: str) -> str:
    return "adaptive" if setup in ADAPTIVE else "schema3" if setup == "lore05" else "schema4" if setup == "lore06" else "fast"


def core(response: dict, setup: str) -> dict:
    return shared.intelligence(response, experience(setup))


def measured_usage(response: dict, setup: str, ledger: dict | None = None) -> dict:
    measured = coding.metering.bind_summary(response.get("usage"), ledger)
    if measured is not None:
        return measured
    nested = core(response, setup)
    return coding.usage(response.get("usage", nested.get("usage")), calls=cross.model_calls(nested))


def combined_usage(values: list[dict]) -> dict:
    result = coding.add_usage(values)
    result["billing_source"] = "Sum of explicitly reported context phases" if result["billed_cost_usd"] is not None else None
    return result


def cache_state(project: Path) -> dict:
    path = project / ".lore" / "context-cache"
    cross.reject_symlink_path(path)
    return cross.fingerprint(path) if path.exists() else {"sha256": cross.digest({}), "files_sha256": {}, "file_count": 0}


def collect_context(binary: str, project: Path, source: Path, task: str, setup: str,
                    policy: dict, environment) -> dict:
    before, sources = coding.registry_content(project), decision.source_state(project)
    argv, env = arguments(setup, task, policy), environment(project, "context")
    cache_before = cache_state(project)
    # Both requests precede every coding attempt. Neither sees an answer,
    # candidate implementation, checker program or checker result.
    warmup, warmup_seconds, warmup_ledger = coding.metered_lore(binary, project, *argv, timeout=policy["timeout"], env=env)
    cache_warm = cache_state(project)
    response, served_seconds, served_ledger = coding.metered_lore(binary, project, *argv, timeout=policy["timeout"], env=env)
    ids = shared.response_citations(warmup) | shared.response_citations(response)
    citations = cross.resolve_citations(binary, project, ids, policy["timeout"],
                                        env=environment(project, "evidence"))
    warmup_usage, served_usage = measured_usage(warmup, setup, warmup_ledger), measured_usage(response, setup, served_ledger)
    record = {"arguments": argv, "host_grants": grant_record(project, policy),
        "response": response, "response_sha256": cross.digest(response),
        "warmup_response": warmup, "warmup_response_sha256": cross.digest(warmup),
        "served_elapsed_seconds": served_seconds, "warmup_elapsed_seconds": warmup_seconds,
        "isolation_seconds": 0.0, "elapsed_seconds": warmup_seconds + served_seconds,
        "served_usage": served_usage, "warmup_usage": warmup_usage,
        "served_usage_ledger": served_ledger, "warmup_usage_ledger": warmup_ledger,
        "usage": combined_usage([warmup_usage, served_usage]),
        "response_json_bytes": len(json.dumps(response).encode("utf-8")),
        "warmup_json_bytes": len(json.dumps(warmup).encode("utf-8")),
        "mode": core(response, setup).get("mode", "fast"),
        "cache_status": core(response, setup).get("cache_status"),
        "registry_before": before, "registry_after": coding.registry_content(project),
        "checkout_before": sources, "checkout_after": decision.source_state(project),
        "cache_before": cache_before, "cache_after_warmup": cache_warm, "cache_after": cache_state(project),
        "citation_integrity": citations, "inspection_sources": sorted(coding.inspection_sources(response))}
    record["checks"] = context_checks(record, task, setup, source, project, policy)
    if not all(record["checks"].values()):
        raise ValueError("Adaptive task context failed: " + ", ".join(key for key, passed in record["checks"].items() if not passed))
    return record


def nonnegative(value) -> bool:
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def same_cached_premises(first: dict, served: dict, setup: str) -> bool:
    """Compare public cache invariants while permitting independent packing.

    Schema-4 runtime::fit may drop optional checks, heuristics, non-constraint
    facts and trailing completion criteria. The cache stores the unpruned draft,
    so those optional collections need not be identical on a smaller-accounting
    cache hit. Both full responses still undergo independent source validation.
    """
    before, after = first.get("brief"), served.get("brief")
    if not isinstance(before, dict) or not isinstance(after, dict) or "preferred_approach" not in before:
        return False
    if any(first.get(key) != served.get(key) for key in ("task", "paths", "model")):
        return False
    if setup == "lore05":
        # Schema 3 has no public whole-brief revision key. Keep the existing
        # runner's preferred-approach contract, without inventing a new one.
        return before["preferred_approach"] == after.get("preferred_approach")
    revision = first.get("revision_key")
    if (not isinstance(revision, str) or not revision or revision != served.get("revision_key")
            or revision != before.get("revision_key") or revision != after.get("revision_key")):
        return False

    def retained(brief):
        result = {key: value for key, value in brief.items()
                  if key not in ("heuristics", "checks", "facts", "completion_criteria")}
        constraints = {item["knowledge_id"] for item in brief.get("constraints", [])}
        result["checks"] = [item for item in brief.get("checks", []) if item.get("priority") != "optional_follow_up"]
        result["facts"] = [item for item in brief.get("facts", []) if item.get("record_id") in constraints]
        result["completion_criteria"] = brief.get("completion_criteria", [])[:1]
        return result
    return retained(before) == retained(after)


def context_checks(record: dict, task: str, setup: str, source: Path, checkout: Path, policy: dict) -> dict:
    answers = (record["warmup_response"], record["response"])
    checks = {}
    for label, answer in zip(("warmup", "served"), answers):
        checks.update({f"{label}_{name}": passed for name, passed in
                       shared.response_checks(answer, experience(setup), task, source).items()})
        shared.verify_relationship_sources(answer, record["citation_integrity"])
        checks[f"{label}_relationship_originals"] = True
        checks[f"{label}_budget_matches_request"] = answer.get("budget", {}).get("max_tokens") == policy["max_tokens"]
        if setup in ADAPTIVE:
            caps = answer["capabilities"]
            checks[f"{label}_grant_ceiling"] = (
                caps.get("inspection") == ("granted" if policy["allow_inspection"] else "disabled_by_caller")
                and (caps.get("hosted_egress") is not True or policy["allow_hosted"])
                and (caps.get("checkout_egress") is not True or policy["allow_checkout_egress"]))
        if not policy["allow_inspection"]:
            nested = core(answer, setup)
            checks[f"{label}_no_ungranted_inspection"] = (not nested.get("inspection", {}).get("observations")
                and nested.get("inspection", {}).get("budget", {}).get("files_read", 0) == 0
                and not nested.get("checkout_egress", {}).get("model_received_checkout", False))
    expected_files = cross.fingerprint(source)["files_sha256"]
    checks.update({
        "arguments_bound": record["arguments"] == arguments(setup, task, policy),
        "host_grants_bound": record["host_grants"] == grant_record(checkout, policy),
        "hashes_bound": record["warmup_response_sha256"] == cross.digest(answers[0])
            and record["response_sha256"] == cross.digest(answers[1]),
        "registry_preserved": cross.valid_registry(record["registry_before"])
            and cross.valid_registry(record["registry_after"])
            and record["registry_before"] == record["registry_after"] == coding.registry_content(checkout),
        "checkout_preserved": record["checkout_before"] == record["checkout_after"] == decision.source_state(checkout),
        "original_sources_bound": {path: value["sha256"] for path, value in record["checkout_before"].items()} == expected_files,
        "citations_resolve": coding.resolver_complete(record["citation_integrity"],
            shared.response_citations(answers[0]) | shared.response_citations(answers[1])),
        "served_usage_bound": record["served_usage"] == measured_usage(answers[1], setup, record.get("served_usage_ledger")),
        "warmup_usage_bound": record["warmup_usage"] == measured_usage(answers[0], setup, record.get("warmup_usage_ledger")),
        "full_context_usage": record["usage"] == combined_usage([record["warmup_usage"], record["served_usage"]]),
        "full_context_time": all(nonnegative(record[key]) for key in
            ("elapsed_seconds", "served_elapsed_seconds", "warmup_elapsed_seconds", "isolation_seconds"))
            and math.isclose(record["elapsed_seconds"], sum(record[key] for key in
                ("served_elapsed_seconds", "warmup_elapsed_seconds", "isolation_seconds")), abs_tol=1e-9),
        "complete_json_bytes": record["response_json_bytes"] == len(json.dumps(answers[1]).encode("utf-8"))
            and record["warmup_json_bytes"] == len(json.dumps(answers[0]).encode("utf-8")),
        "fresh_context_cache": record["cache_before"] == {"sha256": cross.digest({}), "files_sha256": {}, "file_count": 0},
        "cache_files_bound": record["cache_after"] == cache_state(checkout),
        "fast_repeat_identical": setup != "fast" or answers[0] == answers[1],
        "cold_request_not_a_guidance_hit": core(answers[0], setup).get("cache_status") != "hit",
    })
    if core(answers[1], setup).get("cache_status") == "hit":
        first, served = (core(answer, setup) for answer in answers)
        checks["hit_bound_to_warmup"] = (record["cache_after_warmup"]["file_count"] > 0
            and first.get("mode") == served.get("mode") == "intelligent"
            and cross.model_calls(served) == 0 and same_cached_premises(first, served, setup)
            and first.get("investigation", {}).get("steps", []) == served.get("investigation", {}).get("steps", []))
    if setup in ADAPTIVE:
        checks["same_snapshot"] = shared.snapshot_manifest(answers[0]) == shared.snapshot_manifest(answers[1])
        checks["same_capabilities"] = answers[0]["capabilities"] == answers[1]["capabilities"]
    if setup == "adaptive_no_reuse":
        checks["no_cache_reads_or_writes"] = (
            record["cache_before"] == record["cache_after_warmup"] == record["cache_after"]
            and all(core(answer, setup).get("cache_status") != "hit"
                    and core(answer, setup).get("investigation", {}).get("stop_reason") != "cache_reused" for answer in answers))
    return checks


def prepare(output: Path, cases: Path) -> dict:
    report = coding.prepare(output, cases)
    report.update(adaptive_protocol=PROTOCOL, setups=list(SETUPS), reuse_scope=REUSE_SCOPE)
    cross.write_json(output / "prepared.json", report)
    cross.write_json(output / "PREREGISTRATION.json", outcome.preregistration_template(report, arms=list(SETUPS),
        pilot=coding.load_cases(cases).get("pilot") is True))
    return report


def run(args: argparse.Namespace) -> dict:
    policy = policy_from_args(args)
    seed = getattr(args, "seed", None)
    if seed is None:
        seed = secrets.randbits(64)
        args.seed = seed
    context_random = random.Random(seed ^ 0xC07E)
    output = cross.selected_path(args.output)
    environment = environment_factory(policy)
    collected = {}

    def preparation_hook(destination, prepared):
        registration_path = getattr(args, "preregistration", None)
        if registration_path is None:
            return
        registration_path = cross.selected_path(registration_path)
        registration = cross.read_json(registration_path)
        cross.write_json(destination / "PREREGISTRATION.json", registration)
        for case in registration.get("cases", []):
            for review in case.get("independent_gold_reviews", []):
                item = review["capture"]
                outcome.capture(registration_path.parent, item)
                name = coding.relative_file(item["path"])
                target = destination / name
                if target.exists() or name.split("/")[0] not in ("gold-review-captures",):
                    raise ValueError("Preregistration captures must use distinct gold-review-captures paths")
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(registration_path.parent / name, target)

    def configure(config):
        config["context"] = {"cache": True, "inspection": {"root": ".", "enabled": False}}
        config.setdefault("privacy", {})["allow_checkout_egress"] = policy["allow_checkout_egress"]

    def collector(binary, project, task, setup, timeout, max_tokens):
        if (timeout, max_tokens) != (policy["timeout"], policy["max_tokens"]):
            raise ValueError("Coding runner changed the matched context policy")
        key = str(project)
        if key not in collected:
            collected[key] = {}
            order = list(SETUPS[1:])
            context_random.shuffle(order)
            original = cross.fingerprint(project)
            for index, arm in enumerate(order):
                destination = output / "context-projects" / project.name / arm
                start = time.monotonic()
                shutil.copytree(project, destination)
                if cross.fingerprint(destination) != original or cross.fingerprint(project) != original:
                    raise ValueError("An arm did not start from the same initialized project")
                isolation_seconds = round(time.monotonic() - start, 3)
                record = collect_context(binary, destination, output / "snapshots" / project.name,
                                         task, arm, policy, environment)
                record.update(checkout_root=destination.relative_to(output).as_posix(),
                              initialized_project_manifest=original, collection_index=index,
                              isolation_seconds=isolation_seconds)
                record["elapsed_seconds"] += isolation_seconds
                collected[key][arm] = record
        return collected[key][setup]

    metadata = {"protocol": PROTOCOL, "policy": policy, "reuse_scope": REUSE_SCOPE,
        "context_sequence": "same-task warm-up followed by served context, before any agent or checker executes",
        "context_order": "independently shuffled per case; recorded collection_index",
        "warmup_charge": "both warm-up and served requests plus context isolation charged to every Lore arm",
        "cost_scope": "Each Lore arm is charged full shared initialization, context-copy time, both warm-up and served context, all coding/verification attempts and iteration overhead. Upstream snapshot creation and review labor are unmeasured; no preparation amortization is assumed.",
        "permission_scope": "Per-subprocess Lore grant environment; not sandbox enforcement of external agent or checker permissions",
        "repair_policy": coding.repair_policy(args),
        "context_order_seed": seed ^ 0xC07E,
        "iteration_outcomes": "Bounded attempts and time to first independently correct patch are recorded; different-task reuse uses its separate protocol",
        "egress_audit": "unmeasured_pending_independent_capture_review"}
    result = coding.run(args, setups=SETUPS, context_collector=collector, assessor=assess,
                        review_template=review_template, report_metadata=metadata,
                        config_transform=configure, subprocess_environment=environment,
                        preparation_hook=preparation_hook)
    report = cross.read_json(output / "metrics.json")
    cross.write_json(output / "INTEGRITY_AUDIT.json", {
        "schema_version": 1, "metrics_sha256": cross.digest(report), "complete": False,
        "auditor": "", "reviewed_at": "", "method": "", "captures": [],
        "zero_unauthorized_checkout_egress": None, "zero_unrequested_commands": None,
        "no_accidental_source_modification": None, "notes": ""})
    return result


def validate_costs(sample: dict, context: dict | None) -> None:
    costs = sample["costs"]
    for field in ("preparation_seconds", "context_seconds", "coding_seconds", "verification_seconds", "total_seconds"):
        if not nonnegative(costs[field]):
            raise ValueError("Invalid measured task latency")
    if (costs["context_seconds"] != (context["elapsed_seconds"] if context else 0.0)
            or costs["context_usage"] != (context["usage"] if context else coding.no_inference())
            or costs["coding_usage"] != sample["agent_response"]["usage"]
            or not math.isclose(costs["total_seconds"], sum(costs[field] for field in
                ("preparation_seconds", "context_seconds", "coding_seconds", "verification_seconds"))
                + costs.get("iteration_overhead_seconds", 0.0), abs_tol=1e-9)):
        raise ValueError("Task cost drops or substitutes a recorded phase")
    if not nonnegative(costs.get("iteration_overhead_seconds", 0.0)):
        raise ValueError("Invalid measured iteration overhead")
    for field in ("preparation_usage", "context_usage", "coding_usage"):
        coding.usage(costs[field])
    preparation = coding.metering.summary_from_record(costs.get("preparation_usage_ledger"))
    if preparation is not None and costs["preparation_usage"] != preparation:
        raise ValueError("Preparation usage differs from its captured provider ledger")
    if costs["total_usage"] != coding.add_usage([costs[field] for field in
                                               ("preparation_usage", "context_usage", "coding_usage")]):
        raise ValueError("Task usage omits a warm-up or another component")


def paired_bootstrap(values: list[tuple[float, float]], *, statistic="difference",
                     seed=7, resamples=2000) -> dict:
    """Resample matched tasks, never individual arms as independent samples."""
    if not values:
        return {"pairs": 0, "estimate": None, "ci95": None, "small_sample": True}
    if not 200 <= resamples <= 10000:
        raise ValueError("Bootstrap resamples must be 200..10000")
    def estimate(rows):
        if statistic == "difference":
            return statistics.mean(right - left for left, right in rows)
        if statistic == "median_ratio":
            denominator = statistics.median(left for left, _ in rows)
            return statistics.median(right for _, right in rows) / denominator if denominator > 0 else None
        raise ValueError("Unknown paired statistic")
    rng = random.Random(seed)
    draws = [estimate([values[rng.randrange(len(values))] for _ in values]) for _ in range(resamples)]
    finite = sorted(value for value in draws if value is not None and math.isfinite(value))
    interval = [finite[int((len(finite) - 1) * 0.025)], finite[int((len(finite) - 1) * 0.975)]] if len(finite) == resamples else None
    return {"pairs": len(values), "estimate": estimate(values), "ci95": interval,
            "small_sample": len(values) < 30, "resamples": resamples,
            "seed": seed, "unit": "matched task", "statistic": statistic,
            "qualification": "Percentile interval conditional on these projects/tasks; tasks within a project may be correlated."}


def paired_outcomes(paired: dict, *, seed=7) -> dict:
    rows = []
    for (project, case_id), samples in sorted(paired.items()):
        if set(samples) != set(SETUPS):
            continue
        arms = {}
        for arm, sample in samples.items():
            attempts = sample.get("attempts", [])
            failed_constraints = sum(check["kind"] == "constraint" and not check["passed"] for check in sample["tests"]["checks"])
            arms[arm] = {"passed": all(check["passed"] for check in sample["tests"]["checks"]),
                "failed_constraint_checks": failed_constraints,
                "attempts": len(attempts) or 1, "correction_loops": max(0, len(attempts) - 1),
                "total_seconds": sample["costs"]["total_seconds"],
                "time_to_first_correct_seconds": sample.get("time_to_first_correct_seconds"),
                "provider_input_tokens": sample["costs"]["total_usage"]["input_tokens"],
                "provider_output_tokens": sample["costs"]["total_usage"]["output_tokens"],
                "billed_cost_usd": sample["costs"]["total_usage"]["billed_cost_usd"],
                "source_input_utf8_bytes": sample.get("source_input_utf8_bytes"),
                "context_tokens": sample["context"]["response"].get("budget", {}).get("used_tokens") if sample["context"] else 0,
                "warmup_context_tokens": sample["context"]["warmup_response"].get("budget", {}).get("used_tokens") if sample["context"] else 0}
        rows.append({"project": project, "case_id": case_id, "arms": arms})
    comparisons = {}
    for arm in SETUPS[1:]:
        metrics = {}
        for field, statistic in (("passed", "difference"), ("failed_constraint_checks", "difference"),
                                 ("correction_loops", "difference"), ("total_seconds", "median_ratio"),
                                 ("billed_cost_usd", "median_ratio")):
            values = [(row["arms"]["baseline"][field], row["arms"][arm][field]) for row in rows
                      if row["arms"]["baseline"][field] is not None and row["arms"][arm][field] is not None]
            metrics[field] = paired_bootstrap(values, statistic=statistic, seed=seed)
            metrics[field]["coverage"] = len(values) / len(rows) if rows else 0.0
        comparisons[arm] = metrics
    coverage = {arm: {field: {"known": sum(row["arms"][arm][field] is not None for row in rows),
                                    "total": len(rows)}
                      for field in ("provider_input_tokens", "provider_output_tokens", "billed_cost_usd")}
                for arm in SETUPS}
    return {"paired_tasks": len(rows), "per_task": rows, "baseline_comparisons": comparisons,
            "usage_coverage": coverage,
            "qualification": "Failed tasks remain in all-task elapsed-time outcomes. Unfinished time to correct is null, not zero; complete-case billing comparisons must not stand in for missing costs."}


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    report = cross.read_json(directory / "metrics.json")
    if (report.get("schema_version") != 1 or report.get("phase") != "actual_coding_tasks"
            or report.get("study", {}).get("protocol") != PROTOCOL or not report.get("samples")):
        raise ValueError("Expected a completed adaptive coding-task comparison")
    policy = report["study"]["policy"]
    if (set(policy) != {"allow_inspection", "allow_hosted", "allow_checkout_egress", "max_tokens", "timeout"}
            or any(type(policy[key]) is not bool for key in ("allow_inspection", "allow_hosted", "allow_checkout_egress"))
            or type(policy["max_tokens"]) is not int or type(policy["timeout"]) is not int):
        raise ValueError("Invalid matched permission and budget policy")
    policy_from_args(argparse.Namespace(**policy))
    binding = cross.digest(report)
    totals = {arm: {"tasks": 0, "passed_tasks": 0, "failed_constraint_checks": 0,
        "intelligent_tasks": 0, "fallback_tasks": 0, "reference_tasks": 0, "reviewed_tasks": 0,
        "served_context_seconds": 0.0, "warmup_context_seconds": 0.0, "total_seconds": 0.0,
        "served_context_model_calls": 0, "warmup_context_model_calls": 0,
        "guidance_cache_hits": 0, "retained_investigation_reuses": 0,
        "material_decision_mistakes": 0.0, "missed_critical_constraints": 0.0,
        "severe_unsupported_project_assertions": 0, "quality_sum": 0.0,
        "usage_components": []} for arm in SETUPS}
    issues, pending, identities, assignments, source_projects = [], [], set(), {}, {}
    independent_reviews_pending, earlier_review_outcomes = [], []
    preregistration_path = directory / "PREREGISTRATION.json"
    preregistration = cross.read_json(preregistration_path) if preregistration_path.is_file() else {}
    review_excluded = [preregistration.get("operator_id", "")]
    for registered_case in preregistration.get("cases", []):
        review_excluded.extend(registered_case.get("task_authors", []))
        review_excluded.extend(registered_case.get("checker_authors", []))
    declared = {(entry["case"]["project"], entry["case"]["id"]): entry for entry in report["cases"]}
    known = coding.bundled_fingerprints() | cross.bundled_fingerprints()
    recognized = sorted({entry["source_manifest"]["sha256"] for entry in report["cases"]} & known)
    fixture_only = report.get("fixture_only") is not False or bool(recognized) or any("base_project" in entry["case"] for entry in report["cases"])
    paired = {}
    for sample in report["samples"]:
        identity, setup = sample.get("sample_id"), sample.get("setup")
        if (setup not in SETUPS or not isinstance(identity, str) or not identity.startswith("sample-")
                or coding.relative_file(identity) != identity or identity in identities):
            raise ValueError("Invalid or duplicated adaptive comparison sample")
        identities.add(identity)
        key = (sample["project"], sample["case_id"])
        assignments.setdefault(key, []).append(setup)
        try:
            entry = declared[key]
            snapshot = directory / coding.relative_file(entry["source_root"])
            def validate_context(record, task, arm):
                return context_checks(record, task, arm, snapshot,
                                      directory / coding.relative_file(record["checkout_root"]), policy)
            context, tests = coding.validate_sample(directory, sample, entry, context_validator=validate_context, bind_request=True)
            if sample.get("repair_policy") != report.get("repair_policy"):
                raise ValueError("Adaptive sample repair allowance differs from the matched study policy")
            validate_costs(sample, context)
            if context:
                project = directory / "lore" / sample["case_id"]
                checkout = directory / coding.relative_file(context["checkout_root"])
                if (checkout != directory / "context-projects" / sample["case_id"] / setup
                        or cross.fingerprint(project) != context["initialized_project_manifest"]
                        or sample["configuration_sha256"] != cross.digest(cross.read_json(project / "lore.yml"))
                        or sample["configuration_sha256"] != cross.digest(cross.read_json(checkout / "lore.yml"))):
                    raise ValueError("Original initialized project, isolated arm or configuration changed")
            nested = core(context["response"], setup) if context else {}
            cited = set(re.findall(r"\bco_[A-Za-z0-9_-]+\b", json.dumps(sample["agent_response"])))
            available = {item["id"] for item in nested.get("inspection", {}).get("observations", [])}
            if not cited <= available:
                raise ValueError("Coding answer cites an unavailable checkout observation")
            paired.setdefault(key, {})[setup] = sample
            pin = outcome.source_provenance(entry)
            source_identity = f"{pin['repository']}@{pin['commit']}" if pin["complete"] else sample["source_manifest"]["sha256"]
            source_projects.setdefault(sample["project"], set()).add(source_identity)
            total = totals[setup]
            total["tasks"] += 1
            total["passed_tasks"] += all(check["passed"] for check in tests)
            total["failed_constraint_checks"] += sum(check["kind"] == "constraint" and not check["passed"] for check in tests)
            total["intelligent_tasks"] += nested.get("mode") == "intelligent"
            total["fallback_tasks"] += nested.get("mode") == "fast_fallback"
            total["reference_tasks"] += nested.get("mode") == "reference"
            total["total_seconds"] += sample["costs"]["total_seconds"]
            total["usage_components"].append(sample["costs"]["total_usage"])
            if context:
                total["served_context_seconds"] += context["served_elapsed_seconds"]
                total["warmup_context_seconds"] += context["warmup_elapsed_seconds"]
                total["served_context_model_calls"] += context["served_usage"]["model_calls"]
                total["warmup_context_model_calls"] += context["warmup_usage"]["model_calls"]
                hit = nested.get("cache_status") == "hit"
                total["guidance_cache_hits"] += hit
                total["retained_investigation_reuses"] += (hit and nested.get("investigation", {}).get("stop_reason") == "cache_reused"
                    and bool(nested.get("investigation", {}).get("steps")))
            path = directory / "reviews" / f"{identity}.json"
            packet = cross.read_json(path) if path.is_file() else {}
            annotations = decision.validated_reviews(packet, sample, binding) if packet else []
            if not annotations:
                pending.append(identity)
                independent_reviews_pending.append(identity)
                continue
            review_status = answer_review_status(directory, packet, sample, binding, annotations,
                                                 excluded=review_excluded)
            if not review_status["complete"]:
                independent_reviews_pending.append(identity)
            earlier_reviews = earlier_attempt_review_status(directory, packet, sample, binding, excluded=review_excluded)
            independent_reviews_pending.extend(earlier_reviews["pending"])
            earlier_review_outcomes.extend(earlier_reviews["outcomes"])
            total["reviewed_tasks"] += 1
            for field in ("material_decision_mistakes", "missed_critical_constraints"):
                total[field] += sum(len(annotation[field]) for annotation in annotations) / len(annotations)
            total["severe_unsupported_project_assertions"] += sum(len(annotation["severe_unsupported_project_assertions"]) for annotation in annotations)
            total["quality_sum"] += sum(annotation["implementation_quality_0_to_3"] for annotation in annotations) / len(annotations)
        except (ValueError, OSError, KeyError, TypeError, sqlite3.Error) as error:
            issues.append(f"{identity}: {error}")
    for key, samples in paired.items():
        try:
            lore = [samples[arm] for arm in SETUPS[1:]]
            for field in ("source_manifest", "configuration_sha256"):
                if len({cross.digest(sample[field]) for sample in lore}) != 1:
                    raise ValueError("Context arms use different source or model configurations")
            if (len({cross.digest(sample["context"]["initialized_project_manifest"]) for sample in lore}) != 1
                    or len({cross.digest(sample["context"]["registry_before"]) for sample in lore}) != 1
                    or sorted(sample["context"]["collection_index"] for sample in lore) != list(range(len(lore)))):
                raise ValueError("Context arms were not separately collected from one cold registry")
            if any(sample["costs"]["preparation_usage"] != lore[0]["costs"]["preparation_usage"]
                   or sample["costs"]["preparation_seconds"] != lore[0]["costs"]["preparation_seconds"] for sample in lore):
                raise ValueError("Shared initialization cost is not equally charged")
            adaptive = [samples[arm]["context"]["response"] for arm in ADAPTIVE]
            if shared.snapshot_manifest(adaptive[0]) != shared.snapshot_manifest(adaptive[1]) or adaptive[0]["capabilities"] != adaptive[1]["capabilities"]:
                raise ValueError("Adaptive policies use different snapshots or effective grants")
            models = {attempt["response"].get("provider_model") for sample in samples.values()
                      for attempt in sample.get("attempts", [{"response": sample["agent_response"]}])} - {None}
            if len(models) > 1:
                raise ValueError("Coding agent reported different models across matched arms")
        except (ValueError, KeyError, TypeError) as error:
            issues.append(f"{key[1]}: {error}")
    complete = (report.get("setups") == list(SETUPS) and set(assignments) == set(declared)
                and all(sorted(arms) == sorted(SETUPS) for arms in assignments.values()) and not issues)
    reviewed = complete and not pending
    for total in totals.values():
        total["total_usage"] = coding.add_usage(total.pop("usage_components")) if total["tasks"] else None
        quality = total.pop("quality_sum")
        total["mean_implementation_quality_0_to_3"] = quality / total["reviewed_tasks"] if total["reviewed_tasks"] else None
        if not total["reviewed_tasks"]:
            for field in ("material_decision_mistakes", "missed_critical_constraints", "severe_unsupported_project_assertions"):
                total[field] = None
    independent = (not fixture_only and report.get("held_out") is True and report.get("independent_projects") is True
        and len(source_projects) >= 3 and all(len(values) == 1 for values in source_projects.values())
        and len(set().union(*source_projects.values())) == len(source_projects))
    adaptive_intelligent = bool(assignments) and all(totals[arm]["intelligent_tasks"] == len(assignments) for arm in ADAPTIVE)
    intelligent = adaptive_intelligent and totals["lore06"]["intelligent_tasks"] == len(assignments)
    agent_calls = all(attempt["response"]["usage"]["model_calls"] is not None
                      and attempt["response"]["usage"]["model_calls"] > 0 for sample in report["samples"]
                      for attempt in sample.get("attempts", [{"response": sample["agent_response"]}]))
    registration = outcome.preregistration_status(directory, report)
    audit = decision.audit_status(directory, binding)
    independent_answers = reviewed and not independent_reviews_pending
    measured = (complete and independent_answers and independent and intelligent and agent_calls
                and registration["complete"] and audit["passed"])
    reuse, disabled = (totals[arm] for arm in ("adaptive_reuse", "adaptive_no_reuse"))
    comparison = {"served_context_model_calls_difference": reuse["served_context_model_calls"] - disabled["served_context_model_calls"] if complete else None,
        "served_context_seconds_difference": reuse["served_context_seconds"] - disabled["served_context_seconds"] if complete else None,
        "warmup_inclusive_total_seconds_difference": reuse["total_seconds"] - disabled["total_seconds"] if complete else None,
        "passed_task_difference": reuse["passed_tasks"] - disabled["passed_tasks"] if complete else None,
        "relative_reviewed_mistake_reduction": decision.relative_reduction(disabled["material_decision_mistakes"], reuse["material_decision_mistakes"]) if reviewed else None,
        "scope": REUSE_SCOPE, "direction": "reuse minus no_reuse, except relative mistake reduction"}
    return {"schema_version": 1, "protocol": PROTOCOL, "mechanical_contracts_passed": complete,
        "comparison_complete": complete, "human_review_complete": reviewed, "reviews_pending": pending,
        "independent_answer_reviews_complete": independent_answers,
        "independent_answer_reviews_pending": independent_reviews_pending,
        "earlier_attempt_review_outcomes": earlier_review_outcomes,
        "issues": issues, "fixture_only": fixture_only, "recognized_fixture_fingerprints": recognized,
        "independent_projects": len(source_projects), "independent_validation_complete": measured,
        "all_adaptive_tasks_received_intelligence": adaptive_intelligent,
        "current_baseline_received_intelligence": bool(assignments) and totals["lore06"]["intelligent_tasks"] == len(assignments),
        "integrity_audit": audit,
        "preregistration": registration,
        "agent_quality_status": "independently_reviewed_task_attempts" if measured else "unmeasured",
        "productivity_benefit_established": False,
        "iteration_outcomes": paired_outcomes(paired, seed=report.get("order_seed", 7)),
        "reuse_comparison": comparison, "setups": totals,
        "qualification": "Bounded independently checked attempts per arm; public fixtures and offline doubles establish mechanics only. Same-task warm-up contains no solution or checker output and is fully charged. Cache hits do not establish correctness or different-task reuse. Provider identity, tools, blinding and external process permissions need independent attestation/capture review; Lore grant environment is not a sandbox. Unknown token usage and billing remain null. Failed tasks have no observed time to first correct completion."}


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
            command.add_argument("--allow-inspection", action="store_true")
            command.add_argument("--allow-checkout-egress", action="store_true")
            command.add_argument("--agent-command", required=True, help="JSON argv array for the actual coding-agent adapter")
            command.add_argument("--agent-id", required=True, help="Pinned model, tools and settings shared by all six arms")
            command.add_argument("--agent-location", choices=("local", "hosted"), required=True)
            command.add_argument("--timeout", type=int, default=3600)
            command.add_argument("--max-tokens", type=int, default=6000)
            command.add_argument("--max-attempts", type=int, default=1)
            command.add_argument("--attempt-budget-seconds", type=int)
            command.add_argument("--seed", type=int)
            command.add_argument("--preregistration", type=Path)
            bench.add_reasoning_options(command)
    command = commands.add_parser("assess")
    command.add_argument("run", type=Path)
    args = parser.parse_args(argv)
    try:
        result = prepare(args.output, args.cases) if args.command == "prepare" else run(args) if args.command == "run" else assess(args.run)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if args.command == "prepare" or result["mechanical_contracts_passed"] else 2
    except (ValueError, OSError, KeyError, TypeError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"Adaptive-task evaluation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
