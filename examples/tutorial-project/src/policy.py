"""Tiny, non-networked HarborPay policy demonstration.

This program is *sample implementation code*, not evidence of deployment.
The separately runnable tests demonstrate only the sample's local behavior.
"""

MAX_CHARGE_ATTEMPTS = 3


def charge_attempt_keys(key: str, attempts: int) -> tuple[str, ...]:
    """Return the idempotency key used for each planned charge attempt."""
    if not isinstance(key, str) or not key.strip():
        raise ValueError("a non-empty idempotency key is required")
    if isinstance(attempts, bool) or not isinstance(attempts, int):
        raise ValueError("attempts must be an integer")
    if not 1 <= attempts <= MAX_CHARGE_ATTEMPTS:
        raise ValueError("attempts must stay within the accepted limit")
    return (key,) * attempts


def refund_next_step(outcome: str) -> str:
    """Select advice only; never submit a refund or query a provider."""
    if outcome == "unknown":
        return "read_only_reconciliation"
    if outcome == "confirmed_failure":
        return "request_authorized_decision"
    if outcome == "success":
        return "done"
    raise ValueError("unsupported provider outcome")
