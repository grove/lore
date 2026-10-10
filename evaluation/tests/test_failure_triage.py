"""Failure-bound tuning cannot turn fixture outcomes into product evidence."""
import copy
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import failure_triage as triage
import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross


class FailureTriageTests(unittest.TestCase):
    def setUp(self):
        self.diag = {"metrics_sha256": "source-bound", "algorithm_tuning_ready": False, "pilot_case_ids": ["pilot-01", "pilot-passed"],
                     "observed_cases": [{"sample_id": "sample-01", "case_id": "pilot-01"}]}
        self.proposal = {"schema_version": 1, "pilot_metrics_sha256": "source-bound",
            "modules": ["src/context/adaptive.rs"], "failing_samples": ["sample-01"],
            "expected_corrected_behavior": "Retain the observed missing exception.",
            "bounded_change": "Reserve the original condition before optional rationale.",
            "cost_risk": "Measure all investigation calls.", "heldout_plan": "Run the preregistered distinct cases.",
            "heldout_case_ids": ["holdout-01"]}

    def test_diagnostic_proposal_does_not_claim_ready_or_improved(self):
        result = triage.validate_change(self.proposal, self.diag)
        self.assertTrue(result["proposal_complete"])
        self.assertFalse(result["algorithm_tuning_ready"])
        self.assertFalse(result["measured_improvement"])
        self.assertTrue(result["heldout_plan_unverified"])

    def test_distinct_ids_and_independence_flags_do_not_validate_holdout_content(self):
        proposal = dict(self.proposal, heldout_independent=True, heldout_review_complete=True)
        result = triage.validate_change(proposal, dict(self.diag, algorithm_tuning_ready=True))
        self.assertFalse(result["algorithm_tuning_ready"])
        self.assertEqual(result["holdout"]["status"], "heldout_plan_unverified")
        self.assertFalse(result["holdout"]["independent_review_complete"])

    def test_unknown_failure_and_stale_pilot_are_rejected(self):
        for field, value in (("pilot_metrics_sha256", "stale"), ("failing_samples", ["invented"])):
            proposal = dict(self.proposal, **{field: value})
            with self.assertRaises(ValueError): triage.validate_change(proposal, self.diag)

    def test_holdout_leakage_and_speculative_module_are_rejected(self):
        for field, value in (("heldout_case_ids", ["pilot-01"]), ("heldout_case_ids", ["pilot-passed"]), ("modules", ["src/new_agent_framework.rs"])):
            proposal = dict(self.proposal, **{field: value})
            with self.assertRaises(ValueError): triage.validate_change(proposal, self.diag)

    def test_unvalidated_attempt_artifacts_cannot_be_triaged(self):
        with self.assertRaises(ValueError):
            triage.summarize_validated({}, {"mechanical_contracts_passed": False}, Path("."))

    def test_missing_usage_and_fixture_failures_remain_unknown(self):
        report = {"samples": [{"sample_id": "sample-01", "case_id": "pilot-01", "project": "p",
            "setup": "baseline", "source_manifest": {"sha256": "bound"}, "context": None,
            "tests": {"checks": [{"kind": "constraint", "passed": False}]},
            "costs": {"total_seconds": 1.0, "total_usage": {"input_tokens": None, "billed_cost_usd": None}}}]}
        with tempfile.TemporaryDirectory() as root:
            result = triage.summarize_validated(report, {"mechanical_contracts_passed": True, "fixture_only": True}, Path(root))
        self.assertEqual(result["classification"], "diagnostic_only")
        self.assertFalse(result["algorithm_tuning_ready"])
        self.assertIsNone(result["observed_cases"][0]["total_usage"]["billed_cost_usd"])
        self.assertIn("independently_checked_constraint_failure", result["observed_cases"][0]["classes"])

    def source_manifest(self, body="pilot source", name="pilot.py"):
        files = {name: hashlib.sha256(body.encode()).hexdigest()}
        return {"sha256": cross.digest(files), "files_sha256": files, "file_count": len(files)}

    def pilot_entry(self):
        return {"case": {"id": "pilot-01", "project": "pilot-project",
                "task": "Preserve the original pilot scope condition.", "editable_files": ["pilot.py"],
                "critical_constraints": ["Deny an unknown pilot scope."]},
                "source_manifest": self.source_manifest()}

    def bound_diagnostics(self, entry=None):
        entry = copy.deepcopy(entry or self.pilot_entry())
        entry["case"].update(id="pilot-01", project="pilot-project")
        report = {"cases": [entry], "samples": [{"project": "pilot-project", "case_id": "pilot-01",
                   "source_manifest": entry["source_manifest"]}]}
        return dict(self.diag, algorithm_tuning_ready=True,
                    pilot_task_bindings=triage.pilot_task_bindings(report))

    def prepared_holdout(self, root, *, pinned=True):
        source = root / "source"
        source.mkdir()
        (source / "logic.py").write_text("def eligible(value):\n    return value == 'allowed'\n")
        checker = root / "checker.py"
        # Preparation and validation may hash this checker, but must not run it.
        checker.write_text("raise RuntimeError('Holdout outcomes must stay unopened')\n")
        manifest = cross.fingerprint(source)
        case = {"id": "holdout-01", "project": "holdout-project", "task": "Preserve the separate eligibility condition.",
                "source_root": "source", "editable_files": ["logic.py"],
                "critical_constraints": ["Deny an unknown eligibility value."],
                "test_command": [sys.executable, str(checker)], "input_sha256": manifest["sha256"]}
        if pinned:
            case["upstream"] = {"repository": "protocol-test/holdout", "commit": "a" * 40,
                                "files_sha256": manifest["files_sha256"]}
        cases = root / "cases.json"
        cross.write_json(cases, {"schema_version": 1, "fixture_only": False, "held_out": True,
                                 "independent_projects": False, "cases": [case]})
        output = root / "prepared"
        prepared = coding.prepare(output, cases)
        return output, prepared

    def test_prepared_holdout_binds_actual_content_without_reading_outcomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output, prepared = self.prepared_holdout(root)
            (output / "metrics.json").write_text("Deliberately invalid, unopened outcome data")
            result = triage.validate_change(self.proposal, self.bound_diagnostics(), output)
            self.assertTrue(result["algorithm_tuning_ready"])
            self.assertFalse(result["heldout_plan_unverified"])
            self.assertEqual(result["holdout"]["prepared_sha256"], coding.hash_file(output / "prepared.json"))
            self.assertEqual(result["holdout"]["cases"][0]["source_sha256"], prepared["cases"][0]["source_manifest"]["sha256"])
            self.assertFalse(result["holdout"]["independent_review_complete"])
            self.assertFalse(result["holdout"]["outcomes_read"])
            self.assertFalse(result["measured_improvement"])

    def test_renamed_pilot_task_or_source_never_becomes_a_holdout(self):
        for mutation in ("case_id_only", "same_task_new_source", "same_bytes_renamed_path"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                output, prepared = self.prepared_holdout(Path(temporary).resolve())
                pilot = copy.deepcopy(prepared["cases"][0])
                if mutation == "same_task_new_source":
                    pilot["source_manifest"] = self.source_manifest("different pilot source", "logic.py")
                elif mutation == "same_bytes_renamed_path":
                    files = pilot["source_manifest"]["files_sha256"]
                    renamed = {"renamed.py": next(iter(files.values()))}
                    pilot["source_manifest"] = {"sha256": cross.digest(renamed), "files_sha256": renamed, "file_count": 1}
                    pilot["case"].update(task="A different description of the same source.", editable_files=["renamed.py"])
                with self.assertRaisesRegex(ValueError, "Renaming a pilot task"):
                    triage.validate_change(self.proposal, self.bound_diagnostics(pilot), output)

    def test_prepared_holdout_rejects_changed_snapshot_checker_or_manifest(self):
        for mutation in ("snapshot", "checker", "manifest"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                output, prepared = self.prepared_holdout(root)
                path = {"snapshot": output / prepared["cases"][0]["source_root"] / "logic.py",
                        "checker": root / "checker.py", "manifest": root / "cases.json"}[mutation]
                path.write_text(path.read_text() + "\nchanged after preparation\n")
                with self.assertRaises(ValueError):
                    triage.validate_change(self.proposal, self.bound_diagnostics(), output)

    def test_unpinned_or_unknown_holdout_and_unbound_pilot_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            output, _ = self.prepared_holdout(Path(temporary).resolve(), pinned=False)
            with self.assertRaisesRegex(ValueError, "exact upstream pin"):
                triage.validate_change(self.proposal, self.bound_diagnostics(), output)
        with tempfile.TemporaryDirectory() as temporary:
            output, _ = self.prepared_holdout(Path(temporary).resolve())
            with self.assertRaisesRegex(ValueError, "absent from the prepared cohort"):
                triage.validate_change(dict(self.proposal, heldout_case_ids=["invented"]), self.bound_diagnostics(), output)
            with self.assertRaisesRegex(ValueError, "Content-bound pilot tasks"):
                triage.validate_change(self.proposal, self.diag, output)

    def reviewed_report(self, root, *, independent=False):
        entry = self.pilot_entry()
        sample = {"sample_id": "sample-01", "case_id": "pilot-01", "project": "pilot-project",
                  "setup": "baseline", "source_manifest": entry["source_manifest"], "context": None,
                  "task": entry["case"]["task"], "constraints": entry["case"]["critical_constraints"],
                  "answer_sha256": "b" * 64,
                  "tests": {"checks": [{"kind": "constraint", "passed": False}]},
                  "costs": {"total_seconds": 1.0, "total_usage": {"billed_cost_usd": None}}}
        report = {"samples": [sample], "cases": [entry]}
        packet = adaptive.review_template(sample, cross.digest(report))
        for index, annotation in enumerate(packet["assessments"], 1):
            annotation.update(complete=True, reviewer=f"reader-{index}", reviewed_at="2026-10-10T00:00:00Z",
                              blind_confirmed=True, notes="Synthetic protocol annotation, not a study outcome.",
                              material_decision_mistakes=["The documented condition was missed."],
                              missed_critical_constraints=sample["constraints"], severe_unsupported_project_assertions=[],
                              unnecessary_blocking=False, incorrect_hypotheses_identified=0,
                              revised_incorrect_hypotheses=0, implementation_quality_0_to_3=1)
            if independent:
                capture = root / f"review-{index}.txt"
                capture.write_text(f"Synthetic separately bound review artifact {index}")
                packet["independence_evidence"].append({"reviewer_id": f"reader-{index}",
                    "complete": True, "independent": True, "blind_confirmed": True, "conflicts_declared": [],
                    "target_sha256": packet["independence_target_sha256"], "reviewed_at": "2026-10-10T00:00:00Z",
                    "capture": {"path": capture.name, "sha256": coding.hash_file(capture)}})
        cross.write_json(root / "reviews" / "sample-01.json", packet)
        assessment = {"mechanical_contracts_passed": True, "fixture_only": False,
                      "independent_validation_complete": True, "integrity_audit": {"passed": True}}
        return report, assessment, packet

    def test_named_annotations_are_not_counted_as_independent_reviews(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            report, assessment, _ = self.reviewed_report(root)
            result = triage.summarize_validated(report, assessment, root)
            self.assertEqual(result["annotation_count"], 2)
            self.assertEqual(result["independent_review_count"], 0)
            self.assertEqual(result["observed_cases"][0]["annotation_count"], 2)
            self.assertFalse(result["independent_reviews_complete"])
            self.assertFalse(result["algorithm_tuning_ready"])
            self.assertFalse(any(name.startswith("reviewed_") for name in result["observed_cases"][0]["classes"]))

    def test_independent_review_counts_require_bound_captures_and_matching_authors(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            report, assessment, packet = self.reviewed_report(root, independent=True)
            result = triage.summarize_validated(report, assessment, root)
            self.assertEqual(result["independent_review_count"], 2)
            self.assertEqual(result["observed_cases"][0]["independent_review_count"], 2)
            self.assertTrue(result["independent_reviews_complete"])
            self.assertIn("reviewed_material_decision_mistakes", result["observed_cases"][0]["classes"])
            packet["independence_evidence"][0]["reviewer_id"] = "unrelated-reader"
            cross.write_json(root / "reviews" / "sample-01.json", packet)
            with self.assertRaisesRegex(ValueError, "annotation reviewers"):
                triage.summarize_validated(report, assessment, root)

    def test_tampered_or_reused_review_captures_are_rejected(self):
        for mutation in ("capture", "target", "reused"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                report, assessment, packet = self.reviewed_report(root, independent=True)
                if mutation == "capture":
                    (root / "review-1.txt").write_text("altered evidence")
                elif mutation == "target":
                    packet["independence_target_sha256"] = "c" * 64
                else:
                    packet["independence_evidence"][1]["capture"] = packet["independence_evidence"][0]["capture"]
                cross.write_json(root / "reviews" / "sample-01.json", packet)
                with self.assertRaises(ValueError):
                    triage.summarize_validated(report, assessment, root)

    def test_operator_task_and_checker_authors_cannot_supply_independent_reviews(self):
        for role in ("operator_id", "task_authors", "checker_authors"):
            with self.subTest(role=role), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                report, assessment, _ = self.reviewed_report(root, independent=True)
                registration = {"operator_id": "operator", "cases": [{"task_authors": [], "checker_authors": []}]}
                if role == "operator_id":
                    registration[role] = "reader-1"
                else:
                    registration["cases"][0][role] = ["reader-1"]
                cross.write_json(root / "PREREGISTRATION.json", registration)
                with self.assertRaisesRegex(ValueError, "independence"):
                    triage.summarize_validated(report, assessment, root)


if __name__ == "__main__":
    unittest.main()
