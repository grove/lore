"""Offline adaptive-task protocol checks; no live agent/model quality claims."""
from __future__ import annotations

import argparse
import copy
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
import decision_tasks as decision
from test_coding_tasks import registry_fixture
from test_decision_tasks import observation, complete_reviews
from test_shared_intelligence import relationship_fixture, fixture_resolution

GRANTS = ("LORE_INSPECTION_ROOT", "LORE_ALLOW_HOSTED_EGRESS", "LORE_ALLOW_CHECKOUT_EGRESS")


def response_fixture(project: Path, task: str, argv, env: dict) -> dict:
    schema = 2 if "--fast" in argv else int(argv[argv.index("--schema-version") + 1])
    budget = {"max_tokens": int(argv[argv.index("--max-tokens") + 1]), "used_tokens": 600, "tokenizer": "cl100k_base"}
    inspecting = bool(env.get("LORE_INSPECTION_ROOT")) and schema in (4, 5)
    core = {"schema_version": min(schema, 4), "task": task, "model_calls": 0,
        "mode": "fast_fallback", "cache_status": "unused", "budget": budget,
        "fallback_reason": "Offline protocol fixture; no inference performed",
        "evidence": [{"id": "ev_fixture", "source": "docs:contract.md", "excerpt": "Documentary fixture"}],
        "checkout_egress": {"allowed": False, "model_received_checkout": False},
        "inspection": {"observations": [], "budget": {"files_read": 0}}}
    if inspecting:
        filename = sorted((project / "src").glob("*.py"))[0]
        core["inspection"]["observations"] = [observation(filename.relative_to(project).as_posix(), filename.read_bytes())]
        core["inspection"]["budget"]["files_read"] = 1
    if schema != 5:
        return core
    return {"schema_version": 5, "intelligence": core, "budget": budget,
        "snapshot": {"project_id": "fixture-project", "registry_revision": "fixture-registry"},
        "capabilities": {"inspection": "granted" if inspecting else "disabled_by_caller",
            "hosted_egress": False, "checkout_egress": False, "execution": False, "source_write": False}}


def run_fixture(root: Path, *, inspect=False):
    binary = root / "lore-fixture"
    binary.write_text("Offline executable identity; CLI protocol is a double, agent/checkers are real subprocesses", encoding="utf-8")
    program = """import json,os,sys
request=json.load(sys.stdin)
assert 'LORE_ALLOW_HOSTED_EGRESS' not in os.environ
assert 'LORE_ALLOW_CHECKOUT_EGRESS' not in os.environ
assert os.environ.get('LORE_INSPECTION_ROOT') != '/ambient-outside-grant'
print(json.dumps({'schema_version':1,'summary':'Unimplemented offline subprocess fixture; no inference',
 'files':{name:request['files'][name] for name in request['editable_files']},
 'usage':{'model_calls':0,'input_tokens':0,'output_tokens':0}}))
"""
    args = argparse.Namespace(output=root / "run", cases=coding.DEFAULT_CASES, lore_binary=binary,
        provider="ollama", model="fixture-not-a-model", embedding_model=None,
        decision_provider=None, decision_model=None, generative_base_url=None, decision_base_url=None,
        allow_hosted=False, allow_inspection=inspect, allow_checkout_egress=False,
        agent_command=json.dumps([sys.executable, "-c", program]), agent_id="offline adapter; no inference",
        agent_location="local", timeout=30, max_tokens=6000)
    cli_calls, execution_calls = [], []
    def cli(binary, project, *argv, timeout, env=None):
        cli_calls.append((str(project), argv, {key: env.get(key) for key in GRANTS}))
        if argv[0] == "init":
            return {"model_calls": 0}, 0.01
        if argv[0] == "evidence":
            return {"evidence_id": argv[1], "excerpt": "Documentary fixture"}, 0.01
        return response_fixture(project, argv[1], argv, env), 0.01
    invoke = coding.invoke_json
    def child(command, cwd, timeout, request=None, *, env=None):
        if request is not None:
            previous_contexts = [argv for _, argv, _ in cli_calls
                                 if argv[0] == "context" and argv[1] == request["task"]]
            if len(previous_contexts) != 2 * (len(adaptive.SETUPS) - 1):
                raise AssertionError("Every arm's unsolved warm-up and served context must precede coding")
            if set(request) & {"test_command", "verification_files", "warmup_response", "answer"}:
                raise AssertionError("A checker, warm-up packet or answer leaked into the agent protocol")
        execution_calls.append(("agent" if request is not None else "verification", str(cwd),
                                {key: env.get(key) for key in GRANTS}))
        return invoke(command, cwd, timeout, request, env=env)
    with patch.object(coding.bench, "subprocess_json", side_effect=cli), \
         patch.object(coding, "registry_content", return_value=registry_fixture()), \
         patch.object(coding, "invoke_json", side_effect=child):
        result = adaptive.run(args)
    return args.output, result, cli_calls, execution_calls


