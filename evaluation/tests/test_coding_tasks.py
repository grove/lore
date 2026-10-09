"""Offline evaluator bookkeeping and executable fixture checks, not model scores."""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_agent
import coding_tasks as coding
import cross_source as cross


def registry_fixture():
    return {"counts": {"knowledge_units": 1}, "database_sha256": "a" * 64,
            "native_current_sha256": "b" * 64, "native_evidence_ids": [],
            "sqlite_integrity_ok": True, "wiki_sha256": {"index.md": "c" * 64}}


def context_fixture(task, setup, fallback=False):
    response = {"schema_version": 2 if setup == "fast" else 3,
                "task": task, "model_calls": 0 if setup == "fast" or fallback else 1,
                "evidence": [{"id": "ev_abc", "source": "docs:contract.md"}]}
    if setup != "fast":
        response["mode"] = "fast_fallback" if fallback else "intelligent"
        if fallback:
            response["fallback_reason"] = "Fixture unavailable provider"
        else:
            response["brief"] = {"preferred_approach": {"text": "Fixture advice", "evidence_ids": ["ev_abc"]}}
            response["cache_status"] = "miss"
    repeated = copy.deepcopy(response)
    repeated["model_calls"] = 0
    if setup != "fast" and not fallback:
        repeated["cache_status"] = "hit"
    resolved = {"evidence_id": "ev_abc"}
    citations = {"checked": 1, "resolvable": 1, "failures": [],
                 "results": [{"evidence_id": "ev_abc", "response": resolved,
                              "response_sha256": cross.digest(resolved), "error": None}]}
    result = {"response": response, "repeat_response": repeated,
              "response_sha256": cross.digest(response), "repeat_response_sha256": cross.digest(repeated),
              "registry_before": registry_fixture(), "registry_after": registry_fixture(),
              "citation_integrity": citations, "mode": "fast" if setup == "fast" else response["mode"],
              "elapsed_seconds": 0.1, "repeat_elapsed_seconds": 0.01,
              "usage": coding.usage({}, calls=response["model_calls"]),
              "repeat_usage": coding.no_inference(), "inspection_sources": ["docs:contract.md"]}
    result["checks"] = coding.context_checks(result, task, setup)
    return result


def run_fixture(root: Path):
    binary = root / "lore-fixture"
    binary.write_text("unit-test executable identity; not a real model run")
    # This subprocess intentionally returns unimplemented code. The harness
    # must run the actual external tests and record failed task outcomes.
    program = """import json,sys
task=json.load(sys.stdin)
print(json.dumps({'schema_version':1,'summary':'Unimplemented unit-test fixture; no model called',
 'files':{path:task['files'][path] for path in task['editable_files']},
 'usage':{'model_calls':0,'input_tokens':0,'output_tokens':0}}))
"""
    args = argparse.Namespace(output=root / "run", cases=coding.DEFAULT_CASES,
                              lore_binary=binary, provider="ollama", model="fixture-not-a-model",
                              embedding_model=None, decision_provider=None, decision_model=None,
                              allow_hosted=False, generative_base_url=None, decision_base_url=None,
                              agent_command=json.dumps([sys.executable, "-c", program]),
                              agent_id="fixture runner, no inference", agent_location="local",
                              timeout=30, max_tokens=6000)
    def collect(binary, project, task, setup, timeout, max_tokens):
        return context_fixture(task, setup)
    with patch.object(coding.bench, "subprocess_json", return_value=({"model_calls": 1}, 1.0)), patch.object(coding, "collect_context", side_effect=collect):
        result = coding.run(args)
    return args.output, result


