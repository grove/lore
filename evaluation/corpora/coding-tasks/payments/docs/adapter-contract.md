# Payment adapter contract

The payment adapter calls `send(idempotency_token)`. Every attempt for one
payment must reuse the exact token supplied by the caller. Generating a token
per attempt can charge the same purchase more than once.

`TransientError` identifies a transport failure whose result is unknown.
`PaymentDeclined` and other exceptions are definitive failures and must
propagate without retry. A successful send returns its result immediately.

The deployed adapter currently makes one initial attempt plus at most five
retries. The small adapter-completion task preserves this existing behavior;
it does not authorize a further increase or claim that the ADR was replaced.
The three-versus-five discrepancy in the retained policy/history needs an
explicit follow-up. On exhaustion, propagate the last transport exception.
