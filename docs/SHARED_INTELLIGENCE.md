# Adaptive shared project intelligence

The default initialized-project context uses schema 5 and reuses Lore's evidence registry, lexical and
semantic retrieval, decision validation, static checkout inspector and bounded
investigation controller. It adds automatic initiative within caller grants and
a source-selection-independent registry revision. Schemas 2, 3 and 4 keep their
existing explicitly selected command behavior. Pin schema 5 in agent integrations
to keep the contract stable; ordinary human requests need no schema selection.

```bash
lore --json context "Refactor retries without changing behavior"
lore --json context "Refactor retries without changing behavior" --inspect
lore --json context "Refactor retries without changing behavior" --no-inspect
lore --json context "Refactor retries without changing behavior" --schema-version 5
```

## Standing grants

The new experience treats repository configuration as a requested scope. The
invoking host supplies capability grants. To permit automatic read-only work on
one checkout for subsequent commands:

```bash
export LORE_INSPECTION_ROOT="$PWD"
lore context "Refactor retries without changing behavior"
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
compatibility when explicitly pinned. The default does not create new grants:
an unavailable capability leaves the best permitted evidence and honest fallback
usable. It does not become an invented policy or technical blocker.

## Schema 5

| Field | Meaning |
| --- | --- |
| `schema_version` | `5`, the initialized-project default or an explicit schema pin |
| `snapshot.project_id` | Resolved project identity |
| `snapshot.registry_revision` | Digest of the retained source heads, knowledge, imported observations and relationships in the read transaction |
| `capabilities` | Caller-granted inspection scope status, hosted egress permissions, and explicit absence of execution/source-write capabilities |
| `intelligence` | The established schema-4 decision or honest deterministic fallback, including constraints, evidence, observations, investigation trace and completion checks |
| `source_relationships` | When present, complete source-owned cross-source relationship groups, their endpoint revisions, original evidence and review qualifications |
| `budget` | Token bound for the complete envelope in both JSON and Markdown |

The nested decision contract keeps readiness, preferred action, facts,
hypotheses, general engineering judgment, scoped blockers, implementation seams,
counterevidence and future completion criteria distinct. Reading an existing
test declaration does not establish that any test was run or passed. Lack of a
capability is reported separately from a real authority blocker.

Markdown starts with the preferred approach, next action and complete critical
constraints before the supporting rationale and detail. JSON retains the full
decision structure, evidence, provenance, omissions and source relationships.
Both presentations are measured within the same requested token bound. Explicit
schema 4 retains its existing rendering and response shape.

Before compilation, an unpinned context request can instead return the distinct
`lore.bootstrap_context` schema-1 `bootstrap_source_only` contract. Its ephemeral
source digest is not a registry revision. Explicit schema 3/4/5 and `--fast`
return `uninitialized_project` until a registry exists; they never silently
switch to the bootstrap shape.

An exact retained symbolic query such as `What is QUEUE_CAPACITY?` uses
`intelligence.mode: reference` when current selected evidence contains the
symbol and has no unresolved selected disagreement. That path makes no model
calls, opens no checkout files and touches no cache. Broader questions about
changing a constant continue through the decision engine.

The registry revision describes **retained evidence**. It is not a promise that
every file in the live checkout has been scanned. Run `lore update` to reconcile
documentary changes. Granted code observations are bound to full-file hashes
and revalidated before fresh or cached recommendations are returned.

### Source-owned relationship groups

Optional decision prose can be shortened to fit a request. A source-reported
relationship's reason, status and caveats must remain available even when its
endpoints also appear in a shorter decision brief. Schema 5 therefore retains
an endpoint-only `source_relationships` manifest from the exact context
selection used by the decision engine. It is omitted when no cross-source
relationship group was selected. It introduces no additional registry or
mutable source of truth, and does not change the schema-2/3/4 response shapes.

| Manifest field | Retained source data |
| --- | --- |
| `relations` | Original `CrossSourceRelation` objects: IDs, reasons, qualifications, endpoint kinds and immutable revisions, evidence IDs, upstream vocabulary/status, current interpretation membership and review IDs |
| `discrepancies` | The selected verification question and its review disposition, including any qualification added when a review was resolved or dismissed |
| `knowledge` | Complete selected documentary endpoint statements, scope, lifecycle, support status, qualifications and evidence references |
| `knowledge_revisions` | Documentary endpoint ID to knowledge-revision ID; these are distinct from source-capture revisions |
| `observations` | Selected native endpoint records, including source-reported scope, lifecycle, freshness, verification metadata and qualifications |
| `evidence`, `imported_evidence` | The original context evidence types for those groups, including immutable source/native snapshot identities, source metadata and complete endpoint evidence references |

`relations[].active` means that an interpretation belongs to the current
retained projection. `upstream_status` and `upstream_active` retain the native
source's independent meaning. A closed work item, a withdrawn upstream link,
or a dismissed review does not establish that production behavior was checked
or that an accepted policy was replaced. These distinctions remain visible in
both JSON and Markdown.

The manifest is validated against current retained relationship payloads,
immutable endpoint revisions and evidence snapshots. Every relationship
endpoint and evidence reference must resolve within its retained group. A
human presentation rechecks the same manifest, includes it unchanged, and
attaches its source qualifications to claims about the related endpoints.
Manifest evidence remains usable even when optional schema-4 facts were
removed during packing. The human adapter rejects altered reasons, source
status, review qualifications, revision bindings or missing group members
before using those records in presentation inference.

The adapter also checks current registry relationships touching the source
records retained by its presentation, so deleting the whole manifest while
keeping those premises is rejected. This completeness check does not
reconstruct candidates that a supplied answer removed entirely. If checkout
revalidation requires fresh documentary selection, its source relationship
manifest is refreshed from that same selection before presentation resumes.

After retrieval, schema 5 reserves the manifest's complete JSON/Markdown space
before decision synthesis. It then checks both complete final output formats,
including the manifest, nested intelligence and token-accounting metadata.
Once selected, a group is not shortened by removing an exception, endpoint or
review caveat. If the requested budget cannot hold that complete group and the
response envelope, the request returns a budget error asking for more space.
Selection may still omit an entire group under the established retrieval
budget; those omissions remain reported by the nested context contract.

The manifest is bounded to 128 relationships, 256 endpoint records, 1,024
evidence entries and 512 KiB of serialized source data. Native evidence keeps
the existing compact context representation; use `lore evidence <id>` for its
complete immutable imported record. No source report becomes independent
implementation verification merely by being retained here.

These limits bound the retained response. A nonempty manifest still validates
against registry-wide documentary records, relationship facts and reviews;
it does not claim that source validation work is proportional only to the
selected groups. An empty manifest skips those additional validation reads.

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
