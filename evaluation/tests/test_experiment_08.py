"""Offline preflight/audit doubles and real failing subprocesses; no model runs."""
import argparse
import copy
from datetime import datetime, timedelta, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import experiment_08 as experiment
import outcome_protocol as outcome
import provider_usage as metering
import real_coding_corpus as real


def utc(seconds=0):
    return (datetime.now(timezone.utc) + timedelta(seconds=seconds)).isoformat()


class StudyPreflightTests(unittest.TestCase):
    def bundle(self, root, *, ready=True):
        inputs = root / "inputs"
        real.prepare(inputs, pilot=True)
        bundle = root / "study"
        experiment.prepare(bundle, inputs / "cases.json")
        if not ready:
            return bundle
        if os.name != "posix":
            self.skipTest("Passing launch controls require the supported private POSIX runner")
        spec = experiment.specification(bundle)
        agent = root / "offline_agent.py"
        agent.write_text("# Labeled offline protocol double; no actual model execution.\n")
        prompt = bundle / "prompt.txt"
        prompt.write_text("Offline protocol test prompt, not a coding model result.\n")
        spec["run"].update(lore_binary=str(Path(sys.executable).resolve()), model="offline-lore",
            agent_id="offline-only-model-and-tool-control",
            agent_command=[str(Path(sys.executable).resolve()), str(agent)], allow_inspection=True,
            timeout=5, max_attempts=2, attempt_budget_seconds=20)
        spec["agent"].update(model="offline-agent", model_revision="offline-revision",
                             reasoning="none", prompt_file="prompt.txt")
        spec["lore_revisions"]["generative"] = "offline-lore-revision"
        experiment.private_write(bundle / "EXPERIMENT.json", spec)
        computed = experiment.pins(bundle)
        registration = experiment.read(bundle / "PREREGISTRATION.json")
        registration.update(complete=True, study_id="offline-contract-only", operator_id="offline-operator",
                            registered_at=utc(-100))
        registration["agent"].update(model="offline-agent", model_revision="offline-revision",
            reasoning="none", temperature=0, tools=[], tool_call_limit=0, token_limit=1000,
            max_attempts=2, wall_time_seconds=20, command_sha256=computed["agent_command_sha256"],
            prompt_sha256=computed["configuration"]["prompt_sha256"])
        registration["lore"].update(model="offline-lore", model_revision="offline-lore-revision",
            reasoning="default", configuration_sha256_by_case=computed["configuration"]["configuration_sha256_by_case"])
        for index, case in enumerate(registration["cases"]):
            case.update(task_authors=["offline-task-author"], checker_authors=["offline-checker-author"])
            case["independent_gold_reviews"] = self.reviews(bundle, "gold-review-captures",
                                                           case["task_sha256"], f"gold-{index}", utc(-110))
        experiment.private_write(bundle / "PREREGISTRATION.json", registration)
        experiment.seal(bundle)
        readiness = experiment.read(bundle / "READINESS.json")
        readiness.update(complete=True, observed_at=utc(-20), expires_at=utc(3600))
        for value in readiness["providers"].values():
            value["available"] = True
        readiness["enforcement"] = {"method": "Offline audit double, never actual OS sandbox evidence",
                                   **dict.fromkeys(experiment.ENFORCEMENT, True)}
        readiness["captures"] = []
        for kind in ("provider_availability", "runtime_isolation", "egress_enforcement"):
            name = "readiness-captures/" + kind + ".txt"
            path = bundle / name
            path.parent.mkdir(exist_ok=True)
            path.write_text("Offline audit protocol double only: " + kind)
            readiness["captures"].append({"kind": kind, "path": name, "sha256": coding.hash_file(path)})
        authentication = bundle / "readiness-captures/authentication.txt"
        authentication.write_text("Offline authentication protocol double only; no external identity authenticated.")
        readiness["authentication"] = {"method": "Offline identity contract fixture",
            "capture": {"path": "readiness-captures/authentication.txt", "sha256": coding.hash_file(authentication)}}
        subject = {key: value for key, value in readiness.items() if key not in ("reviews", "qualification")}
        readiness["reviews"] = self.reviews(bundle, "readiness-captures", cross.digest(subject), "runtime", utc(-10))
        experiment.private_write(bundle / "READINESS.json", readiness)
        return bundle

    def reviews(self, bundle, folder, target, label, at):
        result = []
        for index in (1, 2):
            name = f"{folder}/{label}-{index}.txt"
            path = bundle / name
            path.parent.mkdir(exist_ok=True)
            path.write_text(f"Offline review fixture {label}/{index}; not a real independent review.")
            result.append({"reviewer_id": f"offline-reviewer-{index}", "complete": True,
                "independent": True, "conflicts_declared": [], "blind_confirmed": True,
                "target_sha256": target, "reviewed_at": at,
                "capture": {"path": name, "sha256": coding.hash_file(path)}})
        return result

    def test_preparation_and_blocked_preflight_execute_no_provider_or_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(subprocess, "Popen", side_effect=AssertionError("Preflight cannot execute")):
                bundle = self.bundle(Path(temporary).resolve(), ready=False)
                result = experiment.run(bundle)
            self.assertFalse(result["preflight_passed"])
            self.assertEqual(result["initial_assignments"], 36)
            self.assertEqual(result["provider_calls"], 0)
            self.assertEqual(result["subprocess_calls"], 0)
            self.assertEqual(result["coding_productivity"], "unmeasured")
            self.assertIn("preregistration_record", {item["gate"] for item in result["blockers"]})
            self.assertFalse((bundle / "EXECUTION.json").exists())
            self.assertFalse((bundle / "run").exists())

    def test_complete_offline_contract_has_separate_readiness_not_outcome_status(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            with patch.object(subprocess, "Popen", side_effect=AssertionError("Preflight cannot execute")):
                result = experiment.preflight(bundle)
            self.assertTrue(result["preflight_passed"], result["blockers"])
            self.assertEqual(result["coding_productivity"], "unmeasured")
            self.assertEqual(result["initial_assignments"], 36)
            self.assertNotIn("samples", result)
            self.assertFalse((bundle / "run").exists())

    def test_mutated_command_model_seed_grant_or_budget_cannot_reuse_seal(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            original = experiment.read(bundle / "EXPERIMENT.json")
            for field, value in (("agent_command", [str(Path(sys.executable).resolve()), "-c", "print('substitute')"]),
                                 ("model", "different-model"), ("seed", 8), ("allow_inspection", False),
                                 ("max_attempts", 3), ("attempt_budget_seconds", 22)):
                spec = copy.deepcopy(original)
                spec["run"][field] = value
                experiment.private_write(bundle / "EXPERIMENT.json", spec)
                with self.subTest(field=field):
                    self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
            experiment.private_write(bundle / "EXPERIMENT.json", original)
            self.assertTrue(experiment.preflight(bundle)["preflight_passed"])
            with self.assertRaises(ValueError):
                experiment.seal(bundle)

    def test_provider_expiry_missing_enforcement_and_reused_reviews_block_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            original = experiment.read(bundle / "READINESS.json")
            for kind in ("expiry", "availability", "isolation", "authentication", "review", "capture"):
                value = copy.deepcopy(original)
                if kind == "expiry":
                    value["expires_at"] = utc(-1)
                elif kind == "availability":
                    value["providers"]["agent"]["available"] = False
                elif kind == "isolation":
                    value["enforcement"]["agent_cannot_read_checkers"] = False
                elif kind == "authentication":
                    value["authentication"]["capture"]["sha256"] = "0" * 64
                elif kind == "review":
                    value["reviews"][1] = value["reviews"][0]
                else:
                    value["captures"][0]["sha256"] = "0" * 64
                experiment.private_write(bundle / "READINESS.json", value)
                with self.subTest(kind=kind), patch.object(subprocess, "Popen",
                                                          side_effect=AssertionError("Blocked run")):
                    self.assertFalse(experiment.run(bundle)["preflight_passed"])
            experiment.private_write(bundle / "READINESS.json", original)

    def test_source_checker_prompt_and_review_bytes_are_bound_before_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            prepared = experiment.read(bundle / "prepared.json")
            entry = prepared["cases"][0]
            source = bundle / entry["source_root"] / next(iter(entry["source_manifest"]["files_sha256"]))
            prompt = bundle / "prompt.txt"
            review = bundle / "gold-review-captures/gold-0-1.txt"
            for path in (source, prompt, review):
                original = path.read_bytes()
                path.write_bytes(original + b"\nchanged")
                self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
                path.write_bytes(original)
            checkers = experiment.checker_separation(bundle, prepared)
            self.assertTrue(checkers["files_sha256"])
            # A checker copied into a source tree cannot gain isolation by a new name.
            copied = copy.deepcopy(prepared)
            copied["cases"][0]["source_manifest"]["files_sha256"]["renamed.py"] = next(
                iter(entry["checker_files_sha256"].values()))
            with self.assertRaises(ValueError):
                experiment.checker_separation(bundle, copied)
            self.assertTrue(experiment.preflight(bundle)["preflight_passed"])

    def test_public_pilot_cannot_be_promoted_to_a_full_heldout_study(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            spec = experiment.specification(bundle)
            spec["mode"] = "full"
            experiment.private_write(bundle / "EXPERIMENT.json", spec)
            result = experiment.preflight(bundle)
            self.assertFalse(result["preflight_passed"])
            self.assertIn("cohort", {item["gate"] for item in result["blockers"]})

    def test_known_adapter_must_match_prompt_model_and_host_grants(self):
        import coding_agent
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve(), ready=False)
            spec = experiment.specification(bundle)
            spec["run"].update(lore_binary=str(Path(sys.executable).resolve()), model="offline-lore",
                agent_id="offline-known-adapter-control", agent_command=[str(Path(sys.executable).resolve()),
                str(experiment.ROOT / "coding_agent.py"), "--provider", "ollama", "--model", "offline-agent"])
            spec["agent"].update(model="offline-agent", model_revision="offline-revision",
                                 reasoning="none", prompt_file="prompt.txt")
            spec["lore_revisions"]["generative"] = "offline-lore-revision"
            (bundle / "prompt.txt").write_bytes(coding_agent.INSTRUCTIONS.encode("utf-8"))
            experiment.private_write(bundle / "EXPERIMENT.json", spec)
            self.assertEqual(experiment.pins(bundle)["provider_calls"], 0)
            for arguments in (["--allow-hosted"], ["--usage-ledger", "/unrelated/private.json"],
                              ["--model", "different-model"], ["--api-key", "PRIVATE_VALUE"]):
                altered = copy.deepcopy(spec)
                altered["run"]["agent_command"].extend(arguments)
                experiment.private_write(bundle / "EXPERIMENT.json", altered)
                with self.subTest(arguments=arguments), self.assertRaises(ValueError) as error:
                    experiment.pins(bundle)
                self.assertNotIn("PRIVATE_VALUE", str(error.exception))

    def test_unrecognized_private_agent_fields_are_not_echoed_in_pins(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve(), ready=False)
            spec = experiment.specification(bundle)
            spec["agent"]["api_key"] = "PRIVATE_VALUE_NEVER_ECHO"
            experiment.private_write(bundle / "EXPERIMENT.json", spec)
            with self.assertRaises(ValueError) as error:
                experiment.pins(bundle)
            self.assertNotIn("PRIVATE_VALUE", str(error.exception))
            report = experiment.preflight(bundle)
            self.assertFalse(report["preflight_passed"])
            self.assertNotIn("PRIVATE_VALUE", json.dumps(report))

    def test_static_preregistration_helper_does_not_bypass_observed_models(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            prepared, registration = experiment.read(bundle / "prepared.json"), experiment.read(bundle / "PREREGISTRATION.json")
            report = {"cases": prepared["cases"], "created_at": utc(), "setups": list(adaptive.SETUPS),
                      "agent_command_sha256": registration["agent"]["command_sha256"],
                      "repair_policy": {"max_attempts": 2, "attempt_budget_seconds": 20}}
            outcome.validate_preregistration_inputs(bundle, registration, report)
            report["samples"] = []
            self.assertFalse(outcome.preregistration_status(bundle, report)["complete"])

    def test_prepared_cohort_cannot_duplicate_one_task_and_omit_another(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve(), ready=False)
            prepared = experiment.read(bundle / "prepared.json")
            prepared["cases"][-1] = copy.deepcopy(prepared["cases"][0])
            experiment.private_write(bundle / "prepared.json", prepared)
            with self.assertRaises(ValueError):
                experiment.prepared_inputs(bundle)

    def test_inputs_changed_after_preflight_never_reach_child_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            writer = experiment.private_write

            def replace_after_gate(path, value, **options):
                writer(path, value, **options)
                if path.name == "PREFLIGHT.json":
                    spec = experiment.read(bundle / "EXPERIMENT.json")
                    spec["run"]["allow_hosted"] = True
                    writer(bundle / "EXPERIMENT.json", spec)

            with patch.object(experiment, "private_write", side_effect=replace_after_gate), \
                 patch.object(subprocess, "Popen", side_effect=AssertionError("Unsealed launch")):
                result = experiment.run(bundle)
            self.assertEqual(result["execution"]["status"], "not_started_inputs_changed")
            self.assertEqual(result["retention"]["success_denominator"], 36)
            self.assertEqual(result["retention"]["attempted_invocations"], 0)
            self.assertFalse(result["mechanical_comparison_complete"])

    @unittest.skipUnless(os.name == "posix", "Study process group termination uses POSIX")
    def test_post_launch_receipt_failure_stops_and_reaps_real_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            writer = experiment.private_write
            failed = False

            def fail_after_launch(path, value, **options):
                nonlocal failed
                if path.name == "EXECUTION.json" and value.get("child_pid") and not failed:
                    failed = True
                    raise OSError("Offline injected persistence failure")
                writer(path, value, **options)

            command = [str(Path(sys.executable).resolve()), "-c", "import time; time.sleep(30)"]
            with patch.object(experiment, "private_write", side_effect=fail_after_launch), \
                 patch.object(experiment, "runner_command", return_value=command):
                result = experiment.run(bundle)
            self.assertTrue(failed)
            self.assertEqual(result["execution"]["status"], "failed_after_start")
            self.assertEqual(result["execution"]["error_type"], "OSError")
            self.assertFalse(result["mechanical_comparison_complete"])
            self.assertEqual(result["retention"]["success_denominator"], 36)
            with self.assertRaises(ProcessLookupError):
                os.kill(result["execution"]["child_pid"], 0)

    def failed_attempt(self, bundle, status):
        prepared = experiment.read(bundle / "prepared.json")
        entry = prepared["cases"][0]
        root = bundle / "run"
        sample = "sample-offline-failure"
        workspace = root / "implementations" / sample
        cross.copy_snapshot(bundle / entry["source_root"], workspace)
        source = {name: (workspace / name).read_text(encoding="utf-8") for name in entry["source_manifest"]["files_sha256"]}
        experiment.private_write(root / "sample-starts" / (sample + ".json"), {
            "schema_version": 1, "sample_id": sample, "case_id": entry["case"]["id"],
            "project": entry["case"]["project"], "setup": "baseline",
            "context_usage": metering.no_inference(), "preparation_usage": metering.no_inference()})
        program = """import os,sys
sys.path.insert(0,sys.argv[1])
from provider_usage import Recorder
r=Recorder(os.environ['LORE_USAGE_LEDGER'])
r.events=[{'id':'offline-event','call_id':'offline-call','operation':'coding','provider':'offline',
 'model':'offline-no-model','attempt':1,'elapsed_ms':3,'http_status':None,
 'provider_request_count':1,'input_tokens':7,'output_tokens':None,'total_tokens':None,
 'billed_cost_usd':None,'billing_source':None,'status':sys.argv[2],'cache_hit':False}]
r.save('failed')
print('SECRET_PROVIDER_OUTPUT_NOT_IN_RECEIPTS')
raise SystemExit(3)
"""
        with self.assertRaises(ValueError):
            coding.run_attempts([str(Path(sys.executable).resolve()), "-c", program, str(experiment.ROOT), status],
                entry["case"], source, None, workspace, root, sample,
                Path(prepared["cases_manifest"]).parent, 5,
                coding.repair_policy(argparse.Namespace(timeout=5, max_attempts=2, attempt_budget_seconds=20)),
                lambda *_: {})
        return root, sample

    def test_real_failed_refused_and_timed_out_invocations_keep_all_denominators(self):
        for status in ("refused", "timed_out", "transport_error"):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as temporary:
                bundle = self.bundle(Path(temporary).resolve(), ready=False)
                self.failed_attempt(bundle, status)
                result = experiment.retention(bundle, {"elapsed_seconds": 1.25})
                self.assertEqual(result["success_denominator"], 36)
                self.assertEqual(result["attempted_assignments"], 1)
                self.assertEqual(result["attempted_invocations"], 1)
                self.assertEqual(result["completed_checked_assignments"], 0)
                self.assertEqual(result["failed_refused_cancelled_or_interrupted_invocations"], 1)
                self.assertIsNone(result["captured_provider_usage"]["billed_cost_usd"])
                self.assertEqual(result["captured_provider_usage"]["input_tokens"], 7)
                self.assertEqual(sum(row["status"] == "not_started" for row in result["assignments"]), 35)
                row = next(row for row in result["assignments"] if row["attempts"])
                self.assertIsNone(row["total_usage"]["billed_cost_usd"])
                self.assertIsNone(row["time_to_first_correct_seconds"])
                self.assertNotIn("SECRET_PROVIDER_OUTPUT", json.dumps(result))

    def test_copied_usage_is_not_charged_twice_and_tampered_sidecar_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve(), ready=False)
            root, sample = self.failed_attempt(bundle, "refused")
            sidecar = root / "attempt-invocations" / sample / "1-provider-usage.json"
            copied = root / "context-projects/case/adaptive_reuse/.lore/provider-usage/copied.json"
            copied.parent.mkdir(parents=True)
            copied.write_bytes(sidecar.read_bytes())
            result = experiment.retention(bundle, {"elapsed_seconds": 1})
            self.assertEqual(result["captured_provider_usage"]["provider_request_count"], 1)
            raw = experiment.read(sidecar)
            raw["events"][0]["input_tokens"] = 999
            raw["summary"] = metering.aggregate(raw["events"])
            experiment.private_write(sidecar, raw)
            with self.assertRaises(ValueError):
                experiment.retention(bundle, {})

    def test_cancelled_or_started_invocation_is_not_erased_by_an_empty_metrics_file(self):
        for state in ("cancelled", "started"):
            with self.subTest(state=state), tempfile.TemporaryDirectory() as temporary:
                bundle = self.bundle(Path(temporary).resolve(), ready=False)
                root, sample = self.failed_attempt(bundle, "transport_error")
                path = root / "attempt-invocations" / sample / "1.json"
                value = experiment.read(path)
                value["status"] = state
                experiment.private_write(path, value)
                experiment.private_write(root / "metrics.json", {"samples": []})
                result = experiment.retention(bundle, {"elapsed_seconds": 3})
                self.assertEqual(result["attempted_invocations"], 1)
                self.assertEqual(result["success_denominator"], 36)
                self.assertEqual(result["failed_refused_cancelled_or_interrupted_invocations"], 1)

    @unittest.skipUnless(os.name == "posix", "Study process group termination uses POSIX")
    def test_whole_study_timeout_reaps_real_child_and_retains_unstarted_assignments(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = self.bundle(Path(temporary).resolve())
            # Short timeout is resealed in this labeled offline fixture before launch.
            spec = experiment.specification(bundle)
            spec["study_wall_time_seconds"] = 1
            experiment.private_write(bundle / "EXPERIMENT.json", spec)
            contract = experiment.read(bundle / "SEAL.json")
            current = experiment.contract_for(bundle, spec, experiment.prepared_inputs(bundle),
                experiment.arguments(bundle, spec, experiment.prepared_inputs(bundle)), utc())
            contract.update(contract=current, contract_sha256=cross.digest(current))
            experiment.private_write(bundle / "SEAL.json", contract)
            readiness = experiment.read(bundle / "READINESS.json")
            readiness["contract_sha256"] = cross.digest(current)
            subject = {key: value for key, value in readiness.items() if key not in ("reviews", "qualification")}
            readiness["reviews"] = self.reviews(bundle, "readiness-captures", cross.digest(subject), "runtime", utc(-1))
            experiment.private_write(bundle / "READINESS.json", readiness)
            command = [str(Path(sys.executable).resolve()), "-c", "import time; time.sleep(10)"]
            # This replaces only the study subprocess with an explicitly labeled
            # offline sleeping control; real provider readiness is never claimed.
            with patch.object(experiment, "runner_command", return_value=command):
                result = experiment.run(bundle)
            self.assertEqual(result["execution"]["status"], "timed_out")
            self.assertLess(result["execution"]["elapsed_seconds"], 5)
            self.assertFalse(result["mechanical_comparison_complete"])
            self.assertEqual(result["retention"]["success_denominator"], 36)
            self.assertEqual(result["retention"]["attempted_invocations"], 0)
            self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
            with self.assertRaises(ProcessLookupError):
                os.kill(result["execution"]["child_pid"], 0)


if __name__ == "__main__":
    unittest.main()
