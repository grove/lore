"""Pinned nonsecret CLI protocol doubles; never calls hosted inference."""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import codex_agent as agent
import coding_tasks as coding
import provider_usage as metering


def request():
    return coding.agent_request({"task": "Return a value", "editable_files": ["src/a.py"]},
                                {"src/a.py": "pass\n", "README.md": "Original documentation"}, None)


def events(answer=None, usage=True):
    answer = answer or {"schema_version": 1, "files": {"src/a.py": "value = 1\n"}, "summary": "Implemented"}
    return [{"type": "thread.started", "model": "offline-model"}, {"type": "turn.started"},
            {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(answer)}},
            {"type": "turn.completed", **({"usage": {"input_tokens": 12, "output_tokens": 3,
                                                   "cached_input_tokens": 8}} if usage else {})}]


def fake(root, stream=None, *, suffix="", help_text=None, version="codex-cli 0.162.1"):
    path = root / "offline-codex"
    text = " ".join(agent.REQUIRED_FLAGS) if help_text is None else help_text
    path.write_text("#!" + str(Path(sys.executable).resolve()) + "\n" +
        "import json, sys, time\n" +
        f"if '--version' in sys.argv: print({version!r}); sys.exit()\n" +
        f"if '--help' in sys.argv: print({text!r}); sys.exit()\n" +
        "data = sys.stdin.read()\n" +
        "assert 'Untrusted task data:' in data\n" +
        "assert '--ignore-user-config' in sys.argv and '--ignore-rules' in sys.argv\n" +
        "assert sys.argv[sys.argv.index('--sandbox') + 1] == 'read-only'\n" +
        "assert '--dangerously-bypass-approvals-and-sandbox' not in sys.argv\n" +
        "assert json.loads(data.split('Untrusted task data:\\n')[1])['files']['README.md'] == 'Original documentation'\n" +
        "assert all(not key.startswith(('LORE_', 'CODEX_')) or key in ('CODEX_API_KEY', 'CODEX_HOME') for key in __import__('os').environ)\n" +
        "assert __import__('pathlib').Path(__import__('os').environ['CODEX_HOME']).is_dir()\n" +
        f"for event in {stream if stream is not None else events()!r}: print(json.dumps(event), flush=True)\n" + suffix,
        encoding="utf-8")
    path.chmod(0o700)
    return argparse.Namespace(codex_bin=path, codex_sha256=coding.hash_file(path),
        codex_version="codex-cli 0.162.1", model="offline-model", reasoning="low", timeout=2, allow_hosted=True)


