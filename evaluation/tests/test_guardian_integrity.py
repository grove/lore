"""Source-writer diagnostics use authored fixtures, never scored model output."""
from __future__ import annotations

import argparse
import copy
from contextlib import contextmanager
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import guardian_integrity as integrity
import guardian_integrity_gate as gate
import guardian_longitudinal as guardian
import cross_source as cross
from test_guardian_longitudinal import record_fixture, write_run


class GuardianIntegrityTests(unittest.TestCase):
    def test_path_and_descriptor_ctime_bases_are_compared_independently(self):
        # CPython on Windows can expose creation time through lstat and metadata
        # change time through fstat (python/cpython#157671). Neither is discarded.
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            source = project / "rule.md"
            source.write_bytes(b"Same bytes under distinct timestamp APIs")
            actual_fstat = os.fstat

            def descriptor_metadata(fd):
                original = actual_fstat(fd)
                values = {name: getattr(original, name) for name in (
                    "st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")}
                values["st_ctime_ns"] += 1_000_000
                return SimpleNamespace(**values)

            with patch.object(integrity.os, "fstat", side_effect=descriptor_metadata):
                captured = guardian.observed_operation(project, {"rule.md"}, "quiet", lambda _: None)["integrity"]
            self.assertTrue(captured["source_preserved"], captured)
            self.assertTrue(guardian.command_integrity_valid({"source_integrity": captured}))
            file = captured["before"]["inventory"]["paths"]["rule.md"]
            self.assertNotEqual(file["ctime_ns"], file["descriptor_ctime_ns"])

    def test_descriptor_ctime_change_during_read_fails_capture(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_bytes(b"Unchanged size and modification time")
            actual_fstat = os.fstat
            calls = 0

            def changing_descriptor(fd):
                nonlocal calls
                calls += 1
                original = actual_fstat(fd)
                values = {name: getattr(original, name) for name in (
                    "st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")}
                values["st_ctime_ns"] += calls * 1_000_000
                return SimpleNamespace(**values)

            with patch.object(integrity.os, "fstat", side_effect=changing_descriptor):
                with self.assertRaisesRegex(ValueError, "changed during integrity capture"):
                    integrity.inventory(project)

    def test_descriptor_only_metadata_effect_is_not_ignored_between_commands(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_bytes(b"Same bytes, size, path metadata and identity")
            actual_fstat = os.fstat
            changed = False

            def descriptor_metadata(fd):
                original = actual_fstat(fd)
                values = {name: getattr(original, name) for name in (
                    "st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns")}
                values["st_ctime_ns"] += (2 if changed else 1) * 1_000_000
                return SimpleNamespace(**values)

            def metadata_change(_observer):
                nonlocal changed
                changed = True

            with patch.object(integrity.os, "fstat", side_effect=descriptor_metadata):
                captured = guardian.observed_operation(project, {"rule.md"}, "changes", metadata_change)["integrity"]
            self.assertEqual(captured["before"]["sources"], captured["after"]["sources"])
            self.assertFalse(captured["source_preserved"])
            self.assertEqual([row["operation"] for row in captured["effects"]], ["metadata_change"])

    def test_same_byte_path_replacement_after_descriptor_read_fails_capture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            project = root / "project"
            project.mkdir()
            source, replacement = project / "rule.md", root / "replacement.md"
            source.write_bytes(b"Same source bytes")
            replacement.write_bytes(source.read_bytes())
            actual_fdopen = os.fdopen

            @contextmanager
            def replace_after_read(*args, **kwargs):
                with actual_fdopen(*args, **kwargs) as stream:
                    yield stream
                replacement.replace(source)

            with patch.object(integrity.os, "fdopen", side_effect=replace_after_read):
                with self.assertRaisesRegex(ValueError, "changed during integrity capture"):
                    integrity.inventory(project)

    def test_same_bytes_restored_during_one_command_still_fails_integrity(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            source = project / "rule.md"
            source.write_bytes(b"A condition and its exception.\n")

            def restore(_observer):
                original = source.read_bytes()
                source.unlink()
                source.write_bytes(original)

            captured = guardian.observed_operation(project, {"rule.md"}, "baseline save", restore)["integrity"]
            self.assertEqual(captured["before"]["sources"], captured["after"]["sources"])
            self.assertFalse(captured["source_preserved"])
            self.assertTrue(captured["effects"])

    def test_allowed_lore_state_writes_are_classified_without_touching_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_text("Original condition", encoding="utf-8")
            (project / ".lore").mkdir()

            def baseline(_observer):
                (project / ".lore" / "baseline.json").write_text("{}", encoding="utf-8")

            captured = guardian.observed_operation(project, {"rule.md"}, "baseline save", baseline)["integrity"]
            self.assertTrue(captured["source_preserved"])
            self.assertTrue(guardian.command_integrity_valid({"source_integrity": captured}))
            self.assertEqual({row["category"] for row in captured["effects"]}, {"lore_state"})

    def test_read_commands_cannot_change_configuration_or_generated_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_text("Original", encoding="utf-8")
            (project / "wiki").mkdir()
            for relative in ("lore.yml", "wiki/index.md"):
                target = project / relative
                target.write_text("Before", encoding="utf-8")
                captured = guardian.observed_operation(project, {"rule.md"}, "changes --since",
                    lambda _observer: target.write_text("After", encoding="utf-8"))["integrity"]
                self.assertTrue(captured["source_preserved"])
                self.assertFalse(guardian.command_integrity_valid({"source_integrity": captured, "arguments": ["changes"]}))

    def test_rename_is_bound_to_file_identity_and_generated_output_is_distinct(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_text("Original", encoding="utf-8")
            (project / "wiki").mkdir()
            before = integrity.inventory(project)
            (project / "rule.md").rename(project / "renamed.md")
            (project / "wiki" / "index.md").write_text("Generated", encoding="utf-8")
            differences = integrity.effects(before, integrity.inventory(project))
            rename = next(row for row in differences if row["operation"] == "rename_observed_identity")
            self.assertEqual((rename["path"], rename["destination"]), ("rule.md", "renamed.md"))
            self.assertEqual(rename["before"]["inode"], rename["after"]["inode"])
            self.assertEqual(next(row for row in differences if row["path"] == "wiki/index.md")["category"], "generated_output")

    def test_snapshot_preflights_forbidden_target_before_deleting_any_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            project, prepared, sibling = (root / name for name in ("project", "prepared", "sibling"))
            for path in (project, prepared, sibling):
                path.mkdir()
            (project / "old.md").write_text("Retain on failure", encoding="utf-8")
            (sibling / "marker.md").write_text("Sibling source", encoding="utf-8")
            (prepared / "lore.yml").write_text("forbidden configuration", encoding="utf-8")
            with self.assertRaises(ValueError):
                guardian.apply_snapshot(prepared, project, {"old.md"})
            self.assertEqual((project / "old.md").read_text(), "Retain on failure")
            self.assertEqual((sibling / "marker.md").read_text(), "Sibling source")
            self.assertFalse((project / "lore.yml").exists())
            with self.assertRaisesRegex(ValueError, "disjoint"):
                guardian.apply_snapshot(project, root, set())

    def test_snapshot_symlink_cannot_redirect_write_or_delete_another_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            project, prepared, sibling = (root / name for name in ("project", "prepared", "sibling"))
            for path in (project, prepared, sibling):
                path.mkdir()
            (project / "old.md").write_text("Original", encoding="utf-8")
            (prepared / "redirect.md").write_text("Replacement", encoding="utf-8")
            (sibling / "outside.md").write_text("Outside", encoding="utf-8")
            try:
                (project / "redirect.md").symlink_to(sibling / "outside.md")
            except OSError:
                self.skipTest("This platform cannot create the test symlink")
            with self.assertRaises(ValueError):
                guardian.apply_snapshot(prepared, project, {"old.md"})
            self.assertEqual((project / "old.md").read_text(), "Original")
            self.assertEqual((sibling / "outside.md").read_text(), "Outside")

    def test_a_to_b_to_a_and_deletion_replay_preserves_exact_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            project = root / "project"
            project.mkdir()
            previous, digests = set(), []
            for index, text in enumerate(("A except acknowledged", "B except acknowledged", "A except acknowledged")):
                snapshot = root / str(index)
                snapshot.mkdir()
                (snapshot / "rule.md").write_text(text, encoding="utf-8")
                if index == 0:
                    (snapshot / "history.md").write_text("Historical", encoding="utf-8")
                previous = guardian.apply_snapshot(snapshot, project, previous)
                actual = guardian.source_fingerprint(project, {"rule.md", "history.md"})
                self.assertEqual(actual, cross.fingerprint(snapshot))
                digests.append(actual["files_sha256"]["rule.md"])
            self.assertEqual(digests[0], digests[2])
            self.assertNotEqual(digests[0], digests[1])
            self.assertFalse((project / "history.md").exists())

    @unittest.skipUnless(platform.system() == "Linux", "Linux kernel observer")
    def test_kernel_observer_captures_transient_create_delete_and_mode_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_text("Unchanged", encoding="utf-8")
            observer = integrity.FilesystemObserver(project, True)
            if observer.status != "captured":
                observer.finish()
                self.skipTest("Kernel inotify unavailable; no observer pass claimed")
            transient = project / ".rsync-tmp-control"
            transient.write_text("Synthetic", encoding="utf-8")
            transient.unlink()
            (project / "rule.md").chmod(0o600)
            observed = observer.finish()
            self.assertTrue(observed["complete"])
            self.assertTrue(observed["source_complete"])
            operations = {value for row in observed["events"] for value in row["operations"]}
            self.assertTrue({"create", "write", "delete", "metadata_change"} <= operations)
            self.assertTrue(all(row["writer_pid"] is None for row in observed["events"]))

    def test_unavailable_observer_and_pid_tree_never_become_audit_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            with patch.object(integrity.platform, "system", return_value="Unsupported"):
                result = integrity.FilesystemObserver(project, True).finish()
            self.assertEqual(result["status"], "unsupported_platform")
            self.assertFalse(result["complete"])
            self.assertFalse(result["source_complete"])
            with patch.object(Path, "read_text", side_effect=PermissionError):
                self.assertEqual(integrity.process_tree(1)["status"], "unavailable_or_partial")

    def test_observed_timeout_retains_source_change_and_withholds_secret_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary).resolve()
            (project / "rule.md").write_text("Before", encoding="utf-8")
            actual_popen = subprocess.Popen

            def start(*_args, **kwargs):
                code = "from pathlib import Path; import time; Path('rule.md').write_text('Changed'); print('provider-private-secret', flush=True); time.sleep(5)"
                return actual_popen([sys.executable, "-c", code], **kwargs)

            with patch.object(guardian.subprocess, "Popen", side_effect=start):
                result = guardian.invoke(sys.executable, project, ["guard"], {}, .2, source_names={"rule.md"})
            self.assertEqual(result["error"], "timeout")
            self.assertFalse(result["source_integrity"]["source_preserved"])
            self.assertNotIn("provider-private-secret", json.dumps(result))
            # The retry gets its own interval and cannot erase the prior failure.
            with patch.object(guardian.subprocess, "Popen", side_effect=OSError):
                retry = guardian.invoke(sys.executable, project, ["guard"], {}, .2, source_names={"rule.md"})
            self.assertEqual(retry["error"], "process_unavailable")
            self.assertTrue(retry["source_integrity"]["source_preserved"])
            self.assertFalse(result["source_integrity"]["source_preserved"])

    def test_no_lore_control_never_spawns_a_child_and_retains_each_transition(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            guardian.prepare(root / "prepared")
            args = argparse.Namespace(prepared=root / "prepared", output=root / "control", observe_filesystem=False,
                                      dwell_ms=0, project="payments", first_event=6, event_count=4)
            with patch.object(guardian.subprocess, "Popen", side_effect=AssertionError("No process is allowed")):
                report = guardian.no_lore_control(args)
            self.assertEqual(report["events"], 4)
            self.assertEqual(report["processes_started"], 0)
            self.assertEqual(report["model_calls"], 0)
            self.assertEqual([row["event_id"] for row in report["records"]], [f"payments-{n:02}" for n in range(6, 10)])
            self.assertEqual(len(report["collector_source_sha256"]), 5)
            self.assertFalse(report["runtime_audit_complete"])

    def test_cancelled_replay_records_failure_and_stops_before_next_source_edit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            prepared = guardian.prepare(root / "prepared")
            state = prepared["projects"][0]["states"][0]
            compiled = root / "compiled"
            compiled.mkdir()
            (compiled / "state.db").write_bytes(b"Synthetic database double; never opened")
            cross.write_json(compiled / "compiled.json", {"fixture_only": True,
                "prepared_sha256": guardian.coding.hash_file(root / "prepared" / "prepared.json"),
                "states": [{"project": "payments", "revision": state["revision"], "database": "state.db",
                            "database_sha256": guardian.coding.hash_file(compiled / "state.db")}]})
            args = argparse.Namespace(prepared=root / "prepared", compiled=compiled, output=root / "run",
                binary=sys.executable, config_template=None, max_tokens=8000, timeout=1,
                allow_inspection=False, allow_hosted=False, allow_checkout_egress=False,
                audit_strace=False, observe_filesystem=False, project="payments", first_event=1, event_count=2)
            cancelled = {"arguments": ["baseline", "save"], "error": "cancelled", "response": None,
                         "response_sha256": None, "elapsed_seconds": .1}
            with patch.object(guardian, "invoke", return_value=cancelled) as invoke, patch.object(
                    guardian, "registry_capture", return_value=record_fixture()["registry_before"]):
                report = guardian.run(args)
            self.assertEqual(invoke.call_count, 1)
            self.assertEqual(report["termination"], "cancelled")
            self.assertEqual(len(report["records"]), 1)
            capture = guardian.read_json(args.output / report["records"][0]["path"])
            self.assertEqual(capture["sources_query"], state["source_fingerprint"])
            self.assertEqual(capture["changes"]["error"], "not_run_after_cancellation")
            with self.assertRaisesRegex(ValueError, "silently drop"):
                guardian.assess(args.output)

    def test_new_run_cannot_drop_command_ledger_to_reuse_legacy_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            record = record_fixture()
            run = write_run(directory, record)
            run["integrity_version"] = 1
            cross.write_json(directory / "run.json", run)
            with self.assertRaisesRegex(ValueError, "per-command"):
                guardian.assess(directory)
            record["integrity_version"] = 1
            write_run(directory, record)
            report = guardian.assess(directory)
            self.assertFalse(report["engineering_capture_pass"])
            self.assertFalse(report["per_command_source_integrity_pass"])

    def test_gate_requires_full_events_zero_provider_calls_and_independent_observer(self):
        valid = {"events": 60, "engineering_capture_pass": True, "per_command_source_integrity_pass": True,
                 "total_model_calls": 0, "command_integrity": {"total": 500, "filesystem_source_observer_complete": 500}}
        self.assertTrue(gate.replay_pass(valid, 60))
        for field, value in (("events", 59), ("engineering_capture_pass", False), ("total_model_calls", None)):
            broken = copy.deepcopy(valid)
            broken[field] = value
            self.assertFalse(gate.replay_pass(broken, 60))
        valid["command_integrity"]["filesystem_source_observer_complete"] = 499
        self.assertFalse(gate.replay_pass(valid, 60))


if __name__ == "__main__":
    unittest.main()
