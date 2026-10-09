"""Offline protocol/integrity fixtures, never live-model quality measurements."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision
from test_coding_tasks import context_fixture, registry_fixture


def observation(path: str, raw: bytes, start=1, end=None):
    parts = raw.split(b"\n")
    lines = [part + b"\n" for part in parts[:-1]] + ([parts[-1]] if parts[-1] else [])
    end = end or len(lines)
    excerpt = b"".join(lines[start - 1:end]).decode("utf-8")
    content_hash = "sha256:" + hashlib.sha256(raw).hexdigest()
    identity = json.dumps([path, start, end, excerpt, content_hash], ensure_ascii=False,
                          separators=(",", ":")).encode("utf-8")
    return {"id": "co_" + hashlib.sha256(identity).hexdigest(), "path": path,
            "start_line": start, "end_line": end, "excerpt": excerpt, "content_hash": content_hash,
            "kind": "static_source", "qualification": "Static source inspection; not proof of runtime behavior or test execution."}


def collected_fixture(project: Path, task: str, setup: str):
    record = context_fixture(task, "fast" if setup == "fast" else "intelligent")
    if setup == "lore06":
        path = sorted((project / "src").glob("*.py"))[0]
        observed = observation(path.relative_to(project).as_posix(), path.read_bytes())
        for response in (record["response"], record["repeat_response"]):
            response["schema_version"] = 4
            response["inspection"] = {"status": "complete", "observations": [observed]}
            response["brief"]["preferred_approach"]["observation_ids"] = [observed["id"]]
            response["checkout_egress"] = {"allowed": False, "model_received_checkout": False,
                                           "reason": "Offline fixture; no model called"}
        record["response_sha256"] = cross.digest(record["response"])
        record["repeat_response_sha256"] = cross.digest(record["repeat_response"])
    record["checkout_before"] = decision.source_state(project)
    record["checkout_after"] = decision.source_state(project)
    record["checks"] = decision.context_checks(record, task, setup)
    return record


def run_fixture(root: Path):
    binary = root / "lore-fixture"
    binary.write_text("Offline test executable identity; no inference")
    program = """import json,sys
request=json.load(sys.stdin)
print(json.dumps({'schema_version':1,'summary':'Unimplemented offline protocol fixture; no inference',
 'files':{name:request['files'][name] for name in request['editable_files']},
 'usage':{'model_calls':0,'input_tokens':0,'output_tokens':0}}))
