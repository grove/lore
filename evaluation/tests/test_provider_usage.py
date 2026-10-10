"""Offline provider accounting controls. No genuine model or billing claims."""
from __future__ import annotations

import argparse
import copy
from io import BytesIO
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coding_agent
import coding_tasks as coding
import provider_usage as metering


def response(*, input_tokens=11, output_tokens=3, content=None):
    return {"model":"fixture-model", "status":"completed", "usage":{"input_tokens":input_tokens,"output_tokens":output_tokens},
        "output":[{"content":[{"type":"output_text","text":json.dumps(content or {
            "schema_version":1,"files":{"src/a.py":"pass\n"},"summary":"Offline fixture"})}]}]}


def args(path=None):
    return argparse.Namespace(provider="openai",model="fixture-model",base_url="https://fixture.invalid/v1",
        api_key_env="TEST_PROVIDER_KEY",allow_hosted=True,reasoning_effort=None,timeout=10,retries=1,
        usage_ledger=str(path) if path else None)


def event(identity="a", *, call="call", status="completed", cached=False):
    measured = metering.no_inference() if cached else metering.tokens("openai",response())
    return {"id":identity,"call_id":call,"operation":"generation","provider":"openai","model":"fixture-model",
        "attempt":0 if cached else 1,"elapsed_ms":1,"http_status":None if cached else 200,
        **{key:measured[key] for key in ("provider_request_count","input_tokens","output_tokens","total_tokens","billed_cost_usd","billing_source")},
        "status":"cached" if cached else status,"cache_hit":cached}


