"""Shared provenance and independent-review gates for real outcome studies."""
from __future__ import annotations
from datetime import datetime, timezone
from pathlib import Path
import re

import coding_tasks as coding
import cross_source as cross

SHA256 = re.compile(r"^[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")


def timestamp(value: str) -> datetime:
    if not isinstance(value, str):
        raise ValueError("A timestamp with an explicit UTC offset is required")
    result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if result.tzinfo is None:
        raise ValueError("Timestamp requires a UTC offset")
    return result.astimezone(timezone.utc)


def capture(root: Path, item: dict) -> str:
    name = coding.relative_file(item["path"])
    path = root / name
    cross.reject_symlink_path(path)
    expected = item.get("sha256")
    if not isinstance(expected, str) or not SHA256.fullmatch(expected) or coding.hash_file(path) != expected:
        raise ValueError("Evidence capture does not match its bound digest")
    if path.stat().st_size > 10_000_000:
        raise ValueError("Evidence capture exceeds 10 MB")
    return expected


def source_provenance(entry: dict) -> dict:
    case = entry["case"]
    upstream = case.get("upstream", {})
    if (not isinstance(upstream, dict) or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", upstream.get("repository", ""))
            or not COMMIT.fullmatch(upstream.get("commit", ""))):
        return {"complete": False, "reason": "Exact upstream repository and commit are missing"}
    files = upstream.get("files_sha256")
    if (not isinstance(files, dict) or not files or any(not SHA256.fullmatch(value) for value in files.values())
            or case.get("input_sha256") != entry["source_manifest"]["sha256"]):
        return {"complete": False, "reason": "Original upstream hashes or transformed input digest are missing"}
    for name in files:
        coding.relative_file(name)
    return {"complete": True, "repository": upstream["repository"], "commit": upstream["commit"],
            "input_sha256": case["input_sha256"], "qualification": "Source pinning does not establish independent task authorship or held-out status."}


def task_contract_sha256(entry: dict) -> str:
    return cross.digest({"case": entry["case"], "checker_files_sha256": entry.get("checker_files_sha256")})


def preregistration_template(prepared: dict, *, arms: list[str], pilot=False) -> dict:
    return {"schema_version": 1, "protocol": "lore-outcome-preregistration-v1", "complete": False,
        "study_id": "", "registered_at": "", "operator_id": "", "pilot": pilot,
        "cases": [{"case_id": entry["case"]["id"], "task_sha256": task_contract_sha256(entry),
                   "checker_files_sha256": entry.get("checker_files_sha256"),
                   "source_sha256": entry["source_manifest"]["sha256"], "source_provenance": source_provenance(entry),
                   "task_authors": [], "checker_authors": [], "independent_gold_reviews": []}
                  for entry in prepared["cases"]],
        "arms": arms, "agent": {"model": "", "model_revision": "", "reasoning": "", "temperature": None,
            "tools": [], "command_sha256": "", "prompt_sha256": "", "max_attempts": 1,
            "wall_time_seconds": None, "tool_call_limit": None, "token_limit": None},
        "lore": {"model": "", "model_revision": "", "reasoning": "", "configuration_sha256_by_case": {}},
        "minimum_attempted_tasks": 6 if pilot else 30,
        "outcomes": ["independently_checked_completion", "critical_constraint_failures", "warmup_inclusive_time", "reported_provider_cost"],
        "exclusions": [], "analysis": {"bootstrap_unit": "matched_task", "bootstrap_resamples": 2000,
            "confidence": 0.95, "pass_rate_gain": 0.10, "time_or_cost_reduction": 0.20},
        "independence_qualification": "Distinct names alone do not prove independent authorship; bind source review captures and declare conflicts."}


def independent_reviews(root: Path, reviews: list, *, excluded=(), target_sha256: str) -> dict:
    if not isinstance(reviews, list) or len(reviews) < 2:
        return {"complete": False, "reviewers": [], "reason": "Two bound independent reviews are pending"}
    identities, captures = set(), set()
    denied = {value.strip().casefold() for value in excluded}
    for review in reviews:
        identity = review.get("reviewer_id", "").strip().casefold()
        if (not identity or identity in identities or identity in denied
                or review.get("complete") is not True or review.get("independent") is not True
                or review.get("conflicts_declared") != [] or review.get("target_sha256") != target_sha256
                or review.get("blind_confirmed") is not True):
            raise ValueError("Review identity, independence, blinding or target binding is invalid")
        timestamp(review["reviewed_at"])
        digest = capture(root, review["capture"])
        if digest in captures:
            raise ValueError("Independent reviews cannot reuse an identical evidence capture")
        identities.add(identity)
        captures.add(digest)
    return {"complete": True, "reviewers": sorted(identities),
            "qualification": "Accountable declarations and review artifacts, not authenticated proof of distinct humans."}


