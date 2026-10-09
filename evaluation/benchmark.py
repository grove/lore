#!/usr/bin/env python3
"""Reproducible, conservative Lore evaluation. Python standard library only.

Runs the real Lore CLI, never a pretend language model. Results are structural
and lexical proxies until reviewed against actual documents by humans.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import time

import relationship_scoring

ROOT = Path(__file__).resolve().parent
TARGETS = ROOT / "targets.json"
MAX_DOCUMENTS = 36
MAX_TOTAL_BYTES = 500_000
MAX_FILE_BYTES = 120_000

def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))

def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()

def corpus_fingerprint(doc_root: Path) -> dict:
    """SHA-256 over both normalized relative paths and original Markdown bytes."""
    files = {p.relative_to(doc_root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(doc_root.rglob("*")) if p.is_file() and not p.is_symlink()
             and p.suffix.lower() in (".md", ".markdown")}
    canonical = json.dumps(files, sort_keys=True, separators=(",", ":"))
    return {"sha256": hashlib.sha256(canonical.encode("utf-8")).hexdigest(),
            "files_sha256": files, "file_count": len(files)}


def err(message: str) -> None:
    raise ValueError(message)

def file_hashes(wiki_dir: Path) -> dict[str, str]:
    if not wiki_dir.is_dir():
        return {}
    return {p.relative_to(wiki_dir).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in wiki_dir.rglob("*.md") if p.is_file() and not p.is_symlink()}

def target_spec(name: str) -> dict:
    targets = load_json(TARGETS)
    if name not in targets:
        err(f"Unknown target {name!r}; available: {', '.join(sorted(targets))}")
    return targets[name]

def collect_docs(root: Path, patterns: list[str], dest: Path) -> dict:
    root = root.resolve(strict=True)
    unique = {}
    for pattern in patterns:
        if Path(pattern).is_absolute() or ".." in Path(pattern).parts:
            err(f"Unsafe input pattern: {pattern}")
        for path in sorted(root.glob(pattern)):
            if not path.is_file() or path.suffix.lower() not in (".md", ".markdown"):
                continue
            rel = path.relative_to(root)
            if any((root / Path(*rel.parts[:i])).is_symlink() for i in range(1, len(rel.parts) + 1)):
                err(f"Source contains a symlink: {rel}")
            unique[rel.as_posix()] = path
    if not unique:
        err(f"No Markdown files matched in {root}")
    if len(unique) > MAX_DOCUMENTS:
        err(f"Target exceeds {MAX_DOCUMENTS} Markdown documents; narrow its allowlist")
    total = 0
    for rel, path in sorted(unique.items()):
        size = path.stat().st_size
        if size > MAX_FILE_BYTES:
            err(f"Document exceeds {MAX_FILE_BYTES} bytes: {rel}")
        total += size
        if total > MAX_TOTAL_BYTES:
            err("Corpus exceeds total byte budget")
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(path.read_bytes())
    return {"files": sorted(unique), "total_bytes": total}

def fetch_git_source(spec: dict, destination: Path) -> Path:
    repo, revision = spec.get("repo", ""), spec.get("revision", "")
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        err("Invalid remote repo spec")
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        err("External target must be pinned to a full Git commit SHA")
    destination.mkdir(parents=True)
    commands = [
        ["git", "init", "--quiet", str(destination)],
        ["git", "-C", str(destination), "remote", "add", "origin", f"https://github.com/{repo}.git"],
        ["git", "-C", str(destination), "-c", "protocol.version=2", "fetch", "--quiet", "--depth=1", "origin", revision],
        ["git", "-C", str(destination), "checkout", "--quiet", "--detach", "FETCH_HEAD"],
    ]
    for command in commands:
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
    checked = subprocess.run(["git", "-C", str(destination), "rev-parse", "HEAD"],
                             check=True, capture_output=True, text=True).stdout.strip()
    if checked != revision:
        err("Remote corpus revision did not match its pin")
    return destination

def prepare(target: str, project_dir: Path) -> dict:
    spec = target_spec(target)
    if project_dir.exists() and any(project_dir.iterdir()):
        err(f"Refusing to overwrite nonempty evaluation project: {project_dir}")
    project_dir.mkdir(parents=True, exist_ok=True)
    corpus_root = project_dir / "docs"
    corpus_root.mkdir()
    checkout = None
    try:
        if spec["kind"] == "bundled":
            root = (ROOT / spec["directory"]).resolve(strict=True)
        elif spec["kind"] == "self":
            root = ROOT.parent.resolve(strict=True)
        elif spec["kind"] == "git":
            checkout = project_dir.parent / (project_dir.name + "-checkout")
            if checkout.exists():
                err(f"Refusing preexisting Git checkout: {checkout}")
            root = fetch_git_source(spec, checkout)
        else:
            err("Unrecognized target kind")
        manifest = collect_docs(root, spec["include"], corpus_root)
        manifest["corpus_fingerprint"] = corpus_fingerprint(corpus_root)
        manifest.update({"target": target, "source_kind": spec["kind"],
                         "source_commit": spec.get("revision"), "prepared_at": now_utc()})
        (project_dir / "corpus-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        return manifest
    finally:
        if checkout is not None and checkout.exists():
            shutil.rmtree(checkout)

REASONING_DEFAULTS = {
    "enabled": True,
    "default": "medium",
    "extraction": "low",
    "reconciliation": "high",
    "synthesis": "medium",
    "overview": "medium",
    "verification": "high",
    "overview_verification": "high",
    "context_synthesis": "medium",
    "context_verification": "high",
}
REASONING_LEVELS = ("none", "low", "medium", "high", "xhigh", "max")

def reasoning_from_args(args: argparse.Namespace) -> dict:
    values = dict(REASONING_DEFAULTS)
    values["enabled"] = not getattr(args, "disable_reasoning", False)
    for field in REASONING_DEFAULTS:
        if field == "enabled":
            continue
        override = getattr(args, "reasoning_" + field, None)
        if override is not None:
            if override not in REASONING_LEVELS:
                err(f"Unsupported reasoning effort {override} for {field}")
            values[field] = override
    return values

def add_reasoning_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--disable-reasoning", action="store_true",
        help="Omit reasoning.effort; let the provider/model select its default")
    for field, default in REASONING_DEFAULTS.items():
        if field == "enabled":
            continue
        parser.add_argument("--reasoning-" + field.replace("_", "-"),
            choices=REASONING_LEVELS, default=None,
            help=f"Responses API reasoning effort for {field} (default: {default})")

def config_for(project_dir: Path, target: str, provider: str, model: str,
               decision_provider: str | None, decision_model: str | None,
               allow_hosted: bool, base_url: str | None, verify: bool, *,
               generative_base_url: str | None = None, decision_base_url: str | None = None,
               reasoning: dict | None = None) -> dict:
    if provider not in ("ollama", "openai"):
        err("Initial generative providers are ollama or openai")
    if (decision_provider is None) != (decision_model is None):
        err("Set decision provider and model together, or neither")
    if decision_provider is not None and decision_provider not in ("ollama", "openai", "typesafe"):
        err("Unknown decision provider")
    hosted = provider != "ollama" or (decision_provider is not None and decision_provider != "ollama")
    if hosted and not allow_hosted:
        err("Hosted evaluation transmits copied source text: pass --allow-hosted explicitly")
    if base_url and len({provider, decision_provider} - {None}) > 1:
        err("--base-url is ambiguous with mixed providers; use their default endpoints")
    if base_url and (generative_base_url or decision_base_url):
        err("Use either --base-url or role-specific endpoint flags, not both")
    if decision_base_url and not decision_provider:
        err("--decision-base-url requires a decision provider and model")
    if provider == decision_provider and generative_base_url and decision_base_url and generative_base_url.rstrip("/") != decision_base_url.rstrip("/"):
        err("This configuration stores one endpoint per provider; role endpoints for the same provider must agree")
    values = dict(REASONING_DEFAULTS) if reasoning is None else dict(reasoning)
    if set(values) != set(REASONING_DEFAULTS) or type(values.get("enabled")) is not bool:
        err("Reasoning settings must include enabled and all task efforts")
    if any(v not in REASONING_LEVELS for k,v in values.items() if k != "enabled"):
        err("Unsupported reasoning effort")
    providers = {}
    roles = {"generative": {"provider": provider, "model": model},
             "reasoning": values}
    for name in {provider, decision_provider} - {None}:
        if name == "ollama":
            providers[name] = {"base_url": base_url or "http://127.0.0.1:11434"}
        elif name == "openai":
            providers[name] = {"base_url": base_url or "https://api.openai.com/v1",
                               "api_key_env": "OPENAI_API_KEY"}
        else:
            providers[name] = {"base_url": base_url or "https://api.typesafe.ai/v1",
                               "api_key_env": "TYPESAFE_API_KEY"}
    if generative_base_url:
        providers[provider]["base_url"] = generative_base_url
    if decision_base_url:
        providers[decision_provider]["base_url"] = decision_base_url
    if decision_provider:
        roles["decision"] = {"provider": decision_provider, "model": decision_model}
    return {"schema_version": 1, "project": {"name": f"lore-evaluation-{target}"},
            "sources": {"roots": [{"id": "docs", "path": "./docs"}]},
            "output": {"wiki_dir": "./wiki", "state_dir": "./.lore"},
            "models": roles, "providers": providers,
            "privacy": {"local_only": not hosted},
            "processing": {"verify_synthesis": bool(verify), "max_context_bytes": 64000}}

def subprocess_json(binary: str, project: Path, *args: str, timeout: int = 3600,
                    env: dict[str, str] | None = None) -> tuple[dict, float]:
    start = time.monotonic()
    completed = subprocess.run([binary, "--config", str(project / "lore.yml"), "--json", *args],
                               capture_output=True, text=True, timeout=timeout, env=env)
    elapsed = round(time.monotonic() - start, 3)
    if completed.returncode != 0:
        # Do not copy potentially sensitive CLI stdout/stderr into benchmark output.
        err(f"Lore {args[0]} failed (exit {completed.returncode}); inspect CLI locally. No excerpts saved")
    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as ex:
        err(f"Lore {args[0]} did not return JSON ({ex})")
    return response, elapsed

def load_gold(path: Path | None, phase: str) -> list[dict]:
    if path is None:
        return []
    data = load_json(path)
    if data.get("schema_version") != 1:
        err("Unsupported gold schema")
    records = [rec for rec in data.get("expected_assertions", [])
               if phase in rec.get("phases", ["initial"])]
    for rec in records:
        if not all(rec.get(k) for k in ("id", "source", "needle", "kind", "lifecycle")):
            err("Incomplete gold assertion entry")
    return records

def all_assertions(conn: sqlite3.Connection) -> list[dict]:
    sql = """SELECT ar.id AS assertion_id, s.root_id || ':' || s.relative_path AS source,
             ar.modality AS kind, ad.proposal_json, e.id AS evidence_id, e.exact_excerpt AS excerpt,
             aa.knowledge_id
        FROM active_assertions act
        JOIN assertion_revisions ar ON ar.id=act.assertion_revision_id
        JOIN sources s ON s.id=ar.source_id
        JOIN assertion_details ad ON ad.assertion_revision_id=ar.id
        JOIN assertion_evidence ae ON ae.assertion_revision_id=ar.id
        JOIN evidence_snapshots e ON e.id=ae.evidence_id
        LEFT JOIN assertion_assignments aa ON aa.assertion_revision_id=ar.id
        ORDER BY ar.id,e.id"""
    return [{**dict(row), "proposal": json.loads(row["proposal_json"])}
            for row in conn.execute(sql)]

def overlap_match(needle: str, candidate: str) -> bool:
    """A narrow lexical proxy, never a semantic accuracy judgment."""
    return needle.casefold().strip() in candidate.casefold()

def score_project(project: Path, gold_path: Path | None = None,
                  phase: str = "initial", run_id: str | None = None) -> dict:
    db = project / ".lore" / "state.db"
    if not db.exists():
        err(f"Lore state database not found: {db}")
    conn = sqlite3.connect(db.as_uri() + "?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    try:
        integrity = conn.execute("PRAGMA integrity_check").fetchone()[0]
        fk_errors = len(conn.execute("PRAGMA foreign_key_check").fetchall())
        assertions = all_assertions(conn)
        unique_assertions = len({row["assertion_id"] for row in assertions})
        evidence_problems = []
        for row in assertions:
            # The harness always configures one source root named docs.
            path = project / "docs" / row["source"].split(":", 1)[1] if row["source"].startswith("docs:") else None
            if path is None or not path.exists():
                evidence_problems.append({"assertion_id": row["assertion_id"], "reason": "source_unavailable"})
            elif row["excerpt"] not in path.read_text(encoding="utf-8"):
                evidence_problems.append({"assertion_id": row["assertion_id"], "reason": "excerpt_not_in_current_source"})
        gold = load_gold(gold_path, phase)
        source_expectations = []
        for expected in gold:
            candidates = [a for a in assertions if a["source"] == expected["source"]
                          and overlap_match(expected["needle"], a["excerpt"])]
            match = next((a for a in candidates if a["proposal"].get("kind") == expected["kind"]
                          and a["proposal"].get("lifecycle") == expected["lifecycle"]), None)
            # Alternatives are explicitly gold-labelled, and scored separately.
            # They never inflate the original strict type/lifecycle proxy.
            accepted_pairs = {(expected["kind"], expected["lifecycle"])}
            accepted_pairs.update(tuple(x) for x in expected.get("acceptable_pairs", []))
            acceptable = next((a for a in candidates if
                (a["proposal"].get("kind"), a["proposal"].get("lifecycle")) in accepted_pairs), None)
            best = match or acceptable
            # A relationship's endpoint is an evidence identity, not a
            # particular type/lifecycle prediction. Keep the label proxies
            # untouched, and refuse to guess when one source checkpoint
            # actually matches multiple distinct knowledge units.
            endpoint_ids = sorted({a["knowledge_id"] for a in candidates
                                   if a["knowledge_id"] is not None})
            relation_unit = endpoint_ids[0] if len(endpoint_ids) == 1 else None
            source_expectations.append({"id": expected["id"], "quote_found": bool(candidates),
                "kind_and_lifecycle_match": bool(match),
                "acceptable_type_and_lifecycle_match": bool(acceptable),
                "matching_knowledge_id": best["knowledge_id"] if best else None,
                "relation_knowledge_id": relation_unit,
                "relation_identity_candidates": len(endpoint_ids)})
        # The checkpoint denotes a candidate set of source assertions, not
        # an arbitrary single knowledge unit. Preserve the label diagnostics;
        # relationship truth requires an active, same-source evidence witness.
        gold_relations, identities = relationship_scoring.score(
            conn, gold,
            load_json(gold_path).get("expected_relations", []) if gold_path else [],
            assertions, {row["assertion_id"] for row in evidence_problems}, phase,
        )
        for match in source_expectations:
            match["relation_identity"] = identities[match["id"]]
        calls_sql = """SELECT task,provider,model,cache_hit,COUNT(*) AS n,
                         SUM(duration_ms) AS duration_ms FROM model_calls"""
        if run_id:
            calls_sql += " WHERE run_id=?"
        calls_sql += " GROUP BY task,provider,model,cache_hit ORDER BY task,provider,model"
        calls = [dict(row) for row in conn.execute(calls_sql, (run_id,) if run_id else ())]
        counts = {
            "source_files_current": conn.execute("SELECT COUNT(*) FROM source_current").fetchone()[0],
            "source_revisions_all": conn.execute("SELECT COUNT(*) FROM source_revisions").fetchone()[0],
            "knowledge_units_all": conn.execute("SELECT COUNT(*) FROM knowledge_units").fetchone()[0],
            "knowledge_units_current": conn.execute("SELECT COUNT(*) FROM knowledge_current").fetchone()[0],
            "evidence_snapshots_all": conn.execute("SELECT COUNT(*) FROM evidence_snapshots").fetchone()[0],
            "source_assertions_current": unique_assertions,
            "pending_reviews": conn.execute("SELECT COUNT(*) FROM review_items WHERE status='pending'").fetchone()[0],
            "wiki_pages": len(file_hashes(project / "wiki")),
        }
        matched = sum(x["quote_found"] for x in source_expectations)
        typed = sum(x["kind_and_lifecycle_match"] for x in source_expectations)
        return {"phase": phase, "sqlite_integrity_ok": integrity == "ok" and fk_errors == 0,
                "sqlite_integrity_detail": integrity if integrity != "ok" else None,
                "foreign_key_errors": fk_errors, "counts": counts,
                "current_excerpt_checks": len(assertions),
                "current_excerpt_failures": evidence_problems,
                "gold": {"total": len(gold), "matched_quote": matched,
                    "scorer_version": relationship_scoring.VERSION,
                    "matched_type_and_lifecycle": typed,
                    "lexical_coverage_proxy": round(matched / len(gold), 3) if gold else None,
                    "typed_coverage_proxy": round(typed / len(gold), 3) if gold else None,
                    "acceptable_type_coverage_proxy": round(
                        sum(x["acceptable_type_and_lifecycle_match"] for x in source_expectations)
                        / len(gold), 3) if gold else None,
                    "acceptable_type_and_lifecycle_count": sum(
                        x["acceptable_type_and_lifecycle_match"] for x in source_expectations),
                    "matches": source_expectations, "relations": gold_relations,
                    "relation_tests_passed": sum(1 for x in gold_relations if x["passed"]),
                    "relation_tests_total": len(gold_relations),
                    "limitations": "Lexical excerpt and relation proxies, not semantic precision or truth. Human review required."},
                "rubric": {"version": load_json(gold_path).get("rubric_version", "legacy-v1") if gold_path else None,
                           "sha256": hashlib.sha256(gold_path.read_bytes()).hexdigest() if gold_path else None},
                "model_calls_by_task": calls,
                "limitations": ["Historical excerpts are intentionally not counted as current evidence",
                                "BLAKE3 digests are not independently recomputed by this standard-library scorer",
                                "LLM output truthfulness cannot be determined from source hashes alone"]}
    finally:
        conn.close()

def manifest_gold_check(project: Path, gold_path: Path | None, phases: list[str]) -> None:
    if gold_path is None:
        return
    for phase in phases:
        for rec in load_gold(gold_path, phase):
            source = rec["source"]
            if not source.startswith("docs:"):
                err(f"Golden source must use benchmark docs root ID: {source}")
            src = project / "docs" / source.split(":", 1)[1]
            if not src.exists():
                if phase == "initial":
                    err(f"Missing golden source: {src}")
                continue
            if not overlap_match(rec["needle"], src.read_text(encoding="utf-8")):
                err(f"Golden excerpt missing from source: {rec['id']}")

def apply_mutations(project: Path, target: str) -> dict:
    rel_dir = target_spec(target).get("mutation_directory")
    if not rel_dir:
        return {"applied": False}
    root = (ROOT / rel_dir).resolve(strict=True)
    changes = collect_docs(root, ["**/*.md"], project / "docs")
    return {"applied": True, "added_files": changes["files"]}

def report_markdown(report: dict) -> str:
    lines = [f"# Lore evaluation — {report['target']}", "",
             f"Run at {report['run_at']}; Provider: {report.get('provider','unknown')}; Model: {report.get('model','unknown')}", "",
             "## Evidence and automated metrics", ""]
    for phase, entry in report.get("phases", {}).items():
        score = entry["score"]
        lines += [f"### {phase}", "",
            f"- SQLite integrity: **{'PASS' if score['sqlite_integrity_ok'] else 'FAIL'}**",
            f"- Current excerpt checks: {score['current_excerpt_checks']} (failures: {len(score['current_excerpt_failures'])})",
            f"- Knowledge units: {score['counts']['knowledge_units_current']}",
            f"- Pending reviews: {score['counts']['pending_reviews']}",
            f"- Recorded successful generative calls: {sum(x['n'] for x in score['model_calls_by_task'] if x['cache_hit'] == 0)}"]
        gold = score["gold"]
        if gold["total"]:
            lines.append(f"- Labeled excerpt proxy: {gold['matched_quote']}/{gold['total']}; strict type/lifecycle: {gold['matched_type_and_lifecycle']}/{gold['total']}")
            lines.append(f"- Acceptable labelled type alternatives (still lexical): {gold.get('acceptable_type_and_lifecycle_count', gold['matched_type_and_lifecycle'])}/{gold['total']}")
        if gold["relation_tests_total"]:
            lines.append(f"- Labeled relationship checks: {gold['relation_tests_passed']}/{gold['relation_tests_total']} (unassessable count as failed)")
            lines.append(f"- Relationship endpoint scorer: {gold.get('scorer_version','legacy')} (source-bound graph witnesses; independent of lifecycle labels)")
        if "elapsed_seconds" in entry:
            lines.append(f"- CLI elapsed seconds: {entry['elapsed_seconds']}")
        degraded=entry.get("report",{}).get("degraded_topics",[])
        if degraded:
            lines.append("- **DEGRADED: documentary excerpts published without passing semantic synthesis:** "+", ".join(degraded))
        diagnostics = entry.get("report",{}).get("quality_diagnostics", [])
        if diagnostics:
            lines.append(f"- Synthesis/verification draft rejections: {len(diagnostics)}")
            for note in diagnostics[:10]:
                tag = f"{note.get('task','unknown')}/{note.get('topic','unknown')}"
                reason = "; ".join(str(s).replace("\n", " ")[:160] for s in note.get("issues", [])[:2])
                lines.append(f"  - {tag}, attempt {note.get('attempt','?')}, {note.get('check','unknown')}: {reason}")
        if entry.get("report",{}).get("degraded_overview") is True:
            lines.append("- **DEGRADED OVERVIEW: source excerpts published; semantic narrative verification did not pass. This run fails the beta gate.**")
        lines.append("")
    noop = report.get("no_op", {})
    if noop:
        lines += ["## Incremental no-op", "",
                  f"- No-op: {noop.get('no_op')}; Zero calls: {noop.get('zero_generations')}; Pages unchanged: {noop.get('pages_unchanged')}", ""]
    cost = report.get("billed_cost_usd")
    lines += ["## Billing and quality limitations", "",
       "- **Provider billing (USD):** " + (str(cost) if cost is not None else
         "Not measured. Lore does not currently persist per-request token usage."),
       "- Gold matching is lexical; it is **not** factual precision, calibrated recall, or usefulness.",
       "- Provider behavior is not validated beyond the specific model actually run.", "",
       "## Human review (complete after reading the wiki)", "",
       "| Criterion | Score 0–3 | Notes and supporting evidence IDs |",
       "| --- | --- | --- |",
       "| Important facts and decisions correctly included | — | |",
       "| Plans not presented as implemented | — | |",
       "| History and supersession accurately explained | — | |",
       "| Contradictions and uncertainty visible | — | |",
       "| Citations materially support each paragraph | — | |",
       "| Topic organization understandable | — | |",
       "| Wiki more useful than original documents | — | |", "",
       "Score 0=wrong/missing, 1=poor, 2=mostly good, 3=reliable. Record concrete counterexamples.", ""]
    return "\n".join(lines)

def run_command(args: argparse.Namespace) -> dict:
    out = args.output.expanduser().absolute()
    if out.exists():
        err(f"Refusing to overwrite results directory: {out}; choose a new --output path")
    # Fail before preparing a workspace if the selected provider violates egress policy.
    checked_config = config_for(out / "project", args.target, args.provider, args.model,
                                args.decision_provider, args.decision_model,
                                args.allow_hosted, args.base_url, not args.skip_verification,
                                generative_base_url=getattr(args, "generative_base_url", None),
                                decision_base_url=getattr(args, "decision_base_url", None),
                                reasoning=reasoning_from_args(args))
    out.mkdir(parents=True)
    project = out / "project"
    manifest = prepare(args.target, project)
    spec = target_spec(args.target)
    gold_path = ROOT / spec["gold"] if spec.get("gold") else None
    manifest_gold_check(project, gold_path, ["initial"])
    (project / "lore.yml").write_text(json.dumps(checked_config, indent=2) + "\n")
    doctor, doctor_time = subprocess_json(args.lore_binary, project, "doctor", "--inference", timeout=1200)
    if not doctor:
        err("Doctor returned an empty result")
    binary = Path(args.lore_binary)
    if not binary.is_file():
        err(f"Lore binary not found: {binary}")
    binary_digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    first, first_time = subprocess_json(args.lore_binary, project, "init", timeout=args.timeout)
    score_first = score_project(project, gold_path, "initial", first.get("generation"))
    before = file_hashes(project / "wiki")
    second, noop_time = subprocess_json(args.lore_binary, project, "update", timeout=args.timeout)
    after = file_hashes(project / "wiki")
    degradation_matches = (
        sorted(second.get("degraded_topics", [])) == sorted(first.get("degraded_topics", []))
        and second.get("degraded_overview") is first.get("degraded_overview")
    )
    noop = {"no_op": second.get("no_op") is True,
            "zero_generations": second.get("model_calls") == 0 and second.get("decision_calls", 0) == 0,
            "pages_unchanged": before == after, "elapsed_seconds": noop_time,
            "degraded_topics": second.get("degraded_topics"),
            "degraded_overview": second.get("degraded_overview"),
            "degradation_status_matches_publication": degradation_matches}
    if not all([noop["no_op"], noop["zero_generations"], noop["pages_unchanged"], degradation_matches]):
        err("Incremental no-op invariant failed, including published degradation status")
    result = {"schema_version": 1, "target": args.target, "run_at": now_utc(),
              "source_manifest": manifest, "lore_binary_sha256": binary_digest,
              "configuration_sha256": hashlib.sha256(json.dumps(checked_config, sort_keys=True).encode()).hexdigest(),
              "synthesis_verification": checked_config["processing"]["verify_synthesis"],
              "rubric": {"version": load_json(gold_path).get("rubric_version", "legacy-v1") if gold_path else None,
                         "sha256": hashlib.sha256(gold_path.read_bytes()).hexdigest() if gold_path else None},
              "provider": args.provider, "model": args.model,
              "reasoning": checked_config["models"]["reasoning"],
              "decision_provider": args.decision_provider, "decision_model": args.decision_model,
              "hosted_opt_in": args.allow_hosted, "doctor_elapsed_seconds": doctor_time,
              "phases": {"initial": {"report": first, "elapsed_seconds": first_time, "score": score_first}},
              "no_op": noop, "billed_cost_usd": args.billed_cost_usd,
              "billing_source": args.billing_source if args.billed_cost_usd is not None else None}
    if args.mutate:
        mutation = apply_mutations(project, args.target)
        if not mutation["applied"]:
            err(f"No mutation scenario for {args.target}")
        manifest_gold_check(project, gold_path, ["after_mutation"])
        previous_pages = file_hashes(project / "wiki")
        updated, elapsed = subprocess_json(args.lore_binary, project, "update", timeout=args.timeout)
        current_pages = file_hashes(project / "wiki")
        result["after_mutation_corpus_fingerprint"] = corpus_fingerprint(project / "docs")
        result["phases"]["after_mutation"] = {
            "report": updated, "elapsed_seconds": elapsed,
            "score": score_project(project, gold_path, "after_mutation", updated.get("generation")),
            "page_paths_changed": sorted(set(previous_pages) ^ set(current_pages) |
               {p for p in previous_pages.keys() & current_pages.keys()
                if previous_pages[p] != current_pages[p]})}
    (out / "metrics.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    (out / "REVIEW.md").write_text(report_markdown(result))
    return result

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate Lore against pinned Markdown without simulating model quality")
    subs = parser.add_subparsers(dest="command", required=True)
    prep = subs.add_parser("prepare", help="Prepare isolated corpus without inference")
    prep.add_argument("--target", required=True, choices=sorted(load_json(TARGETS)))
    prep.add_argument("--output", required=True, type=Path)
    score = subs.add_parser("score", help="Score an existing Lore-generated evaluation project")
    score.add_argument("--project", type=Path, required=True)
    score.add_argument("--gold", type=Path)
    score.add_argument("--phase", default="initial")
    score.add_argument("--output", type=Path)
    run = subs.add_parser("run", help="Run genuine Lore model inference and score results")
    run.add_argument("--target", required=True, choices=sorted(load_json(TARGETS)))
    run.add_argument("--provider", required=True, choices=["ollama", "openai"])
    run.add_argument("--model", required=True)
    run.add_argument("--decision-provider", choices=["ollama", "openai", "typesafe"])
    run.add_argument("--decision-model")
    run.add_argument("--allow-hosted", action="store_true")
    run.add_argument("--base-url")
    run.add_argument("--generative-base-url", help="Explicit generative endpoint; supports Foundry with a different decision provider")
    run.add_argument("--decision-base-url", help="Explicit decision-provider endpoint")
    add_reasoning_options(run)
    run.add_argument("--output", required=True, type=Path)
    run.add_argument("--lore-binary", default="lore")
    run.add_argument("--mutate", action="store_true")
    run.add_argument("--skip-verification", action="store_true",
                     help="Disable synthesis verification for controlled ablations only")
    run.add_argument("--timeout", type=int, default=3600)
    run.add_argument("--billed-cost-usd", type=float,
                     help="Manually observed billed cost; not inferred from model calls")
    run.add_argument("--billing-source", default="Manually supplied provider billing export")
    return parser.parse_args(argv)

def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.command == "prepare":
            print(json.dumps(prepare(args.target, args.output.absolute()), indent=2))
        elif args.command == "score":
            report = score_project(args.project.absolute(), args.gold, args.phase)
            if args.output:
                args.output.write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report, indent=2))
        elif args.command == "run":
            if not (60 <= args.timeout <= 86400):
                err("--timeout must be 60..86400 seconds")
            if args.billed_cost_usd is not None and args.billed_cost_usd < 0:
                err("Billed cost cannot be negative")
            report = run_command(args)
            print(json.dumps({"output": str(args.output), "target": report["target"],
                              "phases": list(report["phases"]), "no_op": report["no_op"]}, indent=2))
    except (ValueError, FileNotFoundError, subprocess.CalledProcessError,
            subprocess.TimeoutExpired, sqlite3.DatabaseError, OSError) as ex:
        print(f"Evaluation error: {ex}", file=sys.stderr)
        return 1
    return 0

if __name__ == "__main__":
    sys.exit(main())
