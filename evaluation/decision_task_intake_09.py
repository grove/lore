#!/usr/bin/env python3
"""Inspect independent candidates using the existing case and review contracts.

Inspection never executes candidate code. Control execution is an explicit
operator action, requiring the independently enforced study sandbox.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path
import sys
import tempfile
import subprocess

import benchmark as bench

import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import experiment_08 as experiment
import failure_triage as triage
import outcome_protocol as outcome

DECISIONS = {"accepted_vs_proposed", "scoped_supersession", "rare_exception",
             "documentation_vs_implementation", "rejected_design", "source_change_or_narrowed_grant"}
VARIANTS = ("original", "correct", "wrong", "unimplemented")


def published_content() -> tuple[set, set]:
    hashes, tasks = set(), set()
    for path in (coding.ROOT / "corpora").rglob("*"):
        if path.is_file():
            hashes.add(coding.hash_file(path))
    for path in (coding.ROOT / "corpora").rglob("cases.json"):
        data = cross.read_json(path)
        if not isinstance(data, dict):
            continue
        for case in data.get("cases", []):
            if all(key in case for key in ("task", "editable_files", "critical_constraints")):
                tasks.add(triage.task_content_sha256(case))
    # Published restoration inputs include transformed code, not only originals.
    import real_coding_corpus as real
    manifest = real.load_manifest()
    for task in manifest["tasks"]:
        raw = (real.CORPUS / "upstream" / task["project"] / task["file"]).read_bytes().decode("utf-8")
        hashes.add(hashlib.sha256(real.mutate(raw, task["symbol"]).encode("utf-8")).hexdigest())
    return hashes, tasks


def entry_for(case: dict, manifest: Path) -> dict:
    source = manifest.parent / coding.relative_file(case["source_root"])
    source_manifest = cross.fingerprint(source)
    _, checker_files = coding.verification_command(case, source, manifest.parent)
    return {"case": case, "source_root": case["source_root"], "source_manifest": source_manifest,
            "checker_files_sha256": checker_files}


def private_material(manifest: Path, declaration: dict, entry: dict, sources: list[Path]) -> dict:
    original = manifest.parent / coding.relative_file(declaration["original_root"])
    original_manifest = cross.fingerprint(original)
    upstream = entry["case"]["upstream"]["files_sha256"]
    if original_manifest["files_sha256"] != upstream:
        raise ValueError("Original snapshot differs from upstream hashes")
    inputs = entry["source_manifest"]["files_sha256"]
    if (set(inputs) != set(upstream) or any(inputs[name] != digest for name, digest in upstream.items()
            if name not in entry["case"]["editable_files"])):
        raise ValueError("Transformed input changed original documents or protected source bytes")
    if any(inputs[name] != upstream[name] for name in declaration["supporting_sources"]):
        raise ValueError("Supporting decision documents must preserve original upstream bytes")
    materials = {}
    for variant in ("correct", "wrong"):
        proposals = declaration["control_files"][variant]
        if not isinstance(proposals, dict) or not proposals or set(proposals) - set(entry["case"]["editable_files"]):
            raise ValueError("Control proposals must use permitted files")
        for name, relative in proposals.items():
            coding.relative_file(name)
            path = manifest.parent / coding.relative_file(relative)
            materials[relative] = coding.hash_file(path)
    for root in sources:
        if original == root or original.is_relative_to(root) or root.is_relative_to(original):
            raise ValueError("Original solution snapshot is inside agent input")
        visible_hashes = set(cross.fingerprint(root)["files_sha256"].values())
        if {upstream[name] for name in entry["case"]["editable_files"]} & visible_hashes:
            raise ValueError("Original solution bytes leak into agent input")
        for relative, digest in materials.items():
            path = manifest.parent / relative
            if path.is_relative_to(root) or digest in visible_hashes:
                raise ValueError("Control solutions leak into agent source bytes")
    return {"original_sha256": original_manifest["sha256"], "control_files_sha256": materials}


def inspect(manifest: Path, *, bundle: Path | None = None, pilot_prepared: Path | None = None) -> dict:
    manifest = cross.selected_path(manifest)
    data = coding.load_cases(manifest)
    pilot = data.get("pilot") is True
    issues, entries = [], []

    def deficit(code, case_id=None):
        issues.append({"code": code, **({"case_id": case_id} if case_id else {})})

    if (pilot and len(data["cases"]) != 6) or (not pilot and len(data["cases"]) < 30):
        deficit("pilot_requires_6_or_holdout_requires_30_tasks")
    known_hashes, known_tasks = published_content()
    seen_tasks, seen_sources, projects = set(), set(), set()
    for case in data["cases"]:
        identity = case["id"]
        try:
            entry = entry_for(case, manifest)
            entries.append(entry)
            task = triage.task_content_sha256(case)
            source = triage.source_content_sha256(entry["source_manifest"])
            if task in seen_tasks or source in seen_sources:
                deficit("duplicated_task_or_renamed_source", identity)
            seen_tasks.add(task)
            seen_sources.add(source)
            if task in known_tasks or set(entry["source_manifest"]["files_sha256"].values()) & known_hashes:
                deficit("published_task_or_source_overlap", identity)
            provenance = outcome.source_provenance(entry)
            if not provenance["complete"]:
                deficit("exact_upstream_and_input_provenance_required", identity)
            else:
                projects.add(provenance["repository"])
            for name in case["editable_files"]:
                if not (manifest.parent / case["source_root"] / name).is_file():
                    deficit("editable_file_missing", identity)
        except (ValueError, OSError, KeyError, TypeError):
            deficit("invalid_source_or_checker_path", identity)
    if len(projects - {None}) < 3:
        deficit("three_pinned_projects_required")
    if data.get("fixture_only") is not False or data.get("independent_projects") is not True:
        deficit("candidate_independence_declarations_pending")
    if not pilot:
        if data.get("held_out") is not True or pilot_prepared is None:
            deficit("separate_pilot_binding_required_for_holdout")
        else:
            try:
                previous = experiment.read(pilot_prepared)
                previous_hashes = {digest for entry in previous["cases"]
                                   for digest in entry["source_manifest"]["files_sha256"].values()}
                previous_tasks = {triage.task_content_sha256(entry["case"]) for entry in previous["cases"]}
                for entry in entries:
                    if (set(entry["source_manifest"]["files_sha256"].values()) & previous_hashes
                            or triage.task_content_sha256(entry["case"]) in previous_tasks):
                        deficit("pilot_source_or_task_overlap", entry["case"]["id"])
            except (ValueError, OSError, KeyError, TypeError):
                deficit("invalid_pilot_binding")
    evidence_root = bundle or manifest.parent
    try:
        intake = experiment.read(evidence_root / "DECISION_INTAKE.json")
        declarations = {item["case_id"]: item for item in intake["cases"]}
        if (intake.get("schema_version") != 1 or len(declarations) != len(intake["cases"])
                or set(declarations) != {case["id"] for case in data["cases"]}):
            raise ValueError("Intake identities changed")
    except (ValueError, OSError, KeyError, TypeError):
        declarations = {}
        deficit("decision_intake_record_required")
    sources = [manifest.parent / entry["source_root"] for entry in entries]
    source_hashes = {digest for entry in entries for digest in entry["source_manifest"]["files_sha256"].values()}
    for entry in entries:
        identity, case = entry["case"]["id"], entry["case"]
        source = manifest.parent / entry["source_root"]
        declaration = declarations.get(identity, {})
        try:
            if (not isinstance(declaration.get("source_license"), str) or not declaration["source_license"].strip()
                    or declaration.get("decision_kind") not in DECISIONS
                    or not declaration.get("supporting_sources")
                    or any(coding.relative_file(name) not in entry["source_manifest"]["files_sha256"]
                           for name in declaration["supporting_sources"])):
                raise ValueError("Decision/license contract is incomplete")
            material = private_material(manifest, declaration, entry, sources)
            for name, digest in entry["checker_files_sha256"].items():
                if any(Path(name).is_relative_to(root) for root in sources) or digest in source_hashes:
                    raise ValueError("Hidden checker is in source input")
            for authored in declaration["authorship"]:
                outcome.timestamp(authored["authored_at"])
                capture_path = evidence_root / coding.relative_file(authored["capture"]["path"])
                if any(capture_path.is_relative_to(root) for root in sources):
                    raise ValueError("Private authorship capture is in input")
                if outcome.capture(evidence_root, authored["capture"]) in source_hashes:
                    raise ValueError("Private authorship bytes are in input")
            controls = experiment.read(evidence_root / coding.relative_file(declaration["controls_capture"]["path"]))
            controls_path = evidence_root / coding.relative_file(declaration["controls_capture"]["path"])
            if (any(controls_path.is_relative_to(root) for root in sources)
                    or outcome.capture(evidence_root, declaration["controls_capture"]) in source_hashes):
                raise ValueError("Private control receipt is in input")
            row = next(item for item in controls["cases"] if item["case_id"] == identity)
            if (row["task_sha256"] != outcome.task_contract_sha256(entry) or row["material"] != material
                    or row["checker_files_sha256"] != entry["checker_files_sha256"]
                    or [item["variant"] for item in row["variants"]] != list(VARIANTS)
                    or any(item["source_preserved"] is not True or item["passed"] is not (item["variant"] in ("original", "correct"))
                           for item in row["variants"])):
                raise ValueError("Checker controls are missing or differ from current inputs")
        except (ValueError, OSError, KeyError, TypeError, StopIteration):
            deficit("decision_license_private_material_or_controls_pending", identity)
    try:
        registration = experiment.read(evidence_root / "PREREGISTRATION.json")
        for entry in entries:
            identity = entry["case"]["id"]
            registered = next(item for item in registration["cases"] if item["case_id"] == identity)
            authored = declarations[identity]["authorship"]
            if any(outcome.timestamp(item["authored_at"]) > outcome.timestamp(registration["registered_at"])
                   for item in authored):
                raise ValueError("Authorship captures must predate preregistration")
            for role, names in (("task", registered["task_authors"]), ("checker", registered["checker_authors"])):
                if {item["person_id"].strip().casefold() for item in authored if item["role"] == role} != {name.strip().casefold() for name in names}:
                    raise ValueError("Authorship captures differ from preregistration")
            for review in registered["independent_gold_reviews"]:
                path = evidence_root / coding.relative_file(review["capture"]["path"])
                if any(path.is_relative_to(root) for root in sources) or coding.hash_file(path) in source_hashes:
                    raise ValueError("Gold review is inside source input")
        planned = {"cases": entries, "setups": list(adaptive.SETUPS), "created_at": registration["registered_at"],
                   "agent_command_sha256": registration["agent"]["command_sha256"],
                   "repair_policy": {"max_attempts": registration["agent"]["max_attempts"],
                                     "attempt_budget_seconds": registration["agent"]["wall_time_seconds"]}}
        outcome.validate_preregistration_inputs(evidence_root, registration, planned)
        if registration["pilot"] != pilot:
            raise ValueError("Pilot/holdout declaration changed")
    except (ValueError, OSError, KeyError, TypeError, StopIteration):
        deficit("independent_authorship_gold_reviews_and_preregistration_pending")
    return {"schema_version": 1, "status": ("eligible_independent_pilot" if pilot else "eligible_independent_holdout")
            if not issues else "candidate_unreviewed", "eligible": not issues,
            "tasks": len(data["cases"]), "projects": len(projects - {None}), "issues": issues,
            "manifest_sha256": coding.hash_file(manifest), "provider_calls": 0, "subprocess_calls": 0,
            "qualification": "Hash-bound declarations and captures do not authenticate distinct humans or historical truth. External review and execution gates remain mandatory."}


def prepare(manifest: Path, output: Path) -> dict:
    result = experiment.prepare(output, manifest)
    data = coding.load_cases(manifest)
    experiment.private_write(output / "DECISION_INTAKE.json", {"schema_version": 1, "cases": [
        {"case_id": case["id"], "source_license": "", "original_root": "", "decision_kind": "",
         "supporting_sources": [], "authorship": [], "control_files": {"correct": {}, "wrong": {}},
         "controls_capture": None} for case in data["cases"]]}, exclusive=True)
    return {**result, "intake": "candidate_unreviewed"}


def generate_manifest(draft: Path, intake_path: Path, output: Path) -> dict:
    """Fill missing byte pins, without correcting changed checksums or authoring evidence."""
    draft, output = cross.selected_path(draft), cross.selected_path(output)
    if output.parent != draft.parent or output.exists():
        raise ValueError("Use a fresh sibling manifest so existing source/checker paths retain meaning")
    data = coding.load_cases(draft)
    declarations = {item["case_id"]: item for item in experiment.read(intake_path)["cases"]}
    for case in data["cases"]:
        entry = entry_for(case, draft)
        upstream = case["upstream"]
        if (not isinstance(upstream.get("repository"), str)
                or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", upstream["repository"])
                or not outcome.COMMIT.fullmatch(upstream.get("commit", ""))):
            raise ValueError("Supply the actual upstream repository and full commit first")
        original = draft.parent / coding.relative_file(declarations[case["id"]]["original_root"])
        files = cross.fingerprint(original)["files_sha256"]
        if ("files_sha256" in upstream and upstream["files_sha256"] != files
                or "input_sha256" in case and case["input_sha256"] != entry["source_manifest"]["sha256"]):
            raise ValueError("Existing input or upstream checksum changed; never re-pin it silently")
        upstream["files_sha256"] = files
        case["input_sha256"] = entry["source_manifest"]["sha256"]
        case.setdefault("expected_sources", [])
        case.setdefault("verification_files", [])
    experiment.private_write(output, data, exclusive=True)
    return {"status": "candidate_unreviewed", "tasks": len(data["cases"]),
            "manifest_sha256": coding.hash_file(output), "provider_calls": 0, "subprocess_calls": 0}


def validate_controls(manifest: Path, intake_path: Path, output: Path, timeout=30) -> dict:
    cross.reject_symlink_path(output)
    if output.exists() or type(timeout) is not int or not 1 <= timeout <= 3600:
        raise ValueError("Fresh output and bounded checker timeout are required")
    manifest = cross.selected_path(manifest)
    data = coding.load_cases(manifest)
    declarations = {item["case_id"]: item for item in experiment.read(intake_path)["cases"]}
    entries = [entry_for(case, manifest) for case in data["cases"]]
    experiment.checker_separation(manifest.parent, {"cases": entries, "cases_manifest": str(manifest)})
    sources = [manifest.parent / entry["source_root"] for entry in entries]
    rows = []
    for entry in entries:
        case, variants = entry["case"], []
        declaration = declarations[case["id"]]
        material = private_material(manifest, declaration, entry, sources)
        contract = None
        for variant in VARIANTS:
            with tempfile.TemporaryDirectory(prefix="lore-independent-control-") as temporary:
                workspace = Path(temporary).resolve() / "source"
                cross.copy_snapshot(manifest.parent / case["source_root"], workspace)
                if variant == "original":
                    for name in case["editable_files"]:
                        (workspace / name).write_bytes((manifest.parent / declaration["original_root"] / name).read_bytes())
                elif variant in ("correct", "wrong"):
                    proposed = {name: (manifest.parent / relative).read_bytes().decode("utf-8")
                                for name, relative in declaration["control_files"][variant].items()}
                    coding.validate_agent_response({"schema_version": 1, "files": proposed, "summary": "Independent control"}, case["editable_files"])
                    for name, content in proposed.items():
                        (workspace / name).write_bytes(content.encode("utf-8"))
                before = cross.fingerprint(workspace)
                result, elapsed = coding.execute_checks(case, workspace, manifest.parent, timeout)
                contract = coding.validate_checker_result(result, contract)
                preserved = cross.fingerprint(workspace) == before
                passed = all(item["passed"] for item in result["checks"])
                if not preserved or passed != (variant in ("original", "correct")):
                    raise ValueError("Independent checker controls failed or changed source bytes")
                variants.append({"variant": variant, "passed": passed, "source_preserved": preserved,
                                 "source_sha256": before["sha256"], "elapsed_seconds": elapsed})
        rows.append({"case_id": case["id"], "task_sha256": outcome.task_contract_sha256(entry),
                     "checker_files_sha256": entry["checker_files_sha256"], "material": material, "variants": variants})
    receipt = {"schema_version": 1, "classification": "operator_selected_checker_controls",
               "checked_at": bench.now_utc(), "cases": rows, "model_calls": 0}
    experiment.private_write(output, receipt, exclusive=True)
    return {"status": "controls_passed", "tasks": len(rows), "model_calls": 0}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("inspect", "manifest", "prepare", "validate-controls"))
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--pilot-prepared", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--intake", type=Path)
    parser.add_argument("--execute-checkers", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.command == "inspect":
            value = inspect(args.cases, bundle=args.bundle, pilot_prepared=args.pilot_prepared)
        elif args.command == "prepare" and args.output:
            value = prepare(args.cases, args.output)
        elif args.command == "manifest" and args.intake and args.output:
            value = generate_manifest(args.cases, args.intake, args.output)
        elif args.command == "validate-controls" and args.execute_checkers and args.intake and args.output:
            value = validate_controls(args.cases, args.intake, args.output)
        else:
            raise ValueError("Explicit output/intake and checker execution authorization are required")
        print(json.dumps(value, indent=2, sort_keys=True))
        return 2 if value.get("eligible") is False else 0
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError):
        print(json.dumps({"status": "invalid_candidate", "provider_calls": 0}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
