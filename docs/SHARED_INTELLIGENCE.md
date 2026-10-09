# Adaptive shared project intelligence

The explicit schema-5 experience reuses Lore's evidence registry, lexical and
semantic retrieval, decision validation, static checkout inspector and bounded
investigation controller. It adds automatic initiative within caller grants and
a source-selection-independent registry revision. Schemas 2, 3 and 4 keep their
existing command behavior; schema 4 remains the default.

```bash
lore --json context "Refactor retries without changing behavior" --schema-version 5
lore --json context "Refactor retries without changing behavior" --schema-version 5 --inspect
lore --json context "Refactor retries without changing behavior" --schema-version 5 --no-inspect
```

## Standing grants

The new experience treats repository configuration as a requested scope. The
invoking host supplies capability grants. To permit automatic read-only work on
one checkout for subsequent commands:

```bash
export LORE_INSPECTION_ROOT="$PWD"
lore context "Refactor retries without changing behavior" --schema-version 5
```

`context.inspection.root` can narrow that root. It cannot select a parent,
sibling or unrelated directory. Alternatively `--inspect` or `--investigate`
grants the configuration directory for the current invocation. `--no-inspect`
always denies inspection. Symlinks, exclusions, secret filtering, size limits,
exact candidate selection and source-file revalidation remain enforced by the
existing inspector. Discovery failure still leaves retained knowledge usable.

Hosted inference also needs a caller grant in the new experience. Set
`LORE_ALLOW_HOSTED_EGRESS=1` in the invoking process **and** explicitly configure
`privacy.local_only: false` when hosted documentary inference is desired.
Hosted checkout content additionally needs `LORE_ALLOW_CHECKOUT_EGRESS=1` and
`privacy.allow_checkout_egress: true`, or the explicit per-invocation
`--allow-checkout-egress` flag. A local-only configuration remains a ceiling even
with that flag. A local non-cloud Ollama model retains local inference access.
No source file, model-generated plan or repository setting can grant host
execution, network transmission or a wider checkout root by itself.

Embedders can construct `adaptive::HostGrants` and apply `adaptive::authorize`
without reading environment variables. No command executes repository code or
changes accepted policy. Existing schema-3/4 grant behavior is preserved for
compatibility; migration to the new contract is explicit.

## Schema 5

| Field | Meaning |
| --- | --- |
| `schema_version` | `5`, explicitly selected by the caller |
| `snapshot.project_id` | Resolved project identity |
| `snapshot.registry_revision` | Digest of the retained source heads, knowledge, imported observations and relationships in the read transaction |
| `capabilities` | Caller-granted inspection scope status, hosted egress permissions, and explicit absence of execution/source-write capabilities |
| `intelligence` | The established schema-4 decision or honest deterministic fallback, including constraints, evidence, observations, investigation trace and completion checks |
| `budget` | Token bound for the complete envelope in both JSON and Markdown |

The nested decision contract keeps readiness, preferred action, facts,
hypotheses, general engineering judgment, scoped blockers, implementation seams,
counterevidence and future completion criteria distinct. Reading an existing
test declaration does not establish that any test was run or passed. Lack of a
capability is reported separately from a real authority blocker.

An exact retained symbolic query such as `What is QUEUE_CAPACITY?` uses
`intelligence.mode: reference` when current selected evidence contains the
symbol and has no unresolved selected disagreement. That path makes no model
calls, opens no checkout files and touches no cache. Broader questions about
changing a constant continue through the decision engine.

The registry revision describes **retained evidence**. It is not a promise that
every file in the live checkout has been scanned. Run `lore update` to reconcile
documentary changes. Granted code observations are bound to full-file hashes
and revalidated before fresh or cached recommendations are returned.

## Reusing investigative experience

The existing decision cache now includes its question, creation time, full
registry revision and permission identity, alongside the original hypotheses,
counterevidence, inspected files, prior/revised recommendation and support
review. Reuse never promotes a derived explanation into an accepted decision.

A new retained ADR invalidates an earlier finding even if its old dependencies
are unchanged. Code dependencies are all rehashed, including deeper inspection
rounds. An incomplete candidate inventory cannot justify reuse. Cache entries
from older formats, changed permissions, stale sources or corrupt writes are
ignored and recomputed as permitted.

```bash
lore memory
lore --json memory
lore memory --clear
```

Listing exposes current-policy **leads**, not revalidated advice; a new task
request performs the full dependency checks. Findings are bounded to 64 entries
of at most 1 MB each with a 30-day maximum reuse age. Writes are atomic and
expired/oversized entries are evicted before new persistence. `--no-cache`
bypasses cache reads, writes and housekeeping. `lore memory --clear` removes
decision findings; `lore purge --all --yes` removes all managed state and output.

## Evaluation status

See [the execution tracker](IMPLEMENTATION_TRACKER.md). Synthetic contract tests
exercise grants, mutation, exact provenance, budgets and source preservation.
They do not establish improved onboarding or coding-agent productivity. The two
product-outcome tracks are evaluated separately on independently checked tasks.
