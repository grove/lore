"""Source-checkpoint relationship scoring, independent of lifecycle predictions.

A checkpoint denotes a set of source assertions, not an arbitrarily chosen
knowledge unit. Existence is witnessed by an active, source-bound edge;
non-existence is checked over the entire candidate Cartesian product. This is
still a graph/evidence proxy, never a semantic accuracy score.
"""
from __future__ import annotations

from collections import defaultdict
import sqlite3

VERSION = "source-witness-sets-v3"


def identity(candidates: list[dict], invalid: set[str]) -> dict:
    assignments: dict[str, set[str]] = defaultdict(set)
    unmapped = set()
    for row in candidates:
        assertion = row["assertion_id"]
        if row["knowledge_id"] is None:
            unmapped.add(assertion)
        else:
            assignments[assertion].add(row["knowledge_id"])
    ids = sorted({k for values in assignments.values() for k in values})
    broken = sorted(a for a, values in assignments.items() if len(values) != 1)
    bad_evidence = sorted({r["assertion_id"] for r in candidates} & invalid)
    return {
        "knowledge_ids": ids,
        "assertion_ids": sorted(assignments),
        "complete": bool(candidates) and not (unmapped or broken or bad_evidence),
        "unmapped_assertions": sorted(unmapped),
        "ambiguous_assignments": broken,
        "invalid_evidence_assertions": bad_evidence,
    }


def active_edges(conn: sqlite3.Connection) -> list[dict]:
    """Keep witnesses; never reduce the graph to opaque endpoint triples."""
    columns = {r[1] for r in conn.execute("PRAGMA table_info(knowledge_relations)")}
    evidence = "rel.evidence_id" if "evidence_id" in columns else "NULL"
    edges = [dict(row) for row in conn.execute(f"""
        SELECT rel.id, fr.knowledge_id AS from_id, tr.knowledge_id AS to_id,
               rel.relation AS relation, ra.assertion_revision_id AS assertion_id,
               {evidence} AS evidence_id
        FROM knowledge_relations rel
        JOIN knowledge_revisions fr ON fr.id=rel.from_revision_id
        JOIN knowledge_revisions tr ON tr.id=rel.to_revision_id
        JOIN relation_assertions ra ON ra.relation_id=rel.id
        JOIN active_assertions act ON act.assertion_revision_id=ra.assertion_revision_id
    """)]
    if conn.execute("SELECT 1 FROM sqlite_master WHERE name='reaffirmation_links'").fetchone():
        columns = {r[1] for r in conn.execute("PRAGMA table_info(reaffirmation_links)")}
        evidence = "link.evidence_id" if "evidence_id" in columns else "NULL"
        identifier = "link.id" if "id" in columns else "'legacy-reaffirmation'"
        edges += [dict(row) for row in conn.execute(f"""
            SELECT {identifier} AS id, from_unit_id AS from_id,
                   to_unit_id AS to_id, 'reaffirms' AS relation,
                   link.assertion_revision_id AS assertion_id,
                   {evidence} AS evidence_id
            FROM reaffirmation_links link
            JOIN active_assertions act ON act.assertion_revision_id=link.assertion_revision_id
        """)]
    return edges


def grounded_edge(conn: sqlite3.Connection, edge: dict, candidates: list[dict]) -> bool:
    """A witness must belong to a candidate assertion, not merely mention it."""
    candidates = [r for r in candidates if r["assertion_id"] == edge["assertion_id"]
                  and r["knowledge_id"] == edge["from_id"]]
    if not candidates or not edge.get("evidence_id"):
        return False
    evidence = conn.execute("SELECT * FROM evidence_snapshots WHERE id=?",
                            (edge["evidence_id"],)).fetchone()
    if evidence is None:
        return False
    excerpt = evidence["exact_excerpt"]
    if not excerpt or not any(excerpt in r["excerpt"] for r in candidates):
        return False
    if "source_id" in evidence.keys():
        assertion = conn.execute("SELECT * FROM assertion_revisions WHERE id=?",
                                 (edge["assertion_id"],)).fetchone()
        if assertion is None:
            return False
        for field in ("source_id", "source_revision_id"):
            if field in evidence.keys() and field in assertion.keys():
                if evidence[field] != assertion[field]:
                    return False
    return True


def assess(rule: dict, left: dict, right: dict, edges: list[dict],
           witness_is_valid) -> dict:
    """Compute actual without consulting expected. Ambiguous unit identity is
    distinct from a legitimate set of several assertions in one passage."""
    result = {"id": rule["id"], "relation": rule["type"],
              "expected": bool(rule["expected"]), "identity_method": VERSION,
              "actual": None, "assessable": False, "passed": False,
              "from_candidates": left["knowledge_ids"],
              "to_candidates": right["knowledge_ids"], "witnesses": [],
              "reason": "missing_or_ambiguous_source_assignment"}
    if not left["complete"] or not right["complete"]:
        return result
    ls, rs = set(left["knowledge_ids"]), set(right["knowledge_ids"])
    if rule["type"] == "equivalent":
        if len(ls) != 1 or len(rs) != 1:
            result["reason"] = "equivalence_requires_unique_identity"
            return result
        actual = ls == rs
    else:
        observed = [e for e in edges if e["from_id"] in ls and e["to_id"] in rs
                    and e["relation"] == rule["type"]]
        valid = [e for e in observed if witness_is_valid(e)]
        if len(valid) != len(observed):
            result["reason"] = "active_edge_has_missing_or_mismatched_witness"
            return result
        actual = bool(valid)
        result["witnesses"] = [{k: e[k] for k in
                                ("id", "from_id", "to_id", "assertion_id", "evidence_id")}
                               for e in sorted(valid, key=lambda e: (e["from_id"], e["to_id"], e["id"]))]
    result.update(actual=actual, assessable=True,
                  passed=actual == result["expected"],
                  reason="source_bound_witness" if actual else "no_edge_between_candidate_sets")
    return result


def score(conn: sqlite3.Connection, gold: list[dict], rules: list[dict],
          assertions: list[dict], invalid: set[str], phase: str) -> tuple[list[dict], dict]:
    candidates = {g["id"]: [a for a in assertions if a["source"] == g["source"]
                           and g["needle"].casefold().strip() in a["excerpt"].casefold()]
                  for g in gold}
    identities = {key: identity(rows, invalid) for key, rows in candidates.items()}
    empty = identity([], invalid)
    edges = active_edges(conn)
    results = []
    for rule in rules:
        if phase not in rule.get("phases", ["initial"]):
            continue
        rows = candidates.get(rule["from"], [])
        results.append(assess(rule, identities.get(rule["from"], empty),
                              identities.get(rule["to"], empty), edges,
                              lambda e: grounded_edge(conn, e, rows)))
    return results, identities
