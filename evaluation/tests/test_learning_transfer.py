"""Real fixture verification, independent of a learner or agent's explanation."""
from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_tasks as coding
import cross_source as cross
import shared_intelligence as shared

CASES = Path(__file__).resolve().parents[1] / "corpora" / "shared-intelligence" / "cases.json"
CORRECT = '''def refund_or_reconcile(send, lookup, request_token):
    try:
        return send(request_token)
    except TransientError:
        receipt = lookup(request_token)
        if receipt is None:
            raise ReconciliationRequired(request_token)
        return receipt
'''
COPIED_RETRY = '''def refund_or_reconcile(send, lookup, request_token):
    for attempt in range(6):
        try:
            return send(request_token)
        except TransientError:
            if attempt == 5:
                raise
'''


def setup(root: Path):
    report = shared.prepare(root / "prepared", CASES)
    entry = next(item for item in report["cases"] if item["case"]["id"] == "payments-refund-reconciliation")
    workspace = root / "prepared" / entry["source_root"]
    return report, entry, workspace


def implement(workspace: Path, function: str):
    path = workspace / "src" / "refund_adapter.py"
    header = path.read_text(encoding="utf-8").split("def refund_or_reconcile", 1)[0]
    path.write_text(header + function, encoding="utf-8")


class LearningTransferTests(unittest.TestCase):
    def test_transfer_is_a_distinct_same_project_task_without_solution_or_check_leakage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            report, entry, workspace = setup(root)
            original = coding.load_cases(CASES)
            journey = original["learning_journeys"][0]
            self.assertNotEqual(journey["first_case"], journey["transfer_case"])
            first, transfer = original["cases"]
            self.assertEqual(first["project"], transfer["project"])
            self.assertNotEqual(first["editable_files"], transfer["editable_files"])
            first_root = root / "prepared" / report["cases"][0]["source_root"]
            self.assertNotIn("refund_adapter.py", json.dumps(cross.fingerprint(first_root)))
            for snapshot in (first_root, workspace):
                paths = cross.fingerprint(snapshot)["files_sha256"]
                self.assertFalse(any("check_transfer" in path or "check_tasks" in path or "test_learning_transfer" in path for path in paths))
            self.assertTrue(report["fixture_only"])
            self.assertFalse(report["held_out"])
            checks, _ = coding.execute_checks(entry["case"], workspace, CASES.parent, 10)
            self.assertTrue(checks["checks"])
            self.assertTrue(all(not item["passed"] for item in checks["checks"]))

    def test_external_checks_accept_scoped_reconciliation_but_reject_copied_retry(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, entry, workspace = setup(Path(temporary).resolve())
            implement(workspace, CORRECT)
            correct, _ = coding.execute_checks(entry["case"], workspace, CASES.parent, 10)
            self.assertEqual(len(correct["checks"]), 5)
            self.assertTrue(all(item["passed"] for item in correct["checks"]), correct)
            self.assertTrue(correct["provenance"]["files_sha256"])
            implement(workspace, COPIED_RETRY)
            unsafe, _ = coding.execute_checks(entry["case"], workspace, CASES.parent, 10)
            by_id = {item["id"]: item for item in unsafe["checks"]}
            self.assertFalse(by_id["ambiguous_submission_looks_up_once_without_resending"]["passed"])
            self.assertFalse(by_id["unknown_outcome_requires_reconciliation"]["passed"])

    def test_rare_false_valued_receipt_and_unavailable_lookup_remain_material(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, entry, workspace = setup(Path(temporary).resolve())
            implement(workspace, CORRECT.replace("if receipt is None:", "if not receipt:"))
            checks, _ = coding.execute_checks(entry["case"], workspace, CASES.parent, 10)
            by_id = {item["id"]: item for item in checks["checks"]}
            self.assertFalse(by_id["ambiguous_submission_looks_up_once_without_resending"]["passed"])
            self.assertTrue(by_id["unavailable_status_is_not_negative_evidence"]["passed"])

    def test_relabeling_public_transfer_corpus_does_not_establish_independence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            report, _, _ = setup(root)
            manifest = coding.load_cases(CASES)
            manifest.update(fixture_only=False, held_out=True, independent_projects=True)
            # Keeping a bundled base is direct provenance; an external snapshot
            # of the known first-task bytes is also recognized by the existing
            # fingerprint mechanism.
            for case in manifest["cases"]:
                case["overlay"] = str((CASES.parent / case["overlay"]).resolve())
            path = root / "relabeled.json"
            cross.write_json(path, manifest)
            relabeled = coding.prepare(root / "relabeled", path)
            self.assertTrue(relabeled["fixture_only"])
            self.assertFalse(relabeled["held_out"])
            self.assertFalse(relabeled["independent_projects"])
            # Also recognize just the new transfer bytes through a manifest
            # which has removed every explicit bundled-source marker.
            transfer = dict(manifest["cases"][1])
            transfer.pop("base_project")
            transfer.pop("overlay")
            transfer["source_root"] = str(root / "prepared" / report["cases"][1]["source_root"])
            manifest["cases"] = [transfer]
            cross.write_json(path, manifest)
            copied = coding.prepare(root / "copied", path)
            self.assertTrue(copied["fixture_only"])
            self.assertEqual(len(copied["recognized_fixture_fingerprints"]), 1)


if __name__ == "__main__":
    unittest.main()