"""
    args = argparse.Namespace(output=root / "run", cases=coding.DEFAULT_CASES,
        lore_binary=binary, provider="ollama", model="fixture-not-a-model", embedding_model=None,
        decision_provider=None, decision_model=None, allow_hosted=False,
        allow_checkout_egress=False, investigate=False,
        generative_base_url=None, decision_base_url=None,
        agent_command=json.dumps([sys.executable, "-c", program]),
        agent_id="offline test adapter; no inference", agent_location="local", timeout=30, max_tokens=6000)
    calls = []
    def collect(binary, project, task, setup, timeout, max_tokens, **options):
        calls.append((setup, project, options))
        return collected_fixture(project, task, setup)
    with patch.object(decision.bench, "subprocess_json", return_value=({"model_calls": 1}, 0.1)), \
         patch.object(decision, "collect_context", side_effect=collect):
        result = decision.run(args)
    return args.output, result, calls


def complete_reviews(directory: Path):
    report = cross.read_json(directory / "metrics.json")
    by_id = {sample["sample_id"]: sample for sample in report["samples"]}
    for path in (directory / "reviews").glob("*.json"):
        packet = cross.read_json(path)
        sample = by_id[packet["sample_id"]]
        packet["metrics_sha256"] = cross.digest(report)
        for index, annotation in enumerate(packet["assessments"]):
            annotation.update(complete=True, reviewer=f"offline-review-fixture-{index}", reviewed_at="2026-01-01",
                blind_confirmed=True, material_decision_mistakes=["Synthetic mistake annotation"] if sample["setup"] == "lore05" else [],
                missed_critical_constraints=[], unnecessary_blocking=False,
                incorrect_hypotheses_identified=1, revised_incorrect_hypotheses=int(sample["setup"] == "lore06"),
                implementation_quality_0_to_3=0, severe_unsupported_project_assertions=[],
                notes="Offline annotation fixture; no quality result or real human review is claimed.")
        cross.write_json(path, packet)


class DecisionTaskTests(unittest.TestCase):
    def test_observations_bind_full_raw_files_exact_crlf_ranges_and_unicode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "src").mkdir()
            raw = "første\r\nnaïve = '✓'\r\nlast".encode("utf-8")
            (root / "src/example.py").write_bytes(raw)
            observed = observation("src/example.py", raw, 2, 3)
            response = {"inspection": {"observations": [observed]}, "brief": {"observation_ids": [observed["id"]]}}
            self.assertEqual(decision.verify_observations(response, root)["checked"], 1)
            for field, value in (("excerpt", "wrong text"), ("content_hash", "sha256:" + "0" * 64),
                                 ("start_line", 0), ("id", "co_fabricated"), ("path", "../outside.py")):
                invalid = copy.deepcopy(response)
                invalid["inspection"]["observations"][0][field] = value
                with self.assertRaises(ValueError, msg=field):
                    decision.verify_observations(invalid, root)
            response["brief"]["observation_ids"].append("co_unresolved")
            with self.assertRaises(ValueError):
                decision.verify_observations(response, root)

    def test_native_observations_remain_distinct_from_checkout_observations(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "original.md").write_text("retained source")
            response = {"imported_observations": [{"id": "ob_native"}],
                        "discrepancies": [{"observation_ids": ["ob_native"]}]}
            self.assertEqual(decision.verify_observations(response, root)["checked"], 0)
            response["discrepancies"][0]["observation_ids"].append("ob_invented")
            with self.assertRaises(ValueError):
                decision.verify_observations(response, root)

    def test_inline_generated_citations_require_manifests_but_source_literals_do_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "src").mkdir()
            (root / "src/example.py").write_text("# The source prints co_source_literal.\nvalue = 1\n")
            record = collected_fixture(root, "Change the adapter", "lore06")
            for answer in (record["response"], record["repeat_response"]):
                identity = answer["inspection"]["observations"][0]["id"]
                answer["brief"]["preferred_approach"].update(text=f"Preserve behavior shown by {identity}.", observation_ids=[])
                answer["brief"]["facts"] = [{"statement": "A source can print co_fact_literal.",
                                              "qualifications": ["Example data: co_qualification_literal"]}]
                answer["investigation"] = {"steps": [], "remaining_uncertainty": []}
            record["response_sha256"] = cross.digest(record["response"])
            record["repeat_response_sha256"] = cross.digest(record["repeat_response"])
            self.assertEqual(decision.verify_observations(record["response"], root)["referenced"], 1)
            self.assertTrue(decision.context_checks(record, "Change the adapter", "lore06", snapshot=root)["exact_local_observations"])
            for location in ("advice", "trace", "uncertainty"):
                with self.subTest(location=location):
                    invalid = copy.deepcopy(record)
                    answer = invalid["response"]
                    if location == "advice":
                        answer["brief"]["preferred_approach"]["text"] = "Consult co_missing_advice."
                    elif location == "trace":
                        answer["investigation"]["steps"] = [{"initial_hypothesis": "Evidence co_missing_trace discriminates.", "observation_ids": []}]
                    else:
                        answer["investigation"]["remaining_uncertainty"] = ["Check co_missing_uncertainty."]
                    invalid["response_sha256"] = cross.digest(answer)
                    self.assertFalse(decision.context_checks(invalid, "Change the adapter", "lore06", snapshot=root)["exact_local_observations"])

    def test_real_cli_arguments_keep_schema_and_checkout_permission_separate(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            response = {"schema_version": 4, "task": "Change the adapter", "model_calls": 0,
                        "mode": "fast_fallback", "inspection": {"observations": []},
                        "checkout_egress": {"allowed": False, "model_received_checkout": False}}
            calls = []
            def cli(binary, project, *arguments, timeout):
                calls.append(arguments)
                return response, 0.1
            with patch.object(coding, "registry_content", return_value=registry_fixture()), \
                 patch.object(decision.bench, "subprocess_json", side_effect=cli):
                record = decision.collect_context("unused", project, response["task"], "lore06", 2, 3000,
                                                  investigate=True, allow_checkout_egress=False)
            self.assertTrue(all(record["checks"].values()))
            self.assertEqual(len(calls), 2)
            for arguments in calls:
                self.assertIn("--inspect", arguments)
                self.assertIn("--investigate", arguments)
                self.assertNotIn("--allow-checkout-egress", arguments)
                self.assertEqual(arguments[arguments.index("--schema-version") + 1], "4")

    def test_four_arms_use_isolated_projects_and_actual_verification_without_quality_claims(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, result, calls = run_fixture(Path(temporary).resolve())
            self.assertTrue(result["comparison_complete"], result["issues"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["human_review_complete"])
            self.assertFalse(result["release_gates_passed"])
            self.assertEqual(result["integrity_audit"]["status"], "unmeasured")
            self.assertIsNone(result["relative_material_decision_mistake_reduction_vs_05"])
            self.assertEqual(len(result["reviews_pending"]), 12)
            self.assertEqual(len({str(project) for _, project, _ in calls}), 9)
            for arm in decision.SETUPS:
                self.assertEqual(result["setups"][arm]["tasks"], 3)
                self.assertEqual(result["setups"][arm]["passed_tasks"], 0)
                self.assertIsNone(result["setups"][arm]["time_to_correct_completion_seconds"])
                self.assertIsNone(result["setups"][arm]["total_usage"]["billed_cost_usd"])
            for path in (directory / "reviews").glob("*.json"):
                packet = cross.read_json(path)
                self.assertNotIn("setup", packet)
                self.assertEqual(len(packet["assessments"]), 2)
                self.assertTrue(all(annotation["material_decision_mistakes"] is None for annotation in packet["assessments"]))

    def test_two_reviews_are_bound_and_fixture_identity_cannot_be_relabeled(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _ = run_fixture(Path(temporary).resolve())
            report = cross.read_json(directory / "metrics.json")
            report.update(fixture_only=False, held_out=True, independent_projects=True)
            cross.write_json(directory / "metrics.json", report)
            complete_reviews(directory)
            result = decision.assess(directory)
            self.assertTrue(result["human_review_complete"], result["issues"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["independent_validation_complete"])
            self.assertFalse(result["release_gates_passed"])
            self.assertEqual(result["relative_material_decision_mistake_reduction_vs_05"], 1.0)
            packet_path = next((directory / "reviews").glob("*.json"))
            packet = cross.read_json(packet_path)
            packet["assessments"][1]["reviewer"] = packet["assessments"][0]["reviewer"].upper()
            cross.write_json(packet_path, packet)
            self.assertFalse(decision.assess(directory)["human_review_complete"])

    def test_source_context_and_request_tampering_invalidates_the_comparison(self):
        for change in ("source", "observation", "request", "permission"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                directory, _, _ = run_fixture(Path(temporary).resolve())
                report = cross.read_json(directory / "metrics.json")
                sample = next(sample for sample in report["samples"] if sample["setup"] == "lore06")
                if change == "source":
                    checkout = directory / sample["context"]["checkout_root"]
                    next((checkout / "src").glob("*.py")).write_text("changed after context collection")
                elif change == "observation":
                    sample["context"]["response"]["inspection"]["observations"][0]["excerpt"] = "invented"
                    sample["context"]["response_sha256"] = cross.digest(sample["context"]["response"])
                elif change == "permission":
                    sample["context"]["response"]["checkout_egress"]["model_received_checkout"] = True
                    sample["context"]["response_sha256"] = cross.digest(sample["context"]["response"])
                else:
                    sample["request_sha256"] = "0" * 64
                cross.write_json(directory / "metrics.json", report)
                result = decision.assess(directory)
                self.assertFalse(result["mechanical_contracts_passed"])

    def test_independent_audit_requires_bound_captures_and_does_not_assume_zero_egress(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            self.assertEqual(decision.audit_status(directory, "binding")["status"], "unmeasured")
            capture = directory / "network-capture.json"
            capture.write_text('{"fixture":true,"events":[]}')
            audit = {"schema_version": 1, "metrics_sha256": "binding", "complete": True,
                     "auditor": "offline audit fixture", "reviewed_at": "2026-01-01", "method": "Test binding only",
                     "notes": "Synthetic audit, not a real network measurement",
                     "captures": [{"path": capture.name, "sha256": coding.hash_file(capture)}],
                     "zero_unauthorized_checkout_egress": True, "zero_unrequested_commands": True,
                     "no_accidental_source_modification": True}
            cross.write_json(directory / "INTEGRITY_AUDIT.json", audit)
            self.assertTrue(decision.audit_status(directory, "binding")["passed"])
            self.assertFalse(decision.audit_status(directory, "changed-binding")["passed"])
            capture.write_text("changed capture")
            self.assertFalse(decision.audit_status(directory, "binding")["passed"])

    def test_zero_baseline_and_partial_review_do_not_create_improvement_numbers(self):
        self.assertIsNone(decision.relative_reduction(0, 0))
        self.assertIsNone(decision.relative_reduction(None, 0))
        self.assertEqual(decision.relative_reduction(10, 8.5), 0.15)
        sample = {"sample_id": "sample-test", "answer_sha256": "hash", "task": "task", "constraints": ["preserve X"]}
        packet = decision.review_template(sample, "binding")
        self.assertEqual(decision.validated_reviews(packet, sample, "binding"), [])
        packet["metrics_sha256"] = "changed"
        with self.assertRaises(ValueError):
            decision.validated_reviews(packet, sample, "binding")


if __name__ == "__main__":
    unittest.main()
