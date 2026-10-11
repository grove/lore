import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import decision_work_09 as work
import experiment_08 as experiment


class DecisionWorkTests(unittest.TestCase):
    def test_all_eight_correct_and_plausibly_wrong_implementations(self):
        with tempfile.TemporaryDirectory() as temp:
            report = work.validate(Path(temp).resolve() / "controls")
        self.assertEqual(report["status"], "passed")
        self.assertEqual(len(report["controls"]), 24)
        self.assertEqual(len({row["case"] for row in report["controls"]}), 8)
        self.assertEqual(report["provider_calls"], 0)
        self.assertFalse(report["productivity_benefit_established"])

    def test_preparation_keeps_answers_external_and_all_six_arms(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp).resolve() / "study"
            prepared = adaptive.prepare(output, work.CASES)
            self.assertEqual(prepared["setups"], list(adaptive.SETUPS))
            self.assertTrue(prepared["fixture_only"])
            experiment.checker_separation(output, prepared)
            for entry in prepared["cases"]:
                source = output / entry["source_root"]
                self.assertEqual(cross.fingerprint(source), entry["source_manifest"])
                self.assertNotIn("controls.json", entry["source_manifest"]["files_sha256"])
                self.assertNotIn("check.py", entry["source_manifest"]["files_sha256"])
                files = {name: (source / name).read_text(encoding="utf-8")
                         for name in entry["source_manifest"]["files_sha256"]}
                requests = [coding.agent_request(entry["case"], files,
                            None if arm == "baseline" else {"response": {"fixture_context": arm}})
                            for arm in adaptive.SETUPS]
                for request in requests:
                    self.assertEqual(request["files"], files)
                    self.assertEqual(request["task"], entry["case"]["task"])
                    self.assertNotIn("probes", json.dumps(request))
            entry = prepared["cases"][0]
            source = output / entry["source_root"]
            shutil.copyfile(work.CORPUS / "controls.json", source / "renamed-notes.json")
            entry["source_manifest"] = cross.fingerprint(source)
            with self.assertRaisesRegex(ValueError, "answer-bearing"):
                experiment.checker_separation(output, prepared)

    def test_relabeling_tasks_does_not_create_a_holdout(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            manifest = coding.load_cases(work.CASES)
            manifest.update(fixture_only=False, held_out=True, independent_projects=True)
            for i, case in enumerate(manifest["cases"]):
                case["id"] = f"renamed-{i}"
                case["project"] = f"upstream-{i}"
                case["source_root"] = str(work.CORPUS / case["source_root"])
                case["test_command"][1] = str(work.CORPUS / "check.py")
                case["verification_files"] = [str(work.CORPUS / p) for p in case["verification_files"]]
            path = root / "cases.json"
            cross.write_json(path, manifest)
            prepared = coding.prepare(root / "study", path)
            self.assertTrue(prepared["fixture_only"])
            self.assertFalse(prepared["held_out"])
            self.assertEqual(len(prepared["recognized_fixture_fingerprints"]), 8)

    def test_changed_checker_and_task_pins_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp).resolve() / "study"
            prepared = adaptive.prepare(output, work.CASES)
            entry = copy.deepcopy(prepared["cases"][0])
            entry["checker_files_sha256"][str(work.CORPUS / "controls.json")] = "0" * 64
            with self.assertRaisesRegex(ValueError, "checker bytes changed"):
                coding.validate_prepared_checker(entry, output / entry["source_root"], work.CORPUS)
            path = output / "prepared.json"
            value = json.loads(path.read_text())
            value["cases"][0]["case"]["task"] = "Substituted task"
            cross.write_json(path, value)
            with self.assertRaisesRegex(ValueError, "no longer match"):
                experiment.prepared_inputs(output)


if __name__ == "__main__":
    unittest.main()
