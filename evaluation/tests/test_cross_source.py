"""Offline contract fixtures; these tests are not a real comparative evaluation."""
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cross_source as cross


def registry_fixture(number: int) -> dict:
    return {"counts": {table: 2 for table in cross.COUNTER_TABLES},
            "native_current_sha256": cross.digest([number, "unit-test snapshots"]),
            "native_evidence_ids": ["ne_" + cross.digest([number, label]) for label in ("a", "b")],
            "sqlite_integrity_ok": True, "database_sha256": cross.digest([number, "unit-test database"]),
            "wiki_sha256": {}}


def contract_fixture(root: Path, number: int, manifest: dict) -> dict:
    """Simulate collector bookkeeping only. No CLI or agent execution is claimed."""
    state = registry_fixture(number)
    response = {"task": "Synthetic task", "model_calls": 0,
                "imported_evidence": [{"id": identity} for identity in state["native_evidence_ids"]]}
    results = [{"evidence_id": identity, "response": {"evidence_id": identity},
                "response_sha256": cross.digest({"evidence_id": identity}), "error": None}
               for identity in state["native_evidence_ids"]]
    phase = {"contexts": [{"task": response["task"], "response": response, "response_sha256": cross.digest(response)}],
             "registry_before_retrieval": state, "registry_after_retrieval": state,
             "citation_integrity": {"checked": 2, "resolvable": 2, "mechanically_verified": True,
                                    "failures": [], "results": results,
                                    "expected_imported_evidence_ids": state["native_evidence_ids"],
                                    "expected_cited_evidence_ids": state["native_evidence_ids"]}}
    return cross.write_run_contract(root / f"contract-{number}.json", manifest, phase,
                                    {"before": state, "after": state, "report": {"no_op": True, "model_calls": 0}},
                                    {"collector": "synthetic offline unit test; not actual execution",
                                     "lore_binary_sha256": cross.digest("test executable"),
                                     "configuration_sha256": cross.digest("test configuration")})


def modify_contract(root: Path, run: dict, edit) -> None:
    path = root / run["run_contract"]["path"]
    artifact = cross.read_json(path)
    edit(artifact)
    cross.write_json(path, artifact)
    run["run_contract"]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()


def study_fixture(root: Path, *, fixture_only=False) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    cases = []
    for number in range(3):
        source = root / f"source-{number}"
        source.mkdir(exist_ok=True)
        (source / "source.txt").write_text(f"Unique synthetic unit-test source {number}.\n", encoding="utf-8")
        manifest = cross.fingerprint(source)
        setups = {}
        for setup in cross.SETUPS:
            answer = f"answer-{number}-{setup}.txt"
            text = "Synthetic reviewed response. No agent was run.\n"
            if setup == "tools_lore":
                text += "Simulated reference: " + registry_fixture(number)["native_evidence_ids"][0] + "\n"
            (root / answer).write_text(text, encoding="utf-8")
            setups[setup] = {
                "answer_path": answer, "coding_agent": "unit-test-only",
                "cost": {"preparation_seconds": 10, "retrieval_seconds": 2,
                         "preparation_model_calls": 3, "retrieval_model_calls": 0,
                         "preparation_includes_all_tools": True, "billed_cost_usd": None},
                "citation_integrity": {"checked": 2, "resolvable": 2, "mechanically_verified": True},
                "no_op": {"no_op": True, "model_calls": 0, "knowledge_churn": 0, "registry_unchanged": True},
            }
            if setup == "tools_lore":
                setups[setup]["run_contract"] = contract_fixture(root, number, manifest)
        cases.append({"project": f"fixture-project-{number}", "case_id": "task",
                      "task": "Synthetic task", "source_root": source.name, "source_fingerprint": manifest["sha256"],
                      "critical_constraints": ["Preserve safety invariant", "Preserve environment scope"], "setups": setups})
    study = {"schema_version": 1, "fixture_only": fixture_only, "held_out": True,
             "independent_projects": True, "cases": cases}
    path = root / "study.json"
    cross.write_json(path, study)
    return path


