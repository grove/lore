#!/usr/bin/env python3
"""Replay named-baseline guardian transitions and assess captured source integrity.

Preparation and assessment never run a model. Replay executes the actual Lore
CLI. Compiled debug snapshots are explicitly synthetic; a disabled provider is
an availability control, never a successful alert-quality experiment.
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import closing
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import time

import coding_tasks as coding
import cross_source as cross
import shared_intelligence as shared

ROOT = Path(__file__).resolve().parent
DEFAULT_EVENTS = ROOT / "corpora" / "guardian" / "events.json"
PROTOCOL = "guardian-longitudinal-v1"
MAX_BYTES = 16_000_000
SIGNIFICANCE = ("consequential", "benign", "ambiguous")
ASSESSMENTS = ("no_documented_change", "change_budget_exhausted", "partial_static_guidance", "source_reviewed_advisory")
IMMUTABLE_TABLES = ("source_revisions", "source_revision_provenance", "assertion_revisions",
                    "assertion_details", "assertion_evidence", "evidence_snapshots", "knowledge_revisions",
                    "knowledge_support", "cross_source_evaluations", "cross_source_relations", "native_snapshots")


def read_json(path: Path):
    cross.reject_symlink_path(path)
    if path.stat().st_size > MAX_BYTES:
        raise ValueError("Guardian artifact exceeds the bounded input size")
    return cross.read_json(path)


def identity(value: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,80}", value):
        raise ValueError("Guardian identities must be simple bounded strings")
    return value


def source_file(value: str) -> str:
    value = coding.relative_file(value)
    if value.split("/")[0] in (".lore", ".git", "wiki") or value == "lore.yml":
        raise ValueError("Source events cannot write configuration, registry, or generated output")
    return value


def validate_files(files: dict) -> None:
    if not isinstance(files, dict) or len(files) > 500:
        raise ValueError("An event needs a bounded source-file map")
    for filename, spec in files.items():
        source_file(filename)
        if spec is None:
            continue
        if not isinstance(spec, dict) or set(spec) != {"text", "records"}:
            raise ValueError("Source files need exact text and explicit debug extraction records")
        if not isinstance(spec["text"], str) or len(spec["text"].encode()) > 2_000_000:
            raise ValueError("Source text is invalid or exceeds its byte limit")
        if not isinstance(spec["records"], list) or len(spec["records"]) > 100:
            raise ValueError("Source extraction records are not bounded")
        for record in spec["records"]:
            required = {"key", "topic", "topic_title", "subject", "statement", "quote", "kind", "lifecycle", "scope", "effective_at"}
            if not isinstance(record, dict) or set(record) != required:
                raise ValueError("A debug extraction record has an invalid shape")
            identity(record["key"])
            if any(not isinstance(record[key], str) or (key != "effective_at" and not record[key].strip()) for key in required):
                raise ValueError("Extraction fields must retain complete nonempty source qualifications")
            if record["quote"] not in spec["text"]:
                raise ValueError("Debug extraction quote is absent from the original source text")


def load_events(path: Path) -> dict:
    manifest = read_json(path)
    if manifest.get("schema_version") != 1 or manifest.get("protocol") != PROTOCOL:
        raise ValueError("Unsupported guardian event protocol")
    if not isinstance(manifest.get("projects"), list) or not manifest["projects"]:
        raise ValueError("A guardian corpus needs projects")
    provenance = manifest.get("label_provenance", {})
    if provenance.get("status") not in ("synthetic_debug", "candidate_external"):
        raise ValueError("Labels must be declared synthetic_debug or candidate_external; review is assessed separately")
    project_ids, event_ids = set(), set()
    for project in manifest["projects"]:
        project_id = identity(project.get("id"))
        if project_id in project_ids:
            raise ValueError("Duplicate guardian project")
        project_ids.add(project_id)
        previous = identity(project.get("initial_revision"))
        validate_files(project.get("initial_files"))
        if any(spec is None for spec in project["initial_files"].values()):
            raise ValueError("An initial source snapshot cannot contain deletions")
        source = project.get("source_identity", {})
        if provenance["status"] == "candidate_external" and (
                not isinstance(source.get("repository"), str)
                or not re.fullmatch(r"[a-f0-9]{40}", source.get("commit", ""))):
            raise ValueError("External project candidates need a pinned repository and full commit identity")
        if not isinstance(project.get("events"), list) or not project["events"]:
            raise ValueError("Each project needs successive events")
        for sequence, event in enumerate(project["events"], 1):
            event_id, revision = identity(event.get("id")), identity(event.get("revision"))
            if event_id in event_ids or event.get("sequence") != sequence or event.get("parent") != previous:
                raise ValueError("Events must be unique, ordered, and connected to the immediately previous revision")
            event_ids.add(event_id)
            previous = revision
            validate_files(event.get("files"))
            expected = event.get("expected", {})
            if (expected.get("significance") not in SIGNIFICANCE
                    or expected.get("alert_necessity") not in ("required", "forbidden", "optional")
                    or not isinstance(expected.get("source_scope"), str) or not expected["source_scope"].strip()
                    or not isinstance(expected.get("safe_next_step"), str) or not expected["safe_next_step"].strip()
                    or type(expected.get("runtime_verified")) is not bool):
                raise ValueError("Event labels need significance, scope, alert necessity, safe next step, and explicit verification status")
            relation = event.get("interpretation")
            if relation is not None:
                if not isinstance(relation, dict) or relation.get("action") not in ("set", "withdraw"):
                    raise ValueError("Interpretation changes must explicitly set or withdraw a named comparison")
                identity(relation.get("pair"))
                if relation["action"] == "set":
                    identity(relation.get("from"))
                    identity(relation.get("to"))
                    if (relation.get("kind") not in ("uncertain", "verification_question", "potential_discrepancy", "consistent_with", "related_to")
                            or not isinstance(relation.get("reason"), str) or not relation["reason"].strip()
                            or not isinstance(relation.get("qualifications"), list) or not relation["qualifications"]
                            or any(not isinstance(item, str) or not item.strip() for item in relation["qualifications"])):
                        raise ValueError("Interpretation changes need original reasons and complete qualifications")
    return manifest


def prepare(output: Path, events_path: Path = DEFAULT_EVENTS) -> dict:
    output, events_path = cross.selected_path(output), cross.selected_path(events_path)
    manifest = load_events(events_path)
    if output.exists():
        raise ValueError("Use a fresh guardian preparation directory")
    output.mkdir(parents=True)
    projects, counts = [], Counter()
    for project in manifest["projects"]:
        current = copy.deepcopy(project["initial_files"])
        states = []
        revisions = [(project["initial_revision"], None), *((event["revision"], event) for event in project["events"])]
        for revision, event in revisions:
            if event:
                counts[event["expected"]["significance"]] += 1
                for filename, spec in event["files"].items():
                    if spec is None:
                        if filename not in current:
                            raise ValueError("An event deletes an absent source")
                        del current[filename]
                    else:
                        current[filename] = copy.deepcopy(spec)
            path = Path("snapshots") / project["id"] / revision
            records = []
            for filename, spec in sorted(current.items()):
                target = output / path / filename
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(spec["text"], encoding="utf-8")
                records.extend(dict(record, file=filename) for record in spec["records"])
            if len({record["key"] for record in records}) != len(records):
                raise ValueError("A current source snapshot has duplicated record keys")
            states.append({"revision": revision, "source_root": path.as_posix(),
                           "source_fingerprint": cross.fingerprint(output / path),
                           "capture_records": records, "event": event})
        projects.append({"id": project["id"], "source_identity": project["source_identity"], "states": states})
    debug = manifest.get("fixture_only") is not False or manifest["label_provenance"]["status"] == "synthetic_debug"
    result = {"schema_version": 1, "protocol": PROTOCOL, "phase": "preparation_only", "inference_calls": 0,
              "fixture_only": debug, "held_out": manifest.get("held_out") is True and not debug,
              "label_provenance": manifest["label_provenance"], "events_manifest_sha256": coding.hash_file(events_path),
              "counts": dict(counts), "projects": projects,
              "full_size": len(projects) >= 3 and all(counts[group] >= 20 for group in SIGNIFICANCE)}
    cross.write_json(output / "prepared.json", result)
    return result


def validate_prepared(directory: Path) -> dict:
    prepared = read_json(directory / "prepared.json")
    if prepared.get("schema_version") != 1 or prepared.get("protocol") != PROTOCOL or not prepared.get("projects"):
        raise ValueError("Invalid prepared guardian source manifest")
    project_ids, event_ids = set(), set()
    for project in prepared["projects"]:
        project_id = identity(project.get("id"))
        if project_id in project_ids or not isinstance(project.get("states"), list) or len(project["states"]) < 2:
            raise ValueError("Prepared projects need distinct identities and a successive source history")
        project_ids.add(project_id)
        revisions = set()
        previous = None
        for index, state in enumerate(project["states"]):
            revision = identity(state.get("revision"))
            if revision in revisions or state.get("source_root") != f"snapshots/{project_id}/{revision}":
                raise ValueError("Prepared source identity differs from its bounded project and revision path")
            revisions.add(revision)
            if index:
                event = state.get("event") or {}
                event_id = identity(event.get("id"))
                if (event_id in event_ids or event.get("sequence") != index
                        or event.get("parent") != previous or event.get("revision") != revision):
                    raise ValueError("Prepared events must retain their exact successive endpoints")
                event_ids.add(event_id)
            source = directory / coding.relative_file(state["source_root"])
            if cross.fingerprint(source) != state["source_fingerprint"]:
                raise ValueError("Prepared source snapshot no longer matches its pinned hash")
            previous = revision
    return prepared


def source_fingerprint(project: Path, filenames: set[str]) -> dict:
    files = {}
    # Include unexpected new source paths as well as originals; checking only
    # old filenames would miss a read command that created another source.
    candidates = set(filenames)
    for path in project.rglob("*"):
        relative = path.relative_to(project).as_posix()
        if relative.split("/")[0] in (".lore", "wiki") or relative == "lore.yml":
            continue
        cross.reject_symlink_path(path)
        if path.is_file():
            candidates.add(relative)
    for filename in sorted(candidates):
        target = project / source_file(filename)
        cross.reject_symlink_path(target)
        if target.exists():
            if not target.is_file():
                raise ValueError("A source path became a nonregular file")
            files[filename] = coding.hash_file(target)
    return {"sha256": cross.digest(files), "files_sha256": files, "file_count": len(files)}


def apply_snapshot(source: Path, project: Path, previous: set[str]) -> set[str]:
    manifest = cross.fingerprint(source)
    next_files = set(manifest["files_sha256"])
    for relative in sorted(previous - next_files):
        target = project / source_file(relative)
        cross.reject_symlink_path(target)
        target.unlink()
    cross.copy_snapshot(source, project)
    return next_files


def registry_capture(project: Path) -> dict:
    path = project / ".lore" / "state.db"
    cross.reject_symlink_path(path)
    with closing(sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)) as conn:
        tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        immutable = {}
        for table in IMMUTABLE_TABLES:
            if table in tables:
                immutable[table] = sorted(cross.digest(list(row)) for row in conn.execute(f'SELECT * FROM "{table}"'))
        history = {}
        query = """SELECT r.knowledge_id,r.id,r.statement,r.kind,r.lifecycle,d.base_lifecycle,
            r.support_state,t.slug,d.subject,d.scope,d.effective_at
            FROM knowledge_revisions r JOIN knowledge_details d ON d.knowledge_id=r.knowledge_id
            JOIN topics t ON t.id=d.topic_id JOIN sealed_knowledge sealed ON sealed.id=r.id"""
        for row in conn.execute(query):
            state = dict(zip(("id", "revision_id", "statement", "kind", "lifecycle", "original_lifecycle",
                              "support_state", "topic", "subject", "scope", "effective_at"), row))
            state["evidence_ids"] = [support[0] for support in conn.execute("""SELECT DISTINCT e.evidence_id
                FROM knowledge_support s JOIN assertion_evidence e ON e.assertion_revision_id=s.assertion_revision_id
                WHERE s.knowledge_revision_id=? ORDER BY e.evidence_id""", (row[1],))]
            history[row[1]] = state
        current = {}
        for knowledge_id, revision in conn.execute("SELECT knowledge_id,revision_id FROM knowledge_current"):
            state = copy.deepcopy(history[revision])
            state["current_evidence_ids"] = [row[0] for row in conn.execute("""SELECT DISTINCT e.evidence_id
                FROM assertion_assignments a JOIN assertion_evidence e ON e.assertion_revision_id=a.assertion_revision_id
                JOIN active_assertions active ON active.assertion_revision_id=a.assertion_revision_id
                WHERE a.knowledge_id=? ORDER BY e.evidence_id""", (knowledge_id,))]
            current[knowledge_id] = state
        integrity = conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    return {"database_sha256": coding.hash_file(path), "sqlite_integrity_ok": integrity,
            "immutable_rows": immutable, "knowledge_current": current, "knowledge_history": history,
            "configuration_sha256": coding.hash_file(project / "lore.yml"),
            "wiki_sha256": cross.fingerprint(project / "wiki")["sha256"] if (project / "wiki").exists() else None}


def retained_history(before: dict, after: dict) -> bool:
    return before.get("sqlite_integrity_ok") is True and after.get("sqlite_integrity_ok") is True and all(
        Counter(rows) <= Counter(after.get("immutable_rows", {}).get(table, []))
        for table, rows in before.get("immutable_rows", {}).items())


def audit_trace(text: str, binary: str, *, network_granted: bool) -> dict:
    """Interpret narrow strace syscall capture, never self-reported capability flags.

    Capture must include execve, execveat, connect, sendto and sendmsg. Trace
    files may contain source data when an operator chooses broader tracing;
    only hashes and bounded syscall counts are copied into the result.
    """
    if not text.strip() or any(value in text for value in ("Operation not permitted", "ptrace:", "detached", "+++ killed")):
        return {"status": "unavailable_or_incomplete", "execution_verified": False, "network_verified": False}
    executions = re.findall(r'\bexecve(?:at)?\([^\n]+', text)
    complete = "+++ exited with 0 +++" in text and len(executions) == 1 and json.dumps(binary) in executions[0]
    network = len(re.findall(r"\b(?:connect|sendto|sendmsg)\(", text))
    return {"status": "captured" if complete else "incomplete", "exec_calls": len(executions),
            "network_calls": network, "execution_verified": complete,
            "network_verified": complete and network == 0,
            "network_granted": network_granted, "trace_sha256": hashlib.sha256(text.encode()).hexdigest()}


def invoke(binary: str, project: Path, argv: list[str], environment: dict, timeout: int,
           *, trace: Path | None = None, network_granted: bool = False) -> dict:
    command = [binary, "--config", str(project / "lore.yml"), "--json", *argv]
    if trace is not None:
        command = ["strace", "-f", "-q", "-e", "trace=execve,execveat,connect,sendto,sendmsg", "-o", str(trace), *command]
    started = time.monotonic()
    try:
        result = subprocess.run(command, cwd=project, env=environment, capture_output=True,
                                text=True, encoding="utf-8", timeout=timeout)
    except subprocess.TimeoutExpired:
        return {"arguments": argv, "elapsed_seconds": round(time.monotonic() - started, 6), "response": None,
                "response_sha256": None, "error": "timeout", "audit": {"status": "incomplete"}}
    elapsed = round(time.monotonic() - started, 6)
    # stderr and failed stdout can include source/provider secrets; keep a code.
    value = None
    error = None
    if result.returncode != 0:
        error = f"exit_{result.returncode}"
    else:
        try:
            if len(result.stdout.encode()) > MAX_BYTES:
                raise ValueError("oversize response")
            value = json.loads(result.stdout)
            if not isinstance(value, dict):
                raise ValueError("expected JSON object")
        except (ValueError, json.JSONDecodeError):
            error = "invalid_json_response"
    audit = audit_trace(trace.read_text(encoding="utf-8"), binary, network_granted=network_granted) if trace is not None and trace.exists() else {
        "status": "not_captured", "execution_verified": False, "network_verified": False}
    return {"arguments": argv, "elapsed_seconds": elapsed, "response": value,
            "response_sha256": cross.digest(value) if value is not None else None,
            "stdout_bytes": len(result.stdout.encode()), "error": error, "audit": audit}


def require_response(record: dict) -> dict:
    if record.get("error") is not None or not isinstance(record.get("response"), dict):
        raise ValueError(f"Guardian source transition command failed: {record.get('error', 'missing_response')}")
    return record["response"]


def configuration(template: Path | None, project: str) -> dict:
    value = read_json(template) if template else {"models": {"generative": {"enabled": False}}}
    if not isinstance(value, dict):
        raise ValueError("Configuration template must be a JSON object (JSON is accepted by Lore's YAML reader)")
    value = copy.deepcopy(value)
    value["project"] = {"name": f"guardian-{project}"}
    value["output"] = {"state_dir": ".lore", "wiki_dir": "wiki"}
    value["sources"] = {"roots": [{"id": "docs", "path": "docs"}], "exclude": ["**/target/**", "**/node_modules/**"]}
    # Imports require a separately prepared protocol with explicit source paths;
    # never let an unrelated operator config read outside an isolated snapshot.
    if value.get("imports"):
        raise ValueError("This replay adapter accepts documentary snapshots; native imports need a source-bound adapter")
    value["imports"] = []
    if template is None:
        value["privacy"] = {"local_only": True, "allow_checkout_egress": False}
    return value


def guard_citations(guard: dict, changes: dict) -> set[str]:
    # Native original payloads are data. Their arbitrary upstream keys do not
    # mint evidence references in Lore's output contract.
    def documentary_ids(report):
        stripped = {key: value for key, value in report.items() if key != "imported_evidence"}
        ids = cross.cited_ids(stripped)
        ids.update(item["evidence_id"] for item in report.get("imported_evidence", []))
        return ids
    ids = documentary_ids(changes)
    ids.update(cross.cited_ids(guard.get("advisories", [])))
    if guard.get("intelligence") is not None:
        ids.update(shared.response_citations(guard["intelligence"]))
    return ids


def run(args: argparse.Namespace) -> dict:
    prepared_dir, output = cross.selected_path(args.prepared), cross.selected_path(args.output)
    prepared = validate_prepared(prepared_dir)
    binary = str(Path(args.binary).resolve(strict=True))
    if output.exists():
        raise ValueError("Use a fresh guardian run directory")
    if not 1024 <= args.max_tokens <= 100000 or not 1 <= args.timeout <= 86400:
        raise ValueError("Invalid guardian output or timeout bound")
    if args.allow_checkout_egress and not args.allow_inspection:
        raise ValueError("Checkout egress needs an explicit inspection grant")
    compiled = read_json(args.compiled / "compiled.json") if args.compiled else None
    if compiled is not None and (compiled.get("prepared_sha256") != coding.hash_file(prepared_dir / "prepared.json")
                                 or compiled.get("fixture_only") is not True or not prepared["fixture_only"]):
        raise ValueError("Compiled debug snapshots must match the exact prepared synthetic corpus")
    if compiled is None and args.config_template is None:
        raise ValueError("Live compilation requires an explicit model configuration; use --compiled for disabled-provider debug replay")
    project_filter = getattr(args, "project", None)
    first_event, event_count = getattr(args, "first_event", 1), getattr(args, "event_count", None)
    if first_event != 1 or event_count is not None:
        if compiled is None or project_filter is None:
            raise ValueError("Event segments require one explicit project and immutable compiled debug history")
    if project_filter is not None:
        prepared = copy.deepcopy(prepared)
        prepared["projects"] = [project for project in prepared["projects"] if project["id"] == project_filter]
        if len(prepared["projects"]) != 1:
            raise ValueError("Unknown guardian project selection")
        states = prepared["projects"][0]["states"]
        count = len(states) - first_event if event_count is None else event_count
        if first_event < 1 or count < 1 or first_event + count > len(states):
            raise ValueError("Guardian event segment is outside its successive project history")
        prepared["projects"][0]["states"] = states[first_event - 1:first_event + count]
        prepared["counts"] = dict(Counter(state["event"]["expected"]["significance"] for state in prepared["projects"][0]["states"][1:]))
        prepared["full_size"] = False
    output.mkdir(parents=True)
    records, initialization = [], []
    all_started = time.monotonic()
    for project in prepared["projects"]:
        workspace = output / "workspaces" / project["id"]
        workspace.mkdir(parents=True)
        config = configuration(args.config_template, project["id"])
        # A provider template is configuration, not an additional permission
        # grant. These explicit run flags bound every subprocess invocation.
        privacy = config.setdefault("privacy", {})
        if not args.allow_hosted:
            privacy["local_only"] = True
        if not args.allow_checkout_egress:
            privacy["allow_checkout_egress"] = False
        # Compiled registry project identities are fixed by the fixture compiler.
        if compiled is not None:
            config["project"]["name"] = f"guardian-debug-{project['id']}"
        cross.write_json(workspace / "lore.yml", config)
        environment = shared.invocation_environment(workspace, allow_inspection=args.allow_inspection,
                        allow_hosted=args.allow_hosted, allow_checkout_egress=args.allow_checkout_egress)
        filenames: set[str] = set()
        all_source_names = {name for state in project["states"] for name in state["source_fingerprint"]["files_sha256"]}

        def materialize(state, first=False):
            nonlocal filenames
            filenames = apply_snapshot(prepared_dir / state["source_root"], workspace, filenames)
            if compiled is not None:
                entry = next((item for item in compiled["states"] if item["project"] == project["id"] and item["revision"] == state["revision"]), None)
                if entry is None:
                    raise ValueError("A compiled retained snapshot is missing")
                source = args.compiled / coding.relative_file(entry["database"])
                if coding.hash_file(source) != entry["database_sha256"]:
                    raise ValueError("Compiled registry source hash mismatch")
                target = workspace / ".lore" / "state.db"
                cross.reject_symlink_path(target)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)
                return {"mode": "explicit_synthetic_registry_capture", "inference_calls": 0,
                        "database_sha256": entry["database_sha256"], "elapsed_seconds": 0.0}
            return invoke(binary, workspace, ["init" if first else "update"], environment, args.timeout)

        initial = project["states"][0]
        build = materialize(initial, first=True)
        if compiled is None:
            require_response(build)
        initialization.append({"project": project["id"], "compilation": build})
        for before, after in zip(project["states"], project["states"][1:]):
            event = after["event"]
            directory = output / "events" / event["id"]
            directory.mkdir(parents=True)
            started = time.monotonic()
            before_sources = source_fingerprint(workspace, all_source_names)
            before_registry = registry_capture(workspace)
            saved = invoke(binary, workspace, ["baseline", "save", "transition", "--replace"], environment, args.timeout)
            require_response(saved)
            build_started = time.monotonic()
            build = materialize(after)
            build["elapsed_seconds"] = round(time.monotonic() - build_started, 6)
            if compiled is None:
                require_response(build)
            query_sources = source_fingerprint(workspace, all_source_names)
            query_registry = registry_capture(workspace)
            changes = invoke(binary, workspace, ["changes", "--since", "transition", "--max-tokens", str(args.max_tokens)], environment, args.timeout,
                             trace=directory / "changes.trace" if args.audit_strace else None)
            guard_args = ["guard", "--since", "transition", "--max-tokens", str(args.max_tokens), "--no-cache"]
            if not args.allow_inspection:
                guard_args.append("--no-inspect")
            guarded = invoke(binary, workspace, guard_args, environment, args.timeout,
                             trace=directory / "guard.trace" if args.audit_strace else None,
                             network_granted=args.allow_hosted or args.config_template is not None)
            changes_value = changes.get("response") or {}
            guard_value = guarded.get("response") or {}
            ids = guard_citations(guard_value, changes_value) if not changes.get("error") and not guarded.get("error") else set()
            resolution = []
            for evidence_id in sorted(ids):
                resolved = invoke(binary, workspace, ["evidence", evidence_id], environment, args.timeout)
                resolution.append({"evidence_id": evidence_id, **resolved})
            final_registry = registry_capture(workspace)
            record = {"event_id": event["id"], "project": project["id"], "event": event,
                      "before_revision": before["revision"], "after_revision": after["revision"],
                      "expected_before_sources": before["source_fingerprint"], "expected_after_sources": after["source_fingerprint"],
                      "sources_before": before_sources, "sources_query": query_sources,
                      "sources_after_queries": source_fingerprint(workspace, all_source_names),
                      "registry_before": before_registry, "registry_query": query_registry,
                      "registry_after_queries": final_registry, "baseline_save": saved,
                      "materialization": build, "changes": changes, "guard": guarded,
                      "resolution": resolution, "elapsed_seconds": round(time.monotonic() - started, 6)}
            cross.write_json(directory / "capture.json", record)
            records.append({"event_id": event["id"], "path": f"events/{event['id']}/capture.json",
                            "sha256": coding.hash_file(directory / "capture.json")})
    report = {"schema_version": 1, "protocol": PROTOCOL, "phase": "actual_cli_capture",
              "fixture_only": prepared["fixture_only"] or compiled is not None,
              "held_out": prepared["held_out"] and compiled is None,
              "label_provenance": prepared["label_provenance"], "prepared_sha256": coding.hash_file(prepared_dir / "prepared.json"),
              "events_manifest_sha256": prepared["events_manifest_sha256"], "prepared": prepared,
              "binary_sha256": coding.hash_file(Path(binary)), "binary": binary,
              "compiler_mode": "synthetic_explicit_records" if compiled else "configured_live_provider",
              "max_tokens": args.max_tokens, "counts": prepared["counts"], "initialization": initialization,
              "permissions": {"inspection": args.allow_inspection, "hosted_egress": args.allow_hosted,
                              "checkout_egress": args.allow_checkout_egress, "repository_execution": False},
              "audit_requested": args.audit_strace, "elapsed_seconds": round(time.monotonic() - all_started, 6),
              "records": records}
    cross.write_json(output / "run.json", report)
    return report


def merge_runs(prepared_directory: Path, directories: list[Path], output: Path) -> dict:
    """Join exact immutable-history segments without dropping failed captures.

    This is useful when an execution host cannot retain a long-lived process.
    Repeated projects must contribute disjoint successive events from the same
    full preparation; assessment checks the entire original cohort afterwards.
    """
    prepared_directory, output = cross.selected_path(prepared_directory), cross.selected_path(output)
    prepared = validate_prepared(prepared_directory)
    if not directories or output.exists():
        raise ValueError("Merge needs input runs and a fresh output directory")
    expected = {state["event"]["id"] for project in prepared["projects"] for state in project["states"][1:]}
    bound_fields = ("schema_version", "protocol", "fixture_only", "held_out", "label_provenance",
                    "prepared_sha256", "events_manifest_sha256", "binary_sha256", "compiler_mode",
                    "max_tokens", "permissions", "audit_requested")
    merged, captures, components, initializations = None, {}, [], []
    elapsed = 0.0
    for directory in directories:
        directory = cross.selected_path(directory)
        run_path = directory / "run.json"
        component = read_json(run_path)
        if (component.get("prepared_sha256") != coding.hash_file(prepared_directory / "prepared.json")
                or component.get("protocol") != PROTOCOL):
            raise ValueError("Segment does not belong to the exact prepared cohort")
        if merged is None:
            merged = copy.deepcopy(component)
        elif any(component.get(key) != merged.get(key) for key in bound_fields):
            raise ValueError("Segments use different binaries, budgets, permissions, or source identities")
        for entry in component["records"]:
            event_id = entry["event_id"]
            capture = directory / coding.relative_file(entry["path"])
            if event_id not in expected or event_id in captures or coding.hash_file(capture) != entry["sha256"]:
                raise ValueError("Segment has an unknown, duplicated, or modified capture")
            captures[event_id] = (capture, entry["sha256"])
        components.append({"run_sha256": coding.hash_file(run_path), "events": len(component["records"])})
        initializations.extend(component["initialization"])
        elapsed += component["elapsed_seconds"]
    if captures.keys() != expected:
        raise ValueError("Segment merge must retain every event in the original cohort")
    output.mkdir(parents=True)
    records = []
    for project in prepared["projects"]:
        for state in project["states"][1:]:
            event_id = state["event"]["id"]
            source, digest = captures[event_id]
            relative = f"events/{event_id}/capture.json"
            destination = output / relative
            destination.parent.mkdir(parents=True)
            shutil.copyfile(source, destination)
            for name in ("changes.trace", "guard.trace"):
                trace = source.parent / name
                if trace.exists():
                    cross.reject_symlink_path(trace)
                    shutil.copyfile(trace, destination.parent / name)
            records.append({"event_id": event_id, "path": relative, "sha256": digest})
    merged.update(phase="merged_actual_cli_segments", prepared=prepared, records=records,
                  counts=prepared["counts"], initialization=initializations,
                  component_runs=components, elapsed_seconds=round(elapsed, 6))
    cross.write_json(output / "run.json", merged)
    return merged


def response_hash_valid(record: dict) -> bool:
    return record.get("error") is None and isinstance(record.get("response"), dict) and record.get("response_sha256") == cross.digest(record["response"])


def original_evidence(changes: dict, resolution: list[dict]) -> bool:
    sources = {entry["evidence_id"]: entry["response"] for entry in resolution if response_hash_valid(entry)}
    for evidence in changes.get("evidence", []):
        original = sources.get(evidence.get("id"), {})
        if {key: value for key, value in original.items() if key != "qualification"} != evidence:
            return False
    for evidence in changes.get("imported_evidence", []):
        original = sources.get(evidence.get("evidence_id"), {})
        if {key: value for key, value in original.items() if key not in ("qualification", "evidence_type")} != evidence:
            return False
    documentary = {item["id"]: item for item in changes.get("evidence", [])}
    revisions = {item["revision_id"]: item for item in changes.get("related_knowledge", []) + changes.get("cross_source_knowledge", [])}
    for change in changes.get("changes", []):
        for state in (change.get("before"), change.get("after")):
            if state is not None:
                revisions[state["revision_id"]] = state
    for state in revisions.values():
        if not set(state.get("evidence_ids", [])) <= documentary.keys():
            return False
        current = state.get("current_evidence_ids")
        if current is not None and (len(current) != len(set(current)) or not set(current) <= set(state["evidence_ids"])):
            return False
    knowledge_ids = {state["id"] for state in revisions.values()}
    for change in changes.get("relationship_changes", []):
        for relation in (change.get("before"), change.get("after")):
            if relation is not None and (relation.get("from") not in knowledge_ids or relation.get("to") not in knowledge_ids
                                        or relation.get("evidence_id") not in documentary
                                        or documentary[relation["evidence_id"]].get("excerpt") != relation.get("exact_excerpt")):
                return False
    for change in changes.get("cross_source_changes", []):
        for state in (change.get("before"), change.get("after")):
            if state is None:
                continue
            relation = state.get("relation", {})
            support = set(relation.get("evidence_ids", []))
            if not support or not support <= sources.keys():
                return False
            for endpoint in [relation.get("from"), *([relation["to"]] if relation.get("to") else [])]:
                if not isinstance(endpoint, dict):
                    return False
                if endpoint.get("kind") == "knowledge":
                    record = revisions.get(endpoint.get("revision_id"))
                    if not record or record.get("id") != endpoint.get("id") or not support.intersection(record.get("evidence_ids", [])):
                        return False
                elif endpoint.get("kind") == "observation":
                    if not any(source.get("id") == endpoint.get("id") and source.get("snapshot_id") == endpoint.get("revision_id") for source in sources.values()):
                        return False
                else:
                    return False
    return True


def exact_checkpoint_states(record: dict, changes: dict) -> bool:
    before = record.get("registry_before", {}).get("knowledge_current", {})
    after = record.get("registry_query", {}).get("knowledge_current", {})
    history = record.get("registry_query", {}).get("knowledge_history", {})
    for change in changes.get("changes", []):
        for side, original in (("before", before), ("after", after)):
            state = change.get(side)
            expected = original.get(change.get("knowledge_id"))
            if state is None:
                if expected is not None:
                    return False
                continue
            if not isinstance(state, dict) or not isinstance(expected, dict):
                return False
            if any(state.get(key) != value for key, value in expected.items() if key != "current_evidence_ids"):
                return False
            if "current_evidence_ids" in state and state["current_evidence_ids"] != expected.get("current_evidence_ids"):
                return False
    for state in changes.get("related_knowledge", []) + changes.get("cross_source_knowledge", []):
        original = history.get(state.get("revision_id"))
        if not isinstance(original, dict) or any(state.get(key) != value for key, value in original.items()):
            return False
    return True


def event_checks(record: dict, max_tokens: int) -> dict:
    changes = record.get("changes", {}).get("response") or {}
    guard = record.get("guard", {}).get("response") or {}
    saved = record.get("baseline_save", {}).get("response") or {}
    resolution = record.get("resolution", [])
    ids = guard_citations(guard, changes)
    cited = [entry.get("evidence_id") for entry in resolution]
    reference_integrity = original_evidence(changes, resolution)
    if guard.get("intelligence") is not None:
        try:
            shared.verify_relationship_sources(guard["intelligence"], {"results": resolution})
        except (ValueError, KeyError, TypeError):
            reference_integrity = False
    status = guard.get("assessment_status")
    nested = guard.get("intelligence")
    status_consistent = (
        status in ("no_documented_change", "change_budget_exhausted") and nested is None and not guard.get("advisories")
        or status in ("partial_static_guidance", "source_reviewed_advisory") and isinstance(nested, dict)
        and nested.get("schema_version") == 5 and isinstance(nested.get("intelligence"), dict)
        and ((nested["intelligence"].get("brief", {}).get("generation_basis") == "model_assessed") == (status == "source_reviewed_advisory"))
        and (status != "partial_static_guidance" or not guard.get("advisories"))
    )
    return {
        "response_hashes": all(response_hash_valid(record.get(key, {})) for key in ("baseline_save", "changes", "guard")),
        "versioned_contracts": changes.get("schema_version") == guard.get("schema_version") == saved.get("schema_version") == 1
            and guard.get("assessment_status") in ASSESSMENTS and status_consistent,
        "explicit_baseline": changes.get("baseline") == guard.get("baseline") == saved.get("baseline") == "transition"
            and changes.get("baseline_snapshot") == saved.get("snapshot") and changes.get("snapshot") == guard.get("snapshot")
            and all(isinstance(snapshot, dict) and all(isinstance(snapshot.get(key), str) and snapshot[key]
                for key in ("project_id", "registry_revision")) for snapshot in (changes.get("baseline_snapshot"), changes.get("snapshot")))
            and changes["baseline_snapshot"]["project_id"] == changes["snapshot"]["project_id"],
        "requested_budget": all(shared.budget_valid(value) and value["budget"]["max_tokens"] == max_tokens
                                and value["budget"]["tokenizer"] == "cl100k_base" for value in (changes, guard)),
        "source_hashes": record.get("expected_before_sources") == record.get("sources_before")
            and record.get("expected_after_sources") == record.get("sources_query") == record.get("sources_after_queries"),
        "retained_history": retained_history(record.get("registry_before", {}), record.get("registry_query", {})),
        "read_only_registry": record.get("registry_query") == record.get("registry_after_queries") and record.get("registry_query", {}).get("sqlite_integrity_ok") is True,
        "references_resolve": len(cited) == len(set(cited)) and set(cited) == ids and all(response_hash_valid(entry)
            and entry["response"].get("evidence_id", entry["response"].get("id")) == entry["evidence_id"] for entry in resolution),
        "original_evidence_and_endpoints": reference_integrity,
        "exact_checkpoint_states": exact_checkpoint_states(record, changes),
        "no_execution_claims": guard.get("execution") is False and guard.get("source_write") is False
            and changes.get("live_checkout_assessed") is False and saved.get("source_write") is False,
    }


def review_template(directory: Path) -> dict:
    run = read_json(directory / "run.json")
    return {"schema_version": 1, "protocol": PROTOCOL, "run_sha256": coding.hash_file(directory / "run.json"),
            "reviewer_id": None, "independent_of_product_and_corpus_authorship": None,
            "reviewed_at": None, "events": [{"event_id": item["event_id"], "capture_sha256": item["sha256"],
                "label_confirmed": None, "source_identity_confirmed": None,
                "alerts": [], "notes": None} for item in run["records"]],
            "instructions": "Fill actual independent reviews only. For every alert give index, supported_current_scope, material_change, and safe_concrete_action booleans; never infer scores from this template."}


def independent_reviews(paths: list[Path], run_path: Path, captures: dict) -> dict:
    reviewers, per_event = set(), {event_id: [] for event_id in captures}
    for path in paths:
        review = read_json(path)
        reviewer = identity(review.get("reviewer_id"))
        if (review.get("schema_version") != 1 or review.get("protocol") != PROTOCOL
                or review.get("run_sha256") != coding.hash_file(run_path) or reviewer in reviewers
                or review.get("independent_of_product_and_corpus_authorship") is not True
                or not isinstance(review.get("reviewed_at"), str) or not review["reviewed_at"].strip()):
            raise ValueError("Independent reviews need unique reviewers, a bound run, time, and explicit independence attestation")
        reviewers.add(reviewer)
        seen = set()
        for item in review.get("events", []):
            event_id = item.get("event_id")
            if event_id not in captures or event_id in seen:
                raise ValueError("Review names an unknown or duplicated event")
            seen.add(event_id)
            capture, expected_hash = captures[event_id]
            alerts = (capture.get("guard", {}).get("response") or {}).get("advisories", [])
            values = item.get("alerts", [])
            if (item.get("capture_sha256") != expected_hash or type(item.get("label_confirmed")) is not bool
                    or type(item.get("source_identity_confirmed")) is not bool
                    or not isinstance(values, list) or len(values) != len(alerts)
                    or {entry.get("index") for entry in values} != set(range(len(alerts)))
                    or any(type(entry.get(field)) is not bool for entry in values for field in
                           ("supported_current_scope", "material_change", "safe_concrete_action"))):
                raise ValueError("Review must bind exact capture and score every actual advisory")
            per_event[event_id].append(item)
    return per_event


def ratio(numerator: int, denominator: int) -> float | None:
    return round(numerator / denominator, 6) if denominator else None


def compilation_calls(record: dict) -> int:
    if record.get("mode") == "explicit_synthetic_registry_capture" and record.get("inference_calls") == 0:
        return 0
    return cross.model_calls(require_response(record))


def assess(directory: Path, review_files: list[Path] | None = None) -> dict:
    run_path = directory / "run.json"
    run = read_json(run_path)
    if run.get("schema_version") != 1 or run.get("protocol") != PROTOCOL:
        raise ValueError("Invalid guardian run protocol")
    prepared = run["prepared"]
    endpoints = {after["event"]["id"]: (project["id"], before, after) for project in prepared["projects"]
                 for before, after in zip(project["states"], project["states"][1:])}
    expected = {event_id: after["event"] for event_id, (_, _, after) in endpoints.items()}
    captures = {}
    for entry in run["records"]:
        event_id = entry["event_id"]
        path = directory / coding.relative_file(entry["path"])
        if event_id not in expected or event_id in captures or coding.hash_file(path) != entry["sha256"]:
            raise ValueError("Missing, duplicated, unknown, or modified event capture")
        record = read_json(path)
        if record.get("event_id") != event_id or record.get("event") != expected[event_id]:
            raise ValueError("Captured event label differs from the pinned source manifest")
        project_id, before, after = endpoints[event_id]
        if (record.get("project") != project_id or record.get("before_revision") != before["revision"]
                or record.get("after_revision") != after["revision"]
                or record.get("expected_before_sources") != before["source_fingerprint"]
                or record.get("expected_after_sources") != after["source_fingerprint"]):
            raise ValueError("Captured event is not bound to its exact successive source endpoints")
        captures[event_id] = (record, entry["sha256"])
    if captures.keys() != expected.keys():
        raise ValueError("A guardian assessment cannot silently drop unavailable or failed transitions")
    reviews = independent_reviews(review_files or [], run_path, captures)
    rows, statuses, failures = [], Counter(), Counter()
    required = required_assessed = required_detected = reviewed_alerts = supported_alerts = actionable = 0
    high_alerts = benign_alerts = duplicate_alerts = total_model_calls = 0
    total_compilation_calls = 0
    usage_known = True
    audit_complete = True
    elapsed = []
    for initial in run["initialization"]:
        try:
            total_compilation_calls += compilation_calls(initial["compilation"])
        except (KeyError, ValueError, TypeError):
            usage_known = False
    for event_id, (record, _) in captures.items():
        guard = record.get("guard", {}).get("response") or {}
        changes = record.get("changes", {}).get("response") or {}
        try:
            checks = event_checks(record, run["max_tokens"])
        except (ValueError, KeyError, TypeError):
            checks = {"valid_capture": False}
        for check, passed in checks.items():
            if not passed:
                failures[check] += 1
        status = guard.get("assessment_status", "command_failed")
        statuses[status] += 1
        count = len(guard.get("advisories", []))
        high = sum(advisory.get("severity") == "high" for advisory in guard.get("advisories", []))
        high_alerts += high
        label = record["event"]["expected"]
        if label["significance"] == "benign":
            benign_alerts += high
        # Same-evidence duplicates are a diagnostic lower bound. Source-level
        # equivalence and action semantics require independent alert review.
        signatures = [cross.digest([sorted(advisory.get("evidence_ids", [])), sorted(advisory.get("observation_ids", []))]) for advisory in guard.get("advisories", [])]
        duplicate_alerts += len(signatures) - len(set(signatures))
        included = sum(len(changes.get(key, [])) for key in ("changes", "relationship_changes", "imported_changes", "cross_source_changes"))
        omitted = sum(changes.get(key, 0) for key in ("omitted_changes", "omitted_relationship_changes", "omitted_imported_changes", "omitted_cross_source_changes"))
        reviewers = reviews[event_id]
        reviewed = len(reviewers) >= 2 and all(item["label_confirmed"] and item["source_identity_confirmed"] for item in reviewers)
        available = all(checks.values()) and status == "source_reviewed_advisory" and not omitted and not changes.get("retrieval_truncated", False) and not guard.get("advisories_in_shared_guidance", 0)
        correctly_alerted = False
        if label["alert_necessity"] == "required":
            required += 1
            if available and reviewed:
                required_assessed += 1
        if reviewed and available:
            for index, advisory in enumerate(guard.get("advisories", [])):
                if advisory.get("severity") != "high":
                    continue
                scores = [next(item for item in review["alerts"] if item["index"] == index) for review in reviewers]
                reviewed_alerts += 1
                supported = all(item["supported_current_scope"] and item["material_change"] for item in scores) and label["alert_necessity"] != "forbidden"
                supported_alerts += supported
                correctly_alerted |= supported
                actionable += all(item["safe_concrete_action"] for item in scores)
        if label["alert_necessity"] == "required" and available and reviewed:
            required_detected += correctly_alerted
        try:
            total_compilation_calls += compilation_calls(record["materialization"])
        except (KeyError, ValueError, TypeError):
            usage_known = False
        try:
            if not response_hash_valid(record.get("guard", {})):
                raise ValueError("A failed guardian may have made unreported provider calls")
            calls = shared.invocation_calls(guard["intelligence"], "adaptive") if guard.get("intelligence") is not None else 0
        except (KeyError, TypeError, ValueError):
            calls, usage_known = None, False
        if calls is not None:
            total_model_calls += calls
        event_audit = all(record.get(key, {}).get("audit", {}).get("status") == "captured"
                          and record[key]["audit"].get("execution_verified") is True
                          and record[key]["audit"].get("network_verified") is True for key in ("changes", "guard"))
        audit_complete &= event_audit
        elapsed.append(record["elapsed_seconds"])
        rows.append({"event_id": event_id, "expected": label, "assessment_status": status,
                     "checks": checks, "included_change_groups": included, "omitted_change_groups": omitted,
                     "high_alerts": high, "advisories": count, "independently_reviewed": reviewed,
                     "advisory_summaries_omitted": guard.get("advisories_in_shared_guidance", 0),
                     "model_calls": calls, "end_to_end_seconds": record["elapsed_seconds"],
                     "guard_reported_tokens": guard.get("budget", {}).get("used_tokens"),
                     "independent_runtime_audit_complete": event_audit})
    counts = Counter(row["expected"]["significance"] for row in rows)
    full_size = len(prepared["projects"]) >= 3 and all(counts[group] >= 20 for group in SIGNIFICANCE)
    external_reviewed = not run["fixture_only"] and all(row["independently_reviewed"] for row in rows)
    precision, recall = ratio(supported_alerts, reviewed_alerts), ratio(required_detected, required_assessed)
    efficacy_measured = external_reviewed and full_size and required_assessed == required and precision is not None and recall is not None
    material_rows = [row for row in rows if row["expected"]["significance"] == "consequential"]
    false_absence = [row["event_id"] for row in material_rows if row["included_change_groups"] == 0 and row["omitted_change_groups"] == 0 and all(row["checks"].values())]
    p95 = sorted(elapsed)[max(0, math.ceil(len(elapsed) * .95) - 1)] if elapsed else None
    summary = {"schema_version": 1, "protocol": PROTOCOL, "run_sha256": coding.hash_file(run_path),
               "fixture_only": run["fixture_only"], "held_out": run["held_out"], "events": len(rows), "projects": len(prepared["projects"]),
               "counts": dict(counts), "assessment_statuses": dict(statuses), "full_size": full_size,
               "engineering_capture_pass": not failures, "failed_checks": dict(failures),
               "independent_runtime_audit_complete": audit_complete,
               "high_severity_precision": precision, "high_severity_precision_numerator": supported_alerts,
               "high_severity_precision_denominator": reviewed_alerts,
               "consequential_alert_recall": recall, "required_events": required,
               "required_events_with_reviewed_available_assessment": required_assessed,
               "required_event_assessment_coverage": ratio(required_assessed, required),
               "high_alerts_total": high_alerts, "high_alerts_per_benign_change": ratio(benign_alerts, counts["benign"]),
               "same_reference_duplicate_alerts": duplicate_alerts,
               "independently_actionable_alert_rate": ratio(actionable, reviewed_alerts),
               "material_changes_with_false_absence": false_absence,
               "guard_model_calls": total_model_calls if usage_known else None,
               "compilation_model_calls": total_compilation_calls if usage_known else None,
               "total_model_calls": total_model_calls + total_compilation_calls if usage_known else None,
               "provider_tokens": 0 if usage_known and total_model_calls + total_compilation_calls == 0 else None,
               "provider_cost_usd": 0.0 if usage_known and total_model_calls + total_compilation_calls == 0 else None,
               "end_to_end_p50_seconds": sorted(elapsed)[len(elapsed) // 2] if elapsed else None,
               "end_to_end_p95_seconds": p95, "initialization_and_compilation": run["initialization"],
               "product_efficacy_measured": efficacy_measured,
               "product_quality_gate_pass": efficacy_measured and not failures and audit_complete and precision >= .90 and recall >= .85 and benign_alerts == 0 and duplicate_alerts == 0,
               "limitations": ["Synthetic/debug labels are not independent real-project evidence.",
                   "Provider-unavailable, budget-limited, and failed transitions remain in coverage denominators.",
                   "An empty alert list does not establish project safety.",
                   "Source byte preservation is captured separately from independent runtime syscall audit.",
                   "Reported whole-output token counts are Lore's accounting; Python does not independently retokenize them.",
                   "Provider usage/cost is unknown when a called provider does not report it.",
                   "Consecutive revisions are correlated; no population-wide efficacy interval is inferred."],
               "rows": rows}
    cross.write_json(directory / "assessment.json", summary)
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    command = sub.add_parser("prepare")
    command.add_argument("--events", type=Path, default=DEFAULT_EVENTS)
    command.add_argument("--output", type=Path, required=True)
    command = sub.add_parser("run")
    command.add_argument("--prepared", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--binary", required=True)
    command.add_argument("--compiled", type=Path)
    command.add_argument("--config-template", type=Path)
    command.add_argument("--max-tokens", type=int, default=8000)
    command.add_argument("--timeout", type=int, default=120)
    command.add_argument("--allow-inspection", action="store_true")
    command.add_argument("--allow-hosted", action="store_true")
    command.add_argument("--allow-checkout-egress", action="store_true")
    command.add_argument("--audit-strace", action="store_true")
    command.add_argument("--project")
    command.add_argument("--first-event", type=int, default=1)
    command.add_argument("--event-count", type=int)
    command = sub.add_parser("merge")
    command.add_argument("--prepared", type=Path, required=True)
    command.add_argument("--runs", type=Path, nargs="+", required=True)
    command.add_argument("--output", type=Path, required=True)
    command = sub.add_parser("review-template")
    command.add_argument("directory", type=Path)
    command.add_argument("--output", type=Path, required=True)
    command = sub.add_parser("assess")
    command.add_argument("directory", type=Path)
    command.add_argument("--reviews", type=Path, nargs="*", default=[])
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(args.output, args.events)
        elif args.command == "run":
            result = run(args)
        elif args.command == "merge":
            result = merge_runs(args.prepared, args.runs, args.output)
        elif args.command == "review-template":
            result = review_template(args.directory)
            if args.output.exists():
                raise ValueError("Refusing to overwrite an existing independent review")
            cross.write_json(args.output, result)
        else:
            result = assess(args.directory, args.reviews)
    except (OSError, ValueError, KeyError, TypeError, sqlite3.Error) as error:
        print(f"guardian_longitudinal: {error}", file=sys.stderr)
        return 1
    print(json.dumps({key: value for key, value in result.items() if key not in ("projects", "records", "rows", "prepared", "initialization", "initialization_and_compilation")}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
