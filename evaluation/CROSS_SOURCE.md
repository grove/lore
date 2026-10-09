# Lore 0.4 cross-source evaluation

This extension separates **interoperability contracts** from **measured benefit
to a coding agent**. It does not claim that Lore has achieved the proposed
quality thresholds. The included projects are synthetic fixtures; an
independent, held-out comparison and blind human review are still required.

`cross_source.py` uses Python 3.10+ and the standard library. Preparing fixtures,
creating blind review packets, and assessing completed annotations do not call
models or access a network. `run` invokes the real Lore executable with the model
configuration supplied by the researcher. It never substitutes a fake model.

## Three project fixtures

| Project | Situations | Expected reviewer questions |
| --- | --- | --- |
| `payments` | Accepted maximum of three retries; OpenWiki observes five; an issue is closed; an agent remembers the change. A mutation changes the implementation evidence to three. | Is the discrepancy visible? Does closed work remain work history? Is policy approval distinguished from implementation? Does the changed evidence retire the old discrepancy while retaining its history? |
| `ledger` | An accepted PostgreSQL ADR explicitly replaces an older MySQL ADR. Current OpenWiki evidence agrees. A stale MySQL memory has an upstream replacement relationship. | Is the accepted constraint preserved? Are obsolete decisions and memories qualified? Does agreement avoid a false discrepancy? |
| `releases` | Production requires signed artifacts. Staging permits unsigned fixtures. Work and agent observations concern staging. | Does the production constraint survive retrieval? Is a staging exception prevented from becoming a production contradiction? |

Each project contains original Markdown decisions, simple inspectable source
code, OKF 0.2 pages with Grounded Claims schema 1 sidecars, Engram export 0.2.0,
and Beads issue JSONL. The sidecars carry the SHA-256 of the exact page bytes and
the upstream `repo-file-v1:sha256:` token for the included source file. Lore does
not independently verify that source file during import: the token continues to
describe upstream evidence at its recorded revision.

The bundled Beads snapshots deliberately include one harmless memory record so
that the default `include_memories: false` boundary is exercised. Prompts are
never imported as Engram observations. The source corpus is copied into a fresh
evaluation workspace; neither the fixture sources nor an upstream database are
edited.

### Prepare without inference

```bash
python evaluation/cross_source.py prepare \
  --output evaluation-results/cross-source-prepared
```

Use `--projects payments` for the first vertical slice. A manifest binds every
relative input path and its original SHA-256. Reusing a nonempty output
directory fails instead of silently mixing runs.

### Run actual Lore preparation and retrieval

```bash
cargo build --release --locked

python evaluation/cross_source.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_LOCAL_MODEL \
  --mutate \
  --output evaluation-results/cross-source-local-01
```

For a hosted generative model, select `--provider openai --model YOUR_MODEL`
and explicitly pass `--allow-hosted`. The same separate decision-provider,
decision-model, endpoint, and reasoning-effort flags as the existing benchmark
are supported. Credentials remain in environment variables. The command does
not invoke OpenWiki, Engram, or Beads: it consumes the prepared snapshots.

Each project records:

- The initial `lore init` report and its preparation time and model calls.
- An unchanged `lore update`, requiring a no-op, zero calls, unchanged current
  native snapshot identities, unchanged registry counts, unchanged database
  bytes, and unchanged wiki bytes. The database hash detects in-place payload
  changes even when row counts and current snapshot IDs stay the same.
- The complete JSON response for each `lore context` task, including omissions
  and retrieval truncation. Retrieval must use zero model calls and leave the
  registry unchanged.
- Mechanical resolution of every current native evidence ID and every cited
  evidence ID through `lore evidence`. For native records, the CLI checks the
  retained content hash and structured record pointers.
- SQLite integrity, the executable fingerprint, configuration fingerprint, and
  the fixture source manifest.
- An `initial-contract.json` artifact binding the source manifest, complete
  context responses and their hashes, expected imported/cited evidence IDs,
  per-ID resolver responses, and unchanged-import before/after registry
  snapshots and CLI report. The optional mutation has a separate artifact;
  its `unchanged_import` is null because the bundled runner does not repeat the
  unchanged-update check after that mutation. It cannot borrow the initial
  snapshot's no-op result for a comparison of the mutated snapshot.
- Preparation, optional mutation preparation, and retrieval costs separately
  and together. The unchanged-update timing is reported separately.

The command exits with code 2 if a recorded interoperability contract fails.
It retains the report so the failure can be inspected. Successful contracts do
not establish semantic correctness or comparative benefit.

Actual USD charges are left `null` unless a measured total and its source are
provided with `--billed-cost-usd` and `--billing-source`. Request counts are not
converted to dollar estimates. **The bundled run does not measure the cost of
creating upstream snapshots.** Its cost report explicitly says so; those costs
must be included in the full comparison below.

## Compare the four setups

Run the same coding-agent tasks, using the same source revisions, agent model,
execution settings, and task budgets, under these four setup identifiers:

