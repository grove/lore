# Project baselines, changes, and advisory review

The project companion compares an explicit saved checkpoint with Lore's current
retained knowledge. `lore changes` shows consequential documentary and imported
changes with their original evidence. `lore guard` uses the same comparison to
request bounded task guidance from the shared intelligence core. Neither command
executes project code or changes a source document.

## A practical workflow

Compile the configured sources, then save a checkpoint with a meaningful name:

```sh
lore update
lore baseline save joined
```

After sources have changed, compile them again and compare the retained results:

```sh
lore update
lore changes --since joined
lore changes --since joined --task "Change queue admission diagnostics" --json
lore guard --since joined --task "Change queue admission diagnostics" --no-inspect
```

`baseline`, `changes`, and `guard` do not run `update` automatically. A source edit
that has not been compiled is outside the documentary comparison. In particular,
an unchanged registry says nothing about uninspected edits in a live checkout.
The explicit `update` steps above use the project's existing update and privacy
configuration; the comparison commands do not expand that configuration.

Save a new name to retain an additional checkpoint, or explicitly replace one:

```sh
lore baseline save before-queue-refactor
lore baseline save joined --replace
lore baseline remove before-queue-refactor
```

Replacement and removal affect only the named local checkpoint. They do not
delete original sources, accepted knowledge, or immutable registry history.

## Command reference

`--json` is a global flag and may be used with each command below.

| Command | Options and defaults |
| --- | --- |
| `lore baseline save NAME` | `--replace` explicitly overwrites an existing name. |
| `lore baseline remove NAME` | Removes the named checkpoint. |
| `lore changes --since NAME` | Optional `--task TASK`; `--max-tokens` defaults to 8000 and accepts 512 through 100000. |
| `lore guard --since NAME` | Optional `--task TASK`, `--inspect`, `--no-inspect`, `--no-cache`, and `--allow-checkout-egress`; `--max-tokens` defaults to 8000 and accepts 1024 through 100000. |

Names contain 1 through 64 ASCII letters, digits, hyphens, or underscores. Paths
and empty names are rejected. `--inspect` conflicts with `--no-inspect`.
`--allow-checkout-egress` also conflicts with `--no-inspect`.

Without `--task`, the comparison covers the retained project records within the
documented bounds. A guard with changes derives a query from at most six complete
topic or native subject labels. Oversized labels and additional topics are
reported as omitted from that advisory query. Supply a specific task to review
another scope. An explicit task requests shared guidance even when the included
comparison has no consequential changes.

Accepted decisions and constraints take precedence over proposals and historical
context when the automatic query reaches its six-topic limit. A change to an
accepted rule's scope, lifecycle, or current support receives the highest
priority. Labels break ties deterministically. This is a bounded selection rule;
it does not estimate a project's risk or establish that excluded topics are safe.

## What counts as a change

Documentary comparisons preserve the complete before and after states: record
identity, sealed revision, statement, kind, effective lifecycle, original
lifecycle, support state, topic, subject, scope, recorded effective time, and the
evidence belonging to that exact revision. They report added, removed,
`lifecycle_changed`, and `understanding_changed` records.

The comparison also includes exact supporting source quotations and their
primary or derived provenance in the meaning comparison. A changed rare
exception is therefore visible even when extraction retained an unchanged short
statement. A new source capture or a cosmetic edit outside the retained quotation
does not by itself become a consequential policy alert. The broader
`registry_changed` flag can still be true for such a capture.

New checkpoints additionally record `current_evidence_ids`, the exact subset of
support current when that checkpoint was saved. The existing `evidence_ids`
continues to contain all support belonging to the immutable knowledge revision,
including historical quotations. Semantic comparison uses each checkpoint's
current supporting quotations when both checkpoints captured this membership.
This makes an exception that changes from A to B and then returns to A visible,
even if the extracted short statement stayed identical and both quotations
remain in history. A change of citation location with an unchanged quotation
alone does not create a material policy change.

Older schema-1 checkpoint files omit `current_evidence_ids`. They remain readable
with their original checksum; their comparisons use the previous complete-support
semantics because historical current membership cannot be reconstructed reliably.
Historical relationship endpoints can likewise omit this field. Saving a new
checkpoint captures current membership for future comparisons. The reader rejects
duplicated, unsorted, or unbound current evidence IDs; it does not remove original
support to make a checkpoint fit.