def reassess(directory):
    with patch.object(coding, "registry_content", return_value=registry_fixture()):
        return adaptive.assess(directory)


class AdaptiveTaskTests(unittest.TestCase):
    def test_arguments_preserve_legacy_arms_and_explicit_adaptive_cache_policy(self):
        policy = {"allow_inspection": True, "allow_hosted": False, "allow_checkout_egress": False,
                  "max_tokens": 6000, "timeout": 30}
        self.assertEqual(adaptive.SETUPS[:4], decision.SETUPS)
        self.assertEqual(adaptive.arguments("fast", "task", policy), ["context", "task", "--max-tokens", "6000", "--fast"])
        for arm, schema in (("lore05", "3"), ("lore06", "4"), ("adaptive_no_reuse", "5"), ("adaptive_reuse", "5")):
            argv = adaptive.arguments(arm, "task", policy)
            self.assertEqual(argv[argv.index("--schema-version") + 1], schema)
            self.assertEqual("--no-cache" in argv, arm == "adaptive_no_reuse")
            self.assertEqual("--investigate" in argv, arm == "lore06")
            self.assertEqual("--inspect" in argv, arm == "lore06")
        policy["allow_inspection"] = False
        for arm in (*adaptive.ADAPTIVE, "lore06"):
            self.assertIn("--no-inspect", adaptive.arguments(arm, "task", policy))
        for inspect, hosted in ((False, True), (True, False)):
            with self.assertRaises(ValueError):
                adaptive.policy_from_args(argparse.Namespace(allow_inspection=inspect, allow_hosted=hosted,
                    allow_checkout_egress=True, max_tokens=6000, timeout=30))

    def test_environment_is_private_per_child_and_ambient_grants_cannot_leak(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            policy = {"allow_inspection": False, "allow_hosted": False, "allow_checkout_egress": False}
            ambient = dict(zip(GRANTS, ("/ambient-outside-grant", "1", "1")))
            with patch.dict(os.environ, ambient | {"ADAPTIVE_TEST_PRIVATE_CREDENTIAL": "fixture-private"}):
                before = dict(os.environ)
                environment = adaptive.environment_factory(policy)
                env = environment(root, "agent")
                self.assertTrue(all(key not in env for key in GRANTS))
                self.assertEqual(env["ADAPTIVE_TEST_PRIVATE_CREDENTIAL"], "fixture-private")
                env["LORE_ALLOW_HOSTED_EGRESS"] = "1"
                self.assertNotIn("LORE_ALLOW_HOSTED_EGRESS", environment(root, "verification"))
                result, _ = coding.invoke_json([sys.executable, "-c", "import json,os; print(json.dumps({k:os.environ.get(k) for k in " + repr(GRANTS) + "}))"],
                    root, 5, env=environment(root, "agent"))
                self.assertEqual(result, dict.fromkeys(GRANTS))
                self.assertEqual(dict(os.environ), before)
            policy.update(allow_inspection=True, allow_hosted=True, allow_checkout_egress=True)
            env = adaptive.environment_factory(policy)(root, "context")
            self.assertEqual(env["LORE_INSPECTION_ROOT"], str(root))
            self.assertEqual(env["LORE_ALLOW_CHECKOUT_EGRESS"], "1")

    def test_six_matched_arms_execute_real_agents_and_independent_failed_checks(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ,
                dict(zip(GRANTS, ("/ambient-outside-grant", "1", "1")))):
            directory, result, calls, children = run_fixture(Path(temporary).resolve())
            self.assertTrue(result["comparison_complete"], result["issues"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["independent_validation_complete"])
            self.assertFalse(result["productivity_benefit_established"])
            self.assertEqual(result["agent_quality_status"], "unmeasured")
            self.assertEqual(len(result["reviews_pending"]), 18)
            self.assertEqual(len(children), 36)
            self.assertEqual({stage for stage, _, _ in children}, {"agent", "verification"})
            self.assertTrue(all(not any(grants.values()) for _, _, grants in calls + children))
            contexts = [(root, argv) for root, argv, _ in calls if argv[0] == "context"]
            self.assertEqual(len(contexts), 30)
            self.assertEqual(len({root for root, _ in contexts}), 15)
            for arm in adaptive.SETUPS:
                total = result["setups"][arm]
                self.assertEqual(total["tasks"], 3)
                self.assertEqual(total["passed_tasks"], 0)
                self.assertIsNone(total["total_usage"]["billed_cost_usd"])
            report = cross.read_json(directory / "metrics.json")
            self.assertNotIn("ambient-outside-grant", json.dumps(report))
            self.assertIn("both warm-up and served context", report["cost_scope"])
            for sample in report["samples"]:
                if sample["context"]:
                    context = sample["context"]
                    self.assertAlmostEqual(sample["costs"]["context_seconds"], 0.02 + context["isolation_seconds"])
                    self.assertEqual(context["response"]["schema_version"], 5 if sample["setup"] in adaptive.ADAPTIVE else
                                     4 if sample["setup"] == "lore06" else 3 if sample["setup"] == "lore05" else 2)
                    self.assertIn("sha256", context["initialized_project_manifest"])
                self.assertTrue(sample["tests"]["provenance"]["files_sha256"])
            for review in (directory / "reviews").glob("*.json"):
                self.assertNotIn("setup", cross.read_json(review))

    def test_inspection_grants_follow_each_copy_and_exact_observations_are_checked(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, result, calls, children = run_fixture(Path(temporary).resolve(), inspect=True)
            self.assertTrue(result["comparison_complete"], result["issues"])
            for root, _, grants in calls:
                self.assertEqual(grants["LORE_INSPECTION_ROOT"], root)
            for stage, root, grants in children:
                self.assertIn("implementations", Path(grants["LORE_INSPECTION_ROOT"]).parts)
                if stage == "verification":
                    self.assertEqual(grants["LORE_INSPECTION_ROOT"], root)
            report = cross.read_json(directory / "metrics.json")
            sample = next(item for item in report["samples"] if item["setup"] == "adaptive_reuse")
            sample["context"]["response"]["intelligence"]["inspection"]["observations"][0]["excerpt"] = "invented source"
            sample["context"]["response_sha256"] = cross.digest(sample["context"]["response"])
            cross.write_json(directory / "metrics.json", report)
            self.assertFalse(reassess(directory)["mechanical_contracts_passed"])

    def test_assessment_recomputes_arguments_grants_snapshots_warmup_costs_and_requests(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _, _ = run_fixture(Path(temporary).resolve())
            original = cross.read_json(directory / "metrics.json")
            for mutation in ("arguments", "grants", "snapshot", "warmup_cost", "request", "configuration", "model", "answer_observation"):
                with self.subTest(mutation=mutation):
                    report = copy.deepcopy(original)
                    sample = next(item for item in report["samples"] if item["setup"] == "adaptive_no_reuse")
                    context = sample["context"]
                    if mutation == "arguments":
                        context["arguments"].remove("--no-cache")
                    elif mutation == "grants":
                        context["host_grants"]["hosted_egress"] = True
                    elif mutation == "snapshot":
                        context["response"]["snapshot"]["registry_revision"] = "different-current-registry"
                    elif mutation == "warmup_cost":
                        sample["costs"]["context_seconds"] = context["served_elapsed_seconds"]
                    elif mutation == "request":
                        sample["request_sha256"] = "0" * 64
                    elif mutation == "configuration":
                        sample["configuration_sha256"] = "0" * 64
                    elif mutation == "model":
                        for item in report["samples"]:
                            item["agent_response"]["provider_model"] = "model-a"
                        sample["agent_response"]["provider_model"] = "model-b"
                    else:
                        sample["agent_response"]["summary"] += " co_unavailable"
                    context["response_sha256"] = cross.digest(context["response"])
                    context["response_json_bytes"] = len(json.dumps(context["response"]).encode("utf-8"))
                    cross.write_json(directory / "metrics.json", report)
                    self.assertFalse(reassess(directory)["mechanical_contracts_passed"])

    def test_no_reuse_arm_cannot_claim_a_cache_hit_or_write_saved_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _, _ = run_fixture(Path(temporary).resolve())
            report = cross.read_json(directory / "metrics.json")
            sample = next(item for item in report["samples"] if item["setup"] == "adaptive_no_reuse")
            cache = directory / sample["context"]["checkout_root"] / ".lore" / "context-cache"
            cache.mkdir(parents=True)
            (cache / "unexpected.json").write_text("{}", encoding="utf-8")
            sample["context"]["cache_after"] = adaptive.cache_state(directory / sample["context"]["checkout_root"])
            cross.write_json(directory / "metrics.json", report)
            self.assertFalse(reassess(directory)["mechanical_contracts_passed"])
            (cache / "unexpected.json").unlink()
            sample["context"]["cache_after"] = adaptive.cache_state(directory / sample["context"]["checkout_root"])
            sample["context"]["response"]["intelligence"]["cache_status"] = "hit"
            sample["context"]["response_sha256"] = cross.digest(sample["context"]["response"])
            cross.write_json(directory / "metrics.json", report)
            self.assertFalse(reassess(directory)["mechanical_contracts_passed"])

    def test_served_cache_hit_is_bound_to_unsolved_warmup_and_both_costs_remain(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source, checkout = root / "source", root / "checkout"
            (source / "src").mkdir(parents=True)
            (source / "src/adapter.py").write_text("def operation():\n    raise NotImplementedError\n", encoding="utf-8")
            cross.copy_snapshot(source, checkout)
            policy = {"allow_inspection": True, "allow_hosted": False, "allow_checkout_egress": False,
                      "max_tokens": 6000, "timeout": 5}
            count = 0
            def cli(binary, project, *argv, timeout, env=None):
                nonlocal count
                if argv[0] == "evidence":
                    return {"evidence_id": argv[1]}, 0.01
                count += 1
                response = response_fixture(project, argv[1], argv, env)
                nested = response["intelligence"]
                nested.update(mode="intelligent", model_calls=2 if count == 1 else 0,
                    cache_status="miss" if count == 1 else "hit",
                    revision_key="blake3:" + "f" * 64,
                    brief={"revision_key": "blake3:" + "f" * 64,
                        "preferred_approach": {"text": "Offline contract fixture only.", "evidence_ids": ["ev_fixture"]}},
                    investigation={"steps": [{"uncertainty": "Offline trace fixture; no investigation executed.",
                        "observation_ids": [nested["inspection"]["observations"][0]["id"]]}],
                        "stop_reason": "fixture_complete" if count == 1 else "cache_reused"})
                cache = project / ".lore" / "context-cache"
                cache.mkdir(parents=True, exist_ok=True)
                (cache / "fixture.json").write_text('{"fixture_only":true}', encoding="utf-8")
                return response, 0.03 if count == 1 else 0.01
            with patch.object(coding.bench, "subprocess_json", side_effect=cli), \
                 patch.object(coding, "registry_content", return_value=registry_fixture()):
                record = adaptive.collect_context("fixture", checkout, source, "Implement operation", "adaptive_reuse",
                                                  policy, adaptive.environment_factory(policy))
                self.assertTrue(record["checks"]["hit_bound_to_warmup"])
                self.assertEqual(record["warmup_usage"]["model_calls"], 2)
                self.assertEqual(record["served_usage"]["model_calls"], 0)
                self.assertEqual(record["usage"]["model_calls"], 2)
                self.assertAlmostEqual(record["elapsed_seconds"], 0.04)
                record["response"]["intelligence"]["brief"]["preferred_approach"]["text"] = "Different unbound advice"
                self.assertFalse(adaptive.context_checks(record, "Implement operation", "adaptive_reuse", source, checkout,
                                                         policy)["hit_bound_to_warmup"])

    def test_cached_decision_allows_optional_packing_changes_but_retains_every_required_premise(self):
        brief = {"revision_key": "blake3:" + "a" * 64,
            "preferred_approach": {"text": "Keep the source condition.", "evidence_ids": ["ev_rule"]},
            "constraints": [{"knowledge_id": "ku_rule", "explanation": "Required scope remains unchanged."}],
            "facts": [{"record_id": "ku_rule", "statement": "The complete original condition."}],
            "checks": [{"priority": "required_before_proceeding", "action": "Resolve the condition."}],
            "completion_criteria": [{"text": "The required behavior remains."}], "heuristics": []}
        first = {"revision_key": brief["revision_key"], "brief": brief}
        served = copy.deepcopy(first)
        served["brief"]["heuristics"] = [{"principle": "Optional general guidance."}]
        served["brief"]["facts"].append({"record_id": "ku_optional", "statement": "Additional source fact."})
        served["brief"]["checks"].append({"priority": "optional_follow_up", "action": "Optional later review."})
        served["brief"]["completion_criteria"].append({"text": "Another complete criterion fits."})
        self.assertTrue(adaptive.same_cached_premises(first, served, "adaptive_reuse"))
        for field in ("preferred_approach", "constraints", "facts", "checks", "completion_criteria", "revision_key"):
            invalid = copy.deepcopy(served)
            if field == "revision_key":
                invalid["brief"][field] = "different-revision"
            elif field == "preferred_approach":
                invalid["brief"][field]["text"] = "A different action."
            else:
                invalid["brief"][field][0] = {"knowledge_id": "ku_rule", "record_id": "ku_rule", "text": "Changed premise."}
            self.assertFalse(adaptive.same_cached_premises(first, invalid, "adaptive_reuse"), field)

    def test_fixture_reviews_cannot_be_promoted_to_measured_agent_quality(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _, _ = run_fixture(Path(temporary).resolve())
            report = cross.read_json(directory / "metrics.json")
            report.update(fixture_only=False, held_out=True, independent_projects=True)
            cross.write_json(directory / "metrics.json", report)
            complete_reviews(directory)
            result = reassess(directory)
            self.assertTrue(result["human_review_complete"], result["issues"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["independent_validation_complete"])
            self.assertFalse(result["all_adaptive_tasks_received_intelligence"])
            self.assertFalse(result["productivity_benefit_established"])
            self.assertIsNone(result["iteration_outcomes"])
            self.assertEqual(result["integrity_audit"]["status"], "unmeasured")

    def test_schema5_relationship_manifest_is_checked_against_retained_originals(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _, _ = run_fixture(Path(temporary).resolve())
            report = cross.read_json(directory / "metrics.json")
            sample = next(item for item in report["samples"] if item["setup"] == "adaptive_reuse")
            record = sample["context"]
            manifest, originals = relationship_fixture()
            originals["ev_fixture"] = {"evidence_id": "ev_fixture", "excerpt": "Documentary fixture"}
            record["citation_integrity"] = fixture_resolution(originals)
            for label in ("response", "warmup_response"):
                record[label]["source_relationships"] = copy.deepcopy(manifest)
                record[label + "_sha256"] = cross.digest(record[label])
            record["response_json_bytes"] = len(json.dumps(record["response"]).encode("utf-8"))
            record["warmup_json_bytes"] = len(json.dumps(record["warmup_response"]).encode("utf-8"))
            policy = report["study"]["policy"]
            with patch.object(coding, "registry_content", return_value=registry_fixture()):
                checked = adaptive.context_checks(record, sample["task"], sample["setup"],
                    directory / sample["source_root"], directory / record["checkout_root"], policy)
                self.assertTrue(all(checked.values()))
                record["response"]["source_relationships"]["imported_evidence"][0]["verification"] = "verified"
                with self.assertRaises(ValueError):
                    adaptive.context_checks(record, sample["task"], sample["setup"],
                        directory / sample["source_root"], directory / record["checkout_root"], policy)

    def test_preparation_is_offline_and_preserves_existing_runner_setup_constants(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(coding.bench, "subprocess_json", side_effect=AssertionError("Preparation must not invoke Lore")):
                report = adaptive.prepare(root / "prepared", coding.DEFAULT_CASES)
            self.assertEqual(report["inference_calls"], 0)
            self.assertEqual(report["adaptive_protocol"], adaptive.PROTOCOL)
            self.assertEqual(report["setups"], list(adaptive.SETUPS))
            self.assertEqual(coding.SETUPS, ("baseline", "fast", "intelligent"))
            self.assertEqual(decision.SETUPS, ("baseline", "fast", "lore05", "lore06"))


if __name__ == "__main__":
    unittest.main()
