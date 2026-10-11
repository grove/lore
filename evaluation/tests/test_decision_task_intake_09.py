"""Authored local contract fixtures, never qualifying independent study evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_tasks as coding
import cross_source as cross
import decision_task_intake_09 as intake
import experiment_08 as experiment
import outcome_protocol as outcome
import test_experiment_08 as fixtures


def candidates(root, count=6):
    cases, declarations = [], []
    for index in range(count):
        identity = "offline-case-" + str(index)
        source, original = root / identity / "source", root / identity / "original"
        for directory in (source, original):
            (directory / "src").mkdir(parents=True)
            (directory / "docs").mkdir()
            (directory / "docs/decision.md").write_text(f"Authored offline decision fixture {index}: accept positive values.")
        (source / "src/policy.py").write_text(f"RESULT = None # offline input {index}\n")
        correct = f"RESULT = {100 + index} # offline implementation {index}\n"
        (original / "src/policy.py").write_text(correct)
        correct_path, wrong_path = root / identity / "correct.py", root / identity / "wrong.py"
        correct_path.write_text(correct)
        wrong_path.write_text(f"RESULT = {-100 - index} # offline decision-violating implementation {index}\n")
        checker = root / identity / "checker.py"
        checker.write_text("import json,sys\nfrom pathlib import Path\n" +
            "text=(Path(sys.argv[1])/'src/policy.py').read_text()\n" +
            f"passed = text.startswith('RESULT = {100 + index} #')\n" +
            "print(json.dumps({'schema_version':1,'checks':[{'id':'goal','kind':'correctness','passed':passed}," +
            "{'id':'accepted-decision','kind':'constraint','passed':passed}]}))\n")
        case = {"id": identity, "project": "offline-project-" + str(index // 2),
                "source_root": source.relative_to(root).as_posix(), "task": f"Implement offline decision fixture {index}",
                "editable_files": ["src/policy.py"], "expected_sources": ["docs:decision.md"],
                "critical_constraints": ["Use the independently accepted positive decision."],
                "test_command": ["{python}", "{manifest}/" + checker.relative_to(root).as_posix(), "{workspace}"],
                "verification_files": [checker.relative_to(root).as_posix()],
                "input_sha256": cross.fingerprint(source)["sha256"],
                "upstream": {"repository": "offline-owner/project-" + str(index // 2), "commit": "a" * 40,
                             "files_sha256": cross.fingerprint(original)["files_sha256"]}}
        cases.append(case)
        authored = []
        for role in ("task", "checker"):
            name = f"authorship/{identity}-{role}.txt"
            path = root / name
            path.parent.mkdir(exist_ok=True)
            path.write_text(f"Offline declaration {identity} {role}; no distinct human authenticated.")
            authored.append({"role": role, "person_id": "offline-" + role + "-author", "authored_at": fixtures.utc(-1000),
                             "capture": {"path": name, "sha256": coding.hash_file(path)}})
        declarations.append({"case_id": identity, "source_license": "Offline test fixture, not upstream license evidence",
                             "original_root": original.relative_to(root).as_posix(), "decision_kind": "accepted_vs_proposed",
                             "supporting_sources": ["docs/decision.md"], "authorship": authored,
                             "control_files": {"correct": {"src/policy.py": correct_path.relative_to(root).as_posix()},
                                               "wrong": {"src/policy.py": wrong_path.relative_to(root).as_posix()}}})
    manifest = root / "cases.json"
    experiment.private_write(manifest, {"schema_version": 1, "pilot": True, "fixture_only": False,
                                      "independent_projects": True, "cases": cases})
    experiment.private_write(root / "DECISION_INTAKE.json", {"schema_version": 1, "cases": declarations})
    return manifest


def reviewed_fixture(root):
    manifest = candidates(root)
    entry = [intake.entry_for(case, manifest) for case in coding.load_cases(manifest)["cases"]]
    registration = outcome.preregistration_template({"cases": entry}, arms=list(intake.adaptive.SETUPS), pilot=True)
    registration.update(complete=True, study_id="offline-intake-contract", operator_id="offline-operator", registered_at=fixtures.utc(-50))
    registration["agent"].update(model="offline-model", model_revision="offline-revision", reasoning="none",
        prompt_sha256="1" * 64, command_sha256="2" * 64, tools=[], tool_call_limit=0, token_limit=1000,
        max_attempts=1, wall_time_seconds=30, temperature=0)
    registration["lore"].update(model="offline-lore", model_revision="offline-revision", reasoning="none")
    for case in registration["cases"]:
        case.update(task_authors=["offline-task-author"], checker_authors=["offline-checker-author"])
        case["independent_gold_reviews"] = fixtures.StudyPreflightTests().reviews(root, "gold-review-captures",
                                                                               case["task_sha256"], case["case_id"], fixtures.utc(-100))
    experiment.private_write(root / "PREREGISTRATION.json", registration)
    intake.validate_controls(manifest, root / "DECISION_INTAKE.json", root / "controls.json")
    declarations = experiment.read(root / "DECISION_INTAKE.json")
    for case in declarations["cases"]:
        case["controls_capture"] = {"path": "controls.json", "sha256": coding.hash_file(root / "controls.json")}
    experiment.private_write(root / "DECISION_INTAKE.json", declarations)
    return manifest


class DecisionIntakeTests(unittest.TestCase):
    def test_editable_declaration_cannot_allow_retroactively_written_decision_documents(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = candidates(root)
            case = experiment.read(manifest)["cases"][0]
            declaration = experiment.read(root / "DECISION_INTAKE.json")["cases"][0]
            case["editable_files"].append("docs/decision.md")
            (root / case["source_root"] / "docs/decision.md").write_text("Retroactively fabricated offline document")
            with self.assertRaises(ValueError):
                intake.private_material(manifest, declaration, intake.entry_for(case, manifest),
                                        [root / case["source_root"]])

    def test_codex_seal_binds_intake_and_rejects_public_or_changed_material(self):
        import test_codex_protocol as codex_fixtures
        import os
        if os.name != "posix":
            self.skipTest("Ready Codex fixture requires the supported POSIX runner")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            bundle, _ = codex_fixtures.CodexProtocolTests().bundle(root)
            self.assertTrue(experiment.preflight(bundle)["preflight_passed"])
            intake_record = bundle / "DECISION_INTAKE.json"
            data = experiment.read(intake_record)
            data["cases"][0]["source_license"] = "Changed declaration after sealing"
            experiment.private_write(intake_record, data)
            with patch("subprocess.Popen", side_effect=AssertionError("Changed intake launched child")):
                self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
            intake_record.unlink()
            rejected = experiment.preflight(bundle)
            self.assertFalse(rejected["preflight_passed"])
            self.assertIn("independent_decision_task_intake", {item["gate"] for item in rejected["blockers"]})

    def test_manifest_generation_fills_missing_pins_and_never_repins_changed_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = candidates(root)
            expected = experiment.read(manifest)
            data = copy.deepcopy(expected)
            for case in data["cases"]:
                del case["input_sha256"]
                del case["upstream"]["files_sha256"]
            experiment.private_write(manifest, data)
            with patch("subprocess.Popen", side_effect=AssertionError("Pinning executed code")):
                value = intake.generate_manifest(manifest, root / "DECISION_INTAKE.json", root / "pinned.json")
            self.assertEqual(value["status"], "candidate_unreviewed")
            self.assertEqual(experiment.read(root / "pinned.json"), expected)
            expected["cases"][0]["input_sha256"] = "0" * 64
            experiment.private_write(manifest, expected)
            with self.assertRaises(ValueError):
                intake.generate_manifest(manifest, root / "DECISION_INTAKE.json", root / "rejected.json")
            self.assertFalse((root / "rejected.json").exists())

    def test_positive_and_negative_controls_are_source_preserving_but_not_human_authentication(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = reviewed_fixture(root)
            before = cross.fingerprint(root)
            with patch("subprocess.Popen", side_effect=AssertionError("Inspection executed candidate")):
                result = intake.inspect(manifest)
            self.assertTrue(result["eligible"], result["issues"])
            self.assertIn("do not authenticate", result["qualification"])
            self.assertEqual(cross.fingerprint(root), before)
            rows = experiment.read(root / "controls.json")["cases"]
            self.assertEqual(len(rows), 6)
            self.assertTrue(all(item["source_preserved"] for row in rows for item in row["variants"]))

    def test_counterfeit_flags_count_provenance_and_review_reuse_are_deficits(self):
        for kind in ("flags", "count", "provenance", "review", "author", "checksum", "projects"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                if kind in ("flags", "count"):
                    manifest = candidates(root, count=5 if kind == "count" else 6)
                else:
                    manifest = reviewed_fixture(root)
                    data = experiment.read(manifest)
                    if kind == "provenance": data["cases"][0]["upstream"]["commit"] = ""
                    if kind == "projects":
                        for case in data["cases"]: case["upstream"]["repository"] = "offline-owner/one"
                    experiment.private_write(manifest, data)
                    registration = experiment.read(root / "PREREGISTRATION.json")
                    if kind == "review": registration["cases"][0]["independent_gold_reviews"][1] = registration["cases"][0]["independent_gold_reviews"][0]
                    if kind == "author": registration["cases"][0]["checker_authors"] = registration["cases"][0]["task_authors"]
                    if kind == "checksum": (root / "controls.json").write_text("{}")
                    experiment.private_write(root / "PREREGISTRATION.json", registration)
                self.assertFalse(intake.inspect(manifest)["eligible"])

    def test_renamed_public_source_duplicate_and_checker_leak_are_rejected(self):
        for kind in ("public", "duplicate", "checker", "original", "symlink", "traversal"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                manifest = candidates(root)
                data = experiment.read(manifest)
                case = data["cases"][0]
                source = root / case["source_root"]
                if kind == "public":
                    known = coding.ROOT / "corpora/decision-work-09/accepted-before-proposal/src/policy.py"
                    (source / "src/renamed.py").write_bytes(known.read_bytes())
                if kind == "duplicate": data["cases"][1]["task"] = case["task"]
                if kind == "original": (source / "hidden.py").write_bytes((root / "offline-case-0/original/src/policy.py").read_bytes())
                if kind == "checker": (source / "hidden.py").write_bytes((root / "offline-case-0/checker.py").read_bytes())
                if kind == "symlink":
                    try: (source / "leak.py").symlink_to(root / "offline-case-0/correct.py")
                    except OSError: continue
                if kind == "traversal": case["source_root"] = "../escape"
                experiment.private_write(manifest, data)
                value = intake.inspect(manifest)
                self.assertFalse(value["eligible"])
                if kind == "public": self.assertIn("published_task_or_source_overlap", {item["code"] for item in value["issues"]})
                if kind == "duplicate": self.assertIn("duplicated_task_or_renamed_source", {item["code"] for item in value["issues"]})

    def test_checker_source_mutation_and_controls_changed_after_review_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = reviewed_fixture(root)
            checker = root / "offline-case-0/checker.py"
            checker.write_text(checker.read_text() + "(Path(sys.argv[1])/'src/policy.py').write_text('mutated')\n")
            self.assertFalse(intake.inspect(manifest)["eligible"])
            with self.assertRaises(ValueError):
                intake.validate_controls(manifest, root / "DECISION_INTAKE.json", root / "new-controls.json")

    def test_holdout_detects_pilot_byte_overlap_under_new_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            manifest = candidates(root)
            entries = [intake.entry_for(case, manifest) for case in coding.load_cases(manifest)["cases"]]
            prepared = root / "pilot.json"
            experiment.private_write(prepared, {"cases": entries})
            data = experiment.read(manifest)
            data.update(pilot=False, held_out=True)
            for case in data["cases"]: case["id"] = "holdout-" + case["id"]
            experiment.private_write(manifest, data)
            result = intake.inspect(manifest, pilot_prepared=prepared)
            self.assertFalse(result["eligible"])
            self.assertIn("pilot_source_or_task_overlap", {item["code"] for item in result["issues"]})


if __name__ == "__main__":
    unittest.main()
