"""Offline operator/report contracts, never empirical observations or authorization."""
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import experiment_08 as experiment
import provider_usage as metering
import study_09 as study
import test_experiment_08 as fixtures


def execution_fixture(bundle):
    checked = experiment.preflight(bundle)
    assert checked["preflight_passed"], checked["blockers"]
    experiment.private_write(bundle / "PREFLIGHT_AT_LAUNCH.json", checked)
    prepared = experiment.read(bundle / "prepared.json")
    execution = {"schema_version": 1, "protocol": experiment.PROTOCOL, "status": "cancelled",
        "started_at": fixtures.utc(), "elapsed_seconds": 12.5,
        "preflight_sha256": coding.hash_file(bundle / "PREFLIGHT_AT_LAUNCH.json"),
        "readiness_sha256": checked["readiness_sha256"], "seal_sha256": coding.hash_file(bundle / "SEAL.json"),
        "contract_sha256": checked["contract_sha256"],
        "planned_assignments": [{"case_id": entry["case"]["id"], "arm": arm}
            for entry in prepared["cases"] for arm in adaptive.SETUPS]}
    experiment.private_write(bundle / "EXECUTION.json", execution)
    for index, state in enumerate(("refused", "timed_out", "cancelled", "started", "transport_error")):
        entry = prepared["cases"][0]
        sample = "offline-" + str(index)
        root = bundle / "run"
        experiment.private_write(root / "sample-starts" / (sample + ".json"), {
            "sample_id": sample, "case_id": entry["case"]["id"], "setup": adaptive.SETUPS[index],
            "context_usage": metering.no_inference(), "preparation_usage": metering.no_inference()})
        invocation = root / "attempt-invocations" / sample / "1.json"
        invocation.parent.mkdir(parents=True, exist_ok=True)
        recorder = metering.Recorder(str(invocation.with_name("1-provider-usage.json")))
        recorder.events = [{"id": sample, "call_id": sample, "operation": "coding", "provider": "offline",
            "model": "no-model", "attempt": 1, "elapsed_ms": 3, "http_status": None,
            "provider_request_count": None, "input_tokens": None, "output_tokens": None, "total_tokens": None,
            "billed_cost_usd": None, "billing_source": None, "status": state, "cache_hit": False}]
        recorder.save("running" if state == "started" else "failed")
        experiment.private_write(invocation, {"number": 1, "status": "started" if state == "started" else "incomplete",
            "stage": "agent", "elapsed_seconds": 0.1, "usage": metering.normalized(None)})
    return execution


class StudyWorkflowTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "posix", "Sealed intake fixture uses supported POSIX boundary")
    def test_report_uses_assessor_launch_binding_when_current_readiness_has_expired(self):
        import test_codex_protocol as codex_fixtures
        with tempfile.TemporaryDirectory() as temporary:
            bundle, _ = codex_fixtures.CodexProtocolTests().bundle(Path(temporary).resolve())
            execution_fixture(bundle)
            # An assessor protocol double isolates report policy, never supplies study evidence.
            assessed = experiment.assess(bundle, persist=False)
            assessed["mechanical_comparison_complete"] = True
            assessed["comparison"]["independent_validation_complete"] = True
            current = study.status(bundle)
            current.update(preflight_passed=False, pilot="blocked_external",
                           blockers=[{"gate": "external_provider_runtime_readiness", "passed": False}])
            with patch.object(study, "status", return_value=current), patch.object(experiment, "assess", return_value=assessed), patch("subprocess.Popen", side_effect=AssertionError("historical report executed")):
                value = study.report(bundle)
            self.assertTrue(value["integrity_qualified"])
            self.assertEqual(value["conclusion"], "not_yet_measurable")
            self.assertFalse(value["productivity_benefit_established"])

    def test_unrun_prepare_report_is_read_only_and_keeps_36_unknown_outcomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            inputs = root / "inputs"
            fixtures.real.prepare(inputs, pilot=True)
            with patch("subprocess.Popen", side_effect=AssertionError("prepare/report launched child")):
                study.prepare(inputs / "cases.json", root / "study")
                before = cross.fingerprint(root)
                value = study.report(root / "study")
                self.assertEqual(cross.fingerprint(root), before)
            self.assertEqual(value["status"], "not_run")
            self.assertEqual(value["initial_assignments"], 36)
            self.assertEqual(value["attempted_assignments"], 0)
            self.assertEqual(value["not_started_assignments"], 36)
            self.assertEqual(value["coding_productivity"], "unmeasured")
            self.assertIsNone(value["captured_provider_usage"]["billed_cost_usd"])
            self.assertEqual(value["billing_status"], "budget_unverifiable")
            self.assertIn("0/36", study.markdown(value))

    @unittest.skipUnless(os.name == "posix", "Ready protocol fixture uses supported private POSIX boundary")
    def test_five_of_36_refused_timeout_interrupted_are_retained_without_process_or_write(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            bundle = fixtures.StudyPreflightTests().bundle(root)
            execution_fixture(bundle)
            before = cross.fingerprint(root)
            with patch("subprocess.Popen", side_effect=AssertionError("report launched child")):
                value = study.report(bundle)
            self.assertEqual(value["status"], "retained_execution")
            self.assertEqual(value["initial_assignments"], 36)
            self.assertEqual(value["attempted_assignments"], 5)
            self.assertEqual(value["started_assignments"], 5)
            self.assertEqual(value["not_started_assignments"], 31)
            self.assertEqual(value["completed_checked_assignments"], 0)
            self.assertEqual(value["reported_successes"], 0)
            self.assertEqual(value["invocation_status_counts"]["interrupted"], 1)
            self.assertIsNone(value["captured_provider_usage"]["input_tokens"])
            self.assertIsNone(value["captured_provider_usage"]["billed_cost_usd"])
            self.assertFalse(value["integrity_qualified"])
            self.assertEqual(cross.fingerprint(root), before)
            self.assertNotIn("RESULT.json", [path.name for path in bundle.iterdir()])
            source = next((bundle / "snapshots").rglob("*.py"))
            source.write_bytes(source.read_bytes() + b"\n# changed after launch\n")
            with patch("subprocess.Popen", side_effect=AssertionError("invalid report launched child")):
                broken = study.report(bundle)
            self.assertEqual(broken["status"], "invalid_retained_evidence")
            self.assertEqual(broken["initial_assignments"], 36)
            self.assertEqual(broken["attempted_assignments"], 5)
            self.assertFalse(broken["integrity_qualified"])

    @unittest.skipUnless(os.name == "posix", "Ready protocol fixture uses supported private POSIX boundary")
    def test_missing_denied_or_changed_budget_stops_before_any_launch(self):
        for kind in ("absent", "null", "zero", "nan", "capture", "enforcement"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                bundle = fixtures.StudyPreflightTests().bundle(Path(temporary).resolve())
                spec = experiment.specification(bundle)
                if kind == "absent": del spec["budget"]
                if kind == "null": spec["budget"] = None
                if kind == "zero": spec["budget"]["max_billed_cost_usd"] = 0
                if kind == "nan": spec["budget"]["max_billed_cost_usd"] = float("nan")
                if kind == "capture": (bundle / spec["budget"]["capture"]["path"]).write_text("Changed authorization")
                if kind == "enforcement":
                    readiness = experiment.read(bundle / "READINESS.json")
                    readiness["enforcement"]["billing_budget_enforced"] = False
                    experiment.private_write(bundle / "READINESS.json", readiness)
                experiment.private_write(bundle / "EXPERIMENT.json", spec)
                with patch("subprocess.Popen", side_effect=AssertionError("denied budget launched child")):
                    result = experiment.run(bundle)
                self.assertFalse(result["preflight_passed"])
                self.assertFalse((bundle / "EXECUTION.json").exists())

    @unittest.skipUnless(os.name == "posix", "Ready protocol fixture uses supported private POSIX boundary")
    def test_favorable_and_unfavorable_intervals_use_existing_estimator_without_private_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = fixtures.StudyPreflightTests().bundle(Path(temporary).resolve())
            execution_fixture(bundle)
            for rows in ([(0, 1)] * 6, [(1, 0)] * 6):
                metric = adaptive.paired_bootstrap(rows, resamples=200)
                comparison = {"mechanical_contracts_passed": False, "independent_validation_complete": False,
                    "iteration_outcomes": {"paired_tasks": 6, "baseline_comparisons": {"fast": {"passed": metric}}},
                    "raw_prompt": "sk-private-prompt", "error": "secret-code", "samples": [{"code": "secret-code"}]}
                with patch.object(adaptive, "assess", return_value=comparison), patch("subprocess.Popen", side_effect=AssertionError("report executed")):
                    value = study.report(bundle)
                self.assertEqual(value["baseline_comparisons"]["fast"]["passed"], metric)
                text = study.markdown(value) + json.dumps(value)
                self.assertIn(str(metric["estimate"]), text)
                self.assertNotIn("sk-private", text)
                self.assertNotIn("secret-code", text)
                self.assertEqual(value["conclusion"], "not_yet_measurable")
                self.assertFalse(value["productivity_benefit_established"])

    def test_explicit_fresh_output_writes_private_result_and_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            bundle = fixtures.StudyPreflightTests().bundle(root, ready=False)
            bundle = bundle.rename(root / "study with spaces")
            output = root / "report"
            with patch("subprocess.Popen", side_effect=AssertionError("output report executed")), patch("sys.stdout", new=io.StringIO()):
                self.assertEqual(study.main(["report", str(bundle), "--output", str(output)]), 0)
                self.assertEqual(study.main(["report", str(bundle), "--output", str(output)]), 1)
            self.assertEqual(experiment.read(output / "RESULT.json")["status"], "not_run")
            self.assertIn("unmeasured", (output / "REPORT.md").read_text())
            linked = experiment.read(output / "RESULT.json")["private_artifacts"]["SEAL.json"]
            self.assertEqual((output / linked).resolve(), bundle / "SEAL.json")
            self.assertIn("study%20with%20spaces/SEAL.json", (output / "REPORT.md").read_text())
            with patch.object(study, "__file__", str(root / "checkout/evaluation/study_09.py")), patch("sys.stdout", new=io.StringIO()):
                self.assertEqual(study.main(["report", str(bundle), "--output", str(root / "checkout/report")]), 1)
                self.assertFalse((root / "checkout/report").exists())
            if os.name == "posix":
                self.assertEqual((output / "RESULT.json").stat().st_mode & 0o777, 0o600)
                self.assertEqual((output / "REPORT.md").stat().st_mode & 0o777, 0o600)


if __name__ == "__main__":
    unittest.main()
