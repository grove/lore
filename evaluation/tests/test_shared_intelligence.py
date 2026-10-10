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


def relationship_fixture() -> tuple[dict, dict]:
    """Source-shaped fixture and independently retained resolver responses."""
    document = {"id": "ev_relationship", "source": "docs:queue.md", "source_revision_id": "sr_queue",
        "material": "primary", "origin": None, "provenance_recorded": True, "current": True,
        "excerpt": "Queue order is preserved except emergency drains after tenant acknowledgement.",
        "line_start": 2, "line_end": 2}
    knowledge = {"id": "ku_queue", "statement": "Queue order must be preserved.", "kind": "decision",
        "lifecycle": "accepted", "support_state": "current_documentary_support", "topic": "queue",
        "subject": "queue ordering", "scope": "production", "documentary_basis": "documented_intent",
        "evidence_ids": [document["id"]], "qualifications": [], "relevance": []}
    native = {"native_id": "queue-42", "kind": "work_state", "title": "Queue follow-up",
        "subject": "queue ordering", "statement": "Closed after a staging replay; production was not checked.",
        "scope": {"repository": None, "component": None, "environment": "staging"},
        "lifecycle": "closed", "verification": "reported", "observed_at": "2026-10-09",
        "evidence": [{"locator": "record://queue-42", "revision": "opaque-upstream-rev", "field": None}],
        "metadata": {"basis": "upstream_note", "text": "co_source_literal is example input data.",
                     "evidence_ids": ["upstream_external_17"]}}
    original_native = {"id": "no_queue", "evidence_id": "ne_queue", "snapshot_id": "ns_queue",
        "import_id": "work", "origin": "beads", "source_path": "work.jsonl", "format": "fixture-work-v1",
        "content_hash": "blake3:fixture-native-content", "captured_at": "2026-10-09", "current": True,
        "record": copy.deepcopy(native)}
    imported = {key: original_native[key] for key in
                ("snapshot_id", "import_id", "origin", "format", "content_hash", "captured_at", "current")}
    imported.update(id="ne_queue", observation_id="no_queue", native_id=native["native_id"],
        source=original_native["source_path"], observed_at=native["observed_at"], verification=native["verification"],
        locators=copy.deepcopy(native["evidence"]), metadata=copy.deepcopy(native["metadata"]))
    observation_record = {key: copy.deepcopy(native[key]) for key in
                          ("native_id", "kind", "title", "subject", "statement", "scope", "lifecycle", "verification")}
    observation_record.update(id="no_queue", import_id="work", origin="beads", current=True,
        freshness="latest_import_not_current_behavior_verification", evidence_ids=["ne_queue"],
        qualifications=["Reported work state; closure does not establish production behavior."], relevance=[])
    relation = {"id": "xrel_queue", "input_signature": "fixture-pair-signature",
        "from": {"kind": "observation", "id": "no_queue", "revision_id": "ns_queue"},
        "to": {"kind": "knowledge", "id": "ku_queue", "revision_id": "kr_queue"},
        "kind": "verification_question", "reason": "Check the scope of the staging-only work report.",
        "qualifications": ["A reported staging replay neither verifies production nor replaces accepted policy."],
        "evidence_ids": ["ev_relationship", "ne_queue"], "upstream_kind": None,
        "upstream_status": None, "upstream_active": None, "review_id": None, "active": True}
    discrepancy = {key: copy.deepcopy(relation[key]) for key in
                   ("id", "kind", "reason", "qualifications", "evidence_ids", "review_id")}
    discrepancy.update(status="unresolved", knowledge_ids=["ku_queue"], observation_ids=["no_queue"])
    manifest = {"relations": [relation], "discrepancies": [discrepancy], "knowledge": [knowledge],
        "knowledge_revisions": {"ku_queue": "kr_queue"}, "observations": [observation_record],
        "evidence": [document], "imported_evidence": [imported]}
    original_document = {key: document[key] for key in
                         ("id", "source_revision_id", "excerpt", "material", "origin", "line_start", "line_end")}
    original_document.update(root="docs", observed_path="queue.md", root_path="docs")
    return manifest, {"ev_relationship": original_document, "ne_queue": original_native}


def fixture_resolution(originals: dict) -> dict:
    return {"results": [{"evidence_id": identity, "response": original,
                         "response_sha256": cross.digest(original), "error": None}
                        for identity, original in originals.items()],
            "checked": len(originals), "resolvable": len(originals), "failures": []}