class CodingTaskTests(unittest.TestCase):
    def test_prepare_preserves_originals_and_initial_implementations_fail(self):
        before = cross.fingerprint(coding.DEFAULT_CASES.parent)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve() / "prepared"
            prepared = coding.prepare(output, coding.DEFAULT_CASES)
            self.assertEqual(prepared["inference_calls"], 0)
            self.assertTrue(prepared["fixture_only"])
            self.assertFalse(prepared["held_out"])
            self.assertEqual(len(prepared["cases"]), 3)
            for entry in prepared["cases"]:
                workspace = output / entry["source_root"]
                checks, _ = coding.execute_checks(entry["case"], workspace, coding.DEFAULT_CASES.parent, 10)
                self.assertEqual(len(checks["checks"]), 4)
                self.assertFalse(all(item["passed"] for item in checks["checks"]))
                self.assertEqual(cross.fingerprint(workspace), entry["source_manifest"])
            with self.assertRaises(ValueError):
                coding.prepare(output, coding.DEFAULT_CASES)
        self.assertEqual(cross.fingerprint(coding.DEFAULT_CASES.parent), before)

    def test_executable_checks_distinguish_scoped_release_policies(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve() / "prepared"
            prepared = coding.prepare(output, coding.DEFAULT_CASES)
            entry = prepared["cases"][2]
            workspace = output / entry["source_root"]
            module = workspace / "src/release_gate.py"
            module.write_text("def may_release(environment, has_valid_signature, is_fixture):\n"
                              "    return environment in ('production', 'staging') and (has_valid_signature or (environment == 'staging' and is_fixture))\n")
            result, _ = coding.execute_checks(entry["case"], workspace, coding.DEFAULT_CASES.parent, 10)
            self.assertTrue(all(check["passed"] for check in result["checks"]))
            module.write_text("def may_release(environment, has_valid_signature, is_fixture):\n    return has_valid_signature or is_fixture\n")
            result, _ = coding.execute_checks(entry["case"], workspace, coding.DEFAULT_CASES.parent, 10)
            failed = {check["id"] for check in result["checks"] if not check["passed"]}
            self.assertIn("production_signature_required", failed)
            self.assertIn("unknown_environment_fails_closed", failed)

    def test_agent_output_paths_and_unknown_billing_are_enforced(self):
        for filename in ("../escape", "/tmp/escape", "C:/escape", "src\\file.py", "src/./file.py"):
            with self.assertRaises(ValueError):
                coding.relative_file(filename)
        with self.assertRaises(ValueError):
            coding.validate_agent_response({"schema_version": 1, "summary": "fixture", "files": {"docs/policy.md": "rewritten"}}, ["src/implementation.py"])
        for value in (float("nan"), float("inf"), -1, True):
            with self.assertRaises(ValueError):
                coding.usage({"billed_cost_usd": value, "billing_source": "invoice"})
        with self.assertRaises(ValueError):
            coding.usage({"billed_cost_usd": 0.1})
        known = coding.usage({"model_calls": 2, "input_tokens": 100, "output_tokens": 30, "billed_cost_usd": 0.2, "billing_source": "provider invoice"})
        unknown = coding.usage({}, calls=1)
        total = coding.add_usage([known, unknown])
        self.assertEqual(total["model_calls"], 3)
        self.assertIsNone(total["input_tokens"])
        self.assertIsNone(total["billed_cost_usd"])

    def test_context_integrity_derives_full_records_not_scalar_flags(self):
        context = context_fixture("Change a subsystem", "intelligent")
        self.assertTrue(all(coding.context_checks(context, "Change a subsystem", "intelligent").values()))
        context["citation_integrity"]["results"] = []
        context["checks"] = {"all_ok": True}
        self.assertFalse(coding.context_checks(context, "Change a subsystem", "intelligent")["citations_resolve"])
        context = context_fixture("Change a subsystem", "fast")
        context["registry_after"]["database_sha256"] = "d" * 64
        self.assertFalse(coding.context_checks(context, "Change a subsystem", "fast")["source_registry_preserved"])
        context = context_fixture("Change a subsystem", "fast")
        context["repeat_response"]["model_calls"] = 1
        self.assertFalse(coding.context_checks(context, "Change a subsystem", "fast")["fast_model_free"])
        self.assertFalse(coding.context_checks(context, "Change a subsystem", "fast")["response_hashes"])

    def test_collector_explicit_fast_and_repeat_are_real_cli_paths(self):
        response = {"schema_version": 2, "task": "Implement a change", "model_calls": 0, "evidence": []}
        calls = []
        def cli(binary, project, *arguments, timeout):
            calls.append(arguments)
            return response, 0.01
        with patch.object(coding, "registry_content", return_value=registry_fixture()), patch.object(coding.bench, "subprocess_json", side_effect=cli):
            result = coding.collect_context("unused", Path("unused"), response["task"], "fast", 1, 1000)
        self.assertEqual(len(calls), 2)
        self.assertTrue(all("--fast" in arguments for arguments in calls))
        self.assertTrue(all(result["checks"].values()))
        self.assertEqual(result["usage"]["billed_cost_usd"], 0)

    def test_actual_subprocess_comparison_records_failed_tasks_without_quality_claims(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, result = run_fixture(Path(temporary).resolve())
            self.assertTrue(result["comparison_complete"])
            self.assertFalse(result["human_review_complete"])
            self.assertFalse(result["independent_validation_complete"])
            self.assertIsNone(result["relative_missed_constraint_reduction_vs_fast"])
            self.assertEqual(len(result["reviews_pending"]), 9)
            for setup in coding.SETUPS:
                self.assertEqual(result["setups"][setup]["tasks"], 3)
                self.assertEqual(result["setups"][setup]["passed_tasks"], 0)
                self.assertIsNone(result["setups"][setup]["total_usage"]["billed_cost_usd"])
            for path in (directory / "reviews").glob("*.json"):
                review = cross.read_json(path)
                self.assertNotIn("setup", review)
                self.assertIsNone(review["useful_recommendation"])
                self.assertFalse(review["complete"])

    def test_proposal_bytes_survive_windows_newline_translation_without_relaxing_hashes(self):
        write_text = Path.write_text
        def windows_write_text(path, data, *args, **kwargs):
            if "implementations" in path.parts:
                kwargs["newline"] = "\r\n"
            return write_text(path, data, *args, **kwargs)
        with tempfile.TemporaryDirectory() as temporary:
            # Force Windows text-mode behavior even when this test runs on Unix.
            with patch.object(Path, "write_text", windows_write_text):
                directory, result = run_fixture(Path(temporary).resolve())
            self.assertTrue(result["comparison_complete"])
            metrics = cross.read_json(directory / "metrics.json")
            for sample in metrics["samples"]:
                workspace = directory / "implementations" / sample["sample_id"]
                for name, content in sample["agent_response"]["files"].items():
                    self.assertIn("\n", content)
                    self.assertEqual((workspace / name).read_bytes(), content.encode("utf-8"))
                self.assertEqual(cross.fingerprint(directory / sample["source_root"]), sample["source_manifest"])
            # A later newline conversion is still source drift, even if the
            # recorded implementation manifest is changed to match those bytes.
            sample = metrics["samples"][0]
            workspace = directory / "implementations" / sample["sample_id"]
            name, content = next(iter(sample["agent_response"]["files"].items()))
            (workspace / name).write_bytes(content.replace("\n", "\r\n").encode("utf-8"))
            sample["implementation_manifest"] = cross.fingerprint(workspace)
            cross.write_json(directory / "metrics.json", metrics)
            self.assertFalse(coding.assess(directory)["mechanical_contracts_passed"])

    def test_fallback_is_never_counted_as_intelligent_evaluation(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _ = run_fixture(Path(temporary).resolve())
            metrics = cross.read_json(directory / "metrics.json")
            for sample in metrics["samples"]:
                if sample["setup"] == "intelligent":
                    sample["context"] = context_fixture(sample["task"], "intelligent", fallback=True)
            cross.write_json(directory / "metrics.json", metrics)
            result = coding.assess(directory)
            self.assertTrue(result["comparison_complete"])
            self.assertFalse(result["all_intelligent_tasks_received_intelligence"])
            self.assertEqual(result["setups"]["intelligent"]["fast_fallback_tasks"], 3)
            self.assertEqual(result["setups"]["intelligent"]["measured_intelligent_tasks"], 0)

    def test_source_answer_and_context_tampering_fail_assessment(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _ = run_fixture(Path(temporary).resolve())
            metrics = cross.read_json(directory / "metrics.json")
            sample = next(item for item in metrics["samples"] if item["setup"] == "intelligent")
            sample["context"]["citation_integrity"]["results"] = []
            cross.write_json(directory / "metrics.json", metrics)
            self.assertFalse(coding.assess(directory)["mechanical_contracts_passed"])
        with tempfile.TemporaryDirectory() as temporary:
            directory, _ = run_fixture(Path(temporary).resolve())
            path = next((directory / "answers").glob("*.json"))
            path.write_text("changed after execution")
            self.assertFalse(coding.assess(directory)["mechanical_contracts_passed"])
        with tempfile.TemporaryDirectory() as temporary:
            directory, _ = run_fixture(Path(temporary).resolve())
            path = next((directory / "snapshots").rglob("*.md"))
            path.write_text("changed policy after execution")
            self.assertFalse(coding.assess(directory)["mechanical_contracts_passed"])

    def test_completed_reviews_bind_metrics_and_synthetic_flags_cannot_be_downgraded(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _ = run_fixture(Path(temporary).resolve())
            metrics = cross.read_json(directory / "metrics.json")
            metrics.update(fixture_only=False, held_out=True, independent_projects=True)
            cross.write_json(directory / "metrics.json", metrics)
            for index, path in enumerate(sorted((directory / "reviews").glob("*.json"))):
                review = cross.read_json(path)
                review.update(metrics_sha256=cross.digest(metrics), complete=True, reviewer=f"fixture-reviewer-{index % 2}",
                              reviewed_at="2026-01-01", blind_confirmed=True, missed_critical_constraints=[],
                              useful_recommendation=False, implementation_quality_0_to_3=0,
                              high_severity_unsupported_claims=0, notes="Synthetic annotation testing binding only")
                cross.write_json(path, review)
            result = coding.assess(directory)
            self.assertTrue(result["human_review_complete"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["independent_validation_complete"])
            self.assertIsNone(result["relative_missed_constraint_reduction_vs_fast"])
            metrics["agent_id"] = "changed after review"
            cross.write_json(directory / "metrics.json", metrics)
            self.assertFalse(coding.assess(directory)["human_review_complete"])

    def test_coding_adapter_requires_explicit_hosted_access_and_disables_redirects(self):
        self.assertEqual(coding_agent.endpoint("ollama", None, False), "http://127.0.0.1:11434/api/chat")
        for provider, url in (("openai", None), ("ollama", "http://remote.example:11434"), ("ollama", "http://localhost:11434")):
            with self.assertRaises(ValueError):
                coding_agent.endpoint(provider, url, False)
        with self.assertRaises(ValueError):
            coding_agent.endpoint("openai", "https://secret@example.test/v1", True)
        with self.assertRaises(ValueError):
            coding_agent.endpoint("openai", "http://remote.example/v1", True)
        self.assertIsNone(coding_agent.NoRedirects().redirect_request(None, None, 302, "redirect", {}, "https://elsewhere.test"))

    def test_adapter_rejects_incomplete_refused_and_tool_call_responses(self):
        text = json.dumps({"schema_version": 1, "files": {"src/a.py": "pass\n"}, "summary": "Fixture"})
        openai = {"model": "fixture-model", "status": "completed",
                  "output": [{"type": "message", "content": [{"type": "output_text", "text": text}]}],
                  "usage": {"input_tokens": 100, "output_tokens": 20}}
        ollama = {"model": "fixture-model", "done": True, "done_reason": "stop",
                  "message": {"content": text}, "prompt_eval_count": 100, "eval_count": 20}
        for provider, response in (("openai", openai), ("ollama", ollama)):
            decoded = coding_agent.decode(provider, response)
            self.assertEqual(decoded["usage"]["input_tokens"], 100)
            self.assertEqual(decoded["usage"]["model_calls"], 1)
            self.assertIsNone(decoded["usage"]["billed_cost_usd"])
        for status in (None, "in_progress", "incomplete"):
            with self.assertRaises(ValueError):
                coding_agent.decode("openai", openai | {"status": status})
        for change in ({"done": False}, {"done": None}, {"done_reason": "length"}):
            with self.assertRaises(ValueError):
                coding_agent.decode("ollama", ollama | change)
        for extra in ({"type": "function_call", "arguments": "{}"},
                      {"type": "message", "content": [{"type": "refusal", "refusal": "no"}]}):
            with self.assertRaises(ValueError):
                coding_agent.decode("openai", openai | {"output": openai["output"] + [extra]})
        with self.assertRaises(ValueError):
            coding_agent.decode("ollama", ollama | {"message": {"content": text, "tool_calls": [{"function": "unexpected"}]}})

    def test_adapter_cloud_tags_fail_before_http_and_responses_disable_storage(self):
        for model in ("example:cloud", "example:cloud-preview", "example-cloud"):
            args = argparse.Namespace(provider="ollama", model=model, base_url=None, allow_hosted=False)
            with patch.object(coding_agent, "build_opener") as opener:
                with self.assertRaises(ValueError):
                    coding_agent.generate(args, {"schema_version": 1})
                opener.assert_not_called()
        args = argparse.Namespace(provider="openai", model="fixture-model", base_url="https://provider.example/v1",
                                  allow_hosted=True, api_key_env="BENCHMARK_TEST_KEY", reasoning_effort="medium", timeout=10)
        provider_response = {"model": "fixture-model", "status": "completed", "output": [{"type": "message", "content": [{
            "type": "output_text", "text": json.dumps({"schema_version": 1, "files": {"src/a.py": "pass\n"}, "summary": "Fixture"})}]}]}
        with patch.dict(coding_agent.os.environ, {"BENCHMARK_TEST_KEY": "fixture-key"}), patch.object(coding_agent, "build_opener") as opener:
            opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(provider_response).encode()
            coding_agent.generate(args, {"schema_version": 1})
            request = opener.return_value.open.call_args.args[0]
            self.assertIs(json.loads(request.data)["store"], False)


if __name__ == "__main__":
    unittest.main()
