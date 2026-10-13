# Migration: task-feedback receipt drafting retired

[Issue #69](https://github.com/grove/lore/issues/69) removes the specialized `lore feedback draft` command and `lore::feedback` Rust API. It removes the draft/export/write workflow and its `lore.task_outcome` schema-1 input and `lore.feedback_draft` schema-1 output *contracts*, not ordinary Markdown ingestion. The last pre-reduction release candidate, **0.9.0-rc.1**, documented those legacy forms.

This is an **intentional breaking change**. Existing receipts and previously generated Markdown notes are **not automatically migrated, approved, rewritten or deleted**.

## Recommended source-backed workflow

Rather than feeding an agent receipt to Lore, have an accountable developer or maintainer prepare and **review** a Markdown task note under an already configured source root (for example, `docs/investigations/retry-check.md`). Review for sensitive paths, secrets, private logs, task details, checker names and other unintended disclosure before saving.

A concise example for a source document:

```markdown
# Retry diagnostics investigation

REPORT retry: The team reports that a staging retry check succeeded. Production behavior and independent execution have not been verified.

## Decision context
- Existing accepted retry policy: cite the original ADR and its scope.
- Reported outcome: staging-only; not an accepted change to production policy.

## Outstanding verification
- Independently execute the relevant tests with authorized tools.
- Obtain a separate maintainer decision before changing an accepted retry limit.
```

Then use the **existing** import/update/review path:

```sh
lore status
lore update                  # explicit; may invoke configured inference
lore context "Check retry diagnostics without changing accepted limits"
lore search "staging retry check"
lore decisions "retry policy"
lore evidence EVIDENCE_ID
```

`lore init --configure-only --source ./docs` can configure an uninitialized project, with user review before `lore init`. No extra agent memory, receipt parser, web frontend or auto-approval system is introduced. Markdown intake retains exact original source quotation, revision identity, provenance and appropriate lifecycle/kind: a `REPORT` or reviewed task narrative is **reported history**, not independently verified execution or accepted architectural policy. A hash establishes byte identity **only**, not that a check ran. A source note does not grant model, network, checkout-read or write permissions.

## Old-to-new interface map

| Previously supported | After #69 |
| --- | --- |
| `lore feedback draft --receipt task-outcome.json` | Review the original account and create ordinary Markdown yourself; do **not** send the JSON receipt to an automatic drafting endpoint |
| `--output path --write` | Write reviewed Markdown through your normal editor/Git workflow; Lore no longer owns receipt-specific file creation, temporary files, destination checks or output publishing |
| `--allow-file PATH` | Use your normal authorized checker to inspect current file bytes and independently record results in the reviewed note. `lore context --inspect` permits bounded *static inspection only*, not test execution or check authentication |
| `lore.task_outcome` JSON schema 1 | No replacement receipt schema. Existing JSON is a caller-owned artifact, **not** ingested automatically by `lore update` |
| `lore.feedback_draft` JSON schema 1 | No replacement schema or `reported_checks`/`verified_changes` result. Use existing `lore --json context TASK`, `lore --json search QUERY`, `lore evidence ID` and `lore status` contracts separately |
| `lore::feedback::draft` | Removed Rust API. If embedding Lore, use ordinary configured Markdown + existing update/evidence/context APIs instead |

`feedback` is no longer a CLI command. A call to `lore --json feedback draft ...` fails before configuration or source access with exit code **2** and `code: "invalid_arguments"`; text-mode gives the normal unknown-command error. No compatibility alias silently drafts a file, hashes a claimed check, imports a receipt or edits a source.

## Preserve historical receipts, evidence and authorship

- **Retain caller-owned receipt JSON and drafts** wherever their owner keeps them. They can be sensitive; review retention and permissions under your project's policy. Their reported checks, timing, charges, attestations and hashes remain *claims* until independently verified. There is no schema migration or auto-approval.
- **Previously written Markdown proposals in a configured source root** remain ordinary sources. Their source bytes and retained evidence are not changed by this command removal. Review them before further sharing or treating them as current guidance. A receipt-derived note must not gain accepted-policy authority because it was produced by Lore.
- **If a previously imported note is later withdrawn**, `lore update` removes its *current* support but preserves historical source/evidence revisions. Use `lore evidence ID` to inspect retained original quotations; do not purge the project to retire the old feedback feature.
- **Never blindly ingest raw receipt JSON or log captures** as if they were Markdown or authenticated source decisions. Keep sensitive records outside configured source roots when disclosure is not intended.

This removal does **not** change primary Markdown source ingestion, optional OpenWiki/native claims and their separate authority, ordinary evidence/decision/case/review paths, `context`, `update`, `audit`, source-integrity checks, or independent Guardian CI. Read-only commands, original evidence and privacy limits retain their existing contracts.

See [the project scope](PRODUCT_SCOPE.md), [first-run guide](tutorial/README.md), and [0.9 historical release candidate notes](V09.md). The legacy receipt specification has been removed from the current documentation rather than advertised as a live feature.