Documented supersession, reaffirmation, and other registry relationships retain
their original witnesses and endpoints. Related original records and evidence
are included with a retained relationship change. A newer date, an issue closure,
or an imported statement does not independently supersede an accepted decision.

Each documentary change contains a short `implication` whose
`implication_basis` is `inferred_from_documented_change`. That sentence explains
how the retained change can affect subsequent advice. It is an inference, not a
new source assertion or a report of deployed behavior.

### Imported work and implementation reports

Imported observations are compared by their source-owned identity, immutable
snapshot, evidence ID, and current membership at each checkpoint. The response
keeps the complete original imported payload, lifecycle, verification field,
scope, and source provenance for included changes.

For example, a closed work item describing a staging-only replay remains a
source report about that staging replay. It does not establish that production
behavior was independently checked, that Lore ran a test, or that accepted policy
was replaced. The full source qualification remains present in JSON and Markdown.

### Cross-source interpretation history

The baseline also captures the current source-owned cross-source interpretation
IDs and immutable content hashes. A change in that set produces
`cross_source_relationships_changed: true`, including when both endpoint records
remain unchanged. The signal is project-wide even when `--task` filters the
included change groups.

`cross_source_changes` pairs the baseline and current interpretation using the
stable comparison key stored in `cross_source_evaluations`. It does not infer
replacement from similar prose, a newer timestamp, or matching display names.
An included group reports `interpretation_added`, `interpretation_revised`, or
`interpretation_withdrawn`, with the original before-and-after
`CrossSourceRelation` payloads. Reasons, qualifications, upstream vocabulary and
status, immutable endpoint revisions, evidence IDs, and review IDs remain intact.

Each present side has `present_at_checkpoint: true`. Its relation's `active`
field keeps the existing history reader's meaning: current retained membership
at report time. These can differ for an earlier interpretation. Likewise, a
native endpoint's compact `imported` entry records source-current membership at
its own checkpoint, while the original payload in `imported_evidence` reports
current membership at report time. An upstream link with `upstream_active: false`
can still be a currently retained source report. None of these flags establish
independent implementation verification.

Documentary endpoint references resolve by immutable knowledge-revision ID in
`cross_source_knowledge`; one knowledge identity can have multiple retained
revisions. All support evidence for each included revision is retained, including
historical support. A relation's original witness set must be a subset of that
exact support and must cover every endpoint. Current interpretations also pass
the existing stricter current-evidence validator. Native endpoints resolve to
their exact snapshot and original full source payload; upstream links are checked
against that snapshot's original vocabulary, direction, target, and status.

Baselines did not capture historical review dispositions. When an earlier
interpretation references a review, `baseline_review_status` is `not_captured`.
`current_reviews` is a separately labeled view of the review status at the report
snapshot; an unavailable review has no fabricated status. The command does not
backdate today's resolved or dismissed status to the baseline. Review disposition
does not verify production behavior or establish agreement between sources.

A withdrawn interpretation remains visible through its immutable payload and
endpoint evidence. Withdrawal describes retained membership; it does not reverse
a source claim, replace accepted policy, or resolve a review. The guard uses
included historical endpoint topics to select useful current shared guidance,
including for withdrawals. The detailed historical comparison remains in the
change report, and the current source manifest remains in shared guidance.

## Evidence and output contracts

The save confirmation, remove confirmation, change report, and guardian report
use JSON schema version 1. Save confirmations contain `baseline`, `snapshot`,
`captured_at`, the documentary `records` count, and `source_write: false`. That
count is not the total number of imported observations or relationships. The full
checkpoint stored on disk also retains those memberships.

The change report contains:

- The baseline and current snapshot identities: `project_id` and
  `registry_revision`.
- `registry_changed`, `retrieval_truncated`, and the cross-source coverage flag.
- Complete included `changes`, `relationship_changes`, `imported_changes`, and
  `cross_source_changes`.
- `related_knowledge`, revision-bound `cross_source_knowledge`, exact documentary
  `evidence`, and full `imported_evidence`.
- Separate omission counts for documentary, relationship, imported, and
  cross-source interpretation changes.
- `generation_basis: "documentary_comparison"`,
  `live_checkout_assessed: false`, and the complete output `budget`.

Documentary evidence IDs resolve through `lore evidence ID`. Exact excerpts and
their digests are retained together with the captured source identity, revision,
location, provenance, and bounded surrounding context. Surrounding context is not
a reconstruction of the full original document.

