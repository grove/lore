"""Offline Codex sealing/identity fixtures; none authenticates a live model."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import codex_agent
import coding_tasks as coding
import cross_source as cross
import experiment_08 as experiment
import outcome_protocol as outcome
import test_experiment_08 as fixtures
import test_codex_agent as doubles
import test_decision_task_intake_09 as intake_fixtures
import shutil


@unittest.skipUnless(os.name == "posix", "Sealed live execution requires the private POSIX runner")
class CodexProtocolTests(unittest.TestCase):
    def bundle(self, root):
        helper = fixtures.StudyPreflightTests()
        independent = root / "independent"
        independent.mkdir()
        manifest = intake_fixtures.reviewed_fixture(independent)
        bundle = helper.bundle(root, cases_manifest=manifest)
        for name in ("DECISION_INTAKE.json", "controls.json"):
            shutil.copyfile(independent / name, bundle / name)
        shutil.copytree(independent / "authorship", bundle / "authorship")
        (bundle / "SEAL.json").unlink()
        (bundle / "READINESS.json").unlink()
        args = doubles.fake(root)
        spec = experiment.specification(bundle)
        spec["run"].update(agent_command=[str(Path(sys.executable).resolve()), str(experiment.ROOT / "codex_agent.py"),
            "--codex-bin", str(args.codex_bin), "--codex-sha256", args.codex_sha256,
            "--codex-version", args.codex_version, "--model", args.model, "--reasoning", args.reasoning,
            "--timeout", "2", "--allow-hosted"], allow_hosted=True, agent_location="hosted")
        spec["agent"].update(provider="openai", endpoint="https://api.openai.com/v1", model=args.model,
                             model_revision="offline-revision", reasoning=args.reasoning)
        (bundle / "prompt.txt").write_text(codex_agent.PROMPT, encoding="utf-8")
        experiment.private_write(bundle / "EXPERIMENT.json", spec)
        pins = experiment.pins(bundle)
        registration = experiment.read(bundle / "PREREGISTRATION.json")
        registration["agent"].update(model=args.model, reasoning=args.reasoning,
            command_sha256=pins["agent_command_sha256"], prompt_sha256=pins["configuration"]["prompt_sha256"],
            codex_adapter=pins["configuration"]["codex_adapter"])
        registration["lore"]["configuration_sha256_by_case"] = pins["configuration"]["configuration_sha256_by_case"]
        experiment.private_write(bundle / "PREREGISTRATION.json", registration)
        experiment.seal(bundle)
        readiness = experiment.read(bundle / "READINESS.json")
        readiness.update(complete=True, observed_at=fixtures.utc(-20), expires_at=fixtures.utc(3600))
        for value in readiness["providers"].values():
            value["available"] = True
        readiness["enforcement"] = {"method": "Offline audit fixture only", **dict.fromkeys((*experiment.ENFORCEMENT, "billing_budget_enforced"), True)}
        readiness["captures"] = []
        for kind in ("provider_availability", "runtime_isolation", "egress_enforcement", "billing_enforcement"):
            name = "readiness-captures/" + kind + ".txt"
            path = bundle / name
            path.write_text("Offline Codex protocol capture " + kind)
            readiness["captures"].append({"kind": kind, "path": name, "sha256": coding.hash_file(path)})
        authentication = bundle / "readiness-captures/authentication.txt"
        readiness["authentication"] = {"method": "Offline identity fixture", "capture": {
            "path": "readiness-captures/authentication.txt", "sha256": coding.hash_file(authentication)}}
        subject = {key: value for key, value in readiness.items() if key not in ("reviews", "qualification")}
        readiness["reviews"] = helper.reviews(bundle, "readiness-captures", cross.digest(subject), "runtime", fixtures.utc(-10))
        experiment.private_write(bundle / "READINESS.json", readiness)
        return bundle, args

    def test_proposals_checks_and_timeout_use_existing_attempt_journals(self):
        for mode in ("correct", "wrong", "timeout"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                answer = {"schema_version": 1, "files": {"src/a.py": "value = " + ("1" if mode == "correct" else "2") + "\n"}, "summary": "Offline proposal"}
                args = doubles.fake(root, doubles.events(answer), suffix="time.sleep(20)\n" if mode == "timeout" else "")
                command = [str(Path(sys.executable).resolve()), str(Path(codex_agent.__file__).resolve()),
                    "--codex-bin", str(args.codex_bin), "--codex-sha256", args.codex_sha256,
                    "--codex-version", args.codex_version, "--model", args.model, "--reasoning", args.reasoning,
                    "--timeout", "1", "--allow-hosted"]
                checker = root / "checker.py"
                checker.write_text("import json,pathlib,sys\nvalue=pathlib.Path(sys.argv[1], 'src/a.py').read_text()\nprint(json.dumps({'schema_version':1,'checks':[{'id':'correct','kind':'correctness','passed':value=='value = 1\\n'}]}))\n")
                case = {"task": "Return a value", "editable_files": ["src/a.py"],
                    "test_command": ["{python}", "{manifest}/checker.py", "{workspace}"], "verification_files": ["checker.py"]}
                source = doubles.request()["files"]
                workspace, output = root / "workspace", root / "run"
                for name, content in source.items():
                    path = workspace / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(content.encode())
                policy = {"max_attempts": 1, "attempt_budget_seconds": 10}
                environment = lambda _workspace, _stage: {"env": {"PATH": os.environ.get("PATH", "")}}
                if mode == "timeout":
                    with self.assertRaises(ValueError):
                        coding.run_attempts(command, case, source, None, workspace, output, "offline", root, 5, policy, environment)
                    invocation = cross.read_json(output / "attempt-invocations/offline/1.json")
                    self.assertEqual(invocation["status"], "incomplete")
                    self.assertEqual(invocation["usage"]["total_tokens"], 15)
                    self.assertFalse((output / "attempt-records").exists())
                    self.assertEqual((workspace / "src/a.py").read_text(), source["src/a.py"])
                else:
                    result = coding.run_attempts(command, case, source, None, workspace, output, "offline", root, 5, policy, environment)
                    self.assertEqual(result["tests"]["checks"][0]["passed"], mode == "correct")
                    self.assertEqual(result["stop_reason"], "correct" if mode == "correct" else "attempt_limit")
                    self.assertTrue((output / "attempt-records/offline/1.json").is_file())
                self.assertEqual((workspace / "README.md").read_text(), "Original documentation")

    def test_exact_pins_pass_read_only_offline_preflight(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle, _ = self.bundle(Path(temporary).resolve())
            before = cross.fingerprint(bundle)
            with patch.object(subprocess, "Popen", side_effect=AssertionError("Inspection launched child")):
                value = experiment.preflight(bundle)
            self.assertTrue(value["preflight_passed"], value["blockers"])
            self.assertEqual(value["initial_assignments"], 36)
            self.assertEqual(value["coding_productivity"], "unmeasured")
            self.assertEqual(cross.fingerprint(bundle), before)

    def test_binary_prompt_schema_argv_tools_or_receipt_changes_reject_before_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle, args = self.bundle(Path(temporary).resolve())
            spec = experiment.read(bundle / "EXPERIMENT.json")
            for field, replacement in (("model", "other"), ("allow_hosted", False)):
                changed = copy.deepcopy(spec)
                changed["run"][field] = replacement
                experiment.private_write(bundle / "EXPERIMENT.json", changed)
                self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
            experiment.private_write(bundle / "EXPERIMENT.json", spec)
            args.codex_bin.write_text("Changed executable")
            self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
        for kind in ("prompt", "schema", "argv", "tools", "receipt"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                bundle, _ = self.bundle(Path(temporary).resolve())
                if kind == "prompt":
                    (bundle / "prompt.txt").write_text("Different prompt")
                elif kind == "schema":
                    with patch.object(codex_agent, "SCHEMA", bundle / "prompt.txt"):
                        self.assertFalse(experiment.preflight(bundle)["preflight_passed"])
                    continue
                elif kind == "argv":
                    spec = experiment.read(bundle / "EXPERIMENT.json")
                    spec["run"]["agent_command"] += ["--config", "sandbox=full"]
                    experiment.private_write(bundle / "EXPERIMENT.json", spec)
                else:
                    registered = experiment.read(bundle / "PREREGISTRATION.json")
                    if kind == "tools": registered["agent"]["tools"] = ["shell"]
                    if kind == "receipt": registered["agent"]["codex_adapter"]["schema_sha256"] = "0" * 64
                    experiment.private_write(bundle / "PREREGISTRATION.json", registered)
                with patch.object(subprocess, "Popen", side_effect=AssertionError("Rejected pins launched")):
                    self.assertFalse(experiment.preflight(bundle)["preflight_passed"])

    def test_missing_model_observation_cannot_be_replaced_by_requested_flag(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundle, args = self.bundle(Path(temporary).resolve())
            registration = experiment.read(bundle / "PREREGISTRATION.json")
            pin = registration["agent"]["codex_adapter"]
            report = {"samples": [{"case_id": entry["case"]["id"],
                "configuration_sha256": registration["lore"]["configuration_sha256_by_case"][entry["case"]["id"]],
                "context": {"offline_fixture": True}, "attempts": [{"response": {"provider_model": None,
                    "adapter_receipt": {**pin, "call_id": entry["case"]["id"]}}}]}
                for entry in experiment.read(bundle / "prepared.json")["cases"]]}
            for sample in report["samples"]:
                sample["agent_response"] = sample["attempts"][0]["response"]
            with self.assertRaises(ValueError):
                outcome.validate_model_pins(registration, report)
            observation = {"schema_version": 1, "complete": True, "metrics_sha256": cross.digest(report),
                           "authentication": {"method": "Offline fixture", "capture": registration["cases"][0]["independent_gold_reviews"][0]["capture"]},
                           "invocations": [], "reviews": []}
            for sample in report["samples"]:
                name = sample["case_id"] + "-identity.txt"
                (bundle / name).write_text("Offline identity observation " + name)
                observation["invocations"].append({"call_id": sample["case_id"], "model": args.model,
                    "model_revision": "offline-revision", "observed_at": fixtures.utc(),
                    "capture": {"path": name, "sha256": coding.hash_file(bundle / name)}})
            subject = {key: value for key, value in observation.items() if key != "reviews"}
            observation["reviews"] = fixtures.StudyPreflightTests().reviews(bundle, "identity-reviews", cross.digest(subject), "identity", fixtures.utc())
            experiment.private_write(bundle / "CODEX_IDENTITIES.json", observation)
            observed = outcome.codex_model_observations(bundle, registration, report)
            outcome.validate_model_pins(registration, report, codex_observations=observed)
            observation["invocations"][0]["model_revision"] = "different"
            experiment.private_write(bundle / "CODEX_IDENTITIES.json", observation)
            with self.assertRaises(ValueError):
                outcome.codex_model_observations(bundle, registration, report)


if __name__ == "__main__":
    unittest.main()
