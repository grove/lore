"""Integrity and holdout exclusion for public, author-labeled development controls."""
import hashlib
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_tasks as coding
import cross_source as cross

CORPUS = Path(__file__).resolve().parents[1] / "corpora" / "real-decisions-v1"


class RealDecisionCorpusTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((CORPUS / "manifest.json").read_text(encoding="utf-8"))

    def passage(self, project, passage):
        path = CORPUS / project["source_root"] / coding.relative_file(passage["source"])
        lines = path.read_bytes().decode("utf-8").splitlines(keepends=True)
        start, end = passage["lines"]
        self.assertTrue(1 <= start <= end <= len(lines))
        quote = "".join(lines[start - 1:end]).rstrip("\n")
        self.assertEqual(quote, passage["quote"])
        self.assertEqual(end, start + quote.count("\n"))
        self.assertEqual(hashlib.sha256(quote.encode()).hexdigest(), passage["sha256"])

    def test_complete_source_pins_and_documentary_status_witnesses(self):
        manifest = self.manifest
        self.assertEqual(manifest["schema_version"], 1)
        self.assertTrue(manifest["fixture_only"])
        self.assertFalse(manifest["held_out"])
        self.assertFalse(manifest["independent_labels"])
        self.assertGreaterEqual(len(manifest["projects"]), 2)
        for project in manifest["projects"].values():
            self.assertRegex(project["revision"], r"^[0-9a-f]{40}$")
            self.assertTrue(project["repository"].startswith("https://github.com/"))
            self.assertTrue(project["license"])
            actual = cross.fingerprint(CORPUS / project["source_root"])
            self.assertEqual(actual["files_sha256"], project["files_sha256"])
        ids = {r["id"] for r in manifest["records"]}
        self.assertEqual(len(ids), len(manifest["records"]))
        for record in manifest["records"]:
            project = manifest["projects"][record["project"]]
            self.passage(project, record)
            self.passage(project, record["status_evidence"])
            self.assertTrue(record["scope"])
            witness = record["status_evidence"]["quote"]
            if record["lifecycle"] in ("accepted", "rejected", "superseded"):
                status = {"accepted": "Final", "rejected": "Rejected", "superseded": "Superseded"}[record["lifecycle"]]
                self.assertRegex(witness, rf"(?m)^Status: {re.escape(status)}$")
        for relation in manifest["relations"]:
            self.assertTrue({relation["from"], relation["to"], relation["witness"]} <= ids)
            witness = next(r for r in manifest["records"] if r["id"] == relation["witness"])
            self.assertIn("Replaces: 622", witness["quote"])
        self.assertEqual(manifest["explorations"], [])
        self.assertTrue(manifest["excluded_uncertain_cases"])
        self.assertTrue(manifest["review"]["separate_human_review"].startswith("pending"))

    def test_cases_bind_required_conditions_negatives_and_unknowns(self):
        manifest = self.manifest
        self.assertGreaterEqual(len(manifest["cases"]), 6)
        self.assertGreaterEqual(len({c["project"] for c in manifest["cases"]}), 2)
        ids = set()
        for case in manifest["cases"]:
            self.assertNotIn(case["id"], ids)
            ids.add(case["id"])
            self.assertTrue(case["task"])
            self.assertTrue(case["required"])
            self.assertTrue(case["included_conditions"])
            self.assertTrue(case["negative_cases"])
            self.assertTrue(case["must_remain_unknown"])
            project_records = {r["id"] for r in manifest["records"] if r["project"] == case["project"]}
            self.assertLessEqual(set(case["required"]), project_records)
        self.assertGreaterEqual(len(manifest["negative_queries"]), 2)

    def test_development_snapshots_cannot_be_relabelled_as_independent_holdout(self):
        known = coding.bundled_fingerprints() & cross.bundled_fingerprints()
        for project in self.manifest["projects"].values():
            self.assertIn(cross.fingerprint(CORPUS / project["source_root"])["sha256"], known)
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp).resolve()
                source = CORPUS / project["source_root"]
                cases = {"schema_version": 1, "fixture_only": False, "held_out": True,
                         "independent_projects": True, "cases": [{
                             "id": "renamed", "project": "unseen", "task": "Relabeled task",
                             "source_root": str(source),
                             "editable_files": [next(iter(project["files_sha256"]))],
                             "critical_constraints": ["Retain documentary qualifications"],
                             "test_command": [sys.executable, "-c", "raise SystemExit(0)"],
                         }]}
                cross.write_json(root / "cases.json", cases)
                prepared = coding.prepare(root / "prepared", root / "cases.json")
                self.assertTrue(prepared["fixture_only"])
                self.assertFalse(prepared["held_out"])
                self.assertFalse(prepared["independent_projects"])


if __name__ == "__main__":
    unittest.main()
