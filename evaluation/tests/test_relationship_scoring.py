"""Generic relationship witness and candidate-set regressions; no inference."""
from pathlib import Path
import sys
import sqlite3
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import relationship_scoring as scoring


def row(assertion, unit):
    return {"assertion_id": assertion, "knowledge_id": unit}


def edge(source="review", target="policy", assertion="a-review", evidence="ev"):
    return {"id": "link", "from_id": source, "to_id": target,
            "relation": "reaffirms", "assertion_id": assertion, "evidence_id": evidence}


class RelationshipScoringTests(unittest.TestCase):
    def test_multiple_assertions_are_not_ambiguous_assignments(self):
        left = scoring.identity([row("a-review", "review"), row("question", "question")], set())
        right = scoring.identity([row("adopted", "policy"), row("constraint", "rule")], set())
        rule = {"id": "review", "type": "reaffirms", "expected": True}
        result = scoring.assess(rule, left, right, [edge()], lambda e: True)
        self.assertTrue(result["passed"])
        self.assertEqual(len(result["witnesses"]), 1)
        self.assertEqual(result["from_candidates"], ["question", "review"])

    def test_expected_does_not_change_actual(self):
        left = scoring.identity([row("a-review", "review")], set())
        right = scoring.identity([row("adopted", "policy"), row("constraint", "rule")], set())
        for expected in (False, True):
            rule = {"id": "rule", "type": "reaffirms", "expected": expected}
            observed = scoring.assess(rule, left, right, [edge()], lambda e: True)
            absent = scoring.assess(rule, left, right, [], lambda e: True)
            self.assertIs(observed["actual"], True)
            self.assertIs(absent["actual"], False)
            self.assertEqual(observed["passed"], expected)
            self.assertEqual(absent["passed"], not expected)

    def test_unmapped_or_multiply_assigned_assertion_fails_closed(self):
        for rows in ([], [row("x", None)], [row("x", "a"), row("x", "b")]):
            self.assertFalse(scoring.identity(rows, set())["complete"])
        self.assertFalse(scoring.identity([row("x", "a")], {"x"})["complete"])
        self.assertTrue(scoring.identity([row("x", "a"), row("x", "a")], set())["complete"])

    def test_invalid_witness_is_not_treated_as_no_edge(self):
        left = scoring.identity([row("a-review", "review")], set())
        right = scoring.identity([row("adopted", "policy")], set())
        for expected in (False, True):
            result = scoring.assess({"id": "r", "type": "reaffirms", "expected": expected},
                                    left, right, [edge()], lambda e: False)
            self.assertFalse(result["assessable"])
            self.assertFalse(result["passed"])
            self.assertIsNone(result["actual"])

    def test_non_candidate_edge_cannot_resolve_missing_identity(self):
        right = scoring.identity([row("adopted", "policy")], set())
        missing = scoring.identity([], set())
        result = scoring.assess({"id": "r", "type": "reaffirms", "expected": False},
                                missing, right, [edge()], lambda e: True)
        self.assertFalse(result["assessable"])
        self.assertFalse(result["passed"])

    def test_equivalence_does_not_use_arbitrary_set_intersection(self):
        left = scoring.identity([row("one", "a"), row("two", "b")], set())
        right = scoring.identity([row("three", "a")], set())
        result = scoring.assess({"id": "r", "type": "equivalent", "expected": True},
                                left, right, [], lambda e: True)
        self.assertFalse(result["assessable"])

    def test_witness_must_be_the_same_source_revision_and_part_of_its_assertion(self):
        conn = sqlite3.connect(":memory:")
        conn.row_factory = sqlite3.Row
        conn.executescript("""
            CREATE TABLE assertion_revisions(id TEXT, source_id TEXT, source_revision_id TEXT);
            CREATE TABLE evidence_snapshots(id TEXT, exact_excerpt TEXT, source_id TEXT, source_revision_id TEXT);
            INSERT INTO assertion_revisions VALUES('a-review','minutes','v1');
            INSERT INTO evidence_snapshots VALUES('ev','reaffirmed the policy','minutes','v1');
        """)
        candidates = [{**row("a-review", "review"), "excerpt": "The committee reaffirmed the policy at its meeting."}]
        self.assertTrue(scoring.grounded_edge(conn, edge(), candidates))
        for field, value in (("source_id", "another-source"), ("source_revision_id", "v2"),
                             ("exact_excerpt", "A quote absent from the candidate passage")):
            conn.execute(f"UPDATE evidence_snapshots SET {field}=? WHERE id='ev'", (value,))
            self.assertFalse(scoring.grounded_edge(conn, edge(), candidates))
            conn.execute("UPDATE evidence_snapshots SET source_id='minutes',source_revision_id='v1',exact_excerpt='reaffirmed the policy'")
        self.assertFalse(scoring.grounded_edge(conn, edge(assertion="different-assertion"), candidates))
        self.assertFalse(scoring.grounded_edge(conn, edge(evidence=None), candidates))
        conn.close()

    def test_negative_expectation_checks_every_candidate_pair(self):
        left = scoring.identity([row("a-review", "review"), row("question", "question")], set())
        right = scoring.identity([row("adopted", "policy"), row("constraint", "rule")], set())
        observed = [edge(source="question", target="rule", assertion="question")]
        result = scoring.assess({"id":"r", "type":"reaffirms", "expected":False},
                                left, right, observed, lambda e: True)
        self.assertTrue(result["assessable"])
        self.assertTrue(result["actual"])
        self.assertFalse(result["passed"])

    def test_candidate_order_does_not_pick_a_convenient_identity(self):
        candidates = [row("a-review", "review"), row("question", "question")]
        self.assertEqual(scoring.identity(candidates, set()),
                         scoring.identity(list(reversed(candidates)), set()))


if __name__ == "__main__":
    unittest.main()
