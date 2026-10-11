# Charge and refund incident runbook

**Status: Active procedure.** Applies to the fictional HarborPay service.

## Charge processing

1. Identify the logical charge and its stable idempotency key.
2. If a response is a confirmed transient failure, retry the same logical charge only within the accepted **three-attempt** limit from ADR-001.
3. If a response is ambiguous or times out after a possible submission, **stop and perform read-only reconciliation first**. Do not retry while the outcome remains unknown.
4. Record the observation and escalate any exception requiring an explicit policy decision.

## Legacy refunds

The legacy refund provider **does not guarantee idempotent writes**. A timeout or unknown result must trigger **read-only reconciliation**, not a second refund submission. A confirmed failure still requires an authorized next-step decision; it is not automatic permission to resubmit.

These operational instructions are documentary intent. They do not prove that a current checkout, production gateway or deployment obeys them.

For the historical motivation, see [INC-014](INC-014.md).
