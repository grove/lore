#!/usr/bin/env python3
"""Preflight and retain one real six-arm study using the existing adaptive runner.

Preparation, sealing, preflight and assessment make no provider or subprocess
calls. Only run launches the existing runner, after every readiness gate passes.
External enforcement and authenticated capture review remain external services;
environment dictionaries, names and hashes do not create an OS sandbox.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
from urllib.parse import urlsplit

import adaptive_tasks as adaptive
import benchmark as bench
import coding_tasks as coding
import cross_source as cross
import outcome_protocol as outcome

PROTOCOL = "lore-experiment-08-v1"
ROOT = Path(__file__).resolve().parent
MAX_JSON_BYTES = 256_000_000
RUN_DEFAULTS = {
    "lore_binary": "", "provider": "ollama", "model": "",
    "embedding_model": None, "decision_provider": None, "decision_model": None,
    "generative_base_url": None, "decision_base_url": None,
    "allow_hosted": False, "allow_inspection": False, "allow_checkout_egress": False,
    "agent_command": [], "agent_id": "", "agent_location": "local",
    "timeout": 3600, "max_tokens": 6000, "max_attempts": 1,
    "attempt_budget_seconds": 7200, "seed": 7, "disable_reasoning": False,
    **{"reasoning_" + name: None for name in bench.REASONING_DEFAULTS if name != "enabled"},
}
ENFORCEMENT = (
    "agent_cannot_read_checkers", "checker_cannot_read_other_arms",
    "tool_and_token_limits_enforced", "wall_time_limits_enforced",
    "egress_allowlist_enforced", "runtime_and_egress_capture_active",
    "private_outputs", "real_model_adapters",
)


class AdapterArguments(argparse.ArgumentParser):
    def error(self, _message):
        # Commands can contain private operator arguments; never echo them.
        raise ValueError("Bundled coding adapter arguments are invalid")


def read(path: Path) -> dict:
    cross.reject_symlink_path(path)
    if not path.exists():
        raise ValueError("Required JSON artifact is missing: " + path.name)
    if not path.is_file() or path.stat().st_size > MAX_JSON_BYTES:
        raise ValueError("A bounded regular JSON file is required")
    value = cross.read_json(path)
    if not isinstance(value, dict):
        raise ValueError("A JSON object is required")
    return value


def private_write(path: Path, value: dict, *, exclusive=False) -> None:
    cross.reject_symlink_path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    flags = os.O_WRONLY | os.O_CREAT | (os.O_EXCL if exclusive else os.O_TRUNC)
    descriptor = os.open(path, flags | getattr(os, "O_NOFOLLOW", 0), 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())


def template(pilot: bool) -> dict:
    return {"schema_version": 1, "protocol": PROTOCOL, "mode": "pilot" if pilot else "full",
        "run": dict(RUN_DEFAULTS),
        "agent": {"provider": "ollama", "model": "", "model_revision": "", "reasoning": "",
                  "endpoint": "http://127.0.0.1:11434", "prompt_file": ""},
        "lore_revisions": {"generative": "", "decision": None, "embedding": None},
        "pilot_bundle": None,
        "study_wall_time_seconds": 43200, "output_retention_days": 30,
        "publish_raw_outputs": False}


def prepare(bundle: Path, cases: Path) -> dict:
    """Retain inputs and incomplete records; do not manufacture reviews or pins."""
    bundle = cross.selected_path(bundle)
    previous = os.umask(0o077)
    try:
        prepared = adaptive.prepare(bundle, cases)
        pilot = coding.load_cases(cases).get("pilot") is True
        private_write(bundle / "EXPERIMENT.json", template(pilot), exclusive=True)
    finally:
        os.umask(previous)
    return {"schema_version": 1, "protocol": PROTOCOL, "status": "prepared_not_run",
            "tasks": len(prepared["cases"]), "arms": list(adaptive.SETUPS),
            "initial_assignments": len(prepared["cases"]) * len(adaptive.SETUPS),
            "model_calls": 0, "coding_productivity": "unmeasured"}


def specification(bundle: Path) -> dict:
    spec = read(bundle / "EXPERIMENT.json")
    if (spec.get("schema_version") != 1 or spec.get("protocol") != PROTOCOL
            or spec.get("mode") not in ("pilot", "full")
            or set(spec.get("run", {})) != set(RUN_DEFAULTS)
            or set(spec.get("agent", {})) != {"provider", "model", "model_revision", "reasoning", "endpoint", "prompt_file"}
            or set(spec.get("lore_revisions", {})) != {"generative", "decision", "embedding"}
            or set(spec) != set(template(spec.get("mode") == "pilot"))):
        raise ValueError("Expected the complete version-1 experiment and run options")
    return spec


def prepared_inputs(bundle: Path) -> dict:
    prepared = read(bundle / "prepared.json")
    if (prepared.get("phase") != "preparation_only" or prepared.get("inference_calls") != 0
            or prepared.get("setups") != list(adaptive.SETUPS)):
        raise ValueError("Expected preparation-only inputs for the six unchanged arms")
    cases_path = Path(prepared["cases_manifest"])
    if coding.hash_file(cases_path) != prepared["cases_manifest_sha256"]:
        raise ValueError("The selected source/task manifest changed after preparation")
    declared = {case["id"]: case for case in coding.load_cases(cases_path)["cases"]}
    entries = prepared["cases"]
    identities = [entry["case"]["id"] for entry in entries]
    if (not 1 <= len(entries) <= 1000 or len(set(identities)) != len(entries)
            or set(identities) != set(declared)):
        raise ValueError("Prepared task identities are missing or duplicated")
    for entry in entries:
        case = entry["case"]
        if declared.get(case["id"]) != case:
            raise ValueError("Prepared tasks no longer match the selected manifest")
        source = bundle / coding.relative_file(entry["source_root"])
        if cross.fingerprint(source) != entry["source_manifest"]:
            raise ValueError("Pinned prepared source bytes changed")
        if "base_project" not in case:
            original = cases_path.parent / case["source_root"]
            if cross.fingerprint(original) != entry["source_manifest"]:
                raise ValueError("Source inputs would differ when the runner copies them")
        if not outcome.source_provenance(entry)["complete"]:
            raise ValueError("Each task needs an exact upstream commit and input digest")
    return prepared


def checker_separation(bundle: Path, prepared: dict) -> dict:
    manifest = Path(prepared["cases_manifest"])
    source_hashes = {value for entry in prepared["cases"]
                     for value in entry["source_manifest"]["files_sha256"].values()}
    files = {}
    for entry in prepared["cases"]:
        source = bundle / coding.relative_file(entry["source_root"])
        coding.validate_prepared_checker(entry, source, manifest.parent)
        pinned = entry.get("checker_files_sha256")
        if not pinned:
            raise ValueError("Independent checker programs have not been pinned")
        for name, digest in pinned.items():
            path = Path(name)
            cross.reject_symlink_path(path)
            if path.is_relative_to(source) or digest in source_hashes:
                raise ValueError("A checker or answer-bearing helper is in model source input")
            files[name] = digest
    return {"files_sha256": files,
            "qualification": "Path and byte separation; external access enforcement is a separate required gate."}


def cohort(spec: dict, prepared: dict) -> dict:
    entries = prepared["cases"]
    projects = {outcome.source_provenance(entry).get("repository") for entry in entries}
    if None in projects or len(projects) < 3:
        raise ValueError("Three exactly pinned upstream projects are required")
    if spec["mode"] == "pilot" and len(entries) != 6:
        raise ValueError("The preregistered pilot requires exactly six tasks")
    if spec["mode"] == "full":
        if (len(entries) < 30 or prepared.get("fixture_only") is not False
                or prepared.get("held_out") is not True or prepared.get("independent_projects") is not True
                or prepared.get("recognized_fixture_fingerprints")
                or any("base_project" in entry["case"] for entry in entries)):
            raise ValueError("Full study requires at least 30 separate, nonpublic held-out tasks")
        known = coding.bundled_fingerprints() | cross.bundled_fingerprints()
        if any(entry["source_manifest"]["sha256"] in known for entry in entries):
            raise ValueError("Relabeled public candidates cannot become a held-out study")
    return {"tasks": len(entries), "projects": len(projects),
            "development_candidates": prepared["fixture_only"]}


def holdout_nonoverlap(bundle: Path, spec: dict, prepared: dict) -> dict:
    if spec["mode"] == "pilot":
        return {"status": "pilot_not_holdout", "outcomes_read": False}
    import failure_triage

    name = spec.get("pilot_bundle")
    if not isinstance(name, str) or not Path(name).is_absolute():
        raise ValueError("Full study requires the separately retained, completed pilot bundle")
    pilot = cross.selected_path(Path(name))
    if pilot == bundle or pilot.is_relative_to(bundle) or bundle.is_relative_to(pilot):
        raise ValueError("Pilot and untouched holdout must use disjoint study directories")
    execution = read(pilot / "EXECUTION.json")
    assessment = adaptive.assess(pilot / "run")
    metrics = read(pilot / "run" / "metrics.json")
    if (execution.get("status") != "completed" or not assessment["mechanical_contracts_passed"]
            or not assessment["integrity_audit"]["passed"] or not assessment["preregistration"]["complete"]
            or not assessment["independent_answer_reviews_complete"]
            or any(not attempt["response"]["usage"]["model_calls"] for sample in metrics["samples"]
                   for attempt in sample["attempts"])):
        raise ValueError("The actual pilot still needs model execution, independent reviews or external audit")
    diagnostics = failure_triage.summarize_validated(metrics, assessment, pilot / "run")
    return failure_triage.validate_holdout(bundle / "prepared.json",
        [entry["case"]["id"] for entry in prepared["cases"]], diagnostics)


def arguments(bundle: Path, spec: dict, prepared: dict) -> argparse.Namespace:
    values = dict(spec["run"])
    for field in ("allow_hosted", "allow_inspection", "allow_checkout_egress", "disable_reasoning"):
        if type(values[field]) is not bool:
            raise ValueError("Run grants and reasoning toggle require explicit booleans")
    for field in ("timeout", "max_tokens", "max_attempts", "attempt_budget_seconds", "seed"):
        if type(values[field]) is not int:
            raise ValueError("Run budgets and the fixed seed require integers")
    if not 0 <= values["seed"] < 2**64:
        raise ValueError("A fixed unsigned 64-bit seed is required")
    if (not values["model"] or not values["agent_id"] or values["agent_location"] not in ("local", "hosted")
            or values["provider"] not in ("ollama", "openai")):
        raise ValueError("Configure a real Lore model and pinned coding-agent identity")
    if values["agent_location"] == "hosted" and not values["allow_hosted"]:
        raise ValueError("Hosted coding execution requires the explicit study egress grant")
    if type(spec["study_wall_time_seconds"]) is not int or not 1 <= spec["study_wall_time_seconds"] <= 604800:
        raise ValueError("The entire study needs a wall-time bound of 1..604800 seconds")
    if not Path(values["lore_binary"]).is_absolute():
        raise ValueError("Select an absolute path to the pinned Lore executable")
    values["lore_binary"] = Path(values["lore_binary"])
    if not values["lore_binary"].is_file() or not os.access(values["lore_binary"], os.X_OK):
        raise ValueError("A built executable Lore binary is required")
    command = values["agent_command"]
    if (not isinstance(command, list) or not command
            or any(not isinstance(item, str) or not item or "\x00" in item for item in command)
            or not Path(command[0]).is_absolute()
            or not Path(command[0]).is_file() or not os.access(command[0], os.X_OK)):
        raise ValueError("Use an exact JSON argv array with an absolute executable coding adapter")
    values.update(agent_command=json.dumps(command), output=bundle / "run",
                  cases=Path(prepared["cases_manifest"]), preregistration=bundle / "PREREGISTRATION.json")
    args = argparse.Namespace(**values)
    adaptive.policy_from_args(args)
    coding.repair_policy(args)
    return args


def endpoint(value: str, allowed: bool) -> str:
    parts = urlsplit(value)
    if (parts.scheme not in ("http", "https") or not parts.hostname or parts.username
            or parts.password or parts.query or parts.fragment):
        raise ValueError("Provider endpoints require explicit HTTP(S) hosts without secrets")
    try:
        local = ipaddress.ip_address(parts.hostname).is_loopback
    except ValueError:
        local = False
    if not local and (not allowed or parts.scheme != "https"):
        raise ValueError("A non-loopback provider requires HTTPS and the hosted egress grant")
    return value.rstrip("/")


def configured_models(bundle: Path, spec: dict, prepared: dict, args: argparse.Namespace) -> dict:
    agent = dict(spec["agent"])
    for key in ("model", "model_revision", "reasoning"):
        if not isinstance(agent.get(key), str) or not agent[key].strip():
            raise ValueError("Configure the actual coding model, revision and reasoning")
    if agent["provider"] not in ("ollama", "openai"):
        raise ValueError("A supported real coding provider must be declared")
    agent["endpoint"] = endpoint(agent["endpoint"], args.allow_hosted)
    prompt = Path(agent.pop("prompt_file"))
    if not prompt.is_absolute():
        prompt = bundle / coding.relative_file(str(prompt))
    prompt_hash = coding.hash_file(prompt)
    command = spec["run"]["agent_command"]
    adapter = str(ROOT / "coding_agent.py")
    if adapter in command:
        # The bundled adapter has a known contract. Other adapters require the
        # captured external tool/model review, in addition to their command pin.
        import coding_agent
        parser = AdapterArguments(add_help=False)
        parser.add_argument("--provider", choices=("ollama", "openai"), required=True)
        parser.add_argument("--model", required=True)
        parser.add_argument("--base-url")
        parser.add_argument("--key-env", default="OPENAI_API_KEY")
        parser.add_argument("--allow-hosted", action="store_true")
        parser.add_argument("--reasoning-effort")
        parser.add_argument("--timeout", type=int, default=600)
        parser.add_argument("--retries", type=int, default=0)
        parser.add_argument("--usage-ledger")
        configured = parser.parse_args(command[command.index(adapter) + 1:])
        base = configured.base_url or ("http://127.0.0.1:11434" if configured.provider == "ollama"
                                       else "https://api.openai.com/v1")
        if (configured.provider != agent["provider"] or configured.model != agent["model"]
                or endpoint(base, configured.allow_hosted) != agent["endpoint"]
                or (configured.reasoning_effort or "none") != agent["reasoning"]
                or configured.allow_hosted and not args.allow_hosted or configured.usage_ledger is not None):
            raise ValueError("Bundled coding adapter arguments broaden grants or differ from declared model/settings")
        if prompt_hash != hashlib.sha256(coding_agent.INSTRUCTIONS.encode("utf-8")).hexdigest():
            raise ValueError("Bundled coding adapter prompt bytes differ from the pinned prompt artifact")
    configs, providers = {}, {"agent": agent}
    for entry in prepared["cases"]:
        case = entry["case"]
        config = coding.config_for_case(args, case, bundle / "run" / "lore" / case["id"])
        config["context"] = {"cache": True, "inspection": {"root": ".", "enabled": False}}
        config.setdefault("privacy", {})["allow_checkout_egress"] = args.allow_checkout_egress
        configs[case["id"]] = cross.digest(config)
        for role in ("generative", "decision", "embedding"):
            model = config["models"].get(role)
            if not model or model.get("enabled") is False:
                continue
            revision = spec["lore_revisions"][role]
            if not isinstance(revision, str) or not revision.strip():
                raise ValueError("Every enabled Lore model role requires a pinned revision")
            provider = model["provider"]
            providers[role] = {"provider": provider, "model": model["model"], "model_revision": revision,
                "endpoint": endpoint(config["providers"][provider]["base_url"], args.allow_hosted),
                "reasoning": config["models"]["reasoning"]}
    return {"configuration_sha256_by_case": configs, "providers": providers, "prompt_sha256": prompt_hash}


def model_configuration(bundle: Path, spec: dict, prepared: dict, args: argparse.Namespace) -> dict:
    registration = read(bundle / "PREREGISTRATION.json")
    outcome.validate_model_declarations(registration)
    configured = configured_models(bundle, spec, prepared, args)
    for key in ("model", "model_revision", "reasoning"):
        if configured["providers"]["agent"][key] != registration["agent"][key]:
            raise ValueError("Coding model declarations differ from preregistration")
    if configured["prompt_sha256"] != registration["agent"]["prompt_sha256"]:
        raise ValueError("The exact coding prompt artifact differs from its preregistered digest")
    if (registration["lore"]["model"] != args.model
            or registration["lore"]["model_revision"] != spec["lore_revisions"]["generative"]
            or registration["lore"]["configuration_sha256_by_case"] != configured["configuration_sha256_by_case"]):
        raise ValueError("Preregistered Lore model or complete per-case configurations differ")
    return configured


def pins(bundle: Path) -> dict:
    bundle = cross.selected_path(bundle)
    spec, prepared = specification(bundle), prepared_inputs(bundle)
    args = arguments(bundle, spec, prepared)
    result = {"schema_version": 1, "status": "computed_pins_not_run",
        "agent_command_sha256": cross.digest(spec["run"]["agent_command"]),
        "configuration": configured_models(bundle, spec, prepared, args),
        "provider_calls": 0, "coding_productivity": "unmeasured"}
    if (bundle / "READINESS.json").exists():
        readiness = read(bundle / "READINESS.json")
        result["readiness_review_target_sha256"] = cross.digest(
            {key: value for key, value in readiness.items() if key not in ("reviews", "qualification")})
    return result


def registration_inputs(bundle: Path, spec: dict, prepared: dict, args: argparse.Namespace, at: str) -> dict:
    registration = read(bundle / "PREREGISTRATION.json")
    planned = {"cases": prepared["cases"], "setups": list(adaptive.SETUPS), "created_at": at,
               "agent_command_sha256": cross.digest(spec["run"]["agent_command"]),
               "repair_policy": coding.repair_policy(args)}
    outcome.validate_preregistration_inputs(bundle, registration, planned)
    if registration["pilot"] != (spec["mode"] == "pilot"):
        raise ValueError("Pilot/full policy differs from preregistration")
    expected = outcome.preregistration_template(prepared, arms=list(adaptive.SETUPS),
                                                pilot=registration["pilot"])
    for field in ("outcomes", "exclusions", "analysis", "minimum_attempted_tasks"):
        if registration[field] != expected[field]:
            raise ValueError("The declared denominators or preregistered effect/analysis policy changed")
    for case in registration["cases"]:
        for review in case["independent_gold_reviews"]:
            if outcome.timestamp(review["reviewed_at"]) > outcome.timestamp(registration["registered_at"]):
                raise ValueError("Gold reviews must predate the sealed preregistration")
            if not review["capture"]["path"].startswith("gold-review-captures/"):
                raise ValueError("Gold-review captures must use the runner's separate capture directory")
    return registration


def code_identity() -> dict:
    names = ("experiment_08.py", "outcome_protocol.py", "adaptive_tasks.py", "coding_tasks.py",
             "coding_agent.py", "provider_usage.py", "decision_tasks.py", "cross_source.py",
             "benchmark.py", "shared_intelligence.py", "failure_triage.py")
    return {name: coding.hash_file(ROOT / name) for name in names}


def contract_for(bundle: Path, spec: dict, prepared: dict, args: argparse.Namespace, at: str) -> dict:
    registration = registration_inputs(bundle, spec, prepared, args, at)
    models = model_configuration(bundle, spec, prepared, args)
    command_files = {}
    for value in spec["run"]["agent_command"]:
        candidate = Path(value) if Path(value).is_absolute() else bundle / value
        if candidate.is_file():
            cross.reject_symlink_path(candidate)
            command_files[str(candidate)] = coding.hash_file(candidate)
    return {"schema_version": 1, "protocol": PROTOCOL,
        "experiment_sha256": coding.hash_file(bundle / "EXPERIMENT.json"),
        "prepared_sha256": coding.hash_file(bundle / "prepared.json"),
        "preregistration_sha256": coding.hash_file(bundle / "PREREGISTRATION.json"),
        "cases_manifest_sha256": prepared["cases_manifest_sha256"],
        "source_sha256_by_case": {entry["case"]["id"]: entry["source_manifest"]["sha256"]
                                 for entry in prepared["cases"]},
        "checker_files_sha256": checker_separation(bundle, prepared)["files_sha256"],
        "agent_command_sha256": cross.digest(spec["run"]["agent_command"]),
        "agent_command_files_sha256": command_files,
        "python_sha256": coding.hash_file(Path(sys.executable).resolve()),
        "lore_binary_sha256": coding.hash_file(args.lore_binary),
        "collector_files_sha256": code_identity(),
        "model_configuration": models,
        "study_id": registration["study_id"], "mode": spec["mode"],
        "arms": list(adaptive.SETUPS), "seed": args.seed,
        "grants": adaptive.policy_from_args(args), "repair_policy": coding.repair_policy(args),
        "agent_limits": {key: registration["agent"][key]
                         for key in ("tools", "token_limit", "tool_call_limit", "temperature")}}


def privacy(bundle: Path, spec: dict, fresh: bool) -> dict:
    cross.reject_symlink_path(bundle)
    if os.name != "posix" or bundle.stat().st_mode & 0o077:
        raise ValueError("Use a private mode-0700 study directory in the externally isolated runner")
    if (type(spec["output_retention_days"]) is not int or not 1 <= spec["output_retention_days"] <= 365
            or spec.get("publish_raw_outputs") is not False):
        raise ValueError("Pin private output retention; raw candidate/checker material is not public output")
    if fresh and any((bundle / name).exists() for name in ("run", "EXECUTION.json", "PREFLIGHT_AT_LAUNCH.json")):
        raise ValueError("A run is already present; never resume or replace an aborted study")
    return {"private_parent": True, "retention_days": spec["output_retention_days"],
            "automatic_deletion": False}


def readiness_template(contract: dict) -> dict:
    return {"schema_version": 1, "protocol": PROTOCOL + "-readiness", "complete": False,
        "contract_sha256": cross.digest(contract), "observed_at": "", "expires_at": "",
        "providers": {role: {**value, "available": None} for role, value in
                      contract["model_configuration"]["providers"].items()},
        "enforcement": {"method": "", **dict.fromkeys(ENFORCEMENT, None)},
        "captures": [], "authentication": {"method": "", "capture": None},
        "reviews": [],
        "qualification": "External enforcement and authenticated reviewer evidence must be supplied; this template is not an audit."}


def readiness_status(bundle: Path, contract: dict, at: str) -> dict:
    value = read(bundle / "READINESS.json")
    if (value.get("schema_version") != 1 or value.get("protocol") != PROTOCOL + "-readiness"
            or value.get("complete") is not True or value.get("contract_sha256") != cross.digest(contract)):
        raise ValueError("Bound external provider/isolation readiness evidence is pending")
    observed, expiry, now = (outcome.timestamp(value["observed_at"]),
                            outcome.timestamp(value["expires_at"]), outcome.timestamp(at))
    if not observed <= now < expiry or (expiry - observed).total_seconds() > 86400:
        raise ValueError("External readiness evidence is expired, future-dated, or longer than 24 hours")
    expected = contract["model_configuration"]["providers"]
    if set(value["providers"]) != set(expected):
        raise ValueError("Availability evidence must cover every enabled model role")
    for role, pin in expected.items():
        if value["providers"][role] != {**pin, "available": True}:
            raise ValueError("A pinned coding or Lore provider is unavailable or has changed identity")
    enforcement = value["enforcement"]
    if (not isinstance(enforcement.get("method"), str) or not enforcement["method"].strip()
            or any(enforcement.get(key) is not True for key in ENFORCEMENT)):
        raise ValueError("Independent OS isolation, checker separation, tool limits and active egress capture are required")
    captures = value["captures"]
    if not isinstance(captures, list) or {item.get("kind") for item in captures} != {
            "provider_availability", "runtime_isolation", "egress_enforcement"}:
        raise ValueError("Retain separate provider, runtime-isolation and egress-enforcement captures")
    for item in captures:
        outcome.capture(bundle, item)
    authentication = value["authentication"]
    if not isinstance(authentication.get("method"), str) or not authentication["method"].strip():
        raise ValueError("The externally authenticated audit needs a retained authentication method and capture")
    outcome.capture(bundle, authentication["capture"])
    subject = {key: item for key, item in value.items() if key not in ("reviews", "qualification")}
    registration = read(bundle / "PREREGISTRATION.json")
    excluded = [registration["operator_id"]]
    for case in registration["cases"]:
        excluded.extend(case["task_authors"] + case["checker_authors"])
    reviewed = outcome.independent_reviews(bundle, value["reviews"], excluded=excluded,
                                          target_sha256=cross.digest(subject))
    if not reviewed["complete"]:
        raise ValueError("Two independent external readiness-capture reviews are required")
    for review in value["reviews"]:
        if not observed <= outcome.timestamp(review["reviewed_at"]) <= now:
            raise ValueError("Readiness reviews must follow the captured control and precede launch")
    return {"status": "bound_external_readiness", "sha256": coding.hash_file(bundle / "READINESS.json"),
            "subject_sha256": cross.digest(subject), "providers": len(expected),
            "authentication": "externally supplied capture; identity is not authenticated by this Python harness",
            "reviewers": reviewed["reviewers"]}


def _preflight(bundle: Path, *, at: str | None = None, fresh=True) -> tuple[dict, dict]:
    bundle = cross.selected_path(bundle)
    at = at or bench.now_utc()
    checks, cache = [], {}

    def check(name, operation, requires=()):
        missing = [key for key in requires if key not in cache]
        if missing:
            checks.append({"gate": name, "passed": False,
                           "reason": "Requires passing gate: " + ", ".join(missing)})
            return
        try:
            cache[name] = operation()
            checks.append({"gate": name, "passed": True})
        except (ValueError, OSError, KeyError, TypeError, SystemExit) as error:
            # Known validation messages contain no provider output or env values.
            checks.append({"gate": name, "passed": False,
                           "reason": str(error) if isinstance(error, ValueError) else type(error).__name__})

    check("specification", lambda: specification(bundle))
    check("pinned_source_snapshots", lambda: prepared_inputs(bundle))
    check("checker_bytes_and_input_separation",
          lambda: checker_separation(bundle, cache["pinned_source_snapshots"]), ("pinned_source_snapshots",))
    check("cohort", lambda: cohort(cache["specification"], cache["pinned_source_snapshots"]),
          ("specification", "pinned_source_snapshots"))
    check("full_study_holdout_nonoverlap", lambda: holdout_nonoverlap(bundle, cache["specification"],
        cache["pinned_source_snapshots"]), ("specification", "pinned_source_snapshots", "cohort"))
    check("grants_models_and_limits", lambda: arguments(bundle, cache["specification"], cache["pinned_source_snapshots"]),
          ("specification", "pinned_source_snapshots"))

    def declaration_complete():
        registration = read(bundle / "PREREGISTRATION.json")
        if registration.get("complete") is not True:
            raise ValueError("Preregistration, independent task/checker authors and two gold reviews per task are pending")
        return registration
    check("preregistration_record", declaration_complete)
    check("preregistration_and_independent_gold_reviews", lambda: registration_inputs(bundle,
        cache["specification"], cache["pinned_source_snapshots"], cache["grants_models_and_limits"], at),
        ("specification", "pinned_source_snapshots", "grants_models_and_limits", "preregistration_record"))
    check("model_command_prompt_and_configuration", lambda: model_configuration(bundle,
        cache["specification"], cache["pinned_source_snapshots"], cache["grants_models_and_limits"]),
        ("specification", "pinned_source_snapshots", "grants_models_and_limits"))
    check("private_output", lambda: privacy(bundle, cache["specification"], fresh), ("specification",))
    check("current_contract", lambda: contract_for(bundle, cache["specification"],
        cache["pinned_source_snapshots"], cache["grants_models_and_limits"], at),
        ("specification", "pinned_source_snapshots", "grants_models_and_limits",
         "preregistration_and_independent_gold_reviews", "model_command_prompt_and_configuration"))

    def validate_seal():
        seal = read(bundle / "SEAL.json")
        current = cache["current_contract"]
        if (seal.get("contract") != current or seal.get("contract_sha256") != cross.digest(current)
                or outcome.timestamp(seal["sealed_at"]) > outcome.timestamp(at)):
            raise ValueError("The sealed source/command/model/preregistration contract changed")
        return seal
    check("sealed_contract", validate_seal, ("current_contract",))
    check("external_readiness_record", lambda: read(bundle / "READINESS.json"))
    check("provider_availability_and_external_runtime_egress_audit", lambda:
          readiness_status(bundle, cache["sealed_contract"]["contract"], at),
          ("sealed_contract", "external_readiness_record"))
    passed = all(item["passed"] for item in checks)
    prepared = cache.get("pinned_source_snapshots", {})
    report = {"schema_version": 1, "protocol": PROTOCOL, "checked_at": at,
        "status": "ready_for_external_execution" if passed else "not_run",
        "preflight_passed": passed, "checks": checks,
        "blockers": [item for item in checks if not item["passed"]],
        "tasks": len(prepared.get("cases", [])), "arms": list(adaptive.SETUPS),
        "initial_assignments": len(prepared.get("cases", [])) * len(adaptive.SETUPS),
        "contract_sha256": cross.digest(cache["current_contract"]) if "current_contract" in cache else None,
        "readiness_sha256": cache.get("provider_availability_and_external_runtime_egress_audit", {}).get("sha256"),
        "provider_calls": 0, "subprocess_calls": 0,
        "coding_productivity": "unmeasured",
        "qualification": "Readiness authorizes no empirical claim. Post-run model identity, checker/answer reviews and execution/egress audit remain required. No environment allowlist is treated as a sandbox."}
    return report, cache


def preflight(bundle: Path, *, at: str | None = None, fresh=True) -> dict:
    return _preflight(bundle, at=at, fresh=fresh)[0]


def seal(bundle: Path) -> dict:
    bundle = cross.selected_path(bundle)
    at = bench.now_utc()
    spec, prepared = specification(bundle), prepared_inputs(bundle)
    args = arguments(bundle, spec, prepared)
    cohort(spec, prepared)
    holdout_nonoverlap(bundle, spec, prepared)
    privacy(bundle, spec, True)
    contract = contract_for(bundle, spec, prepared, args, at)
    value = {"schema_version": 1, "sealed_at": at, "contract": contract,
             "contract_sha256": cross.digest(contract)}
    if (bundle / "SEAL.json").exists() or (bundle / "READINESS.json").exists():
        raise ValueError("Use a fresh seal and readiness record; existing evidence cannot be replaced")
    private_write(bundle / "SEAL.json", value, exclusive=True)
    private_write(bundle / "READINESS.json", readiness_template(contract), exclusive=True)
    return {"schema_version": 1, "status": "sealed_readiness_pending",
            "contract_sha256": value["contract_sha256"], "coding_productivity": "unmeasured"}


def runner_command(bundle: Path, spec: dict, prepared: dict) -> list[str]:
    command = [str(Path(sys.executable).resolve()), str(ROOT / "adaptive_tasks.py"), "run",
               "--cases", str(prepared["cases_manifest"]),
               "--output", str(bundle / "run"), "--preregistration", str(bundle / "PREREGISTRATION.json")]
    for name, value in spec["run"].items():
        flag = "--" + name.replace("_", "-")
        if type(value) is bool:
            if value:
                command.append(flag)
        elif value is not None:
            command += [flag, json.dumps(value) if name == "agent_command" else str(value)]
    return command


def invocation_outcome(record: dict, provider: dict | None) -> str:
    events = provider["ledger"]["events"] if provider else []
    statuses = {event["status"] for event in events}
    if record.get("status") == "completed":
        return "completed"
    if record.get("status") == "cancelled" or "cancelled" in statuses:
        return "cancelled"
    if "timed_out" in statuses or record.get("error_type") == "TimeoutExpired":
        return "timed_out"
    if "refused" in statuses:
        return "refused"
    return "interrupted" if record.get("status") == "started" else "failed"


def retention(bundle: Path, execution: dict) -> dict:
    """Reconcile assignments and actual journals without completing missing work."""
    import provider_usage as metering

    root = bundle / "run"
    assignments = execution.get("planned_assignments")
    if assignments is None:  # Direct inspection of a pre-wrapper invocation journal.
        prepared = read(bundle / "prepared.json")
        assignments = [{"case_id": entry["case"]["id"], "arm": arm}
                       for entry in prepared["cases"] for arm in adaptive.SETUPS]
    slots = {(item["case_id"], item["arm"]): {
        **item, "status": "not_started",
        "sample_id": None, "attempts": [], "reported_checker_success": None,
        "time_to_first_correct_seconds": None, "total_seconds": None,
        "total_usage": metering.normalized(None)}
        for item in assignments}
    if len(slots) != len(assignments) or any(arm not in adaptive.SETUPS for _, arm in slots):
        raise ValueError("Planned assignments are duplicated or contain an unknown arm")
    identities, files, completed_attempts = {}, {}, {}
    for path in sorted((root / "sample-starts").glob("*.json")):
        value = read(path)
        key = (value["case_id"], value["setup"])
        if key not in slots or slots[key]["sample_id"] is not None or path.stem != value["sample_id"]:
            raise ValueError("Started sample identities are duplicated or outside the sealed assignments")
        identities[value["sample_id"]] = key
        slots[key].update(status="started", sample_id=value["sample_id"],
                          context_usage=value["context_usage"], preparation_usage=value["preparation_usage"])
        files[str(path.relative_to(bundle))] = coding.hash_file(path)
    metrics_path = root / "metrics.json"
    metrics = read(metrics_path) if metrics_path.is_file() else {"samples": []}
    if metrics_path.is_file():
        files[str(metrics_path.relative_to(bundle))] = coding.hash_file(metrics_path)
    for sample in metrics["samples"]:
        key = (sample["case_id"], sample["setup"])
        if (key not in slots or slots[key]["status"] == "completed"
                or identities.get(sample["sample_id"]) != key):
            raise ValueError("Completed samples differ from their pre-invocation journal")
        coding.validate_checker_result(sample["tests"])
        slots[key].update(status="completed", reported_checker_success=all(
            item["passed"] for item in sample["tests"]["checks"]),
            total_seconds=sample["costs"]["total_seconds"], total_usage=sample["costs"]["total_usage"],
            context_usage=sample["costs"]["context_usage"], coding_usage=sample["costs"]["coding_usage"],
            time_to_first_correct_seconds=sample["time_to_first_correct_seconds"])
        completed_attempts[key] = len(sample["attempts"])
    invocations = list((root / "attempt-invocations").glob("*/*.json"))
    if len(invocations) > 100000:
        raise ValueError("Attempt journal exceeds the bounded study limit")
    for path in sorted(invocations):
        if path.name.endswith("-provider-usage.json"):
            continue
        if path.parent.name not in identities:
            raise ValueError("An attempted invocation has no case/arm start record")
        value = read(path)
        provider = metering.read(path.with_name(f"{value['number']}-provider-usage.json"))
        captured = value.get("usage_ledger")
        if captured is not None and captured != provider:
            raise ValueError("Invocation usage differs from its exact retained sidecar")
        usage = metering.summary_from_record(provider) or metering.normalized(value.get("usage"))
        state = invocation_outcome(value, provider)
        slot = slots[identities[path.parent.name]]
        slot["attempts"].append({"number": value["number"], "status": state,
            "stage": value.get("stage"), "elapsed_seconds": value.get("elapsed_seconds"),
            "usage": usage, "provider_ledger_sha256": provider["sha256"] if provider else None,
            "record_sha256": coding.hash_file(path)})
        if slot["status"] != "completed":
            slot["status"] = state if state != "completed" else "interrupted_after_checked_attempt"
        files[str(path.relative_to(bundle))] = coding.hash_file(path)
    # Raw event IDs identify actual requests. Initialization sidecars copied to
    # isolated context roots are retained but not charged repeatedly here.
    events = {}
    ledger_paths = list(root.glob("lore/*/.lore/provider-usage/*.json"))
    ledger_paths += list(root.glob("context-projects/*/*/.lore/provider-usage/*.json"))
    ledger_paths += list(root.glob("attempt-invocations/*/*-provider-usage.json"))
    if len(ledger_paths) > 100000:
        raise ValueError("Provider ledger file count exceeds the bounded study limit")
    for path in sorted(ledger_paths):
        record = metering.read(path)
        files[str(path.relative_to(bundle))] = record["sha256"]
        for event in record["ledger"]["events"]:
            previous = events.get(event["id"])
            if previous is not None and previous != event:
                raise ValueError("A copied provider event changed its usage or identity")
            events[event["id"]] = event
    rows = list(slots.values())
    for key, row in slots.items():
        row["attempts"].sort(key=lambda item: item["number"])
        if [item["number"] for item in row["attempts"]] != list(range(1, len(row["attempts"]) + 1)):
            raise ValueError("An attempted invocation was dropped or duplicated")
        if key in completed_attempts and (len(row["attempts"]) != completed_attempts[key]
                or any(item["status"] != "completed" for item in row["attempts"])):
            raise ValueError("Completed sample dropped a started, failed or cancelled invocation")
        if row["status"] != "completed":
            row["coding_usage"] = metering.add([item["usage"] for item in row["attempts"]]) if row["attempts"] else metering.normalized(None)
    return {"schema_version": 1, "protocol": PROTOCOL,
        "planned_initial_assignments": len(rows),
        "attempted_assignments": sum(bool(row["attempts"]) for row in rows),
        "completed_checked_assignments": sum(row["status"] == "completed" for row in rows),
        "reported_successes": sum(row["reported_checker_success"] is True for row in rows),
        "success_denominator": len(rows), "assignments": rows,
        "attempted_invocations": sum(len(row["attempts"]) for row in rows),
        "failed_refused_cancelled_or_interrupted_invocations": sum(
            item["status"] != "completed" for row in rows for item in row["attempts"]),
        "observed_study_wall_seconds": execution.get("elapsed_seconds"),
        "captured_provider_usage": metering.aggregate(list(events.values())) if ledger_paths else metering.normalized(None),
        "provider_usage_complete": False,
        "files_sha256": files, "artifact_manifest_sha256": cross.digest(files),
        "coding_productivity": "unmeasured",
        "qualification": "All planned arms and actual attempts remain in denominators. Raw captured events are a lower coverage ledger, not a complete bill; unobserved cost stays null. Shared initialization is charged in full per Lore comparison arm by the existing assessor. Aborted tasks have unknown all-in cost and no observed time to success."}


def stop(process: subprocess.Popen) -> None:
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGINT)
        else:
            process.terminate()
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    # Also terminate same-group descendants after a parent exits on SIGINT.
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        elif process.poll() is None:
            process.kill()
    except ProcessLookupError:
        pass
    process.wait()


def run(bundle: Path) -> dict:
    bundle = cross.selected_path(bundle)
    report, checked = _preflight(bundle)
    # Build argv exclusively from the validated in-memory snapshot. Nothing
    # read after the gate may substitute another command, grant or manifest.
    spec = checked.get("specification")
    command = runner_command(bundle, spec, checked["pinned_source_snapshots"]) if report["preflight_passed"] else None
    private_write(bundle / "PREFLIGHT.json", report)
    if not report["preflight_passed"]:
        return report
    private_write(bundle / "PREFLIGHT_AT_LAUNCH.json", report, exclusive=True)
    execution = {"schema_version": 1, "protocol": PROTOCOL, "status": "started",
        "started_at": bench.now_utc(), "preflight_sha256": coding.hash_file(bundle / "PREFLIGHT_AT_LAUNCH.json"),
        "readiness_sha256": report["readiness_sha256"], "seal_sha256": coding.hash_file(bundle / "SEAL.json"),
        "contract_sha256": report["contract_sha256"], "elapsed_seconds": None,
        "planned_assignments": [{"case_id": entry["case"]["id"], "arm": arm}
                               for entry in checked["pinned_source_snapshots"]["cases"] for arm in adaptive.SETUPS],
        "returncode": None, "coding_productivity": "unmeasured"}
    private_write(bundle / "EXECUTION.json", execution, exclusive=True)
    start = time.monotonic()
    process = None
    previous = os.umask(0o077)
    old_term = signal.getsignal(signal.SIGTERM)

    def cancelled(_signal, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, cancelled)
    try:
        barrier, _ = _preflight(bundle, fresh=False)
        if (not barrier["preflight_passed"] or barrier["contract_sha256"] != execution["contract_sha256"]
                or barrier["readiness_sha256"] != execution["readiness_sha256"]
                or coding.hash_file(bundle / "SEAL.json") != execution["seal_sha256"]):
            execution["status"] = "not_started_inputs_changed"
        else:
            process = subprocess.Popen(command, cwd=bundle,
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                start_new_session=os.name == "posix")
            execution["child_pid"] = process.pid
            private_write(bundle / "EXECUTION.json", execution)
            execution["returncode"] = process.wait(timeout=spec["study_wall_time_seconds"])
            execution["status"] = "completed" if execution["returncode"] == 0 else "failed"
    except BaseException as error:
        execution["status"] = ("timed_out" if isinstance(error, subprocess.TimeoutExpired) else
            "cancelled" if isinstance(error, (KeyboardInterrupt, SystemExit)) else
            "failed_after_start" if process is not None else "failed_to_start")
        execution["error_type"] = type(error).__name__
        if process is not None:
            stop(process)
            execution["returncode"] = process.returncode
        if not isinstance(error, (OSError, subprocess.TimeoutExpired, KeyboardInterrupt, SystemExit)):
            raise
    finally:
        if process is not None:
            stop(process)
        signal.signal(signal.SIGTERM, old_term)
        os.umask(previous)
        execution["elapsed_seconds"] = time.monotonic() - start
        execution["finished_at"] = bench.now_utc()
        private_write(bundle / "EXECUTION.json", execution)
    if execution["status"] == "not_started_inputs_changed":
        retained = retention(bundle, execution)
        private_write(bundle / "RETENTION.json", retained)
        result = {"schema_version": 1, "protocol": PROTOCOL, "status": "not_run",
            "preflight_passed": False, "execution": execution, "retention": retained,
            "blockers": [{"gate": "launch_barrier", "passed": False,
                          "reason": "Sealed inputs or readiness changed before child creation"}],
            "mechanical_comparison_complete": False, "coding_productivity": "unmeasured"}
        private_write(bundle / "RESULT.json", result)
        return result
    return assess(bundle)


def assess(bundle: Path) -> dict:
    bundle = cross.selected_path(bundle)
    execution = read(bundle / "EXECUTION.json")
    retained = retention(bundle, execution)
    private_write(bundle / "RETENTION.json", retained)
    before = preflight(bundle, at=execution["started_at"], fresh=False)
    if (before["contract_sha256"] != execution["contract_sha256"]
            or before["readiness_sha256"] != execution["readiness_sha256"]
            or coding.hash_file(bundle / "SEAL.json") != execution["seal_sha256"]
            or coding.hash_file(bundle / "PREFLIGHT_AT_LAUNCH.json") != execution["preflight_sha256"]):
        raise ValueError("The executed study no longer matches the sealed run contract")
    try:
        comparison = adaptive.assess(bundle / "run")
    except (ValueError, OSError, KeyError, TypeError) as error:
        comparison = {"mechanical_contracts_passed": False, "independent_validation_complete": False,
                      "error_type": type(error).__name__}
    complete = (execution["status"] == "completed" and before["preflight_passed"]
                and comparison["mechanical_contracts_passed"]
                and retained["completed_checked_assignments"] == retained["planned_initial_assignments"]
                and retained["failed_refused_cancelled_or_interrupted_invocations"] == 0)
    triage = {"status": "unavailable_incomplete_comparison", "algorithm_tuning_ready": False}
    if complete:
        import failure_triage
        triage = failure_triage.triage(bundle / "run")
        private_write(bundle / "FAILURE_TRIAGE.json", triage)
    result = {"schema_version": 1, "protocol": PROTOCOL, "execution": execution,
        "mechanical_comparison_complete": complete, "retention": retained,
        "comparison": comparison, "triage": triage,
        "coding_productivity": "unmeasured", "productivity_benefit_established": False,
        "qualification": "The existing independently reviewed paired-task assessment remains authoritative. A readiness pass, completed run or cache hit is not a productivity result. Aborted studies retain coverage and cost uncertainty and cannot become a passing independent comparison."}
    private_write(bundle / "RESULT.json", result)
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("prepare")
    command.add_argument("--cases", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    for name in ("pins", "seal", "preflight", "run", "assess"):
        commands.add_parser(name).add_argument("bundle", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(args.output, args.cases)
        elif args.command == "preflight":
            result = preflight(args.bundle)
            private_write(args.bundle / "PREFLIGHT.json", result)
        else:
            result = globals()[args.command](args.bundle)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if (args.command in ("prepare", "pins", "seal") or result.get("preflight_passed")
                     or result.get("mechanical_comparison_complete")) else 2
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(json.dumps({"status": "invalid", "error_type": type(error).__name__,
                          "coding_productivity": "unmeasured"}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
