# Named baselines and source-backed change history

Lore retains **deterministic change history** and **one** task-guidance interface. `lore baseline` and `lore changes` compare two retained knowledge snapshots, including historical source excerpts, source-authority/scope distinctions and review qualifications. For a recommended action, use `lore context TASK` **separately**, against the current retained registry. The old `lore guard` advice product was retired in [#68](https://github.com/grove/lore/issues/68); see the [migration guide](GUARDIAN_MIGRATION.md).

## A safe workflow

Run an authorized update to compile the intended sources before saving a checkpoint:

```sh
lore update
lore baseline save joined
```

After a documentary source changes, deliberately refresh the compiled registry, inspect the historical difference and ask one task:

```sh
lore status
lore update
lore changes --since joined
lore changes --since joined --task "Improve queue diagnostics"
lore context "Improve queue diagnostics without violating accepted policy" --no-inspect
```

`lore changes` is a **source comparison**, not a risk detector. `lore context` is the **task-guidance engine**; it does not automatically load the named baseline or secretly run `update`. When historical transitions matter, read `changes` and its cited original sources before making the task request. If you need no model calls, use `lore context "TASK" --fast` on an initialized registry. A safe recommendation is not permission to edit code or proof that a test executed.

Named checkpoints can be kept, explicitly replaced or removed:

```sh
lore baseline save before-release
lore baseline save joined --replace
lore baseline remove before-release
```

Replacement/removal affect the named local baseline only, **not** the original source documents or retained immutable history. The project must already be initialized. All commands support global `--json`.

## Public command and JSON contracts

| Command | Behavior |
| --- | --- |
| `lore baseline save NAME` | Creates a named checkpoint; `--replace` deliberately replaces the previous named capture |
| `lore baseline remove NAME` | Removes one named checkpoint; no source or knowledge deletion |
| `lore changes --since NAME` | Complete deterministic before/after comparison; optional `--task TASK` scopes retrieval; `--max-tokens` defaults to 8000 and allows 512–100000 |
| `lore context "TASK"` | Separate current-knowledge guidance; schema 5 when initialized, bootstrap source-only when not; optional `--no-inspect`, `--inspect`, `--fast`, and other documented grants |

Baseline names contain **1–64 ASCII letters, digits, hyphens or underscores**, not paths. An invalid name or output budget fails explicitly. A source edit not yet compiled is **not** a retained knowledge change, even if `lore status` can report drift.

Baseline save/remove confirmations use **schema 1**. The baseline save result includes the name, snapshot ID/revision, timestamp, documentary record count, and `source_write: false`. The comparison result is schema-1 `ChangeReport`, with:

- `baseline_snapshot`, `snapshot`, `registry_changed`, `task`, `retrieval_truncated` and `cross_source_relationships_changed`
- Complete included `changes`, `relationship_changes`, `imported_changes` and `cross_source_changes` with exact before/after statuses and support
- `related_knowledge`, exact-version `cross_source_knowledge`, immutable documentary `evidence` and `imported_evidence` containing the original native payload
- Separate whole-group omission counts, `generation_basis: documentary_comparison`, `live_checkout_assessed: false` and a complete `cl100k_base` output-token budget

The current task response from `context` is a **different contract**. In initialized schema 5 it includes versioned `snapshot`, the *actual* `capabilities`, `intelligence`, optional complete `source_relationships`, usage and the bounded response `budget`. Use the source-backed risks, constraints and next action there; do not treat a schema-1 history report as executed diagnosis. See [Guardian migration](GUARDIAN_MIGRATION.md) for the retired `assessment_status` and `advisories` fields.

## What is compared

A record retains a source-owned kind, lifecycle and scope. An accepted decision is documentary policy evidence, not runtime proof; a proposal is not approval, a reported deployment or work completion is not independent execution proof, and a source withdrawal does not retroactively erase historical evidence. The comparison retains current source support **separately from** immutable assertion and knowledge revisions. A condition changing **A→B→A** remains observable even when its short statement is unchanged.

Relations (including explicit supersession/reaffirmation) preserve their exact source witnesses and original conditions. A replaced accepted decision does not silently make an unrelated proposal accepted. Editing an original source or its exception can affect current applicability even when a summary appears unchanged. A current `review` disposition is still not accepted policy.

Optional native work/import snapshots keep their **original authority labels and captured revisions**. A Beads issue closure is work history; an OpenWiki observation is implementation evidence scoped to its snapshot, not proof of today's checkout. The report retains native lifecycle, verification, original payload and current membership; a later import cannot overwrite its historical source record.

Cross-source interpretation history retains source-owned relationship reasons, qualifications, native vocabulary/status, both exact endpoint revisions and their original documentary/native evidence. It distinguishes a new pair, revised interpretation, review disposition and withdrawn membership. Baselines do **not** backdate today's review resolution to a prior checkpoint: `baseline_review_status: not_captured` remains explicit when not known. `current_reviews` describes only the report snapshot. Withdrawal neither reverses an original claim nor establishes source agreement.

All included `ev_...` and `ne_...` citations should resolve with `lore evidence ID`. Exact source revisions, path, location, excerpt hash, provenance and bounded surrounding context are the evidence, not a generated assertion of live behavior. When a current or historical comparison requires additional investigation, use `lore context TASK` under the caller's actual read/egress grants.

## Bounds, safety and retention

| Resource | Bound / protection |
| --- | --- |
| Named baselines | At most 16 files under the private configured `.lore/baselines/` directory |
| Serialized checkpoint | 8,000,000 bytes with checked project/source scope and checksum |
| Documentary and imported current records | At most 10,000 of each |
| Documentary and cross-source relationships | At most 20,000 of each |
| Historical interpretation endpoints | Up to 20,000 documentary revisions and 20,000 distinct source witnesses, with independent cumulative byte preflight |
| Evidence payload inputs | Separate complete 8,000,000-byte preflights for documentary, relation and native support |
| Change-report output | Whole items within the requested 512–100000 token budget, not clipped excerpts |

Input limits are errors, **not silent partial baseline saves**. The named checkpoint checksum protects against accidental damage; it is not an authenticated human decision. Every restored/reused record must still resolve to immutable project history, with evidence revision membership and source permission boundaries checked. Symlinks or unmanaged paths fail closed. A changed source root/import configuration may invalidate a checkpoint rather than silently widening the scope.

The comparison budget conservatively covers complete JSON (including its final newline) and Markdown. It may omit **whole groups** with explicit `omitted_changes`, `omitted_imported_changes`, `omitted_relationship_changes`, `omitted_cross_source_changes`, or `retrieval_truncated`. A relevant omitted group can contain a critical exception. An empty included list, a small budget or an uninitialized/disabled model **does not** establish that there is no policy or risk.

`baseline save` writes only the deliberately requested local checkpoint. `changes` uses the stored registry read-only; it makes **zero model calls**, does not read or write the checkout for diagnosis and never mutates the source corpus. `context` may perform permitted bounded **static** checkout inspection and model synthesis, but it never executes project tests or grants itself access based on repository text. Its `--no-inspect` veto, hosted privacy ceiling and separate checkout egress consent remain in force.

## Validation and the independent CI gate

The cross-platform [companion tests](../tests/project_companion.rs) retain supersession, documentary/current support, **A→B→A**, historical exact excerpts, imported scope, source-revision withdrawal, unknown earlier review status, complete group budget, privacy/read-only and checkpoint integrity tests. The [CLI tests](../tests/project_companion_cli.rs) exercise named checkpoints, scoped queries, the ordinary context result and explicit retired-command failure.

The **Guardian source-integrity harness is deliberately retained**: [the dedicated workflow](../.github/workflows/guardian-integrity.yml) and [the replay oracle](../evaluation/guardian_integrity_gate.py) compare the pinned historical binary with the exact candidate head under independent source/event observation, cancellation, source-restoration regression and a no-Lore control. The candidate replay invokes `context`, not the removed advisory command. Both replay modes must retain full command/source ledgers, exact checkpoints and read-only constraints. These are source-safety tests, **not** evidence that alert quality, coding correctness or actual production behavior has been independently measured.

See [historical study design](../evaluation/GUARDIAN_LONGITUDINAL.md) for the synthetic prior protocol and measurement limits. Its past Guardian advice outcomes are archival, not supported product commands.
