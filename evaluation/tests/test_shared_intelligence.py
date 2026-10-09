"""Contract fixtures only: no actual provider or human participation is claimed."""
from __future__ import annotations

import argparse
import copy
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_tasks as coding
import cross_source as cross
import shared_intelligence as shared
from test_coding_tasks import registry_fixture
from test_decision_tasks import observation


def fixture_response(project: Path, experience: str, task: str) -> dict:
    schema = 2 if experience == "fast" else 3 if experience == "schema3" else 4
    budget = {"max_tokens": 6000, "used_tokens": 600, "tokenizer": "cl100k_base"}
    core = {"schema_version": schema, "task": task, "model_calls": 0,
        "mode": "fast_fallback", "fallback_reason": "Offline protocol fixture; no inference",
        "budget": budget, "evidence": [{"id": "ev_fixture", "excerpt": "Documentary fixture"}],
        "sections": {}, "checkout_egress": {"allowed": False, "model_received_checkout": False},
        "inspection": {"observations": [], "budget": {"files_read": 0}}}
    if experience in ("adaptive", "onboard"):
        path = sorted((project / "src").glob("*.py"))[0]
        core["inspection"]["observations"] = [observation(path.relative_to(project).as_posix(), path.read_bytes())]
        core["inspection"]["budget"]["files_read"] = 1
    if experience.startswith(("onboard", "adaptive")):
        response = {"schema_version": 1 if experience.startswith("onboard") else 5,
            "intelligence": core, "snapshot": {"project_id": "fixture-project", "registry_revision": "fixture-revision"},
            "budget": budget, "capabilities": {
                "inspection": "disabled_by_caller" if experience.endswith("_no_inspect") else "granted",
                "checkout_egress": False, "hosted_egress": False, "execution": False, "source_write": False}}
        if experience.startswith("onboard"):
            claim = {"basis": "documentary", "text": "A source-grounded fixture claim.",
                "evidence_ids": ["ev_fixture"], "observation_ids": [], "qualifications": []}
            response.update(project="fixture", goal=task, mode="explanation",
                generation_basis="deterministic_fixture", model_calls=0, presentation_model_calls=0,
                orientation={key: [copy.deepcopy(claim)] for key in ("purpose", "concepts", "architecture", "workflow", "constraints", "next_exploration")})
        return response
    return core


def fixture_run(root: Path):
    binary = root / "lore-fixture"
    binary.write_text("No binary execution; mocked protocol fixture identity", encoding="utf-8")
    args = argparse.Namespace(output=root / "run", cases=coding.DEFAULT_CASES,
        lore_binary=binary, provider="ollama", model="fixture-not-a-model", embedding_model=None,
        decision_provider=None, decision_model=None, allow_hosted=False,
        allow_inspection=True, allow_checkout_egress=False,
        generative_base_url=None, decision_base_url=None, timeout=30, max_tokens=6000)
    calls = []
    def cli(binary, project, *arguments, timeout, env=None):
        calls.append(arguments)
        if arguments[0] in ("init", "update"):
            return {"model_calls": 0, "no_op": arguments[0] == "update"}, 0.01
        if arguments[0] == "evidence":
            return {"evidence_id": arguments[1], "excerpt": "Documentary fixture"}, 0.01
        if arguments[0] == "onboard":
            experience = "onboard"
            task = arguments[arguments.index("--topic") + 1]
        else:
            task = arguments[1]
            if "--fast" in arguments:
                experience = "fast"
            else:
                schema = arguments[arguments.index("--schema-version") + 1]
                experience = "schema" + schema if schema in ("3", "4") else "adaptive"
        if "--no-inspect" in arguments:
            experience += "_no_inspect"
        return fixture_response(project, experience, task), 0.01
    with patch.object(shared.bench, "subprocess_json", side_effect=cli), \
            patch.object(shared.coding, "registry_content", return_value=registry_fixture()):
        result = shared.run(args)
    return args.output, result, calls


def assess_fixture(directory: Path):
    with patch.object(shared.coding, "registry_content", return_value=registry_fixture()):
        return shared.assess(directory)