| Identifier | Agent inputs |
| --- | --- |
| `code_docs` | Code and project documents |
| `openwiki` | Code and documents plus OpenWiki |
| `tools` | Code and documents plus OpenWiki, Engram, and Beads |
| `tools_lore` | The same tools and sources plus Lore |

The CLI fixture report measures Lore's behavior. It is not an agent completion
and must not be substituted for the four independently recorded task outcomes.
Collect those outcomes using the researcher's agent runner, with code changes,
findings, or task answers in reviewable UTF-8 files. The evaluator does not
silently install or execute an additional coding agent.

Prepare a JSON study manifest with:

```json
{
  "schema_version": 1,
  "fixture_only": true,
  "held_out": false,
  "independent_projects": false,
  "cases": []
}
```

Add one object per task to `cases`. Each case needs `project`, `case_id`, `task`,
`source_root`, one common `source_fingerprint`, a list of reviewed
`critical_constraints`, and `setups` containing all four identifiers above.
`source_root` names the isolated **input snapshot directory**, relative to the
study manifest or as an absolute path. Keep generated outputs, configuration,
model logs, and evaluation reports outside it. The fingerprint is the `sha256`
value from `cross_source.fingerprint`, which hashes a canonical mapping of
relative input paths to their exact file-byte SHA-256 values:

```bash
PYTHONPATH=evaluation python - <<'PY'
import json
from pathlib import Path
from cross_source import fingerprint
print(json.dumps(fingerprint(Path("evaluation-results/snapshots/project-a")), indent=2))
PY
```

For example, a case can contain `"source_root": "snapshots/project-a"` and
`"source_fingerprint": "<the manifest's sha256 value>"`. `blind` reads the
directory, checks that value, and retains the full manifest and canonical root
in the assignment. `assess` reads those source bytes again. Missing files, new
files, changed contents, or symlinks invalidate the binding. Leave the original
snapshot available at its bound path through assessment. The evaluator
canonicalizes the caller-selected workspace parent for portable system
temporary paths, then rejects root and descendant symlinks. Mutation copying
also rejects destination symlinks before writing any files.

Use at least three independently selected real projects for release assessment.
One project must retain one consistent snapshot identity across its cases, and
different projects must have different fingerprints. Use separate studies when
comparing different revisions of the same project. Keep `fixture_only: true`
for synthetic data. The evaluator recognizes the bundled initial snapshots,
mutation overlays, and merged initial-plus-mutation snapshots by fingerprint
and forces fixture-only status at both blinding and assessment, even if flags
claim otherwise. Different fingerprints alone cannot establish independence;
unrecognized or modified synthetic inputs still require an honest researcher
declaration. The program cannot independently prove that a project was held
out from development.

Each setup's object has the following shape. The null values are deliberately
unscored placeholders; fill them from actual execution and evidence checks
before invoking `blind`:

```json
{
  "answer_path": "answers/project-task-result.txt",
  "coding_agent": "provider/model/version and execution settings",
  "cost": {
    "preparation_seconds": null,
    "retrieval_seconds": null,
    "preparation_model_calls": null,
    "retrieval_model_calls": null,
    "preparation_includes_all_tools": false,
    "billed_cost_usd": null
  },
  "run_contract": {
    "path": "runs/project-a/initial-contract.json",
    "sha256": "<SHA-256 of the exact artifact file bytes>"
  }
}
```

`answer_path` is relative to the study manifest. All four entries within a case
must identify the same coding agent. Full preparation includes creating the
wiki, preparing or exporting memories and issue history, and building Lore
when those operations are used by that setup. Record amortization assumptions
consistently across cases. Retrieval time and calls are then added to that
preparation cost. The `tools_lore` entry needs the `run_contract` reference;
other setup entries may omit it. The reference path is relative to the study
manifest. Copying the reference from a run's metrics may require adjusting its
path while retaining the exact artifact digest.

### Bound mechanical records

The bundled runner writes contract artifacts from the real CLI calls it makes.
For independent projects, collect the same records in the researcher's runner
using `registry_state`, `resolve_citations`, and `write_run_contract` with actual
CLI results. The collector records have this schema:

| Field | Required evidence |
| --- | --- |
| `schema_version` | Integer `1` |
| `source_fingerprint`, `source_manifest` | The exact common input fingerprint and complete file manifest |
| `contexts` | Objects containing `task`, the complete JSON `response`, and `response_sha256`, computed with `cross_source.digest(response)`; the relevant case task must be present |
| `registry_before_retrieval`, `registry_after_retrieval` | Equal snapshots from `registry_state`, including the database hash, native current-state hash, complete current native evidence IDs, row counts, wiki hashes, and SQLite integrity result |
| `citation_integrity` | Complete `expected_imported_evidence_ids` and `expected_cited_evidence_ids`, plus `results`, `checked`, `resolvable`, `failures`, and `mechanically_verified` |
| Each citation `results` entry | `evidence_id`, the CLI's complete `response`, its canonical `response_sha256`, and `error` (`null` for success) |
| `unchanged_import` | `before` and `after` registry snapshots and the actual unchanged-update CLI `report` |
| `provenance` | `collector`, `lore_binary_sha256`, and `configuration_sha256` identifying the measured executable and configuration |