def reviewed_packets(root: Path, *, fixture_only=False, edit_study=None) -> Path:
    study_path = study_fixture(root / "study", fixture_only=fixture_only)
    if edit_study is not None:
        study = cross.read_json(study_path)
        edit_study(study, study_path.parent)
        cross.write_json(study_path, study)
    output = root / "blind"
    cross.blind(study_path, output)
    assignment = cross.read_json(output / "assignment.json")
    for index, sample in enumerate(assignment["samples"]):
        path = output / "reviews" / f"{sample['sample_id']}.json"
        review = cross.read_json(path)
        review.update({"reviewer": f"unit-test-reviewer-{index % 2}", "reviewed_at": "2026-10-09",
                       "complete": True, "blind_confirmed": True,
                       "missed_critical_constraints": [] if sample["setup"] == "tools_lore" else [sample["critical_constraints"][0]],
                       "unsupported_authoritative_claims": 0, "high_severity_unsupported_claims": 0,
                       "discrepancy_alerts": 2 if sample["setup"] == "tools_lore" else 0,
                       "false_discrepancy_alerts": 0, "notes": "Synthetic unit-test bookkeeping, not a human evaluation."})
        cross.write_json(path, review)
    return output


class CrossSourceTests(unittest.TestCase):
    def test_prepare_three_projects_preserves_inputs_and_real_native_hashes(self):
        before = cross.fingerprint(cross.CORPORA)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve() / "prepared"
            report = cross.prepare(output)
            self.assertEqual({p["project"] for p in report["projects"]}, {"payments", "ledger", "releases"})
            self.assertTrue(report["fixture_only"])
            for project in report["projects"]:
                root = output / project["project"]
                self.assertFalse((root / ".lore").exists())
                self.assertFalse(project["held_out"])
                export = cross.read_json(root / "imports" / "engram.json")
                self.assertEqual(export["version"], "0.2.0")
                for sidecar in (root / "openwiki" / ".claims").rglob("*.json"):
                    claims = cross.read_json(sidecar)
                    page = root / "openwiki" / sidecar.relative_to(root / "openwiki" / ".claims").with_suffix(".md")
                    self.assertEqual(claims["schemaVersion"], 1)
                    self.assertEqual(claims["pageVersion"], "sha256:" + hashlib.sha256(page.read_bytes()).hexdigest())
                    evidence = claims["claims"][0]["evidence"][0]
                    code = root / evidence["resource"].removeprefix("repo://")
                    self.assertEqual(evidence["version"], "repo-file-v1:sha256:" + hashlib.sha256(code.read_bytes()).hexdigest())
            with self.assertRaises(ValueError):
                cross.prepare(output)
        self.assertEqual(before, cross.fingerprint(cross.CORPORA))

    def test_payment_mutation_changes_claim_revision_without_editing_corpus(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve() / "payments"
            cross.prepare_project("payments", project)
            path = project / "openwiki" / ".claims" / "payment-retry-policy.json"
            before = cross.read_json(path)
            cross.copy_snapshot(cross.CORPORA / "payments" / "mutation", project)
            after = cross.read_json(path)
            self.assertEqual(before["claims"][0]["id"], after["claims"][0]["id"])
            self.assertIn("five", before["claims"][0]["statement"])
            self.assertIn("three", after["claims"][0]["statement"])
            self.assertNotEqual(before["claims"][0]["evidence"], after["claims"][0]["evidence"])

    def test_unknown_duplicate_projects_and_symlinks_fail(self):
        for projects in [["unknown"], ["payments", "payments"], ["../payments"]]:
            with self.assertRaises(ValueError):
                cross.selected_projects(projects)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "original").write_text("original")
            try:
                (root / "link").symlink_to(root / "original")
            except OSError:
                self.skipTest("Symlinks unavailable on this platform")
            with self.assertRaises(ValueError):
                cross.fingerprint(root)

    def test_evidence_collection_and_resolution_are_mechanical(self):
        answer = {"sections": {"constraints": [{"evidence_ids": ["ev-1", "ne-2"]}]},
                  "evidence": [{"id": "ev-1"}], "imported_evidence": [{"id": "ne-3"}],
                  "unrelated_ids": ["not-an-evidence-reference"]}
        self.assertEqual(cross.cited_ids(answer), {"ev-1", "ne-2", "ne-3"})
        def fake_cli(binary, project, command, identity, timeout):
            if identity == "ne-2":
                raise ValueError("Unresolvable")
            return {"evidence_id": identity}, 0.0
        with patch.object(cross.bench, "subprocess_json", side_effect=fake_cli):
            result = cross.resolve_citations("unused", Path("unused"), cross.cited_ids(answer), 1)
        self.assertEqual(result["checked"], 3)
        self.assertEqual(result["resolvable"], 2)
        self.assertEqual(result["failures"], ["ne-2"])
        self.assertFalse(result["mechanically_verified"])
        self.assertEqual({record["evidence_id"] for record in result["results"]}, cross.cited_ids(answer))
        with patch.object(cross.bench, "subprocess_json", side_effect=fake_cli):
            self.assertTrue(cross.resolve_citations("unused", Path("unused"), {"ev-1"}, 1)["mechanically_verified"])
            self.assertFalse(cross.resolve_citations("unused", Path("unused"), set(), 1)["mechanically_verified"])

    def test_phase_accounts_preparation_and_retrieval_separately(self):
        state = {"native_evidence_ids": ["ne-fixture"], "sqlite_integrity_ok": True}
        def fake_cli(binary, project, command, *args, timeout):
            if command == "init":
                return {"model_calls": 3, "decision_calls": 2}, 11.0
            if command == "context":
                return {"model_calls": 0, "evidence_ids": ["ne-fixture"], "retrieval_truncated": False, "omissions": {}}, 0.2
            if command == "evidence":
                return {"evidence_id": args[0]}, 0.1
            self.fail("Unexpected CLI invocation")
        with tempfile.TemporaryDirectory() as temporary, patch.object(cross, "registry_state", return_value=state), patch.object(cross.bench, "subprocess_json", side_effect=fake_cli):
            result = cross.run_phase("unused", Path(temporary).resolve(), Path(temporary).resolve(), {"queries": [{"id": "a", "task": "Task"}]}, "initial", "init", 1, 5000)
        self.assertEqual(result["preparation_model_calls"], 5)
        self.assertEqual(result["preparation_seconds"], 11.0)
        self.assertEqual(result["retrieval_model_calls"], 0)
        self.assertEqual(result["retrieval_seconds"], 0.2)
        self.assertTrue(result["read_only_registry_unchanged"])
        self.assertEqual(result["citation_integrity"]["resolvable"], 1)

    def test_blind_templates_hide_setup_names_and_cannot_pass_unsigned(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            study = study_fixture(root / "study")
            output = root / "blind"
            cross.blind(study, output)
            for path in (output / "reviews").glob("*.json"):
                review = cross.read_json(path)
                self.assertNotIn("setup", review)
                self.assertIsNone(review["missed_critical_constraints"])
                self.assertFalse(review["complete"])
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertFalse(result["candidate_validated"])
            self.assertFalse(result["gates"]["all_four_setups_and_complete_blind_reviews"])

    def test_four_setup_assessment_uses_full_cost_and_does_not_invent_billing(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertTrue(result["candidate_validated"])
            self.assertEqual(result["relative_missed_constraint_reduction"], 1.0)
            self.assertEqual(result["false_discrepancy_rate"], 0.0)
            self.assertEqual(result["setups"]["tools_lore"]["total_preparation_and_retrieval_seconds"], 36.0)
            self.assertEqual(result["setups"]["tools_lore"]["total_preparation_and_retrieval_model_calls"], 9)
            self.assertIsNone(result["setups"]["tools_lore"]["billed_cost_usd"])

    def test_synthetic_or_non_heldout_projects_cannot_establish_release_readiness(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve(), fixture_only=True)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertFalse(result["candidate_validated"])
            self.assertFalse(result["gates"]["at_least_three_independent_held_out_projects"])

    def test_response_edits_and_unknown_constraint_annotations_invalidate_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            assignment = cross.read_json(output / "assignment.json")
            sample = assignment["samples"][0]
            answer = output / "answers" / f"{sample['sample_id']}.txt"
            answer.write_text("Changed after review")
            self.assertFalse(cross.assess(output / "assignment.json", output / "reviews")["candidate_validated"])
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            path = next((output / "reviews").glob("*.json"))
            review = cross.read_json(path)
            review["missed_critical_constraints"] = ["invented requirement"]
            cross.write_json(path, review)
            self.assertFalse(cross.assess(output / "assignment.json", output / "reviews")["candidate_validated"])

    def test_missing_setup_and_cost_that_excludes_upstream_preparation_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = study_fixture(root / "study")
            study = cross.read_json(path)
            study["cases"][0]["setups"].pop("tools")
            cross.write_json(path, study)
            with self.assertRaisesRegex(ValueError, "all four setups"):
                cross.blind(path, root / "blind")
            self.assertFalse((root / "blind").exists())
            path = study_fixture(root / "study")
            study = cross.read_json(path)
            study["cases"][0]["setups"]["tools_lore"]["cost"]["preparation_includes_all_tools"] = False
            cross.write_json(path, study)
            with self.assertRaisesRegex(ValueError, "every tool"):
                cross.blind(path, root / "blind")

    def test_comparison_with_zero_baseline_misses_cannot_claim_twenty_percent_improvement(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            assignment = cross.read_json(output / "assignment.json")
            for sample in assignment["samples"]:
                if sample["setup"] == "openwiki":
                    path = output / "reviews" / f"{sample['sample_id']}.json"
                    review = cross.read_json(path)
                    review["missed_critical_constraints"] = []
                    cross.write_json(path, review)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertEqual(result["strongest_constraint_baseline"], "openwiki")
            self.assertIsNone(result["relative_missed_constraint_reduction"])
            self.assertFalse(result["candidate_validated"])

    def test_citation_failures_noop_calls_high_severity_and_false_alerts_fail_gates(self):
        with tempfile.TemporaryDirectory() as temporary:
            def bad_contract(study, root):
                def edit(artifact):
                    artifact["citation_integrity"]["resolvable"] = 1
                    artifact["unchanged_import"]["report"]["model_calls"] = 1
                modify_contract(root, study["cases"][0]["setups"]["tools_lore"], edit)
            output = reviewed_packets(Path(temporary).resolve(), edit_study=bad_contract)
            assignment_path = output / "assignment.json"
            assignment = cross.read_json(assignment_path)
            sample = next(sample for sample in assignment["samples"] if sample["setup"] == "tools_lore")
            path = output / "reviews" / f"{sample['sample_id']}.json"
            review = cross.read_json(path)
            review.update({"unsupported_authoritative_claims": 1, "high_severity_unsupported_claims": 1,
                           "discrepancy_alerts": 2, "false_discrepancy_alerts": 1})
            cross.write_json(path, review)
            result = cross.assess(assignment_path, output / "reviews")
            self.assertFalse(result["candidate_validated"])
            for gate in ("all_imported_citations_mechanically_resolvable", "unchanged_import_zero_churn_and_model_calls",
                         "zero_high_severity_unsupported_claims", "false_discrepancy_rate_at_most_10_percent",
                         "no_increase_in_unsupported_authoritative_claims"):
                self.assertFalse(result["gates"][gate], gate)

    def test_source_fingerprint_is_checked_before_blinding_and_again_after_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = study_fixture(root / "study")
            study = cross.read_json(path)
            study["cases"][0]["source_fingerprint"] = "0" * 64
            cross.write_json(path, study)
            with self.assertRaisesRegex(ValueError, "source fingerprint"):
                cross.blind(path, root / "blind")
            self.assertFalse((root / "blind").exists())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = reviewed_packets(root)
            (root / "study" / "source-0" / "source.txt").write_text("Changed after review")
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertFalse(result["source_snapshots_verified"])
            self.assertFalse(result["candidate_validated"])

    def test_bundled_initial_and_mutated_snapshots_force_fixture_only_despite_false_labels(self):
        known = cross.bundled_fingerprints()
        for project in cross.selected_projects(None):
            self.assertIn(cross.fingerprint(cross.CORPORA / project / "initial")["sha256"], known)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            merged = root / "merged"
            cross.prepare_project("payments", merged)
            cross.copy_snapshot(cross.CORPORA / "payments" / "mutation", merged)
            self.assertIn(cross.fingerprint(merged)["sha256"], known)
            self.assertIn(cross.fingerprint(cross.CORPORA / "payments" / "mutation")["sha256"], known)
            def bundled(study, study_root):
                for index, source in enumerate((cross.CORPORA / "ledger" / "initial", merged)):
                    manifest = cross.fingerprint(source)
                    case = study["cases"][index]
                    case["source_root"], case["source_fingerprint"] = str(source), manifest["sha256"]
                    modify_contract(study_root, case["setups"]["tools_lore"],
                                    lambda artifact: artifact.update(source_manifest=manifest, source_fingerprint=manifest["sha256"]))
            output = reviewed_packets(root / "packets", edit_study=bundled)
            assignment_path = output / "assignment.json"
            assignment = cross.read_json(assignment_path)
            self.assertTrue(assignment["fixture_only"])
            self.assertEqual(len(assignment["recognized_fixture_fingerprints"]), 2)
            # Even an explicitly relabeled and newly reviewed assignment is
            # checked against actual bundled bytes again during assessment.
            assignment["fixture_only"] = False
            assignment["recognized_fixture_fingerprints"] = []
            cross.write_json(assignment_path, assignment)
            for path in (output / "reviews").glob("*.json"):
                review = cross.read_json(path)
                review["assignment_sha256"] = cross.digest(assignment)
                cross.write_json(path, review)
            result = cross.assess(assignment_path, output / "reviews")
            self.assertTrue(result["fixture_only"])
            self.assertFalse(result["gates"]["at_least_three_independent_held_out_projects"])

    def test_renaming_one_source_snapshot_does_not_establish_three_projects(self):
        def aliases(study, root):
            first = study["cases"][0]
            manifest = cross.fingerprint(root / first["source_root"])
            for case in study["cases"][1:]:
                case["source_root"], case["source_fingerprint"] = first["source_root"], first["source_fingerprint"]
                modify_contract(root, case["setups"]["tools_lore"],
                                lambda artifact: artifact.update(source_manifest=manifest, source_fingerprint=manifest["sha256"]))
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve(), edit_study=aliases)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertEqual(result["project_count"], 3)
            self.assertEqual(result["distinct_source_fingerprint_count"], 1)
            self.assertFalse(result["gates"]["at_least_three_independent_held_out_projects"])

    def test_assignment_mapping_and_cost_edits_invalidate_existing_annotations(self):
        for edit in (lambda sample: sample.update(setup="openwiki" if sample["setup"] != "openwiki" else "tools"),
                     lambda sample: sample["run"]["cost"].update(preparation_seconds=999)):
            with self.subTest(edit=edit), tempfile.TemporaryDirectory() as temporary:
                output = reviewed_packets(Path(temporary).resolve())
                path = output / "assignment.json"
                assignment = cross.read_json(path)
                edit(assignment["samples"][0])
                cross.write_json(path, assignment)
                result = cross.assess(path, output / "reviews")
                self.assertFalse(result["candidate_validated"])
                self.assertTrue(any("assignment/review binding mismatch" in issue for issue in result["issues"]))

    def test_unbound_scalar_measurements_cannot_pass_mechanical_gates(self):
        def unbound(study, root):
            study["cases"][0]["setups"]["tools_lore"].pop("run_contract")
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve(), edit_study=unbound)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertTrue(result["gates"]["all_four_setups_and_complete_blind_reviews"])
            self.assertFalse(result["gates"]["all_imported_citations_mechanically_resolvable"])
            self.assertFalse(result["gates"]["unchanged_import_zero_churn_and_model_calls"])

    def test_missing_resolver_coverage_wrong_context_and_changed_registry_fail(self):
        def incomplete(artifact):
            citations = artifact["citation_integrity"]
            citations["results"].pop()
            citations.update(checked=1, resolvable=1, mechanically_verified=True)
        def wrong_context(artifact):
            context = artifact["contexts"][0]
            context["response"]["task"] = "An unrelated task"
            context["response_sha256"] = cross.digest(context["response"])
        def changed_registry(artifact):
            artifact["unchanged_import"]["after"]["database_sha256"] = "0" * 64
        for edit, gate in ((incomplete, "all_imported_citations_mechanically_resolvable"),
                           (wrong_context, "all_imported_citations_mechanically_resolvable"),
                           (changed_registry, "unchanged_import_zero_churn_and_model_calls")):
            with self.subTest(edit=edit), tempfile.TemporaryDirectory() as temporary:
                def edit_study(study, root):
                    modify_contract(root, study["cases"][0]["setups"]["tools_lore"], edit)
                output = reviewed_packets(Path(temporary).resolve(), edit_study=edit_study)
                result = cross.assess(output / "assignment.json", output / "reviews")
                self.assertFalse(result["gates"][gate])
                self.assertFalse(result["candidate_validated"])

    def test_run_artifact_edit_after_review_is_detected(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            path = next((output / "contracts").glob("*.json"))
            path.write_bytes(path.read_bytes() + b"\n")
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertFalse(result["candidate_validated"])
            self.assertTrue(any("artifact digest mismatch" in issue for issue in result["issues"]))

    def test_fabricated_answer_only_native_citation_fails_coverage(self):
        def fabricated(study, root):
            run = study["cases"][0]["setups"]["tools_lore"]
            (root / run["answer_path"]).write_text("Fabricated imported citation: ne_deadbeef\n", encoding="utf-8")
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve(), edit_study=fabricated)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertFalse(result["gates"]["all_imported_citations_mechanically_resolvable"])
            self.assertTrue(any("answer contains imported evidence IDs" in issue for issue in result["issues"]))

    def test_registry_state_detects_payload_updates_with_unchanged_row_counts(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / ".lore").mkdir()
            db = project / ".lore" / "state.db"
            with sqlite3.connect(db) as connection:
                connection.execute("CREATE TABLE knowledge_units (id INTEGER, payload TEXT)")
                connection.execute("INSERT INTO knowledge_units VALUES (1, 'before')")
            connection.close()
            before = cross.registry_state(project)
            with sqlite3.connect(db) as connection:
                connection.execute("UPDATE knowledge_units SET payload = 'changed'")
            connection.close()
            after = cross.registry_state(project)
            self.assertEqual(before["counts"], after["counts"])
            self.assertEqual(before["native_current_sha256"], after["native_current_sha256"])
            self.assertNotEqual(before["database_sha256"], after["database_sha256"])

    def test_destination_root_file_and_directory_symlinks_cannot_redirect_mutation(self):
        for location in ("root", "directory", "file"):
            with self.subTest(location=location), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                source, destination, outside = root / "source", root / "destination", root / "outside"
                (source / "nested").mkdir(parents=True)
                (source / "nested" / "record.txt").write_text("replacement")
                (outside / "nested").mkdir(parents=True)
                outside_file = outside / "nested" / "record.txt"
                outside_file.write_text("must remain original")
                try:
                    if location == "root":
                        destination.symlink_to(outside, target_is_directory=True)
                    elif location == "directory":
                        destination.mkdir()
                        (destination / "nested").symlink_to(outside / "nested", target_is_directory=True)
                    else:
                        (destination / "nested").mkdir(parents=True)
                        (destination / "nested" / "record.txt").symlink_to(outside_file)
                except OSError:
                    self.skipTest("Symlinks unavailable on this platform")
                with self.assertRaisesRegex(ValueError, "symlink"):
                    cross.copy_snapshot(source, destination)
                self.assertEqual(outside_file.read_text(), "must remain original")

    def test_no_discrepancy_alerts_is_an_unmeasured_denominator(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = reviewed_packets(Path(temporary).resolve())
            for path in (output / "reviews").glob("*.json"):
                review = cross.read_json(path)
                review["discrepancy_alerts"] = 0
                cross.write_json(path, review)
            result = cross.assess(output / "assignment.json", output / "reviews")
            self.assertIsNone(result["false_discrepancy_rate"])
            self.assertFalse(result["candidate_validated"])


if __name__ == "__main__":
    unittest.main()
