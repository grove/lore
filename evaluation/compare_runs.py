#!/usr/bin/env python3
"""Compare independent Lore evaluation runs without invoking inference.

Inputs and model identities must match before interpreting output variation.
All textual fingerprints are lexical proxies, not semantic correctness scores.
"""
from __future__ import annotations
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import sqlite3
from typing import Any


def tree_fingerprint(root: Path) -> tuple[str, int]:
    if not root.is_dir():
        raise ValueError(f"Missing source corpus: {root}")
    files = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(root.rglob("*")) if p.is_file() and not p.is_symlink()
             and p.suffix.lower() in (".md", ".markdown")}
    canonical = json.dumps(files, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest(), len(files)


def jaccard(a: set[str], b: set[str]) -> float:
    return round(len(a & b) / len(a | b), 3) if a or b else 1.0


def snapshot(run: Path) -> dict[str, Any]:
    run = run.resolve()
    project = run / "project" if (run / "project").is_dir() else run
    metrics_path = run / "metrics.json"
    metrics = json.loads(metrics_path.read_text("utf-8")) if metrics_path.exists() else {}
    doc_digest, file_count = tree_fingerprint(project / "docs")
    db_path = project / ".lore" / "state.db"
    if not db_path.is_file():
        raise ValueError(f"No completed Lore SQLite baseline: {db_path}")
    conn = sqlite3.connect(db_path.as_uri() + "?mode=ro", uri=True)
    try:
        statements = {" ".join(x[0].casefold().split()) for x in conn.execute("""
            SELECT r.statement FROM knowledge_current c
            JOIN knowledge_revisions r ON r.id=c.revision_id
        """)}
        records = conn.execute("""
            SELECT model,provider,COUNT(*) FROM model_calls
            WHERE cache_hit=0 GROUP BY model,provider
        """).fetchall()
    finally:
        conn.close()
    topics_dir = project / "wiki" / "topics"
    topics = {p.stem for p in topics_dir.glob("*.md")} if topics_dir.exists() else set()
    assert len(statements) >= 0
    settings=(metrics.get("provider"),metrics.get("model"),
              metrics.get("decision_provider"),metrics.get("decision_model"))
    reasoning=metrics.get("reasoning")
    reasoning_canonical=(json.dumps(reasoning, sort_keys=True)
                         if isinstance(reasoning, dict) else None)
    return {
        "run": str(run), "source_sha256": doc_digest,
        "source_documents": file_count, "provider_settings": list(settings),
        "reasoning": reasoning, "reasoning_fingerprint": reasoning_canonical,
        "binary_sha256": metrics.get("lore_binary_sha256"),
        "configuration_sha256": metrics.get("configuration_sha256"),
        "rubric": metrics.get("rubric"),
        "observed_model_pairs": sorted([list(row) for row in records]),
        "topics": sorted(topics), "topic_count": len(topics),
        "knowledge_count": len(statements),
        "_topic_set": topics, "_statement_set": statements,
    }


def compare(a: dict, b: dict) -> dict:
    reasons = []
    if a["source_sha256"] != b["source_sha256"]:
        reasons.append("Different source corpus (possibly an evolved mutation phase)")
    if a["provider_settings"] != b["provider_settings"] or None in a["provider_settings"][:2]:
        reasons.append("Different or unspecified provider/model settings")
    if a["reasoning_fingerprint"] is None or b["reasoning_fingerprint"] is None:
        reasons.append("Reasoning configuration was not recorded")
    elif a["reasoning_fingerprint"] != b["reasoning_fingerprint"]:
        reasons.append("Different per-task reasoning efforts")
    if a["binary_sha256"] is None or b["binary_sha256"] is None:
        reasons.append("At least one Lore binary fingerprint was not recorded")
    elif a["binary_sha256"] != b["binary_sha256"]:
        reasons.append("Different Lore binary builds")
    if a.get("configuration_sha256") != b.get("configuration_sha256"):
        reasons.append("Different effective provider/pipeline configurations")
    if a.get("rubric") != b.get("rubric"):
        reasons.append("Different evaluation rubrics; label scores are not comparable")
    models_a={(m,p) for m,p,_ in a["observed_model_pairs"]}
    models_b={(m,p) for m,p,_ in b["observed_model_pairs"]}
    if models_a and models_b and models_a != models_b:
        reasons.append("Different observed provider/model response identities")
    pair = {"left": a["run"], "right": b["run"],
            "same_source": a["source_sha256"]==b["source_sha256"],
            "strictly_comparable": not reasons, "limitations": reasons}
    if pair["same_source"]:
        pair["topic_jaccard"] = jaccard(a["_topic_set"],b["_topic_set"])
        pair["statement_lexical_jaccard"] = jaccard(
            a["_statement_set"],b["_statement_set"])
        pair["topic_count_delta"]=b["topic_count"]-a["topic_count"]
        pair["knowledge_count_delta"]=b["knowledge_count"]-a["knowledge_count"]
    return pair


def summarize(runs: list[Path]) -> dict:
    if len(runs) < 2:
        raise ValueError("Provide at least two independent evaluation directories")
    shots=[snapshot(path) for path in runs]
    pairs=[compare(a,b) for a,b in itertools.combinations(shots,2)]
    for shot in shots:
        shot.pop("_topic_set");shot.pop("_statement_set")
    return {
        "schema_version": 1,
        "runs": shots, "comparisons": pairs,
        "strictly_comparable_pairs": sum(x["strictly_comparable"] for x in pairs),
        "interpretation": ("Exact-topic and normalized-statement Jaccard values "
            "measure lexical stability only, not correctness. Differences between "
            "mutated corpora or model/binary versions are not repeatability evidence. "
            "Unknown resolved model revisions remain a comparability limitation.")
    }


def main(argv: list[str] | None = None) -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs",nargs="+",type=Path)
    parser.add_argument("--output",type=Path)
    args=parser.parse_args(argv)
    try:
        result=summarize(args.runs)
        serialized=json.dumps(result,indent=2,sort_keys=True)+"\n"
        if args.output:
            if args.output.exists():
                raise ValueError("Refusing to replace existing report")
            args.output.parent.mkdir(parents=True,exist_ok=True)
            args.output.write_text(serialized,encoding="utf-8")
        print(serialized,end="")
    except (ValueError, OSError, sqlite3.Error, json.JSONDecodeError) as ex:
        parser.error(str(ex))
    return 0


if __name__=="__main__":
    raise SystemExit(main())
