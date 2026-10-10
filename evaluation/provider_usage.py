"""Provider-neutral attempt ledger validation and accounting; no price inference.

Only metadata named by this contract can be retained. Provider bodies, prompts,
URLs and credentials are never accepted as usage evidence.
"""
from __future__ import annotations

import hashlib
import json
import math
import os
from pathlib import Path
import struct
import tempfile

COUNTS = ("model_calls", "provider_request_count", "input_tokens", "output_tokens", "total_tokens", "cache_hits", "event_count")
STATUSES = {"started", "completed", "http_error", "transport_error", "timed_out", "refused", "incomplete",
            "validation_failed", "cancelled", "cached", "rejected"}
EVENT_KEYS = {"id", "call_id", "operation", "provider", "model", "attempt", "elapsed_ms", "http_status",
              "provider_request_count", "input_tokens", "output_tokens", "total_tokens", "billed_cost_usd",
              "billing_source", "status", "cache_hit"}


def integer(value):
    return value if type(value) is int and 0 <= value <= 2**64 - 1 else None


def tokens(provider: str, raw: dict, *, embedding=False) -> dict:
    """Read documented units only. Never use raw generated text as accounting."""
    raw = raw if isinstance(raw, dict) else {}
    if provider == "openai":
        used = raw.get("usage")
        used = used if isinstance(used, dict) else {}
        input_tokens = integer(used.get("prompt_tokens" if embedding else "input_tokens"))
        output_tokens = 0 if embedding and used else integer(used.get("output_tokens"))
        explicit_total = used.get("total_tokens") is not None
        total_tokens = integer(used.get("total_tokens"))
    elif provider == "ollama":
        input_tokens = integer(raw.get("prompt_eval_count"))
        output_tokens = 0 if embedding and "prompt_eval_count" in raw else integer(raw.get("eval_count"))
        explicit_total, total_tokens = False, None
    else:
        input_tokens = output_tokens = total_tokens = None
        explicit_total = False
    if input_tokens is not None and output_tokens is not None:
        derived = integer(input_tokens + output_tokens)
        if not explicit_total:
            total_tokens = derived
        elif total_tokens != derived:
            total_tokens = None
    return {"provider_request_count": 1, "input_tokens": input_tokens, "output_tokens": output_tokens,
            "total_tokens": total_tokens, "billed_cost_usd": None, "billing_source": None}


def normalized(value: dict | None, *, calls: int | None = None) -> dict:
    value = value if isinstance(value, dict) else {}
    result = {key: value.get(key) for key in (*COUNTS, "billed_cost_usd", "billing_source")}
    if calls is not None and result["model_calls"] is None:
        result["model_calls"] = calls
    for key in COUNTS:
        number = result[key]
        if number is not None and integer(number) is None:
            raise ValueError(f"Invalid measured {key}")
    cost = result["billed_cost_usd"]
    source = result["billing_source"]
    if cost is not None and (type(cost) not in (int, float) or not math.isfinite(cost) or cost < 0
                            or not isinstance(source, str) or not source.strip() or len(source) > 128
                            or any(ord(character) < 32 for character in source)):
        raise ValueError("Measured billing requires a finite nonnegative amount and billing_source")
    if cost is None:
        result["billing_source"] = None
    if all(result[key] is not None for key in ("input_tokens", "output_tokens", "total_tokens")):
        if result["input_tokens"] + result["output_tokens"] != result["total_tokens"]:
            raise ValueError("Measured total_tokens contradicts input and output tokens")
    return result


def add(values: list[dict]) -> dict:
    """One missing component keeps the aggregate unknown."""
    result = {key: sum(value[key] for value in values) if all(value.get(key) is not None for value in values) else None
              for key in (*COUNTS, "billed_cost_usd")}
    for key in COUNTS:
        result[key] = integer(result[key])
    if result["billed_cost_usd"] is not None and not math.isfinite(result["billed_cost_usd"]):
        result["billed_cost_usd"] = None
    result["billing_source"] = ("sum_of_explicit_event_billing" if result["provider_request_count"] != 0
                                 else "no_provider_requests") if result["billed_cost_usd"] is not None else None
    return result


def no_inference() -> dict:
    return normalized({**dict.fromkeys(COUNTS, 0), "billed_cost_usd": 0.0,
                       "billing_source": "no_provider_requests"})


def aggregate(events: list[dict]) -> dict:
    if not isinstance(events, list) or len(events) > 100_000:
        raise ValueError("Provider usage events must be a bounded list")
    seen, calls, measured = set(), set(), []
    for event in events:
        if not isinstance(event, dict) or set(event) != EVENT_KEYS:
            raise ValueError("Usage event contains missing, unexpected, or sensitive fields")
        for field in ("id", "call_id", "operation", "provider", "model"):
            value = event[field]
            if not isinstance(value, str) or not value or len(value) > 512 or any(ord(character) < 32 for character in value):
                raise ValueError("Invalid provider usage identity")
        if event["id"] in seen:
            raise ValueError("A provider usage event is charged twice")
        seen.add(event["id"])
        if event["status"] not in STATUSES or type(event["cache_hit"]) is not bool:
            raise ValueError("Invalid provider usage outcome")
        if integer(event["attempt"]) is None or integer(event["elapsed_ms"]) is None:
            raise ValueError("Invalid provider attempt or elapsed time")
        if event["http_status"] is not None and (type(event["http_status"]) is not int or not 100 <= event["http_status"] <= 599):
            raise ValueError("Invalid provider HTTP status")
        row = normalized(event)
        if event["cache_hit"] and (event["status"] != "cached" or any(row[key] != 0 for key in
                ("provider_request_count", "input_tokens", "output_tokens", "total_tokens", "billed_cost_usd"))):
            raise ValueError("Cached usage cannot contain historical provider charges")
        if not event["cache_hit"]:
            calls.add(event["call_id"])
        row.update(model_calls=0, event_count=1, cache_hits=int(event["cache_hit"]))
        measured.append(row)
    total = add(measured)
    total["model_calls"] = len(calls)
    return {"schema_version": 1, **total}


