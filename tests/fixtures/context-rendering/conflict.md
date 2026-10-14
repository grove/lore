## Proceed after check

**Preferred approach:** Resolve the conflicting production limits before changing queue admission. [ev_rule, ev_counterpart]

**Next action:** Compare both policy sources with their owner; establish which limit applies to production. [ev_rule, ev_counterpart]

### Constraints to preserve

- Retained documentation: Production queues hold at most 128 entries. [ev_rule] (constraint; accepted; scope: production)
  Staging alone may use 256 entries under the accepted experiment.
  Source documentation; not checkout verification.
  Check applicability: Keep the production bound; apply the exception only in staging. [ev_rule, ev_exception]
- Retained documentation: Production queues hold at most 64 entries. [ev_counterpart] (constraint; accepted; scope: production)
  Unresolved disagreement with the 128-entry policy; neither source supersedes the other.
  Check applicability: Check which production limit applies; retain both source claims until resolved. [ev_rule, ev_counterpart]

### Retained evidence, exceptions and alternatives

- Retained documentation: Staging may use 256 entries during the bounded experiment. [ev_exception] (decision; accepted; scope: staging)
  Expires when the experiment ends; production remains capped at 128.
- Retained documentation: Unbounded production queues were rejected because they caused latency spikes. [ev_rejected] (decision; rejected; scope: production)
  Historical rationale, not an approved alternative.

**Local checks:** inspection disabled; 0 static observations. Tests and runtime checks: not run.

### Counterevidence

- Initial hypothesis: The production limit is unambiguously 128 entries.
  A second current accepted policy says 64 entries for production; resolve both sources before choosing a limit. [ev_rule, ev_counterpart] This changes the preferred action.

**Why:** Both production policies claim current applicability; the staging exception supplies no supersession. [ev_rule, ev_counterpart]

**Main trade-off:** Keeping per-key order limits parallelism for a single key. [ev_rule]

### Checks still to run

- **Required before proceeding:** Compare both policy sources with their owner; establish which limit applies to production. [ev_rule, ev_counterpart]
  Decision impact: This check determines whether an implementation choice is justified.

### Remaining uncertainty

- The production limit is unresolved; the staging exception does not resolve this conflict. [ev_rule, ev_counterpart, ev_exception]

### Completion criteria (not yet verified)

- Repeated-key tests preserve ordering and reject production queue overflow. [ev_rule]

Advice is not approval. Static inspection does not prove runtime behavior.

Inspection: disabled. Investigation: not_requested. Cache: miss. Model calls: 2 / 6. Output: 2200 / 2600 tokens.

[ev\_rule] docs/queue.md — source-revision (documentary, current: true)

[ev\_exception] docs/staging.md — source-revision (documentary, current: true)

[ev\_rejected] docs/history.md — source-revision (documentary, current: false)

[ev\_counterpart] docs/other-policy.md — source-revision (documentary, current: true)

Shared project snapshot: `registry-revision`

Inspection: not_granted. Execution: unavailable.

Provider requests: 2. Provider tokens: unknown. Billed USD: unknown.
