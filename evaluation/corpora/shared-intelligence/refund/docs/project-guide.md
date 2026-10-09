# Payment service: from purchase to resolution

The payment service coordinates purchase charges and subsequent refunds. Its
transport adapters distinguish a confirmed provider outcome from a connection
failure whose outcome is unknown. The service exists to make that distinction
explicit so a connection problem does not create duplicate financial actions.

A caller supplies a token before entering a transport adapter. For the current
payment provider, that token is an idempotency key: repeated payment requests
with the same key refer to the same charge. The legacy refund provider accepts
the token only as a correlation reference and does not deduplicate submissions.
Sharing a token-shaped parameter does not make the two interfaces equivalent.

The representative refund workflow enters `refund_or_reconcile` in
`src/refund_adapter.py`, submits one request, and returns a confirmed receipt.
If transport fails ambiguously, the adapter performs a read-only status lookup.
An unresolved outcome is handed to reconciliation with the original token. A
definitive decline returns the provider's failure without retry or lookup.

These are documented interface requirements. Reading the adapter or its tests
does not prove that the provider was contacted or that a refund ran successfully.
The accepted payment retry ADR and implementation history concern charge
attempts; they do not authorize repeated non-idempotent refund submissions.