def validate_ledger(value: dict) -> dict:
    if not isinstance(value, dict) or set(value) != {"contract", "schema_version", "invocation_status", "summary", "events"}:
        raise ValueError("Invalid provider usage ledger envelope")
    if value["contract"] != "lore.provider_usage" or value["schema_version"] != 1:
        raise ValueError("Unsupported provider usage ledger")
    if value["invocation_status"] not in ("running", "completed", "failed", "cancelled"):
        raise ValueError("Invalid usage invocation status")
    if value["summary"] != aggregate(value["events"]):
        raise ValueError("Provider usage aggregate does not equal its attempt ledger")
    return value


def ledger(events: list[dict], status: str) -> dict:
    return validate_ledger({"contract": "lore.provider_usage", "schema_version": 1,
                            "invocation_status": status, "summary": aggregate(events), "events": events})


def read(path: Path) -> dict | None:
    """Read only the exact output path selected by the caller, never a response URL."""
    for component in (path, *path.parents):
        if component.is_symlink():
            raise ValueError("Usage ledger path may not contain symlinks")
    if not path.exists():
        return None
    if not path.is_file() or path.stat().st_size > 16_000_000:
        raise ValueError("Usage ledger is not a bounded regular file")
    raw = path.read_bytes()
    value = validate_ledger(json.loads(raw))
    return {"sha256": hashlib.sha256(raw).hexdigest(), "content_sha256": digest(value), "ledger": value}


def summary_from_record(record: dict | None) -> dict | None:
    if record is None:
        return None
    validate_record(record)
    return normalized(validate_ledger(record["ledger"])["summary"])


def validate_record(record: dict | None) -> bool:
    if record is None:
        return False
    validate_ledger(record["ledger"])
    if not isinstance(record.get("sha256"), str) or len(record["sha256"]) != 64:
        raise ValueError("Usage capture lacks its exact original-byte hash")
    if record.get("content_sha256") != digest(record["ledger"]):
        raise ValueError("Captured provider usage content changed")
    return True


def events_digest(events: list[dict]) -> str:
    """Match Rust's UTF-8 canonical hash, including exact float billing bits."""
    canonical = [{**event, "billed_cost_usd": struct.pack("!d", event["billed_cost_usd"]).hex()
                  if event["billed_cost_usd"] is not None else None} for event in events]
    return hashlib.sha256(json.dumps(canonical, sort_keys=True, separators=(",", ":"),
                                    ensure_ascii=False).encode()).hexdigest()


def bind_summary(summary: dict | None, record: dict | None) -> dict | None:
    """Verify a public summary against the captured event prefix it names.

    The caller chooses and captures the file. A response's path is never opened.
    The returned accounting covers the entire invocation, including later work.
    """
    measured = summary_from_record(record)
    if measured is None or summary is None:
        return measured
    reference = summary.get("ledger")
    events = record["ledger"]["events"]
    if reference is not None:
        if not isinstance(reference, dict) or set(reference) != {"path", "events_sha256", "event_count"}:
            raise ValueError("Invalid usage ledger reference")
        count = integer(reference["event_count"])
        if count is None or count > len(events) or not isinstance(reference["path"], str):
            raise ValueError("Invalid usage ledger event prefix")
        events = events[:count]
        if reference["events_sha256"] != events_digest(events):
            raise ValueError("Usage summary names a different event ledger")
    if normalized(summary) != normalized(aggregate(events)):
        raise ValueError("Usage summary does not equal its saved attempt events")
    return measured


def digest(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


class Recorder:
    """Private explicit output; started attempts survive timeout/process errors."""
    def __init__(self, path: str | None = None):
        self.path = Path(path) if path else None
        self.events = []
        self.claimed = False
        if self.path is not None:
            if not self.path.is_absolute() or not self.path.parent.is_dir() or self.path.exists():
                raise ValueError("Usage output must be a new absolute file in an existing directory")
            for component in (self.path, *self.path.parents):
                if component.is_symlink():
                    raise ValueError("Usage output may not contain symlinks")

    def save(self, status="running") -> dict:
        value = ledger(self.events, status)
        if self.path is not None:
            for component in (self.path, *self.path.parents):
                if component.is_symlink():
                    raise ValueError("Usage output may not contain symlinks")
            if not self.claimed:
                descriptor = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                os.close(descriptor)
                self.claimed = True
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=self.path.parent, delete=False) as handle:
                temporary = Path(handle.name)
                json.dump(value, handle, separators=(",", ":"))
            try:
                os.replace(temporary, self.path)
            finally:
                temporary.unlink(missing_ok=True)
        return value