The guardian report contains an `assessment_status`, source-linked `advisories`,
`warnings`, an optional `intelligence`, the current `snapshot`, and a whole-output
`budget`. Its `execution` and `source_write` fields are always false. When present,
`intelligence` is the unchanged shared schema-5 envelope containing the established
schema-4 decision result or useful fallback. Evidence IDs and static observation
IDs in advisory explanations and recommended actions resolve in that nested
result. Model-call and inspection accounting remain available there.

When selected cross-source relationships exist, the schema-5 envelope also
includes `source_relationships`. This copies the original relationship reasons,
qualifications, native vocabulary and status, review dispositions, endpoint
records, and their complete selected evidence. Documentary endpoint knowledge
revisions are recorded separately from captured source revisions; native
endpoints keep their original snapshot bindings. The shared core validates this
manifest against the same registry read snapshot. It remains available when
optional prose or endpoint facts are removed from the compact decision brief.
The human adapter consumes this same manifest; the companion adds no separate
relationship store. The complete manifest counts toward the guardian budget.

| Guardian assessment | Meaning |
| --- | --- |
| `no_documented_change` | No consequential documentary, imported, or cross-source change was included, and no explicit task requested assessment. The live checkout was not assessed. |
| `change_budget_exhausted` | Complete change evidence could not be retained for an automatic assessment. Increase the budget or specify a task. This does not mean that no risk exists. |
| `partial_static_guidance` | Fresh model-supported assessment was unavailable; the shared evidence and any permitted observations remain available. Inspect the nested status to distinguish documentary-only output from actual static inspection. |
| `source_reviewed_advisory` | A bounded model-supported shared assessment produced guidance. Material blockers and selected high-severity risks can become advisories. This remains source review, not execution or a complete audit. |

An empty advisory list never establishes a clean checkout. The guardian exposes
that qualification for both source-reviewed and fallback results. Advisories do
not enforce a merge or deployment gate; an integration must decide how to use
the disclosed evidence, scope, omissions, and status.

The guardian groups high-severity risks and material blockers that refer to the
same changed source record or relationship. The group retains the union of their
original evidence and observation citations, explanations, and recommended
actions. A shared action citing several sources does not itself merge unrelated
risks. Each explanation identifies associated change groups where the evidence
allows that association, points to the exact named-baseline comparison, and
states the static observations and checkout files read. A risk recommendation
uses the shared engine's concrete `next_action`; blockers keep their explicit
decision requirement. These are advisory summaries of the original shared
brief. They do not make additional source claims or execute a suggested action.

## Evaluating longitudinal guardian behavior

[`evaluation/GUARDIAN_LONGITUDINAL.md`](../evaluation/GUARDIAN_LONGITUDINAL.md)
defines preparation, successive real CLI replay, independent source and alert
review, and assessment. The checked-in debug corpus contains sixty transitions
across three synthetic projects, with twenty consequential, twenty benign, and
twenty ambiguous labels. It exercises source reversion, unchanged summaries with
changed exact exceptions, citation churn, scope-qualified reports, source
deletion, and cross-source interpretation revision and withdrawal.

These labels and source assertions were authored as regression fixtures. A
disabled provider can validate source preservation, explicit baselines, complete
evidence, output contracts, and unavailable-assessment reporting. Such a replay
cannot establish high-severity alert precision, developer productivity, or
independent real-project usefulness. The evaluator keeps these outcomes
unmeasured until real assessments and independent reviews exist.

## Inspection, privacy, and local state

The companion reuses the shared core's caller-owned grants. Repository
configuration alone cannot grant checkout inspection. `--inspect` is an explicit
grant for the invocation; a trusted invoking host can instead provide
`LORE_INSPECTION_ROOT`. A configured narrower root must remain inside the granted
root. `--no-inspect` vetoes inspection for the request.

The shared core may investigate automatically within an effective inspection
grant and its existing file, byte, time, and model-call limits. It does not execute
shell commands or project tests. Static observations retain their file content
hashes and static-evidence meaning.

Hosted document egress requires the caller's grant, such as
`LORE_ALLOW_HOSTED_EGRESS=1`, and remains disabled by `privacy.local_only`.
Checkout egress additionally requires the existing explicit checkout permission.
`--allow-checkout-egress` supplies the caller's hosted and checkout consent for
that invocation, subject to the local-only veto; it does not itself grant
inspection. An invoking host may instead combine
`LORE_ALLOW_CHECKOUT_EGRESS=1` with configured checkout egress permission and
hosted consent. Source files, imported instructions, and model responses cannot
create these grants.

