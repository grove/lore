# Legacy refund provider contract

The task completes `refund_or_reconcile(send, lookup, request_token)` in
`src/refund_adapter.py`. It returns the exact confirmed receipt object supplied
by the provider or status service, without copying or replacing that object.

| Operation | Outcome | Required behavior |
| --- | --- | --- |
| `send(request_token)` | Returns a receipt | Return it immediately; do not look up status. |
| `send(request_token)` | Raises `TransientError` | Perform exactly one `lookup(request_token)`. Do not submit again. |
| `lookup(request_token)` | Returns any value other than `None` | Return that confirmed receipt, including false-valued receipt objects. |
| `lookup(request_token)` | Returns `None` | Raise `ReconciliationRequired(request_token)`; the refund outcome is still unknown. |
| `send(request_token)` | Raises `RefundDeclined` or another exception | Propagate that exact exception immediately without lookup or retry. |
| `lookup(request_token)` | Raises an exception | Propagate that exact exception; no further lookup or submission. |

**The legacy refund provider does not support idempotency.** The token identifies
a reconciliation record; it cannot make a second submission safe. This is a
critical exception to the retry pattern used by the current payment provider.
Every call above receives the exact original token object.

An ambiguous response is not proof of failure. Reporting success without a
receipt, reporting final failure after an unknown outcome, and resubmitting the
refund are all incorrect. A status lookup that is unavailable is also not
evidence that no refund occurred.

The user-provided implementation task is deliberately bounded to this adapter.
No source document grants permission to run provider calls, change policy, or
perform an actual financial operation.
