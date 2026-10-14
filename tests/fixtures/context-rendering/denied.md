## Proceed after check

**Preferred approach:** Extract queue allocation while preserving per-key order and the 128-entry production bound. [ev_rule]

**Next action:** Check the queue admission path with an authorized local reader before extracting allocation. [ev_rule]

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

- **Required before proceeding:** Check the queue admission path with an authorized local reader before extracting allocation. [ev_rule]
  Decision impact: This check determines whether an implementation choice is justified.

### Remaining uncertainty

- Inspection was denied by the caller. Retained policy remains usable; current implementation compatibility is unverified. [ev_rule]

### Completion criteria (not yet verified)

- Repeated-key tests preserve ordering and reject production queue overflow. [ev_rule]

Advice is not approval. Static inspection does not prove runtime behavior.

Inspection: disabled. Investigation: not_requested. Cache: miss. Model calls: 2 / 6. Output: 2200 / 2600 tokens.

[ev\_rule] docs/queue.md — source-revision (documentary, current: true)

[ev\_exception] docs/staging.md — source-revision (documentary, current: true)

[ev\_rejected] docs/history.md — source-revision (documentary, current: false)

Shared project snapshot: `registry-revision`

Inspection: disabled_by_caller. Execution: unavailable.

Provider requests: 2. Provider tokens: unknown. Billed USD: unknown.