def fixture_run(root: Path, *, with_relationships=False):
    binary = root / "lore-fixture"
    binary.write_text("No binary execution; mocked protocol fixture identity", encoding="utf-8")
    args = argparse.Namespace(output=root / "run", cases=coding.DEFAULT_CASES,
        lore_binary=binary, provider="ollama", model="fixture-not-a-model", embedding_model=None,
        decision_provider=None, decision_model=None, allow_hosted=False,
        allow_inspection=True, allow_checkout_egress=False,
        generative_base_url=None, decision_base_url=None, timeout=30, max_tokens=6000)
    calls = []
    relationship_manifest, relationship_originals = relationship_fixture()
    def cli(binary, project, *arguments, timeout, env=None):
        calls.append(arguments)
        if arguments[0] in ("init", "update"):
            return {"model_calls": 0, "no_op": arguments[0] == "update"}, 0.01
        if arguments[0] == "evidence":
            if with_relationships and arguments[1] in relationship_originals:
                return copy.deepcopy(relationship_originals[arguments[1]]), 0.01
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
        response = fixture_response(project, experience, task)
        if with_relationships and experience.startswith(("adaptive", "onboard")):
            response["source_relationships"] = copy.deepcopy(relationship_manifest)
            if experience.startswith("onboard"):
                response["orientation"]["workflow"][0]["evidence_ids"] = ["ev_relationship", "ne_queue"]
        return response, 0.01
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

    def test_relationship_manifest_can_supply_source_ids_after_optional_brief_facts_are_omitted(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            response = fixture_response(project, "onboard_no_inspect", "Understand queue ordering")
            manifest, originals = relationship_fixture()
            response["source_relationships"] = manifest
            response["orientation"]["workflow"][0]["evidence_ids"] = ["ev_relationship", "ne_queue"]
            self.assertEqual(shared.verify_shared_references(response, "onboard_no_inspect", project)["checked"], 0)
            self.assertEqual(shared.response_citations(response), {"ev_fixture", "ev_relationship", "ne_queue"})
            shared.verify_relationship_sources(response, fixture_resolution(originals))
            # Metadata can contain source-native identifiers or example prose.
            # Neither grants evidence or static-observation citations to a claim.
            for field, value in (("evidence_ids", ["upstream_external_17"]),
                                 ("observation_ids", ["co_source_literal"])):
                forged = copy.deepcopy(response)
                forged["orientation"]["workflow"][0][field] = value
                with self.assertRaises(ValueError):
                    shared.verify_shared_references(forged, "onboard_no_inspect", project)

    def test_relationship_manifest_rejects_dangling_revisions_evidence_and_lost_qualifications(self):
        mutations = {
            "missing_endpoint": lambda m: m["relations"][0]["from"].update(id="no_absent"),
            "wrong_endpoint_kind": lambda m: m["relations"][0]["from"].update(kind="knowledge"),
            "knowledge_revision": lambda m: m["knowledge_revisions"].update(ku_queue="kr_other"),
            "native_revision": lambda m: m["relations"][0]["from"].update(revision_id="ns_other"),
            "missing_evidence": lambda m: m["evidence"].clear(),
            "substituted_support": lambda m: m["relations"][0].update(evidence_ids=["ev_fixture", "ne_queue"]),
            "lost_condition": lambda m: m["discrepancies"][0].update(qualifications=[]),
            "invented_disposition": lambda m: m["discrepancies"][0].update(status="resolved"),
            "orphan_evidence": lambda m: m["evidence"].append(dict(m["evidence"][0], id="ev_unrelated")),
            "duplicate_evidence": lambda m: m["evidence"].append(copy.deepcopy(m["evidence"][0])),
            "missing_discrepancy": lambda m: m["discrepancies"].clear(),
            "foreign_native_binding": lambda m: m["imported_evidence"][0].update(observation_id="no_foreign"),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name):
                manifest, _ = relationship_fixture()
                mutate(manifest)
                with self.assertRaises(ValueError):
                    shared.verify_source_relationships({"source_relationships": manifest})

    def test_relationship_source_payload_must_match_resolved_originals(self):
        mutations = {
            "rare_exception": lambda m: m["evidence"][0].update(excerpt="Queue order is always preserved."),
            "source_revision": lambda m: m["evidence"][0].update(source_revision_id="sr_replaced"),
            "native_scope": lambda m: m["observations"][0]["scope"].update(environment="production"),
            "native_claim": lambda m: m["observations"][0].update(statement="Production was independently verified."),
            "source_metadata": lambda m: m["imported_evidence"][0]["metadata"].update(text="Fabricated source note."),
            "native_path": lambda m: m["imported_evidence"][0].update(source="different-work.jsonl"),
            "opaque_revision": lambda m: m["imported_evidence"][0]["locators"][0].update(revision="other-revision"),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name):
                manifest, originals = relationship_fixture()
                mutate(manifest)
                with self.assertRaises(ValueError):
                    shared.verify_relationship_sources({"source_relationships": manifest}, fixture_resolution(originals))

    def test_historical_endpoint_evidence_is_preserved_without_becoming_current_relation_support(self):
        manifest, originals = relationship_fixture()
        historical = dict(manifest["evidence"][0], id="ev_history", source_revision_id="sr_history",
                          current=False, excerpt="Previous draft considered unbounded admission.")
        manifest["evidence"].append(historical)
        manifest["knowledge"][0]["evidence_ids"].append("ev_history")
        originals["ev_history"] = dict(originals["ev_relationship"], id="ev_history",
            source_revision_id="sr_history", excerpt=historical["excerpt"])
        evidence, _ = shared.verify_source_relationships({"source_relationships": manifest})
        self.assertIn("ev_history", evidence)
        shared.verify_relationship_sources({"source_relationships": manifest}, fixture_resolution(originals))
        manifest["relations"][0]["evidence_ids"].append("ev_history")
        with self.assertRaises(ValueError):
            shared.verify_source_relationships({"source_relationships": manifest})

    def test_collected_relationship_payloads_are_checked_again_during_assessment(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, result, calls = fixture_run(Path(temporary).resolve(), with_relationships=True)
            self.assertTrue(result["mechanical_contracts_passed"], result["issues"])
            self.assertTrue(any(arguments == ("evidence", "ne_queue") for arguments in calls))
            self.assertFalse(any("upstream_external_17" in arguments for arguments in calls))
            report = cross.read_json(directory / "metrics.json")
            adaptive = report["samples"][0]["experiences"]["adaptive"]
            adaptive["response"]["source_relationships"]["evidence"][0]["excerpt"] = "Removed the rare exception."
            adaptive["response_sha256"] = cross.digest(adaptive["response"])
            cross.write_json(directory / "metrics.json", report)
            self.assertFalse(assess_fixture(directory)["mechanical_contracts_passed"])


if __name__ == "__main__":
    unittest.main()
