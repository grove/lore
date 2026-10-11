"""Inspection has no execution or write authority. All inputs here are offline fixtures."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cross_source as cross
import study_09 as study
import test_experiment_08 as fixtures


class StudyStatusTests(unittest.TestCase):
    def test_absent_malformed_incomplete_and_tampered_are_read_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            helper = fixtures.StudyPreflightTests()
            bundle = root / "absent"
            with patch("subprocess.Popen", side_effect=AssertionError("inspection executed")):
                self.assertEqual(study.status(bundle)["initial_assignments"], 0)
                self.assertFalse(bundle.exists())
                bundle = helper.bundle(root, ready=False)
                for kind in ("prepared", "malformed", "tampered"):
                    if kind == "malformed":
                        (bundle / "EXPERIMENT.json").write_text("{broken")
                    if kind == "tampered":
                        (bundle / "snapshots").joinpath(next((bundle / "snapshots").iterdir()).name,
                                                      "extra.txt").write_text("tampered")
                    before = cross.fingerprint(root)
                    first = study.status(bundle)
                    self.assertEqual(first, study.status(bundle))
                    self.assertFalse(first["preflight_passed"])
                    self.assertEqual(first["provider_calls"], 0)
                    self.assertEqual(cross.fingerprint(root), before)

    def test_fully_prepared_offline_fixture_is_readiness_never_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle = fixtures.StudyPreflightTests().bundle(Path(temporary).resolve())
            before = cross.fingerprint(bundle)
            with patch("subprocess.Popen", side_effect=AssertionError("inspection executed")):
                value = study.status(bundle)
            self.assertTrue(value["preflight_passed"])
            self.assertEqual(value["initial_assignments"], 36)
            self.assertEqual(value["coding_productivity"], "unmeasured")
            self.assertFalse(value["productivity_benefit_established"])
            self.assertEqual(cross.fingerprint(bundle), before)
            self.assertIn("36", study.markdown(value))


if __name__ == "__main__":
    unittest.main()
