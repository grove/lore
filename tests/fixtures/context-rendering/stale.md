## Proceed after check

**Preferred approach:** Preserve the accepted production bound, after checking the current admission path. [ev_rule, ev_report]

**Next action:** Inspect current src/queue.rs; reconcile the stale implementation report before extracting allocation. [ev_rule, ev_report]

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
- Imported report (not independent verification): The queue implementation enforces a 128-entry production bound. [ev_report] (implementation; reported; scope: production)
  Imported OpenWiki report; the current checkout has changed since its snapshot.

**Local checks:** inspection complete; 0 static observations. Tests and runtime checks: not run.

OpenWiki freshness [ev\_report]: repo://src/queue.rs — changed_since_snapshot (content_changed)
Checked path: src/queue.rs; digest: sha256:current-digest.
Checked bytes differ from the imported snapshot. Inspect current source before relying on the report; no tests were run.

**Why:** The accepted bound protects production latency; the staging exception does not extend to production. [ev_rule]

**Main trade-off:** Keeping per-key order limits parallelism for a single key. [ev_rule]

### Hypotheses

- Hypothesis (Low confidence): The current admission path may still enforce the documented bound. [ev_rule]
  Applies when: Only if current-source inspection confirms the admission check.
  Alternative: The implementation may have drifted from the accepted rule. [ev_rule]

### Checks still to run

- **Required before proceeding:** Inspect current src/queue.rs; reconcile the stale implementation report before extracting allocation. [ev_rule, ev_report]
  Decision impact: This check determines whether an implementation choice is justified.

### Completion criteria (not yet verified)

- Repeated-key tests preserve ordering and reject production queue overflow. [ev_rule]

Advice is not approval. Static inspection does not prove runtime behavior.

Inspection: complete. Investigation: not_requested. Cache: miss. Model calls: 2 / 6. Output: 2200 / 2600 tokens.

[ev\_rule] docs/queue.md — source-revision (documentary, current: true)

[ev\_exception] docs/staging.md — source-revision (documentary, current: true)

[ev\_rejected] docs/history.md — source-revision (documentary, current: false)

[ev\_report] openwiki/queue.md — source-revision (imported\_report, current: true)

Shared project snapshot: `registry-revision`

Inspection: granted. Execution: unavailable.

Provider requests: 2. Provider tokens: unknown. Billed USD: unknown.