class ProviderUsageTests(unittest.TestCase):
    def test_optional_provider_units_and_invalid_numbers_are_not_fabricated(self):
        for provider, raw in (("openai",response()),("ollama",{"prompt_eval_count":11,"eval_count":3})):
            measured = metering.tokens(provider,raw)
            self.assertEqual(measured["total_tokens"],14)
            self.assertIsNone(measured["billed_cost_usd"])
        for value in (-1,1.0,"1",True,2**64):
            raw = response(input_tokens=value)
            self.assertIsNone(metering.tokens("openai",raw)["input_tokens"])
            self.assertIsNone(metering.tokens("openai",raw)["total_tokens"])
        raw = response()
        raw["usage"]["total_tokens"] = "14 tokens"
        self.assertIsNone(metering.tokens("openai",raw)["total_tokens"])
        raw["usage"]["total_tokens"] = 999
        self.assertIsNone(metering.tokens("openai",raw)["total_tokens"])
        self.assertEqual(metering.tokens("ollama",{"prompt_eval_count":22},embedding=True)["total_tokens"],22)
        self.assertIsNone(metering.tokens("typesafe",{"usage":{"input_tokens":22}})["total_tokens"])

    def test_all_attempts_are_charged_but_cache_history_is_not(self):
        first, retry, cached = event(), event("b"), event("c",cached=True)
        first.update(status="http_error",http_status=503)
        summary = metering.aggregate([first,retry,cached])
        self.assertEqual(summary["model_calls"],1)
        self.assertEqual(summary["provider_request_count"],2)
        self.assertEqual(summary["input_tokens"],22)
        self.assertEqual(summary["total_tokens"],28)
        self.assertEqual(summary["cache_hits"],1)
        self.assertIsNone(summary["billed_cost_usd"])
        self.assertEqual(metering.aggregate([cached])["billed_cost_usd"],0)
        first.update(input_tokens=None,output_tokens=None,total_tokens=None)
        self.assertIsNone(metering.aggregate([first,retry])["total_tokens"])

    def test_ledger_rejects_tampering_duplicate_charges_and_sensitive_fields(self):
        ledger = metering.ledger([event()],"completed")
        broken = copy.deepcopy(ledger)
        broken["summary"]["provider_request_count"] = 0
        with self.assertRaises(ValueError): metering.validate_ledger(broken)
        with self.assertRaises(ValueError): metering.aggregate([event(),event()])
        with self.assertRaises(ValueError): metering.aggregate([event() | {"raw_response":"private"}])
        with self.assertRaises(ValueError): metering.aggregate([event(cached=True) | {"input_tokens":1000}])
        with self.assertRaises(ValueError): metering.normalized({"billed_cost_usd":0.1})

    def test_raw_usage_and_currency_totals_must_equal_all_attempts(self):
        first, retry = event(), event("retry")
        first.update(status="http_error", http_status=503, billed_cost_usd=0.125,
                     billing_source="offline invoice fixture")
        retry.update(attempt=2, billed_cost_usd=0.375, billing_source="offline invoice fixture")
        raw = metering.ledger([first, retry], "completed")
        self.assertEqual(raw["summary"]["billed_cost_usd"], 0.5)
        for substituted in ({"input_tokens":11,"output_tokens":3,"total_tokens":14},
                            {"billed_cost_usd":0.375}, {"billed_cost_usd":0.0}):
            broken = copy.deepcopy(raw)
            broken["summary"].update(substituted)
            with self.assertRaises(ValueError):
                metering.validate_ledger(json.loads(json.dumps(broken)))
        retry.update(billed_cost_usd=None, billing_source=None)
        self.assertIsNone(metering.aggregate([first, retry])["billed_cost_usd"])
        self.assertIsNone(coding.add_usage([metering.no_inference(),
            metering.normalized(metering.aggregate([first, retry]))])["billed_cost_usd"])

    def test_public_summary_is_bound_to_the_captured_event_prefix(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve()/"usage.json"
            recorder = metering.Recorder(str(path))
            recorder.events.append(event())
            summary = metering.aggregate(recorder.events)
            summary["ledger"] = {"path":"never-open-this-response-path", "event_count":1,
                                 "events_sha256":metering.events_digest(recorder.events)}
            recorder.events.append(event("later", call="verification"))
            recorder.save("completed")
            captured = metering.read(path)
            self.assertEqual(metering.bind_summary(summary,captured)["provider_request_count"],2)
            summary["input_tokens"], summary["total_tokens"] = 1, 4
            with self.assertRaises(ValueError): metering.bind_summary(summary,captured)

    def test_event_hash_matches_rust_for_unicode_and_small_currency_amounts(self):
        value = event("event-1", call="call-1")
        value.update(provider="ollama", model="modèle", elapsed_ms=23,
                     billed_cost_usd=0.000001, billing_source="offline fixture")
        self.assertEqual(metering.events_digest([value]),
                         "bf9c88fabbaf5a0a8be487cc01fc566dcbfdc107a7afe460b9aa5d07c51551c1")

    def test_cancelled_coding_subprocess_keeps_attempt_usage_journal(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            def cancelled(command, cwd, timeout, request, *, env):
                recorder = metering.Recorder(env["LORE_USAGE_LEDGER"])
                recorder.events.append(event(status="started") | {
                    "input_tokens":None, "output_tokens":None, "total_tokens":None})
                recorder.save()
                raise KeyboardInterrupt()
            with patch.object(coding, "invoke_json", side_effect=cancelled), self.assertRaises(KeyboardInterrupt):
                coding.run_attempts(["fixture"], {"task":"Change", "editable_files":["a.py"]},
                    {"a.py":"pass"}, None, root/"workspace", root/"output", "sample-1", root,
                    10, {"max_attempts":1,"attempt_budget_seconds":10}, lambda *_: {})
            journal = json.loads((root/"output/attempt-invocations/sample-1/1.json").read_text())
            self.assertEqual(journal["status"], "cancelled")
            self.assertEqual(journal["error_type"], "KeyboardInterrupt")
            self.assertEqual(journal["usage"]["provider_request_count"],1)
            self.assertIsNone(journal["usage"]["billed_cost_usd"])
            self.assertIsNone(journal["usage"]["total_tokens"])
            self.assertTrue(metering.validate_record(journal["usage_ledger"]))

    def test_adapter_retry_ledger_keeps_provider_counts_and_unknown_bill(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve()/"usage.json"
            failure = HTTPError("https://fixture.invalid",503,"private message",{},BytesIO(json.dumps({
                "error":"PRIVATE_ERROR_BODY","usage":{"input_tokens":7,"output_tokens":2}}).encode()))
            with patch.dict(coding_agent.os.environ,{"TEST_PROVIDER_KEY":"PRIVATE_API_KEY"}), patch.object(coding_agent,"build_opener") as opener, patch.object(coding_agent.time,"sleep"):
                opener.return_value.open.side_effect = [failure,opener.return_value.open.return_value]
                http = opener.return_value.open.return_value.__enter__.return_value
                http.read.return_value = json.dumps(response()).encode()
                http.getcode.return_value = 200
                result = coding_agent.generate(args(path),{"schema_version":1,"source":"PRIVATE_SOURCE"})
            self.assertEqual(result["usage"]["provider_request_count"],2)
            self.assertEqual(result["usage"]["model_calls"],1)
            self.assertEqual(result["usage"]["input_tokens"],18)
            self.assertEqual(result["usage"]["total_tokens"],23)
            self.assertIsNone(result["usage"]["billed_cost_usd"])
            captured = metering.read(path)
            self.assertEqual(result["usage_ledger"],captured["ledger"])
            for secret in ("PRIVATE_API_KEY","PRIVATE_ERROR_BODY","PRIVATE_SOURCE"):
                self.assertNotIn(secret,path.read_text())

    def test_timeout_failure_retains_unknown_usage_and_does_not_become_zero(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve()/"usage.json"
            selected = args(path)
            selected.retries = 0
            with patch.dict(coding_agent.os.environ,{"TEST_PROVIDER_KEY":"fixture"}), patch.object(coding_agent,"build_opener") as opener:
                opener.return_value.open.side_effect = URLError(TimeoutError("PRIVATE_TIMEOUT"))
                with self.assertRaises(URLError): coding_agent.generate(selected,{"schema_version":1})
            captured = metering.read(path)["ledger"]
            self.assertEqual(captured["invocation_status"],"failed")
            self.assertEqual(captured["events"][0]["status"],"timed_out")
            self.assertEqual(captured["summary"]["provider_request_count"],1)
            self.assertIsNone(captured["summary"]["billed_cost_usd"])
            self.assertIsNone(captured["summary"]["total_tokens"])
            self.assertNotIn("PRIVATE_TIMEOUT",path.read_text())

    def test_refusal_and_invalid_generated_json_keep_usage(self):
        for output, expected in (([{"content":[{"type":"refusal","refusal":"PRIVATE_REFUSAL"}]}],"refused"),
                                  ([{"content":[{"type":"output_text","text":"invalid JSON"}]}],"validation_failed")):
            with tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary).resolve()/"usage.json"
                raw = response() | {"output":output}
                with patch.dict(coding_agent.os.environ,{"TEST_PROVIDER_KEY":"fixture"}), patch.object(coding_agent,"build_opener") as opener:
                    opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(raw).encode()
                    with self.assertRaises(ValueError): coding_agent.generate(args(path),{"schema_version":1})
                captured = metering.read(path)["ledger"]
                self.assertEqual(captured["events"][0]["status"],expected)
                self.assertEqual(captured["summary"]["total_tokens"],14)
                self.assertIsNone(captured["summary"]["billed_cost_usd"])

    def test_generated_usage_claims_are_discarded_and_denied_egress_has_zero_calls(self):
        content = {"schema_version":1,"files":{"a.py":"pass"},"summary":"fixture",
                   "usage":{"billed_cost_usd":999,"input_tokens":99999},"usage_ledger":{"raw":"PRIVATE"}}
        decoded = coding_agent.decode("openai",response(content=content))
        self.assertEqual(decoded["usage"]["input_tokens"],11)
        self.assertIsNone(decoded["usage"]["billed_cost_usd"])
        self.assertNotIn("usage_ledger",decoded)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve()/"usage.json"
            selected = args(path)
            selected.allow_hosted = False
            with patch.object(coding_agent,"build_opener") as opener:
                with self.assertRaises(ValueError): coding_agent.generate(selected,{"schema_version":1})
                opener.assert_not_called()
            self.assertEqual(metering.read(path)["ledger"]["summary"]["provider_request_count"],0)

    def test_explicit_capture_rejects_overwrite_and_content_tampering(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve()/"usage.json"
            recorder = metering.Recorder(str(path))
            recorder.events.append(event())
            recorder.save("completed")
            before = path.read_bytes()
            with self.assertRaises(ValueError): metering.Recorder(str(path))
            self.assertEqual(path.read_bytes(),before)
            captured = metering.read(path)
            captured["ledger"]["events"][0]["elapsed_ms"] = 99
            with self.assertRaises(ValueError): metering.summary_from_record(captured)


if __name__ == "__main__": unittest.main()