Baseline files are stored under the configured state directory's `baselines`
directory. Writes use the existing private-directory and atomic-write helpers;
symlinked paths are rejected. Baseline and comparison operations use read-only
registry access and add no database migration. A permitted guard may use the
shared disposable cache; `--no-cache` disables that cache for the invocation.

## Bounds and checkpoint validation

The companion has the following explicit bounds:

| Resource | Bound |
| --- | --- |
| Named baseline files | At most 16 entries in the baseline directory; remove an obsolete name before adding another. |
| Serialized baseline | 8,000,000 bytes. |
| Documentary records | At most 10,000 current records. |
| Imported records | At most 10,000 latest imported observations. |
| Documentary relationships | At most 20,000 retained relations and reaffirmation links. |
| Cross-source interpretations | At most 20,000 retained interpretation records, with at most 8,000,000 payload bytes. |
| Historical interpretation endpoint evidence | At most 20,000 documentary revisions and 20,000 unique documentary/native evidence entries, with a separate cumulative 8,000,000-byte preflight for historical endpoint fields, exact excerpts, surrounding context and native payloads. |
| Complete evidence preflights | Separate 8,000,000-byte bounds for current documentary evidence, documentary relationship evidence, and imported source payloads. |
| Shared relationship manifest in guidance | At most 128 selected relationships, 256 endpoint records, 1024 evidence entries, and 524,288 serialized bytes; the requested output token limit still applies. |

Exceeding an input bound returns an error instead of silently saving an
incomplete checkpoint. Loading also bounds the saved file before parsing.
Checkpoint project identity and configured source/import scope must match.
Included documentary states and their exact evidence memberships are validated
against sealed registry revisions; included native observations and relationships
must resolve to retained immutable history. A valid old checkpoint continues to
resolve after later source revisions or relationship withdrawals, within these
resource bounds.

The checkpoint checksum detects accidental corruption. It is **not cryptographic
authentication of the user-selected checkpoint**. Someone who edits the file and
recomputes its checksum can select a different valid membership or timestamp.
The immutable-history checks prevent altered fact content and substituted
revision evidence from becoming accepted history; they do not prove who selected
the checkpoint or when. Baselines are derived comparison points, not a separate
accepted knowledge authority.

The response budget uses `cl100k_base` and conservatively accounts for the larger
of the complete JSON, including its final newline, and Markdown output. Nested
guidance, original evidence, qualifications, warnings, metadata, and the printed
budget itself count toward the limit. The comparison drops complete change
groups when needed and reports each omitted group. It never shortens an included
quote or native payload to hide a later exception. Large lists shrink
geometrically before smaller final adjustments.

One cross-source group includes both sides of the stored comparison, every
endpoint revision, its complete source evidence, and the separately labeled
current review statuses. Budget packing removes that whole group and increments
`omitted_cross_source_changes`; it does not retain a new interpretation while
discarding the earlier caveat. Evidence shared by another retained group remains
available, while evidence used only by an omitted group is removed. If all
automatic change groups are omitted, the guardian reports
`change_budget_exhausted` rather than treating the missing history as no change.

Task relevance uses the existing bounded shared retrieval; its omissions propagate
to `retrieval_truncated`. The guardian reserves space for the complete shared
result and its wrapper. Duplicate advisory summaries may be omitted, with
`advisories_in_shared_guidance` reporting that they remain in the nested guidance.
If the required complete envelope cannot fit, the command returns an error.
These are construction and output bounds, not a new storage-scale latency claim.

## Validation and outcome limits

The real SQLite fixtures in [project_companion.rs](../tests/project_companion.rs)
cover cosmetic changes, supersession, source scope, exact revision evidence,
changed rare exceptions with unchanged summaries, explicit replacement and
retention, whole-group output budgets, imported staging-only work, relationship-only
revisions and withdrawals, exact historical endpoint support, independent native
relationship status, unknown baseline review disposition, source-payload tampering,
useful guardian fallback, source safety, and interrupted or symlinked checkpoints. CLI contracts live in
[project_companion_cli.rs](../tests/project_companion_cli.rs).

These fixtures establish concrete evidence, privacy, and behavior contracts.
They do not measure real team onboarding, defect prevention, reviewer trust, or
productivity. Those outcomes and the independent review requirements remain
separate gates in
[SHARED_INTELLIGENCE.md](../evaluation/SHARED_INTELLIGENCE.md).