@unittest.skipUnless(os.name == "posix", "Real process doubles use the supported POSIX execution boundary")
class CodexProcessTests(unittest.TestCase):
    def run_double(self, root, args, value=None):
        ledger = root / "usage.json"
        with patch.dict(os.environ, {"LORE_USAGE_LEDGER": str(ledger), "CODEX_PROFILE": "untrusted",
                                     "LORE_MODEL": "untrusted", "OPENAI_BASE_URL": "untrusted"}):
            result = agent.generate(args, value or request())
        return result, metering.read(ledger)

    def test_success_pins_usage_and_source_docs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            value, record = self.run_double(root, fake(root))
            self.assertEqual(value["files"], {"src/a.py": "value = 1\n"})
            self.assertEqual(value["provider_model"], "offline-model")
            self.assertEqual(value["usage"], metering.summary_from_record(record))
            self.assertEqual(value["usage"]["total_tokens"], 15)
            self.assertIsNone(value["usage"]["provider_request_count"])
            self.assertIsNone(value["usage"]["billed_cost_usd"])
            self.assertEqual(value["adapter_receipt"]["cached_input_tokens"], 8)
            self.assertIsNone(value["adapter_receipt"]["effective_backend_revision"])
            self.assertEqual(set(root.iterdir()), {root / "usage.json", root / "offline-codex"})

    def test_interim_messages_do_not_replace_the_final_structured_answer(self):
        for final_valid in (True, False):
            with self.subTest(final_valid=final_valid), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                interim = {"type": "item.completed", "item": {"type": "agent_message", "text": "Offline progress"}}
                stream = [*events()[:2], interim, *events()[2:]]
                if not final_valid:
                    stream.insert(-1, interim)
                args = fake(root, stream)
                if final_valid:
                    value, _ = self.run_double(root, args)
                    self.assertEqual(value["files"], {"src/a.py": "value = 1\n"})
                else:
                    with self.assertRaises(ValueError):
                        self.run_double(root, args)
                    self.assertEqual(metering.read(root / "usage.json")["ledger"]["summary"]["total_tokens"], 15)

    def test_missing_usage_or_ambiguous_cumulative_usage_remains_unknown(self):
        for stream in (events(usage=False), [*events(), events()[-1]]):
            with self.subTest(stream=stream), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                result, _ = self.run_double(root, fake(root, stream))
                self.assertIsNone(result["usage"]["input_tokens"])
                self.assertIsNone(result["usage"]["output_tokens"])
                self.assertIsNone(result["usage"]["billed_cost_usd"])

    def test_invalid_final_files_traversal_usage_spoof_and_oversize_fail(self):
        for answer in ({"schema_version": True, "files": {"src/a.py": "x"}, "summary": "x"},
                       {"schema_version": 1, "files": {}, "summary": "x"},
                       {"schema_version": 1, "files": {"../secret": "x"}, "summary": "x"},
                       {"schema_version": 1, "files": {"src/a.py": "x"}, "summary": " "},
                       {"schema_version": 1, "files": {"src/a.py": "x"}, "summary": "x", "usage": {}},
                       {"schema_version": 1, "files": {"src/a.py": "x" * 500001}, "summary": "x"}):
            with self.subTest(keys=list(answer)), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                args = fake(root, events(answer))
                with self.assertRaises(ValueError):
                    self.run_double(root, args)
                record = metering.read(root / "usage.json")
                self.assertEqual(record["ledger"]["invocation_status"], "failed")
                self.assertEqual(record["ledger"]["summary"]["input_tokens"], 12)

    def test_refusal_failure_blank_broken_json_and_model_mismatch_retained(self):
        streams = [[], [{"type": "turn.failed"}], [{"type": "refusal"}],
                   [{"type": "thread.started", "model": "other"}],
                   [{"type": "item.completed", "item": {"type": "command_execution"}}],
                   events()[:-1]]
        for stream in streams:
            with self.subTest(stream=stream), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                with self.assertRaises(ValueError):
                    self.run_double(root, fake(root, stream))
                self.assertEqual(metering.read(root / "usage.json")["ledger"]["invocation_status"], "failed")
        for suffix in ("print('not JSON')\n", "print('x' * 2000001)\n", "sys.exit(3)\n"):
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                with self.assertRaises(ValueError):
                    self.run_double(root, fake(root, events(), suffix=suffix))

    def test_timeout_retains_observed_usage_and_kills_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            args = fake(root, events(), suffix="time.sleep(20)\n")
            args.timeout = 1
            with self.assertRaises(agent.AdapterFailure):
                self.run_double(root, args)
            record = metering.read(root / "usage.json")
            self.assertEqual(record["ledger"]["events"][0]["status"], "timed_out")
            self.assertEqual(record["ledger"]["summary"]["total_tokens"], 15)

    def test_pin_flag_version_or_grant_rejection_leaves_ledger_absent(self):
        for kind in ("sha", "version", "flags", "missing", "grant"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                args = fake(root, help_text="--json" if kind == "flags" else None)
                if kind == "sha": args.codex_sha256 = "0" * 64
                if kind == "version": args.codex_version = "codex-cli 0.0.1"
                if kind == "missing": args.codex_bin.unlink()
                if kind == "grant": args.allow_hosted = False
                with self.assertRaises((ValueError, OSError)):
                    self.run_double(root, args)
                self.assertFalse((root / "usage.json").exists())

    def test_private_output_does_not_escape_on_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            args = fake(root, [], suffix="print('sk-secret-protocol-double', flush=True)\n")
            command = [sys.executable, str(Path(agent.__file__).resolve()), "--codex-bin", str(args.codex_bin),
                       "--codex-sha256", args.codex_sha256, "--codex-version", args.codex_version,
                       "--model", args.model, "--reasoning", args.reasoning, "--allow-hosted"]
            import subprocess
            result = subprocess.run(command, input=json.dumps(request()), text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertNotIn("sk-secret", result.stderr)


class CodexRequestTests(unittest.TestCase):
    def test_contract_rejects_untrusted_extra_fields_and_bad_repair(self):
        for name, value in (("checker", "hidden"), ("files", {"../secret": "x"}),
                            ("editable_files", ["src/a.py", "src/a.py"]), ("task", ""),
                            ("repair", {"feedback": "raw checker output"})):
            with self.subTest(name=name), self.assertRaises(ValueError):
                agent.visible_request({**request(), name: value})
        value = request()
        value["repair"] = {"attempt": 2, "max_attempts": 2,
                           "feedback": {"independent_checks_passed": False, "messages": [coding.DEFAULT_REPAIR_MESSAGE]}}
        self.assertEqual(agent.visible_request(value)["repair"], value["repair"])


if __name__ == "__main__":
    unittest.main()
