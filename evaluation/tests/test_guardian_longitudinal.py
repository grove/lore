"""Adversarial protocol fixtures, not a real provider or independently rated study."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import guardian_longitudinal as guardian
import cross_source as cross


def invocation(value):
    return {"response": value, "response_sha256": cross.digest(value), "error": None,
            "elapsed_seconds": .01, "audit": {"status": "not_captured"}}


def record_fixture(*, status="no_documented_change", required=False):
    snapshot = {"project_id": "fixture", "registry_revision": "fixture-revision"}
    budget = {"max_tokens": 8000, "used_tokens": 300, "tokenizer": "cl100k_base"}
    changes = {"schema_version": 1, "baseline": "transition", "baseline_snapshot": snapshot,
               "snapshot": snapshot, "budget": budget, "changes": [], "relationship_changes": [],
               "imported_changes": [], "cross_source_changes": [], "evidence": [], "imported_evidence": [],
               "related_knowledge": [], "cross_source_knowledge": [], "live_checkout_assessed": False,
               "omitted_changes": 0, "omitted_relationship_changes": 0, "omitted_imported_changes": 0,
               "omitted_cross_source_changes": 0, "retrieval_truncated": False}
    guarded = {"schema_version": 1, "baseline": "transition", "snapshot": snapshot,
               "budget": budget, "execution": False, "source_write": False, "advisories": [],
               "assessment_status": status, "intelligence": None, "advisories_in_shared_guidance": 0}
    if status in ("partial_static_guidance", "source_reviewed_advisory"):
        guarded["intelligence"] = {"schema_version": 5, "intelligence": {
            "schema_version": 4, "model_calls": 0, "brief": {"generation_basis":
                "model_assessed" if status == "source_reviewed_advisory" else "deterministic_fallback"}}}
    source = {"sha256": "source-fixture", "files_sha256": {}, "file_count": 0}
    registry = {"sqlite_integrity_ok": True, "immutable_rows": {"evidence_snapshots": ["old-quote"]},
                "knowledge_current": {}, "knowledge_history": {}, "database_sha256": "fixture-db"}
    return {"event_id": "project-01", "project": "project", "before_revision": "fixture-before", "after_revision": "fixture-after", "event": {
                "id": "project-01", "expected": {"significance": "consequential" if required else "benign",
                "alert_necessity": "required" if required else "forbidden", "source_scope": "production",
                "safe_next_step": "Review the cited source before changing the documented bound.", "runtime_verified": False}},
            "expected_before_sources": copy.deepcopy(source), "expected_after_sources": copy.deepcopy(source),
            "sources_before": copy.deepcopy(source), "sources_query": copy.deepcopy(source),
            "sources_after_queries": copy.deepcopy(source), "registry_before": copy.deepcopy(registry),
            "registry_query": copy.deepcopy(registry), "registry_after_queries": copy.deepcopy(registry),
            "baseline_save": invocation({"schema_version": 1, "baseline": "transition", "snapshot": snapshot, "source_write": False}),
            "changes": invocation(changes), "guard": invocation(guarded), "resolution": [],
            "materialization": {"mode": "explicit_synthetic_registry_capture", "inference_calls": 0},
            "elapsed_seconds": .25}


def write_run(directory, record):
    capture = directory / "capture.json"
    cross.write_json(capture, record)
    value = {"schema_version": 1, "protocol": guardian.PROTOCOL, "fixture_only": True, "held_out": False,
             "max_tokens": 8000, "initialization": [], "prepared": {"projects": [
                 {"id": "project", "states": [{"revision": record["before_revision"], "source_fingerprint": record["expected_before_sources"]},
                    {"revision": record["after_revision"], "source_fingerprint": record["expected_after_sources"], "event": record["event"]}]}]},
             "records": [{"event_id": record["event_id"], "path": "capture.json", "sha256": guardian.coding.hash_file(capture)}]}
    cross.write_json(directory / "run.json", value)
    return value


def reviewed_file(directory, record, reviewer):
    value = guardian.review_template(directory)
    value.update(reviewer_id=reviewer, reviewed_at="2026-10-10", independent_of_product_and_corpus_authorship=True)
    for event in value["events"]:
        event.update(label_confirmed=True, source_identity_confirmed=True,
                     alerts=[{"index": index, "supported_current_scope": True, "material_change": True,
                              "safe_concrete_action": True} for index, _ in enumerate(record["guard"]["response"]["advisories"])])
    path = directory / f"{reviewer}.json"
    cross.write_json(path, value)
    return path


def segment_fixture(prepared_directory, project, directory):
    """Mocked captures for merge validation; no process or model is executed."""
    directory.mkdir()
    prepared = guardian.read_json(prepared_directory / "prepared.json")
    records = []
    for before, after in zip(project["states"], project["states"][1:]):
        record = record_fixture()
        event_id = after["event"]["id"]
        record.update(event_id=event_id, project=project["id"], event=after["event"],
                      before_revision=before["revision"], after_revision=after["revision"],
                      expected_before_sources=before["source_fingerprint"], sources_before=before["source_fingerprint"],
                      expected_after_sources=after["source_fingerprint"], sources_query=after["source_fingerprint"],
                      sources_after_queries=after["source_fingerprint"])
        path = directory / f"{event_id}.json"
        cross.write_json(path, record)
        records.append({"event_id": event_id, "path": path.name, "sha256": guardian.coding.hash_file(path)})
    prepared["projects"] = [project]
    run = {"schema_version": 1, "protocol": guardian.PROTOCOL, "fixture_only": True, "held_out": False,
           "label_provenance": prepared["label_provenance"], "prepared_sha256": guardian.coding.hash_file(prepared_directory / "prepared.json"),
           "events_manifest_sha256": prepared["events_manifest_sha256"], "binary_sha256": "a" * 64,
           "compiler_mode": "synthetic_explicit_records", "max_tokens": 8000, "permissions": {},
           "audit_requested": False, "prepared": prepared, "records": records, "initialization": [], "elapsed_seconds": 1.0}
    cross.write_json(directory / "run.json", run)


class GuardianSegmentTests(unittest.TestCase):
    def test_merge_requires_all_original_events_same_binary_and_no_duplicates(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            prepared_directory = root / "prepared"
            prepared = guardian.prepare(prepared_directory)
            segments = []
            for project in prepared["projects"]:
                directory = root / project["id"]
                segment_fixture(prepared_directory, project, directory)
                segments.append(directory)
            for name, paths in (("missing", segments[:-1]), ("duplicate", [segments[0], *segments])):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    guardian.merge_runs(prepared_directory, paths, root / name)
            merged = guardian.merge_runs(prepared_directory, segments, root / "complete")
            self.assertEqual(len(merged["records"]), 60)
            self.assertEqual(len(merged["prepared"]["projects"]), 3)
            self.assertEqual(len(merged["component_runs"]), 3)
            self.assertEqual(guardian.assess(root / "complete")["events"], 60)
            changed = guardian.read_json(segments[-1] / "run.json")
            changed["binary_sha256"] = "b" * 64
            cross.write_json(segments[-1] / "run.json", changed)
            with self.assertRaisesRegex(ValueError, "different binaries"):
                guardian.merge_runs(prepared_directory, segments, root / "binary-mismatch")

    def test_assessment_rejects_rehashed_capture_with_wrong_successive_endpoint(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture()
            run = write_run(directory, record)
            record["before_revision"] = "unrelated-history"
            cross.write_json(directory / "capture.json", record)
            run["records"][0]["sha256"] = guardian.coding.hash_file(directory / "capture.json")
            cross.write_json(directory / "run.json", run)
            with self.assertRaisesRegex(ValueError, "successive source endpoints"):
                guardian.assess(directory)


class GuardianLongitudinalTests(unittest.TestCase):
    def test_corpus_has_sixty_successive_explicitly_synthetic_transitions(self):
        value = guardian.load_events(guardian.DEFAULT_EVENTS)
        counts = guardian.Counter(event["expected"]["significance"] for project in value["projects"] for event in project["events"])
        self.assertEqual(counts, {"consequential": 20, "benign": 20, "ambiguous": 20})
        self.assertEqual(len(value["projects"]), 3)
        self.assertTrue(value["fixture_only"])
        self.assertFalse(value["label_provenance"]["independent_review"])

    def test_events_cannot_overwrite_registry_or_escape_sources(self):
        for path in ("../outside", "/absolute", ".lore/state.db", "lore.yml", ".git/hooks/pre-commit", "wiki/index.md"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                guardian.source_file(path)

    def test_corpus_rejects_disconnected_parent_and_fabricated_quote(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events.json"
            original = guardian.load_events(guardian.DEFAULT_EVENTS)
            for mutation in ("parent", "quote", "external"):
                value = copy.deepcopy(original)
                if mutation == "parent":
                    value["projects"][0]["events"][1]["parent"] = "unrelated"
                elif mutation == "quote":
                    next(iter(value["projects"][0]["initial_files"].values()))["records"][0]["quote"] = "not in original source"
                else:
                    value["label_provenance"]["status"] = "candidate_external"
                cross.write_json(path, value)
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    guardian.load_events(path)

    def test_preparation_pins_every_snapshot_and_detects_mutated_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "prepared"
            prepared = guardian.prepare(directory)
            self.assertTrue(prepared["full_size"])
            self.assertEqual(sum(len(project["states"]) for project in prepared["projects"]), 63)
            guardian.validate_prepared(directory)
            state = prepared["projects"][0]["states"][0]
            path = directory / state["source_root"] / next(iter(state["source_fingerprint"]["files_sha256"]))
            path.write_text(path.read_text() + "Changed externally.\n")
            with self.assertRaisesRegex(ValueError, "pinned hash"):
                guardian.validate_prepared(directory)

    def test_prepared_identities_cannot_redirect_run_writes_or_reorder_history(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "prepared"
            original = guardian.prepare(directory)
            for mutation in ("project", "event", "parent", "revision_path"):
                prepared = copy.deepcopy(original)
                project = prepared["projects"][0]
                if mutation == "project":
                    project["id"] = "../../outside"
                elif mutation == "event":
                    project["states"][1]["event"]["id"] = "../../outside"
                elif mutation == "parent":
                    project["states"][1]["event"]["parent"] = "unrelated-history"
                else:
                    project["states"][1]["source_root"] = "snapshots/unrelated/project"
                cross.write_json(directory / "prepared.json", prepared)
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    guardian.validate_prepared(directory)

    def test_source_integrity_includes_unexpected_new_files_and_deletions(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "docs").mkdir()
            source = directory / "docs" / "rule.md"
            source.write_text("Original quotation")
            before = guardian.source_fingerprint(directory, {"docs/rule.md"})
            extra = directory / "docs" / "unexpected.md"
            extra.write_text("Unexpected write")
            self.assertNotEqual(before, guardian.source_fingerprint(directory, {"docs/rule.md"}))
            extra.unlink()
            source.unlink()
            self.assertEqual(guardian.source_fingerprint(directory, {"docs/rule.md"})["file_count"], 0)

    def test_source_symlinks_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "source.md").write_text("Source")
            (directory / "alias.md").symlink_to(directory / "source.md")
            with self.assertRaises(ValueError):
                guardian.source_fingerprint(directory, set())

    def test_invocation_failure_does_not_publish_provider_secret(self):
        fake = subprocess.CompletedProcess([], 7, "private-source-secret", "private-provider-secret")
        with patch.object(guardian.subprocess, "run", return_value=fake):
            result = guardian.invoke("/lore", Path("/project"), ["guard"], {}, 1)
        self.assertEqual(result["error"], "exit_7")
        self.assertNotIn("secret", json.dumps(result))
        self.assertIsNone(result["response"])

    def test_invocation_timeout_is_unavailable_not_empty_success(self):
        with patch.object(guardian.subprocess, "run", side_effect=subprocess.TimeoutExpired("lore", 1)):
            result = guardian.invoke("/lore", Path("/project"), ["guard"], {}, 1)
        self.assertEqual(result["error"], "timeout")
        self.assertFalse(guardian.response_hash_valid(result))

    def test_read_only_zero_call_fixture_satisfies_mechanical_contract(self):
        self.assertTrue(all(guardian.event_checks(record_fixture(), 8000).values()))

    def test_absent_provider_cannot_be_reclassified_as_model_assessed(self):
        record = record_fixture(status="partial_static_guidance", required=True)
        self.assertTrue(all(guardian.event_checks(record, 8000).values()))
        record["guard"]["response"]["assessment_status"] = "source_reviewed_advisory"
        record["guard"] = invocation(record["guard"]["response"])
        self.assertFalse(guardian.event_checks(record, 8000)["versioned_contracts"])

    def test_responses_cannot_exceed_budget_or_change_named_checkpoint(self):
        for mutation, check in (("tokens", "requested_budget"), ("baseline", "explicit_baseline"), ("execution", "no_execution_claims")):
            record = record_fixture()
            guarded = record["guard"]["response"]
            if mutation == "tokens":
                guarded["budget"] = {"max_tokens": 8000, "used_tokens": 8001, "tokenizer": "cl100k_base"}
            elif mutation == "baseline":
                guarded["baseline"] = "silently-selected"
            else:
                guarded["execution"] = True
            record["guard"] = invocation(guarded)
            with self.subTest(mutation=mutation):
                self.assertFalse(guardian.event_checks(record, 8000)[check])

    def test_history_rows_cannot_be_replaced_even_when_counts_match(self):
        record = record_fixture()
        record["registry_query"]["immutable_rows"]["evidence_snapshots"] = ["rewritten-quote"]
        self.assertFalse(guardian.event_checks(record, 8000)["retained_history"])
        self.assertFalse(guardian.event_checks(record, 8000)["read_only_registry"])

    def test_before_and_after_must_match_independent_sql_checkpoint(self):
        state = {"id": "ku_rule", "revision_id": "kr_rule", "statement": "Preserve the exception.",
                 "evidence_ids": ["ev_current", "ev_historical"], "current_evidence_ids": ["ev_current"]}
        record = record_fixture()
        record["registry_query"]["knowledge_current"] = {state["id"]: copy.deepcopy(state)}
        changes = {"changes": [{"knowledge_id": state["id"], "before": None, "after": copy.deepcopy(state)}]}
        self.assertTrue(guardian.exact_checkpoint_states(record, changes))
        changes["changes"][0]["after"]["current_evidence_ids"] = ["ev_historical"]
        self.assertFalse(guardian.exact_checkpoint_states(record, changes))
        del changes["changes"][0]["after"]["current_evidence_ids"]
        self.assertTrue(guardian.exact_checkpoint_states(record, changes), "Old baselines lack membership but retain exact immutable state")
        changes["changes"][0]["after"]["statement"] = "Drop the exception."
        self.assertFalse(guardian.exact_checkpoint_states(record, changes))

    def test_original_evidence_requires_exact_quote_and_complete_historical_support(self):
        evidence = {"id": "ev_rule", "excerpt": "Except after acknowledgement.", "line_start": 3, "line_end": 3}
        changes = {"evidence": [copy.deepcopy(evidence)], "related_knowledge": [
            {"id": "ku_rule", "revision_id": "kr_rule", "evidence_ids": ["ev_rule"], "current_evidence_ids": ["ev_rule"]}]}
        resolution = [{"evidence_id": "ev_rule", **invocation(evidence)}]
        self.assertTrue(guardian.original_evidence(changes, resolution))
        changes["evidence"][0]["excerpt"] = "Always."
        self.assertFalse(guardian.original_evidence(changes, resolution))
        changes["evidence"][0] = copy.deepcopy(evidence)
        changes["related_knowledge"][0]["evidence_ids"].append("ev_missing_history")
        self.assertFalse(guardian.original_evidence(changes, resolution))

    def test_relationships_need_both_exact_revisions_and_witness_quote(self):
        ev = {"id": "ev_link", "excerpt": "The new rule supersedes the old only in staging."}
        changes = {"evidence": [ev], "related_knowledge": [
            {"id": "ku_old", "revision_id": "kr_old", "evidence_ids": ["ev_link"]},
            {"id": "ku_new", "revision_id": "kr_new", "evidence_ids": ["ev_link"]}],
            "relationship_changes": [{"before": None, "after": {"from": "ku_old", "to": "ku_new",
                "evidence_id": "ev_link", "exact_excerpt": ev["excerpt"]}}],
            "cross_source_changes": [{"before": None, "after": {"relation": {
                "from": {"kind": "knowledge", "id": "ku_old", "revision_id": "kr_old"},
                "to": {"kind": "knowledge", "id": "ku_new", "revision_id": "kr_new"}, "evidence_ids": ["ev_link"]}}}]}
        resolution = [{"evidence_id": "ev_link", **invocation(ev)}]
        self.assertTrue(guardian.original_evidence(changes, resolution))
        changes["cross_source_changes"][0]["after"]["relation"]["to"]["revision_id"] = "kr_unrelated"
        self.assertFalse(guardian.original_evidence(changes, resolution))

    def test_unavailable_assessment_stays_in_denominator_and_efficacy_unmeasured(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            write_run(directory, record_fixture(status="partial_static_guidance", required=True))
            result = guardian.assess(directory)
            self.assertTrue(result["engineering_capture_pass"])
            self.assertEqual(result["required_events"], 1)
            self.assertEqual(result["required_event_assessment_coverage"], 0)
            self.assertIsNone(result["high_severity_precision"])
            self.assertIsNone(result["consequential_alert_recall"])
            self.assertFalse(result["product_efficacy_measured"])
            self.assertFalse(result["product_quality_gate_pass"])
            self.assertFalse(result["independent_runtime_audit_complete"])

    def test_missing_or_modified_capture_cannot_disappear_from_assessment(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            run = write_run(directory, record_fixture())
            run["records"] = []
            cross.write_json(directory / "run.json", run)
            with self.assertRaisesRegex(ValueError, "silently drop"):
                guardian.assess(directory)
            write_run(directory, record_fixture())
            cross.write_json(directory / "capture.json", {"altered": True})
            with self.assertRaisesRegex(ValueError, "modified"):
                guardian.assess(directory)

    def test_source_hash_failure_fails_overall_capture_even_with_valid_cli_contracts(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture()
            record["sources_after_queries"]["files_sha256"]["docs/restored.md"] = "unexpected-restored-source"
            write_run(directory, record)
            result = guardian.assess(directory)
            self.assertFalse(result["engineering_capture_pass"])
            self.assertEqual(result["failed_checks"], {"source_hashes": 1})
            self.assertFalse(result["product_quality_gate_pass"])

    def test_failed_guardian_does_not_fabricate_zero_provider_cost(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture()
            record["guard"] = {"response": None, "response_sha256": None, "error": "exit_1"}
            write_run(directory, record)
            result = guardian.assess(directory)
            self.assertEqual(result["assessment_statuses"], {"command_failed": 1})
            self.assertIsNone(result["total_model_calls"])
            self.assertIsNone(result["provider_cost_usd"])

    def test_review_template_never_counts_as_completed_independent_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            write_run(directory, record_fixture())
            path = directory / "blank-review.json"
            cross.write_json(path, guardian.review_template(directory))
            with self.assertRaises(ValueError):
                guardian.assess(directory, [path])

    def test_two_reviewers_score_actual_alerts_but_debug_result_is_not_efficacy(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture(status="source_reviewed_advisory", required=True)
            record["guard"]["response"]["advisories"] = [{"severity": "high", "evidence_ids": [], "observation_ids": []}]
            record["guard"] = invocation(record["guard"]["response"])
            write_run(directory, record)
            files = [reviewed_file(directory, record, name) for name in ("fixture-reviewer-a", "fixture-reviewer-b")]
            result = guardian.assess(directory, files)
            self.assertEqual(result["high_severity_precision"], 1)
            self.assertEqual(result["consequential_alert_recall"], 1)
            self.assertEqual(result["independently_actionable_alert_rate"], 1)
            self.assertFalse(result["product_efficacy_measured"])
            review = guardian.read_json(files[1])
            review["events"][0]["alerts"][0]["supported_current_scope"] = False
            cross.write_json(files[1], review)
            result = guardian.assess(directory, files)
            self.assertEqual(result["high_severity_precision"], 0)
            self.assertEqual(result["consequential_alert_recall"], 0)

    def test_one_reviewer_or_duplicate_identity_cannot_establish_precision(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture(status="source_reviewed_advisory", required=True)
            write_run(directory, record)
            path = reviewed_file(directory, record, "fixture-reviewer")
            self.assertIsNone(guardian.assess(directory, [path])["high_severity_precision"])
            with self.assertRaises(ValueError):
                guardian.assess(directory, [path, path])

    def test_compilation_and_adaptive_model_calls_are_all_charged(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture(status="source_reviewed_advisory", required=True)
            record["materialization"] = invocation({"model_calls": 3, "decision_calls": 2})
            record["guard"]["response"]["intelligence"]["intelligence"]["model_calls"] = 4
            record["guard"] = invocation(record["guard"]["response"])
            run = write_run(directory, record)
            run["initialization"] = [{"compilation": invocation({"model_calls": 7, "decision_calls": 1})}]
            cross.write_json(directory / "run.json", run)
            result = guardian.assess(directory)
            self.assertEqual(result["total_model_calls"], 17)
            self.assertEqual(result["compilation_model_calls"], 13)
            self.assertEqual(result["guard_model_calls"], 4)
            self.assertIsNone(result["provider_tokens"])
            self.assertIsNone(result["provider_cost_usd"])

    def test_budget_omissions_and_hidden_advisories_do_not_become_false_negatives(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = record_fixture(status="source_reviewed_advisory", required=True)
            record["guard"]["response"]["advisories_in_shared_guidance"] = 1
            record["guard"] = invocation(record["guard"]["response"])
            write_run(directory, record)
            files = [reviewed_file(directory, record, name) for name in ("fixture-a", "fixture-b")]
            result = guardian.assess(directory, files)
            self.assertEqual(result["required_events_with_reviewed_available_assessment"], 0)
            self.assertIsNone(result["consequential_alert_recall"])

    def test_audit_needs_complete_real_syscall_capture(self):
        binary = "/absolute/lore"
        good = '42 execve("/absolute/lore", ["lore", "guard"], 0x0) = 0\n42 +++ exited with 0 +++\n'
        result = guardian.audit_trace(good, binary, network_granted=False)
        self.assertTrue(result["execution_verified"])
        self.assertTrue(result["network_verified"])
        for invalid in ("", "ptrace: Operation not permitted", good.replace("+++ exited with 0 +++", ""), good + '43 execve("/project/run.sh", [], 0x0) = 0\n'):
            self.assertFalse(guardian.audit_trace(invalid, binary, network_granted=False)["execution_verified"])
        network = good + '42 connect(4, {sa_family=AF_INET, sin_port=htons(443)}, 16) = 0\n'
        self.assertFalse(guardian.audit_trace(network, binary, network_granted=True)["network_verified"], "A broad grant is not independent proof of an authorized destination")


if __name__ == "__main__":
    unittest.main()
