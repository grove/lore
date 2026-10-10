#!/usr/bin/env python3
"""Cross-source contracts and blinded comparative assessment; standard library only.

Bundled fixtures test interoperability. They cannot establish an incremental
benefit on real projects. `run` executes the real Lore CLI only when requested;
`prepare`, `blind`, and `assess` never call an inference provider.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
from pathlib import Path
import re
import secrets
import shutil
import sqlite3
import subprocess
import sys
import time

import benchmark as bench

ROOT = Path(__file__).resolve().parent
CORPORA = ROOT / "corpora" / "cross-source"
SETUPS = ("code_docs", "openwiki", "tools", "tools_lore")
COUNTER_TABLES = ("native_records", "native_snapshots", "knowledge_units",
                  "knowledge_revisions", "cross_source_evaluations", "model_calls")


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def digest(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def reject_symlink_path(path: Path) -> None:
    for component in (path, *path.parents):
        if component.is_symlink():
            raise ValueError(f"Refusing a symlink path: {component}")


def selected_path(path: Path) -> Path:
    """Canonicalize the trusted caller-selected parent, preserving root checks.

    System temporary parents can be symlinks (for example /var on macOS).
    Descendants of this selected boundary are still checked without resolving
    away candidate-created symlinks.
    """
    path = path.absolute()
    if path.is_symlink():
        raise ValueError(f"Refusing a symlink root: {path}")
    return path.parent.resolve() / path.name


def fingerprint(root: Path) -> dict:
    root = selected_path(root)
    reject_symlink_path(root)
    if not root.is_dir():
        raise ValueError(f"Snapshot root must be a directory: {root}")
    files = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"Snapshot contains a symlink: {path.relative_to(root)}")
        if path.is_file():
            files[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
        elif not path.is_dir():
            raise ValueError(f"Snapshot contains a nonregular file: {path.relative_to(root)}")
    return {"sha256": digest(files), "files_sha256": files, "file_count": len(files)}


def bundled_fingerprints() -> set[str]:
    """Recognize original fixtures, mutation overlays, and their merged snapshots."""
    known = set()
    for name in selected_projects(None):
        initial = fingerprint(CORPORA / name / "initial")
        known.add(initial["sha256"])
        mutation = CORPORA / name / "mutation"
        if mutation.is_dir():
            changed = fingerprint(mutation)
            known.add(changed["sha256"])
            known.add(digest(initial["files_sha256"] | changed["files_sha256"]))
    return known


def copy_snapshot(source: Path, destination: Path) -> dict:
    source = selected_path(source)
    manifest = fingerprint(source)
    if not manifest["file_count"] or manifest["file_count"] > 100:
        raise ValueError("Snapshot file count must be between 1 and 100")
    total = sum((source / path).stat().st_size for path in manifest["files_sha256"])
    if total > 5_000_000:
        raise ValueError("Snapshot exceeds the 5 MB fixture budget")
    # The candidate CLI has already operated on mutation destinations. Preflight
    # every target before copying so an existing symlink cannot redirect writes.
    reject_symlink_path(destination)
    destination_root = destination.resolve()
    for relative in manifest["files_sha256"]:
        target = destination / relative
        reject_symlink_path(target)
        if not target.resolve().is_relative_to(destination_root):
            raise ValueError("Snapshot destination escapes the evaluation project")
    for relative in manifest["files_sha256"]:
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        # Inputs are copied into an isolated evaluation workspace, never edited.
        shutil.copyfile(source / relative, target)
    return manifest


def selected_projects(names: list[str] | None) -> list[str]:
    available = read_json(CORPORA / "cases.json")["projects"]
    names = names or list(available)
    if not names or len(names) != len(set(names)) or set(names) - set(available):
        raise ValueError("Choose unique known cross-source fixture projects")
    return names


def prepare_project(name: str, destination: Path) -> dict:
    selected_projects([name])
    if destination.exists() and any(destination.iterdir()):
        raise ValueError(f"Refusing nonempty evaluation project: {destination}")
    destination.mkdir(parents=True, exist_ok=True)
    manifest = copy_snapshot(CORPORA / name / "initial", destination)
    return {"project": name, "fixture_only": True, "held_out": False,
            "bundle_version": "cross-source-v1", "source_fingerprint": manifest}


def prepare(output: Path, projects: list[str] | None = None) -> dict:
    output = selected_path(output)
    if output.exists():
        raise ValueError(f"Use a fresh output directory: {output}")
    names = selected_projects(projects)
    output.mkdir(parents=True)
    result = {"schema_version": 1, "fixture_only": True,
              "projects": [prepare_project(name, output / name) for name in names]}
    write_json(output / "manifest.json", result)
    return result


def registry_state(project: Path) -> dict:
    db = project / ".lore" / "state.db"
    with closing(sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True)) as conn:
        tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        counts = {table: conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
                  for table in COUNTER_TABLES if table in tables}
        current = list(conn.execute("SELECT observation_id,snapshot_id FROM native_current ORDER BY observation_id")) if "native_current" in tables else []
        native_ids = [row[0] for row in conn.execute(
            "SELECT s.evidence_id FROM native_snapshots s JOIN native_current c ON c.snapshot_id=s.id ORDER BY s.evidence_id"
        )] if "native_current" in tables else []
        integrity = conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    return {"counts": counts, "native_current_sha256": digest(current),
            "native_evidence_ids": native_ids, "sqlite_integrity_ok": integrity,
            "database_sha256": hashlib.sha256(db.read_bytes()).hexdigest(),
            "wiki_sha256": bench.file_hashes(project / "wiki")}


def cited_ids(value) -> set[str]:
    result: set[str] = set()
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "evidence_id" and isinstance(child, str) and child:
                result.add(child)
            elif key == "evidence_ids" and isinstance(child, list):
                result.update(item for item in child if isinstance(item, str) and item)
            elif key in ("evidence", "imported_evidence") and isinstance(child, list):
                result.update(item["id"] for item in child if isinstance(item, dict) and isinstance(item.get("id"), str))
            result.update(cited_ids(child))
    elif isinstance(value, list):
        for child in value:
            result.update(cited_ids(child))
    return result


def resolve_citations(binary: str, project: Path, ids: set[str], timeout: int,
                      *, env: dict[str, str] | None = None) -> dict:
    failures, results = [], []
    for evidence_id in sorted(ids):
        evidence, error = None, None
        try:
            evidence, _ = bench.subprocess_json(binary, project, "evidence", evidence_id, timeout=timeout,
                                                **({"env": env} if env is not None else {}))
            if evidence.get("evidence_id", evidence.get("id")) != evidence_id:
                failures.append(evidence_id)
        except ValueError as exception:
            failures.append(evidence_id)
            error = str(exception)
        results.append({"evidence_id": evidence_id, "response": evidence,
                        "response_sha256": digest(evidence) if evidence is not None else None,
                        "error": error})
    return {"checked": len(ids), "resolvable": len(ids) - len(failures),
            "failures": failures, "results": results,
            "mechanically_verified": bool(ids) and not failures,
            "method": "lore evidence; native hash and JSON pointers validated by the CLI"}


def model_calls(report: dict) -> int:
    generative, decision = report.get("model_calls"), report.get("decision_calls", 0)
    if type(generative) is not int or type(decision) is not int or min(generative, decision) < 0:
        raise ValueError("CLI report lacks valid model-call accounting")
    return generative + decision


def run_phase(binary: str, project: Path, result_dir: Path, case: dict,
              name: str, command: str, timeout: int, max_tokens: int) -> dict:
    report, preparation_seconds = bench.subprocess_json(binary, project, command, timeout=timeout)
    before = registry_state(project)
    answers, contexts, evidence_ids = [], [], set(before["native_evidence_ids"])
    retrieval_seconds = 0.0
    for query in case["queries"]:
        answer, elapsed = bench.subprocess_json(binary, project, "context", query["task"],
                                               "--fast", "--max-tokens", str(max_tokens), timeout=timeout)
        evidence_ids.update(cited_ids(answer))
        contexts.append({"task": query["task"], "response": answer, "response_sha256": digest(answer)})
        path = result_dir / f"{name}-{query['id']}.json"
        write_json(path, answer)
        answers.append({"query_id": query["id"], "answer_file": path.name,
                        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                        "elapsed_seconds": elapsed, "model_calls": model_calls(answer),
                        "retrieval_truncated": answer.get("retrieval_truncated"),
                        "omissions": answer.get("omissions")})
        retrieval_seconds += elapsed
    integrity = resolve_citations(binary, project, evidence_ids, timeout)
    integrity["expected_imported_evidence_ids"] = sorted(before["native_evidence_ids"])
    integrity["expected_cited_evidence_ids"] = sorted(set().union(*(cited_ids(c["response"]) for c in contexts)))
    after = registry_state(project)
    return {"report": report, "preparation_seconds": preparation_seconds,
            "preparation_model_calls": model_calls(report),
            "retrieval_seconds": round(retrieval_seconds, 3),
            "retrieval_model_calls": sum(answer["model_calls"] for answer in answers),
            "read_only_registry_unchanged": before == after,
            "citation_integrity": integrity, "answers": answers,
            "contexts": contexts, "registry_before_retrieval": before,
            "registry_after_retrieval": after,
            "sqlite_integrity_ok": after["sqlite_integrity_ok"]}


def contract_checks(phases: dict, no_op: dict) -> dict:
    return {
        "unchanged_import_is_no_op": no_op.get("no_op") is True,
        "unchanged_import_zero_model_calls": no_op.get("model_calls") == 0,
        "unchanged_import_zero_knowledge_churn": no_op.get("knowledge_churn") == 0 and no_op.get("registry_unchanged") is True,
        "all_imported_citations_resolve": all(p["citation_integrity"]["checked"] > 0 and p["citation_integrity"]["checked"] == p["citation_integrity"]["resolvable"] for p in phases.values()),
        "retrieval_is_model_free": all(p["retrieval_model_calls"] == 0 for p in phases.values()),
        "retrieval_does_not_change_registry": all(p["read_only_registry_unchanged"] for p in phases.values()),
        "sqlite_integrity": all(p["sqlite_integrity_ok"] for p in phases.values()),
    }


def write_run_contract(path: Path, source_manifest: dict, phase: dict,
                       unchanged_import: dict | None, provenance: dict) -> dict:
    artifact = {"schema_version": 1, "source_fingerprint": source_manifest["sha256"],
                "source_manifest": source_manifest, "contexts": phase["contexts"],
                "citation_integrity": phase["citation_integrity"],
                "registry_before_retrieval": phase["registry_before_retrieval"],
                "registry_after_retrieval": phase["registry_after_retrieval"],
                "unchanged_import": unchanged_import, "provenance": provenance}
    write_json(path, artifact)
    return {"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def run(args: argparse.Namespace) -> dict:
    output = selected_path(args.output)
    names = selected_projects(args.projects)
    if output.exists():
        raise ValueError(f"Use a fresh output directory: {output}")
    binary_path = Path(args.lore_binary).resolve(strict=True)
    if args.billed_cost_usd is not None and (not 0 <= args.billed_cost_usd < float("inf") or not args.billing_source):
        raise ValueError("Measured billed cost requires a nonnegative amount and --billing-source")
    # Validate hosted consent and model configuration before creating outputs.
    config = bench.config_for(output, "cross-source", args.provider, args.model,
                              args.decision_provider, args.decision_model, args.allow_hosted,
                              None, True, generative_base_url=args.generative_base_url,
                              decision_base_url=args.decision_base_url,
                              reasoning=bench.reasoning_from_args(args))
    output.mkdir(parents=True)
    cases = read_json(CORPORA / "cases.json")
    result = {"schema_version": 1, "fixture_only": True, "held_out": False,
              "run_at": bench.now_utc(), "lore_binary_sha256": hashlib.sha256(binary_path.read_bytes()).hexdigest(),
              "provider": args.provider, "model": args.model, "projects": [],
              "upstream_snapshot_creation_cost": None,
              "cost_scope": "Measured Lore preparation, mutations, and retrieval. Creating upstream snapshots is not measured by the bundled fixture run.",
              "incremental_benefit_measured": False,
              "billed_cost_usd": args.billed_cost_usd,
              "billing_source": args.billing_source if args.billed_cost_usd is not None else None}
    for name in names:
        result_dir, project = output / name, output / name / "project"
        started = time.monotonic()
        manifest = prepare_project(name, project)
        project_config = json.loads(json.dumps(config))
        project_config["schema_version"] = 2
        project_config["project"]["name"] = name
        project_config["imports"] = [
            {"id": "implementation", "kind": "openwiki", "path": "./openwiki"},
            {"id": "agent-memory", "kind": "engram", "path": "./imports/engram.json", "project": name},
            {"id": "work-history", "kind": "beads", "path": "./imports/beads.jsonl", "include_memories": False},
        ]
        write_json(project / "lore.yml", project_config)
        fixture_copy_seconds = time.monotonic() - started
        initial = run_phase(str(binary_path), project, result_dir, cases["projects"][name],
                            "initial", "init", args.timeout, args.max_tokens)
        before = registry_state(project)
        noop_report, noop_seconds = bench.subprocess_json(str(binary_path), project, "update", timeout=args.timeout)
        after = registry_state(project)
        noop = {"no_op": noop_report.get("no_op") is True, "model_calls": model_calls(noop_report),
                "knowledge_churn": sum(abs(before["counts"].get(table, 0) - after["counts"].get(table, 0)) for table in COUNTER_TABLES),
                "registry_unchanged": before == after, "elapsed_seconds": noop_seconds}
        provenance = {"collector": "cross_source.py run", "lore_binary_sha256": result["lore_binary_sha256"],
                      "configuration_sha256": digest(project_config),
                      "execution_attestation": "Collector reports real CLI invocations; this artifact is not cryptographic proof of execution."}
        initial["run_contract"] = write_run_contract(
            result_dir / "initial-contract.json", manifest["source_fingerprint"], initial,
            {"before": before, "after": after, "report": noop_report}, provenance)
        phases = {"initial": initial}
        if args.mutate and cases["projects"][name]["has_mutation"]:
            mutation = copy_snapshot(CORPORA / name / "mutation", project)
            phases["after_mutation"] = run_phase(str(binary_path), project, result_dir, cases["projects"][name],
                                                 "after_mutation", "update", args.timeout, args.max_tokens)
            merged_files = manifest["source_fingerprint"]["files_sha256"] | mutation["files_sha256"]
            merged_manifest = {"sha256": digest(merged_files), "files_sha256": merged_files,
                               "file_count": len(merged_files)}
            phases["after_mutation"]["run_contract"] = write_run_contract(
                result_dir / "after-mutation-contract.json", merged_manifest,
                phases["after_mutation"], None, provenance)
        costs = {"fixture_copy_seconds": round(fixture_copy_seconds, 3),
                 "preparation_seconds": round(fixture_copy_seconds + sum(p["preparation_seconds"] for p in phases.values()), 3),
                 "retrieval_seconds": round(sum(p["retrieval_seconds"] for p in phases.values()), 3),
                 "preparation_model_calls": sum(p["preparation_model_calls"] for p in phases.values()),
                 "retrieval_model_calls": sum(p["retrieval_model_calls"] for p in phases.values())}
        costs["total_preparation_and_retrieval_seconds"] = round(costs["preparation_seconds"] + costs["retrieval_seconds"], 3)
        costs["total_preparation_and_retrieval_model_calls"] = costs["preparation_model_calls"] + costs["retrieval_model_calls"]
        project_report = {"project": name, "manifest": manifest, "phases": phases, "no_op": noop,
                          "costs": costs, "configuration_sha256": digest(project_config),
                          "contract_checks": contract_checks(phases, noop),
                          "human_quality_review": "pending", "held_out_comparison": "pending"}
        write_json(result_dir / "metrics.json", project_report)
        result["projects"].append(project_report)
    result["all_contracts_passed"] = all(all(project["contract_checks"].values()) for project in result["projects"])
    write_json(output / "metrics.json", result)
    return result


def nonnegative_int(value, name: str) -> int:
    if type(value) is not int or value < 0:
        raise ValueError(f"{name} must be a nonnegative integer")
    return value


def check_cost(cost: dict) -> None:
    for field in ("preparation_seconds", "retrieval_seconds"):
        value = cost.get(field)
        if type(value) not in (int, float) or not 0 <= value < float("inf"):
            raise ValueError(f"Full setup cost lacks valid {field}")
    for field in ("preparation_model_calls", "retrieval_model_calls"):
        nonnegative_int(cost.get(field), field)
    if cost.get("preparation_includes_all_tools") is not True:
        raise ValueError("Comparison preparation cost must include every tool in that setup")
    billed = cost.get("billed_cost_usd")
    if billed is not None and (type(billed) not in (int, float) or not 0 <= billed < float("inf")):
        raise ValueError("billed_cost_usd must be a measured nonnegative amount or null")


def is_sha256(value) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value)


def read_contract(base: Path, reference: dict) -> tuple[dict, bytes]:
    if not isinstance(reference, dict) or not isinstance(reference.get("path"), str) or not reference["path"] or not is_sha256(reference.get("sha256")):
        raise ValueError("Run-contract references require path and SHA-256")
    path = base / reference["path"]
    reject_symlink_path(path)
    if not path.is_file() or path.stat().st_size > 20_000_000:
        raise ValueError("Run contracts must be regular files up to 20 MB")
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != reference["sha256"]:
        raise ValueError("Run-contract artifact digest mismatch")
    artifact = json.loads(data)
    if not isinstance(artifact, dict) or artifact.get("schema_version") != 1:
        raise ValueError("Expected a schema_version: 1 run-contract artifact")
    return artifact, data


def id_set(value) -> set[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value) or len(value) != len(set(value)):
        raise ValueError("Evidence identity manifests need unique nonempty strings")
    return set(value)


def valid_registry(state) -> bool:
    return (isinstance(state, dict) and is_sha256(state.get("database_sha256"))
            and is_sha256(state.get("native_current_sha256"))
            and state.get("sqlite_integrity_ok") is True
            and isinstance(state.get("counts"), dict)
            and all(type(n) is int and n >= 0 for n in state["counts"].values())
            and isinstance(state.get("wiki_sha256"), dict)
            and all(is_sha256(value) for value in state["wiki_sha256"].values()))


def assess_contract(artifact: dict, source_manifest: dict, task: str) -> tuple[bool, bool, list[str]]:
    """Validate bound collector records; execution itself remains an attestation."""
    issues = []
    if artifact.get("source_fingerprint") != source_manifest["sha256"] or artifact.get("source_manifest") != source_manifest:
        return False, False, ["run contract does not match the bound source snapshot"]
    provenance = artifact.get("provenance", {})
    if not isinstance(provenance, dict) or not provenance.get("collector") or not all(is_sha256(provenance.get(k)) for k in ("lore_binary_sha256", "configuration_sha256")):
        return False, False, ["run contract lacks collector, executable, or configuration provenance"]
    before, after = artifact.get("registry_before_retrieval"), artifact.get("registry_after_retrieval")
    if not valid_registry(before) or not valid_registry(after) or before != after:
        return False, False, ["retrieval registry content snapshots are missing, invalid, or changed"]
    try:
        imported = id_set(before.get("native_evidence_ids"))
        contexts = artifact.get("contexts")
        if not isinstance(contexts, list) or not contexts:
            raise ValueError("run contract lacks context responses")
        cited, tasks = set(), set()
        for context in contexts:
            response = context.get("response") if isinstance(context, dict) else None
            if not isinstance(response, dict) or context.get("response_sha256") != digest(response) or not isinstance(context.get("task"), str) or response.get("task") != context["task"]:
                raise ValueError("context response hash or task binding mismatch")
            if model_calls(response) != 0:
                raise ValueError("context retrieval invoked models")
            tasks.add(context["task"])
            cited.update(cited_ids(response))
        if task not in tasks:
            raise ValueError("run contract does not include this task's context")
        citations = artifact.get("citation_integrity", {})
        if not isinstance(citations, dict) or id_set(citations.get("expected_imported_evidence_ids")) != imported or id_set(citations.get("expected_cited_evidence_ids")) != cited:
            raise ValueError("citation coverage does not match imported registry IDs and context references")
        expected, results = imported | cited, citations.get("results")
        if not imported or not isinstance(results, list) or any(not isinstance(r, dict) for r in results) or id_set([r.get("evidence_id") for r in results]) != expected:
            raise ValueError("per-ID resolver records do not cover every expected imported/cited reference")
        for result in results:
            response = result.get("response")
            if not isinstance(response, dict) or response.get("evidence_id", response.get("id")) != result["evidence_id"] or result.get("response_sha256") != digest(response) or result.get("error") is not None:
                raise ValueError("an evidence resolver response is missing, mismatched, or failed")
        if type(citations.get("checked")) is not int or type(citations.get("resolvable")) is not int or citations["checked"] != len(expected) or citations["resolvable"] != len(expected) or citations.get("failures") != [] or citations.get("mechanically_verified") is not True:
            raise ValueError("citation summary disagrees with complete successful resolver records")
        citations_ok = True
    except (ValueError, TypeError, AttributeError) as error:
        citations_ok = False
        issues.append(str(error))
    unchanged = artifact.get("unchanged_import")
    try:
        if not isinstance(unchanged, dict) or unchanged.get("before") != after or unchanged.get("after") != after:
            raise ValueError("unchanged-import before/after registry content snapshots are missing or changed")
        report = unchanged.get("report")
        if not isinstance(report, dict) or report.get("no_op") is not True or model_calls(report) != 0:
            raise ValueError("unchanged import did not report a no-op with zero model calls")
        no_op_ok = True
    except ValueError as error:
        no_op_ok = False
        issues.append(str(error))
    return citations_ok, no_op_ok, issues


def blind(study_path: Path, output: Path) -> dict:
    """Create anonymous answer packets; retain setup labels only in assignment.json."""
    study_path, output = selected_path(study_path), selected_path(output)
    study = read_json(study_path)
    if study.get("schema_version") != 1 or not isinstance(study.get("cases"), list) or not study["cases"]:
        raise ValueError("Expected a nonempty schema_version: 1 comparison study")
    if output.exists():
        raise ValueError("Use a fresh blind-review directory")
    samples, case_keys, recognized_fixtures = [], set(), set()
    known_fixtures = bundled_fingerprints()
    for case in study["cases"]:
        key = (case.get("project"), case.get("case_id"))
        if not all(isinstance(x, str) and x.strip() for x in key) or key in case_keys:
            raise ValueError("Cases need unique nonempty project/case_id identities")
        case_keys.add(key)
        constraints = case.get("critical_constraints")
        if not isinstance(constraints, list) or not constraints or any(not isinstance(v, str) or not v.strip() for v in constraints) or len(constraints) != len(set(constraints)):
            raise ValueError("Each case needs unique reviewed critical-constraint strings")
        if set(case.get("setups", {})) != set(SETUPS):
            raise ValueError(f"Each case must include all four setups: {', '.join(SETUPS)}")
        if not isinstance(case.get("task"), str) or not case["task"].strip():
            raise ValueError("Each case needs a nonempty task")
        if not isinstance(case.get("source_root"), str) or not case["source_root"]:
            raise ValueError("Each case must provide its source_root snapshot directory")
        source_root = selected_path(study_path.parent / case["source_root"])
        source_manifest = fingerprint(source_root)
        if not source_manifest["file_count"] or case.get("source_fingerprint") != source_manifest["sha256"]:
            raise ValueError("The source fingerprint must match the nonempty source_root byte manifest")
        if source_manifest["sha256"] in known_fixtures:
            recognized_fixtures.add(source_manifest["sha256"])
        agents = {run.get("coding_agent") for run in case["setups"].values()}
        if len(agents) != 1 or not next(iter(agents)):
            raise ValueError("The same identified coding agent must run all setups for a case")
        for setup in SETUPS:
            result = case["setups"][setup]
            check_cost(result.get("cost", {}))
            answer_path = study_path.parent / result["answer_path"]
            reject_symlink_path(answer_path)
            if not answer_path.is_file() or answer_path.stat().st_size > 2_000_000:
                raise ValueError("Comparison answers must be regular files up to 2 MB")
            answer_path = answer_path.resolve(strict=True)
            answer = answer_path.read_bytes().decode("utf-8")
            if not answer.strip():
                raise ValueError("Comparison answer is empty")
            sample_id = "sample-" + secrets.token_hex(10)
            sample = {"sample_id": sample_id, "setup": setup, "project": key[0], "case_id": key[1],
                      "source_fingerprint": case["source_fingerprint"], "critical_constraints": constraints,
                      "source_root": str(source_root.resolve()), "source_manifest": source_manifest,
                      "task": case.get("task", ""), "answer_sha256": hashlib.sha256(answer.encode()).hexdigest(),
                      "run": json.loads(json.dumps(result)), "answer": answer}
            if setup == "tools_lore" and result.get("run_contract") is not None:
                artifact, data = read_contract(study_path.parent, result["run_contract"])
                if artifact.get("source_fingerprint") != source_manifest["sha256"] or artifact.get("source_manifest") != source_manifest:
                    raise ValueError("Run-contract artifact does not match its case source snapshot")
                sample["contract_bytes"] = data
                sample["run"]["run_contract"]["path"] = f"contracts/{sample_id}.json"
            samples.append(sample)
    # Validate all inputs before producing a review directory.
    output.mkdir(parents=True)
    secrets.SystemRandom().shuffle(samples)
    for sample in samples:
        sample_id = sample["sample_id"]
        answer = sample.pop("answer")
        (output / "answers").mkdir(exist_ok=True)
        (output / "answers" / f"{sample_id}.txt").write_bytes(answer.encode("utf-8"))
        if "contract_bytes" in sample:
            (output / "contracts").mkdir(exist_ok=True)
            (output / sample["run"]["run_contract"]["path"]).write_bytes(sample.pop("contract_bytes"))
    assignment = {"schema_version": 1, "study_sha256": hashlib.sha256(study_path.read_bytes()).hexdigest(),
                  "fixture_only": study.get("fixture_only") is not False or bool(recognized_fixtures),
                  "recognized_fixture_fingerprints": sorted(recognized_fixtures),
                  "held_out": study.get("held_out", False),
                  "independent_projects": study.get("independent_projects", False), "samples": samples}
    assignment_sha256 = digest(assignment)
    for sample in samples:
        sample_id = sample["sample_id"]
        review = {"schema_version": 1, "sample_id": sample_id, "answer_sha256": sample["answer_sha256"],
                  "assignment_sha256": assignment_sha256,
                  "task": sample["task"], "critical_constraints": sample["critical_constraints"],
                  "reviewer": "", "reviewed_at": "", "complete": False, "blind_confirmed": False,
                  "missed_critical_constraints": None, "unsupported_authoritative_claims": None,
                  "high_severity_unsupported_claims": None, "discrepancy_alerts": None,
                  "false_discrepancy_alerts": None, "notes": ""}
        write_json(output / "reviews" / f"{sample_id}.json", review)
    write_json(output / "assignment.json", assignment)
    return {"samples": len(samples), "assignment": str(output / "assignment.json"),
            "assignment_sha256": assignment_sha256, "fixture_only": assignment["fixture_only"],
            "instructions": "Give reviewers only answers/ and reviews/. Retain assignment.json until reviews are complete. The response itself may reveal its setup; reviewers must report whether blinding held."}


def assess(assignment_path: Path, reviews_dir: Path) -> dict:
    assignment_path, reviews_dir = selected_path(assignment_path), selected_path(reviews_dir)
    assignment = read_json(assignment_path)
    if assignment.get("schema_version") != 1 or not assignment.get("samples"):
        raise ValueError("Invalid blind assignment")
    assignment_sha256 = digest(assignment)
    known_fixtures = bundled_fingerprints()
    totals = {setup: {"critical_constraints": 0, "missed_critical_constraints": 0,
                     "unsupported_authoritative_claims": 0, "high_severity_unsupported_claims": 0,
                     "discrepancy_alerts": 0, "false_discrepancy_alerts": 0,
                     "preparation_seconds": 0.0, "retrieval_seconds": 0.0,
                     "preparation_model_calls": 0, "retrieval_model_calls": 0,
                     "billed_cost_usd": 0.0, "billing_complete": True} for setup in SETUPS}
    issues, reviewers, projects, seen, setups_by_case, case_contracts = [], set(), set(), set(), {}, {}
    measurement_issues, source_cache, sources_by_project, recognized_fixtures = [], {}, {}, set()
    source_bindings_ok = True
    mechanical_citations, no_op = True, True
    for sample in assignment["samples"]:
        setup, sample_id = sample.get("setup"), sample.get("sample_id")
        if setup not in SETUPS or not isinstance(sample_id, str) or not sample_id.startswith("sample-") or "/" in sample_id or "\\" in sample_id or sample_id in seen:
            raise ValueError("Unknown setup or invalid/duplicate sample identity")
        seen.add(sample_id)
        projects.add(sample["project"])
        sources_by_project.setdefault(sample["project"], set()).add(sample.get("source_fingerprint"))
        if sample.get("source_fingerprint") in known_fixtures:
            recognized_fixtures.add(sample["source_fingerprint"])
        try:
            root = Path(sample["source_root"])
            if not root.is_absolute():
                raise ValueError("source_root must retain its absolute bound path")
            # This root was already canonicalized at blinding. Do not resolve
            # away any symlink introduced into its ancestry since that binding.
            reject_symlink_path(root)
            if str(root) not in source_cache:
                source_cache[str(root)] = fingerprint(root)
            actual = source_cache[str(root)]
            if actual != sample.get("source_manifest") or actual["sha256"] != sample.get("source_fingerprint") or not actual["file_count"]:
                raise ValueError("source snapshot bytes no longer match the bound manifest")
        except (ValueError, OSError, TypeError, KeyError) as error:
            source_bindings_ok = False
            issues.append(f"{sample_id}: {error}")
        case = (sample["project"], sample["case_id"])
        setups_by_case.setdefault(case, []).append(setup)
        contract = digest((sample["critical_constraints"], sample["source_fingerprint"], sample["task"], sample["run"].get("coding_agent")))
        if case in case_contracts and contract != case_contracts[case]:
            issues.append(f"{sample_id}: case sources, constraints, task, or coding-agent identity differ across setups")
        case_contracts[case] = contract
        review_path = reviews_dir / f"{sample_id}.json"
        answer_path = assignment_path.parent / "answers" / f"{sample_id}.txt"
        if not review_path.is_file():
            issues.append(f"{sample_id}: missing human review")
            continue
        review = read_json(review_path)
        if review.get("assignment_sha256") != assignment_sha256:
            issues.append(f"{sample_id}: assignment/review binding mismatch")
            continue
        if not answer_path.is_file() or hashlib.sha256(answer_path.read_bytes()).hexdigest() != sample["answer_sha256"] or review.get("answer_sha256") != sample["answer_sha256"] or review.get("sample_id") != sample_id:
            issues.append(f"{sample_id}: response/review binding mismatch")
            continue
        if review.get("schema_version") != 1 or review.get("critical_constraints") != sample["critical_constraints"] or review.get("task") != sample["task"] or review.get("complete") is not True or review.get("blind_confirmed") is not True or not all(isinstance(review.get(k), str) and review[k].strip() for k in ("reviewer", "reviewed_at", "notes")):
            issues.append(f"{sample_id}: incomplete or unblinded human review")
            continue
        missed = review.get("missed_critical_constraints")
        constraints = sample["critical_constraints"]
        if not isinstance(missed, list) or len(missed) != len(set(missed)) or any(item not in constraints for item in missed):
            issues.append(f"{sample_id}: unknown or duplicate missed constraint")
            continue
        try:
            metrics = {field: nonnegative_int(review.get(field), field) for field in
                       ("unsupported_authoritative_claims", "high_severity_unsupported_claims", "discrepancy_alerts", "false_discrepancy_alerts")}
            if metrics["high_severity_unsupported_claims"] > metrics["unsupported_authoritative_claims"] or metrics["false_discrepancy_alerts"] > metrics["discrepancy_alerts"]:
                raise ValueError("Adjudicated subcounts exceed totals")
            check_cost(sample["run"].get("cost", {}))
        except ValueError as error:
            issues.append(f"{sample_id}: {error}")
            continue
        reviewers.add(review["reviewer"])
        total = totals[setup]
        total["critical_constraints"] += len(constraints)
        total["missed_critical_constraints"] += len(missed)
        for field, number in metrics.items():
            total[field] += number
        cost = sample["run"]["cost"]
        for field in ("preparation_seconds", "retrieval_seconds", "preparation_model_calls", "retrieval_model_calls"):
            total[field] += cost[field]
        if cost.get("billed_cost_usd") is None:
            total["billing_complete"] = False
        else:
            total["billed_cost_usd"] += cost["billed_cost_usd"]
        if setup == "tools_lore":
            try:
                artifact, _ = read_contract(assignment_path.parent, sample["run"].get("run_contract"))
                citations_ok, no_op_ok, failures = assess_contract(artifact, sample["source_manifest"], sample["task"])
                answer_ids = set(re.findall(rb"\bne_[0-9a-fA-F]+\b", answer_path.read_bytes()))
                resolved_ids = {result["evidence_id"].encode() for result in artifact.get("citation_integrity", {}).get("results", [])
                                if isinstance(result, dict) and isinstance(result.get("evidence_id"), str)} if citations_ok else set()
                if not answer_ids <= resolved_ids:
                    citations_ok = False
                    failures.append("answer contains imported evidence IDs absent from successful resolver records")
                mechanical_citations &= citations_ok
                no_op &= no_op_ok
                measurement_issues.extend(f"{sample_id}: {failure}" for failure in failures)
            except (ValueError, OSError) as error:
                mechanical_citations, no_op = False, False
                measurement_issues.append(f"{sample_id}: {error}")
    complete_design = all(sorted(setups) == sorted(SETUPS) for setups in setups_by_case.values())
    if not complete_design:
        issues.append("The study does not contain exactly one result per setup for every case")
    lore = totals["tools_lore"]
    best = min(SETUPS[:-1], key=lambda setup: totals[setup]["missed_critical_constraints"])
    baseline_missed = totals[best]["missed_critical_constraints"]
    reduction = (baseline_missed - lore["missed_critical_constraints"]) / baseline_missed if baseline_missed else None
    false_rate = lore["false_discrepancy_alerts"] / lore["discrepancy_alerts"] if lore["discrepancy_alerts"] else None
    human_complete = not issues and len(reviewers) >= 2
    consistent_sources = all(len(sources) == 1 for sources in sources_by_project.values())
    distinct_sources = set().union(*sources_by_project.values())
    fixture_only = assignment.get("fixture_only") is not False or bool(recognized_fixtures)
    gates = {
        "all_four_setups_and_complete_blind_reviews": human_complete and complete_design,
        "at_least_three_independent_held_out_projects": source_bindings_ok and consistent_sources and len(projects) >= 3 and len(distinct_sources) == len(projects) and assignment.get("held_out") is True and assignment.get("independent_projects") is True and not fixture_only,
        "missed_critical_constraints_reduced_by_at_least_20_percent": reduction is not None and reduction >= 0.2,
        "no_increase_in_unsupported_authoritative_claims": lore["unsupported_authoritative_claims"] <= min(totals[setup]["unsupported_authoritative_claims"] for setup in SETUPS[:-1]),
        "zero_high_severity_unsupported_claims": lore["high_severity_unsupported_claims"] == 0,
        "false_discrepancy_rate_at_most_10_percent": false_rate is not None and false_rate <= 0.1,
        "all_imported_citations_mechanically_resolvable": mechanical_citations and human_complete,
        "unchanged_import_zero_churn_and_model_calls": no_op and human_complete,
    }
    for total in totals.values():
        total["total_preparation_and_retrieval_seconds"] = total["preparation_seconds"] + total["retrieval_seconds"]
        total["total_preparation_and_retrieval_model_calls"] = total["preparation_model_calls"] + total["retrieval_model_calls"]
        if not total["billing_complete"]:
            total["billed_cost_usd"] = None
    return {"schema_version": 1, "candidate_validated": all(gates.values()), "gates": gates,
            "assignment_sha256": assignment_sha256, "source_snapshots_verified": source_bindings_ok,
            "fixture_only": fixture_only, "recognized_fixture_fingerprints": sorted(recognized_fixtures),
            "issues": issues + measurement_issues, "reviewer_count": len(reviewers), "project_count": len(projects),
            "distinct_source_fingerprint_count": len(distinct_sources),
            "strongest_constraint_baseline": best, "relative_missed_constraint_reduction": reduction,
            "false_discrepancy_rate": false_rate, "setups": totals,
            "qualification": "Snapshot bytes, assignment bindings, and contract-artifact consistency are checked. Actual command execution, full cost accounting, independent held-out selection, and honest blinding remain researcher/reviewer attestations, not cryptographic proof. Unknown fixtures cannot be recognized from bytes alone. Missing evidence and bundled fixtures cannot establish release readiness. USD remains null when not completely measured."}


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("prepare", "run"):
        sub = commands.add_parser(command)
        sub.add_argument("--output", type=Path, required=True)
        sub.add_argument("--projects", nargs="+")
        if command == "run":
            sub.add_argument("--lore-binary", required=True)
            sub.add_argument("--provider", choices=("ollama", "openai"), required=True)
            sub.add_argument("--model", required=True)
            sub.add_argument("--decision-provider", choices=("ollama", "openai", "typesafe"))
            sub.add_argument("--decision-model")
            sub.add_argument("--generative-base-url")
            sub.add_argument("--decision-base-url")
            sub.add_argument("--allow-hosted", action="store_true")
            sub.add_argument("--mutate", action="store_true")
            sub.add_argument("--timeout", type=int, default=3600)
            sub.add_argument("--max-tokens", type=int, default=6000)
            sub.add_argument("--billed-cost-usd", type=float)
            sub.add_argument("--billing-source")
            bench.add_reasoning_options(sub)
    sub = commands.add_parser("blind")
    sub.add_argument("--study", type=Path, required=True)
    sub.add_argument("--output", type=Path, required=True)
    sub = commands.add_parser("assess")
    sub.add_argument("--assignment", type=Path, required=True)
    sub.add_argument("--reviews", type=Path, required=True)
    sub.add_argument("--output", type=Path)
    return parser.parse_args(argv)


def main(argv=None) -> int:
    args = parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(args.output.absolute(), args.projects)
        elif args.command == "run":
            result = run(args)
        elif args.command == "blind":
            result = blind(args.study.absolute(), args.output.absolute())
        else:
            result = assess(args.assignment.absolute(), args.reviews.absolute())
            if args.output:
                write_json(args.output, result)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 2 if (args.command == "assess" and not result["candidate_validated"]) or (args.command == "run" and not result["all_contracts_passed"]) else 0
    except (ValueError, OSError, sqlite3.Error, subprocess.SubprocessError) as error:
        print(f"Cross-source evaluation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