def validate_model_pins(registration: dict, report: dict) -> None:
    agent, lore = registration["agent"], registration["lore"]
    for label, role in (("agent", agent), ("Lore", lore)):
        for key in ("model", "model_revision", "reasoning"):
            if not isinstance(role.get(key), str) or not role[key].strip():
                raise ValueError(f"Pinned {label} model, revision and reasoning are required")
    if not isinstance(agent.get("prompt_sha256"), str) or not SHA256.fullmatch(agent["prompt_sha256"]):
        raise ValueError("The coding prompt identity must be a SHA-256 digest")
    tools = agent.get("tools")
    if (not isinstance(tools, list) or any(not isinstance(item, str) or not item.strip() for item in tools)
            or len(tools) != len(set(tools))):
        raise ValueError("The coding agent's tools must be an explicit unique list")
    reported = {attempt["response"].get("provider_model") for sample in report["samples"]
                for attempt in sample.get("attempts", [{"response": sample["agent_response"]}])}
    if reported != {agent["model"]}:
        raise ValueError("Actual reported coding-model identities differ from the preregistered model")
    configurations = {sample["case_id"]: sample["configuration_sha256"] for sample in report["samples"]
                      if sample.get("context") is not None}
    if (not configurations or any(not isinstance(value, str) or not SHA256.fullmatch(value) for value in configurations.values())
            or lore.get("configuration_sha256_by_case") != configurations):
        raise ValueError("Preregistered Lore configurations do not match all collected cases")


def preregistration_status(directory: Path, report: dict) -> dict:
    path = directory / "PREREGISTRATION.json"
    if not path.is_file():
        return {"complete": False, "status": "unmeasured", "issues": ["Preregistration and independent gold review pending"]}
    registration = cross.read_json(path)
    issues = []
    try:
        if registration.get("schema_version") != 1 or registration.get("protocol") != "lore-outcome-preregistration-v1":
            raise ValueError("Unknown study preregistration format")
        if registration.get("complete") is not True:
            raise ValueError("Preregistration has not been completed")
        if (type(registration.get("pilot")) is not bool or any(not isinstance(registration.get(key), str)
                or not registration[key].strip() for key in ("study_id", "operator_id"))):
            raise ValueError("Preregistration needs an explicit pilot flag, study identity and operator")
        if timestamp(registration["registered_at"]) > timestamp(report["created_at"]):
            raise ValueError("Preregistration occurred after coding outcomes were collected")
        if registration["arms"] != report["setups"]:
            raise ValueError("Preregistered comparison arms changed")
        registered = {item["case_id"]: item for item in registration["cases"]}
        if len(registered) != len(registration["cases"]) or set(registered) != {entry["case"]["id"] for entry in report["cases"]}:
            raise ValueError("Preregistered task cohort changed")
        if len(registered) < (6 if registration.get("pilot") is True else 30):
            raise ValueError("Preregistered cohort is below its minimum task count")
        projects = set()
        for entry in report["cases"]:
            gold = registered[entry["case"]["id"]]
            if (gold["task_sha256"] != task_contract_sha256(entry)
                    or not entry.get("checker_files_sha256")
                    or gold.get("checker_files_sha256") != entry["checker_files_sha256"]
                    or gold["source_sha256"] != entry["source_manifest"]["sha256"]
                    or gold["source_provenance"] != source_provenance(entry) or not gold["source_provenance"]["complete"]):
                raise ValueError("Preregistered task, checker contract or exact source pin changed")
            authors, checkers = gold["task_authors"], gold["checker_authors"]
            for identities in (authors, checkers):
                if (not isinstance(identities, list) or not identities
                        or any(not isinstance(name, str) or not name.strip() for name in identities)
                        or len({name.strip().casefold() for name in identities}) != len(identities)):
                    raise ValueError("Task/checker authors need distinct nonempty accountable identities")
            if {name.strip().casefold() for name in authors} & {name.strip().casefold() for name in checkers}:
                raise ValueError("Independent nonempty task and checker authorship is required")
            reviewed = independent_reviews(directory, gold["independent_gold_reviews"],
                excluded=[registration["operator_id"], *authors, *checkers], target_sha256=gold["task_sha256"])
            if not reviewed["complete"]:
                raise ValueError(reviewed["reason"])
            projects.add(gold["source_provenance"]["repository"])
        if len(projects) < 3:
            raise ValueError("Three independent upstream projects are required")
        agent = registration["agent"]
        validate_model_pins(registration, report)
        if agent["command_sha256"] != report["agent_command_sha256"] or agent["max_attempts"] != report["repair_policy"]["max_attempts"]:
            raise ValueError("Preregistered command or attempt limit changed")
        if agent["wall_time_seconds"] != report["repair_policy"]["attempt_budget_seconds"]:
            raise ValueError("Preregistered wall-time budget changed")
        for key in ("token_limit", "tool_call_limit"):
            if type(agent[key]) is not int or agent[key] < 0:
                raise ValueError("Token and tool limits must be explicitly pinned")
        if type(agent["temperature"]) not in (int, float) or not 0 <= agent["temperature"] <= 2:
            raise ValueError("Temperature must be pinned")
    except (ValueError, KeyError, TypeError, OSError) as error:
        issues.append(str(error))
    return {"complete": not issues, "status": "bound_preregistered_study" if not issues else "unmeasured",
            "pilot": registration.get("pilot"), "issues": issues, "sha256": coding.hash_file(path)}