Assessment derives the expected ID set from the bound registry snapshot and
all retained context references, compares it with the artifact's expected-ID
manifests, and requires exactly one matching successful resolver response for
every ID. It verifies context and resolver response hashes and the enclosing
artifact's byte digest. Any explicit imported `ne_<hex>` evidence reference in
the coding-agent answer must also occur in that successful resolver set. The
no-op gate requires matching before/after registry content snapshots, matched
to the context run, and an actual report stating no-op with zero model calls.
Unsigned scalar declarations such as `mechanically_verified: true`, positive
counts, or zero churn/calls cannot satisfy these gates without the bound
records. Missing contracts may be blinded for human review, but their
mechanical gates remain unassessable and fail the release assessment.

These checks establish artifact completeness and consistency. They are **not
cryptographic proof that commands were executed**, that the contexts were
actually supplied to the agent, or that a researcher submitted authentic
collector outputs. Execution provenance, complete upstream cost accounting,
project independence, held-out selection, and honest human blinding remain
explicit researcher/reviewer attestations. Do not fabricate records or relabel
fixtures as an independent evaluation.

### Blind the review packets

```bash
python evaluation/cross_source.py blind \
  --study evaluation-results/study.json \
  --output evaluation-results/blind-review
```

The command validates every input before creating the output directory. It
creates random sample identifiers, copies the actual responses into `answers/`,
shuffles packet emission order, and creates unsigned annotation templates in
`reviews/`. Supplied contract artifacts are digest-checked and copied into
`contracts/`. Setup assignments, source manifests, cost metadata, and artifact
references stay in `assignment.json`.

Give reviewers only `answers/` and `reviews/`. Keep `assignment.json` and
`contracts/` out of the review distribution. The response itself can disclose which tools were used;
use a consistent response format and require the reviewer to confirm whether
blinding held. A reviewer who recognizes the setup should leave
`blind_confirmed` false. A study requires at least two distinct reviewer
identities across its completed annotations; each response must have a complete
review. For research conclusions, allocate cases independently and adjudicate
disagreements before entering final counts.

Reviewers record the exact missed constraint strings, unsupported
authoritative-claim count, its high-severity subset, discrepancy-alert count,
and false-alert subset, plus their identity, date, and substantive notes. Every
annotation is bound to the SHA-256 of the response and the canonical
`assignment_sha256` retained in its template. Keep both digests unchanged while
annotating. Editing the response, setup mapping, source binding, costs,
artifact references, or displayed constraints after review invalidates the
annotation. This detects changes to reviewed materials; these digests are not
reviewer signatures and cannot prove authorship.

### Assess the release gates

```bash
python evaluation/cross_source.py assess \
  --assignment evaluation-results/blind-review/assignment.json \
  --reviews evaluation-results/blind-review/reviews \
  --output evaluation-results/cross-source-assessment.json
```

Assessment exits with code 2 until all gates pass:

| Gate | Rule |
| --- | --- |
| Comparable complete review | All four setups for every case, matching task/source/agent identity, complete blind annotations, at least two reviewer identities |
| Independent coverage | At least three independently selected held-out projects, distinct bound source fingerprints, consistent source identity within each project; recognized synthetic fixtures excluded regardless of labels |
| Critical constraints | At least 20% fewer misses than the baseline setup with the fewest misses across the same cases |
| Unsupported authority | No increase compared with any baseline; zero reviewed high-severity unsupported claims |
| False discrepancy warnings | At most 10% of adjudicated Lore alerts |
| Citation integrity | A digest-bound run artifact covering all expected imported/cited references, with successful matching per-ID resolver records; answer-only imported IDs must also resolve |
| Identical repeated import | Bound before/after registry content snapshots unchanged, and a measured no-op report with zero model calls |

If the strongest baseline misses zero constraints, the improvement ratio is
unmeasured and cannot establish a 20% reduction. If Lore emits no adjudicated
alerts, a false-alert rate is likewise unmeasured. Both cases remain pending
instead of being advertised as perfect performance.

Reports compare total preparation plus retrieval time and calls for every
setup. Currency totals stay null if any component lacks measured billing.
There is no invented cost threshold or inferred savings claim. The gate values
are proposed release criteria; a passing unit test of their arithmetic is not a
passing product evaluation.

## Run the evaluator's offline tests

```bash
python -m unittest discover -s evaluation/tests -p test_cross_source.py -v
```

These tests cover snapshot byte binding before and after review, automatic
fixture recognition, duplicate-project rejection, assignment tampering,
source and destination symlinks, in-place database changes, complete resolver
coverage, answer-only fabricated citations, contract digest changes, no-op
records, full-cost accounting, anonymous review templates, zero-denominator
handling, missing setups, and quality-gate arithmetic. They invoke no inference
models. Synthetic collector records used in unit tests exercise bookkeeping
only; they are not measured execution or a real comparative evaluation.