class SharedIntelligenceTests(unittest.TestCase):
    def test_standing_grants_are_per_subprocess_and_never_inherited_accidentally(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            with patch.dict(os.environ, {"LORE_INSPECTION_ROOT": "/unrelated", "LORE_ALLOW_HOSTED_EGRESS": "1",
                                         "LORE_ALLOW_CHECKOUT_EGRESS": "1", "PROVIDER_PRIVATE_TOKEN": "not-written-to-artifacts"}):
                denied = shared.invocation_environment(project, allow_inspection=False, allow_hosted=False, allow_checkout_egress=False)
                self.assertNotIn("LORE_INSPECTION_ROOT", denied)
                self.assertNotIn("LORE_ALLOW_HOSTED_EGRESS", denied)
                self.assertNotIn("LORE_ALLOW_CHECKOUT_EGRESS", denied)
                self.assertEqual(denied["PROVIDER_PRIVATE_TOKEN"], "not-written-to-artifacts")
                local = shared.invocation_environment(project, allow_inspection=True, allow_hosted=False, allow_checkout_egress=False)
                self.assertEqual(local["LORE_INSPECTION_ROOT"], str(project))
                self.assertNotIn("LORE_ALLOW_CHECKOUT_EGRESS", local)
                self.assertEqual(os.environ["LORE_INSPECTION_ROOT"], "/unrelated")

    def test_explicit_schema_and_restriction_arguments_never_enable_execution(self):
        self.assertIn("--fast", shared.arguments("fast", "task", 6000))
        for experience, schema in (("schema3", "3"), ("schema4", "4"), ("adaptive", "5")):
            arguments = shared.arguments(experience, "task", 6000)
            self.assertEqual(arguments[arguments.index("--schema-version") + 1], schema)
            self.assertNotIn("--inspect", arguments)
            self.assertNotIn("--investigate", arguments)
        for experience in ("adaptive_no_inspect", "onboard_no_inspect"):
            self.assertIn("--no-inspect", shared.arguments(experience, "task", 6000))
        with self.assertRaises(ValueError):
            shared.arguments("unknown", "task", 6000)

    def test_original_sources_and_shared_manifests_have_executable_contract_checks(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, result, calls = fixture_run(Path(temporary).resolve())
            self.assertTrue(result["mechanical_contracts_passed"], result["issues"])
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["product_acceptance_passed"])
            self.assertEqual(result["human_learning_outcome"], "unmeasured")
            self.assertEqual(result["agent_productivity_outcome"], "unmeasured")
            self.assertEqual(result["orientation_review_status"], "pending")
            self.assertEqual(len(result["results"]), 3)
            self.assertTrue(any("--schema-version" in arguments and "5" in arguments for arguments in calls))
            for review in (directory / "reviews").glob("*.json"):
                record = cross.read_json(review)
                self.assertTrue(all(slot["complete"] is False for slot in record["assessments"]))

    def test_human_claim_references_must_resolve_in_shared_source_manifests(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "src").mkdir()
            (project / "src/example.py").write_text("value = 'følge'\n", encoding="utf-8")
            original = fixture_response(project, "onboard", "Understand the workflow")
            self.assertEqual(shared.verify_shared_references(original, "onboard", project)["checked"], 1)
            for field, value in (("evidence_ids", ["ev_not_in_core"]), ("observation_ids", ["co_invented"]),
                                 ("text", "Inspect co_inline_fabrication."), ("text", "Based on ev_inline_fabrication.")):
                invalid = copy.deepcopy(original)
                invalid["orientation"]["workflow"][0][field] = value
                with self.assertRaises(ValueError, msg=field):
                    shared.verify_shared_references(invalid, "onboard", project)
            # A caller mentioning an identifier is input data, not a citation.
            original["goal"] = "What does co_caller_literal mean?"
            shared.verify_shared_references(original, "onboard", project)

    def test_source_snapshot_output_and_noop_tampering_fail_reassessment(self):
        for change in ("source", "snapshot", "observation", "no_op", "schema", "execution", "restriction"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                directory, _, _ = fixture_run(Path(temporary).resolve())
                report = cross.read_json(directory / "metrics.json")
                sample = report["samples"][0]
                adaptive = sample["experiences"]["adaptive"]["response"]
                if change == "source":
                    source = directory / sample["source_root"]
                    next((source / "src").glob("*.py")).write_text("changed source", encoding="utf-8")
                elif change == "snapshot":
                    adaptive["snapshot"]["registry_revision"] = "different-source-revision"
                elif change == "observation":
                    adaptive["intelligence"]["inspection"]["observations"][0]["excerpt"] = "fabricated"
                elif change == "no_op":
                    sample["no_op"]["report"]["model_calls"] = 1
                elif change == "schema":
                    adaptive["intelligence"]["schema_version"] = 5
                elif change == "execution":
                    adaptive["capabilities"]["execution"] = True
                else:
                    denied = sample["experiences"]["adaptive_no_inspect"]["response"]
                    denied["intelligence"]["inspection"] = copy.deepcopy(adaptive["intelligence"]["inspection"])
                    sample["experiences"]["adaptive_no_inspect"]["response_sha256"] = cross.digest(denied)
                sample["experiences"]["adaptive"]["response_sha256"] = cross.digest(adaptive)
                cross.write_json(directory / "metrics.json", report)
                self.assertFalse(assess_fixture(directory)["mechanical_contracts_passed"])

    def test_filled_orientation_reviews_do_not_become_human_learning_or_agent_outcomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _ = fixture_run(Path(temporary).resolve())
            for path in (directory / "reviews").glob("*.json"):
                review = cross.read_json(path)
                for index, slot in enumerate(review["assessments"]):
                    slot.update(complete=True, reviewer=f"offline reviewer fixture {index}", reviewed_at="2026-10-10",
                                severe_unsupported_claims=[])
                    for criterion in slot["criteria"].values():
                        criterion.update(score_0_to_3=2, notes="Offline annotation fixture only; no actual learner or model result.")
                cross.write_json(path, review)
            result = assess_fixture(directory)
            self.assertEqual(result["orientation_review_status"], "complete")
            self.assertTrue(result["orientation_review_passed"])
            self.assertFalse(result["product_acceptance_passed"])
            self.assertEqual(result["human_learning_outcome"], "unmeasured")
            path = next((directory / "reviews").glob("*.json"))
            review = cross.read_json(path)
            review["assessments"][1]["reviewer"] = review["assessments"][0]["reviewer"].upper()
            cross.write_json(path, review)
            self.assertFalse(assess_fixture(directory)["orientation_review_passed"])

    def test_source_review_binding_and_fixture_provenance_cannot_be_relabeled(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, _, _ = fixture_run(Path(temporary).resolve())
            report = cross.read_json(directory / "metrics.json")
            report.update(fixture_only=False, held_out=True, independent_projects=True)
            cross.write_json(directory / "metrics.json", report)
            result = assess_fixture(directory)
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["orientation_review_passed"])
            self.assertFalse(result["product_acceptance_passed"])


if __name__ == "__main__":
    unittest.main()
