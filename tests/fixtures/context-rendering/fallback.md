## Proceed after check

**Preferred approach:** Check compatibility with the retained production bound before choosing an implementation. [ev_rule]

**Next action:** Determine whether the queue refactor preserves the production bound and the staging-only exception. [ev_rule, ev_exception]

This is deterministic fallback guidance; readiness is unassessed. Only the stated check is justified before choosing an implementation.

### Constraints to preserve

- Retained documentation: Production queues hold at most 128 entries. [ev_rule] (constraint; accepted; scope: production)
  Staging alone may use 256 entries under the accepted experiment.
  Source documentation; not checkout verification.
  Proposed preservation: Keep the production bound; apply the exception only in staging. [ev_rule, ev_exception]

### Retained evidence, exceptions and alternatives

- Retained documentation: Staging may use 256 entries during the bounded experiment. [ev_exception] (decision; accepted; scope: staging)
  Expires when the experiment ends; production remains capped at 128.
- Retained documentation: Unbounded production queues were rejected because they caused latency spikes. [ev_rejected] (decision; rejected; scope: production)
  Historical rationale, not an approved alternative.

**Local checks:** inspection disabled; 0 static observations. Tests and runtime checks: not run.

**Why:** The accepted bound protects production latency; the staging exception does not extend to production. [ev_rule]

**Main trade-off:** Keeping per-key order limits parallelism for a single key. [ev_rule]

### Checks still to run

- **Required before proceeding:** Determine whether the queue refactor preserves the production bound and the staging-only exception. [ev_rule, ev_exception]
  Decision impact: This check determines whether an implementation choice is justified.

### Completion criteria (not yet verified)

- Repeated-key tests preserve ordering and reject production queue overflow. [ev_rule]

Advice is not approval. Static inspection does not prove runtime behavior.

Fallback: The configured provider is unavailable; retained evidence remains usable without fresh reasoning.

Inspection: disabled. Investigation: inference_unavailable. Cache: unused. Model calls: 0 / 6. Output: 2200 / 2600 tokens.

[ev\_rule] docs/queue.md — source-revision (documentary, current: true)

[ev\_exception] docs/staging.md — source-revision (documentary, current: true)

[ev\_rejected] docs/history.md — source-revision (documentary, current: false)

Shared project snapshot: `registry-revision`

Inspection: not_granted. Execution: unavailable.

Provider requests: 0. Provider tokens: 0. Billed USD: $0.
