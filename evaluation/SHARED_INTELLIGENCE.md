# Shared intelligence and learning-transfer evaluation

`shared_intelligence.py` extends the existing evaluation machinery for the
explicit schema-5 agent experience and schema-1 human orientation. It uses
`coding_tasks.py` source snapshots, `cross_source.py` registry fingerprints and
evidence resolution, and `decision_tasks.py` exact static-observation checking.
The human and agent outputs are collected from the same prepared project.

This is an integration and source-contract evaluation. It does not establish
that a human learned a project, that a coding agent made a better change, or
that source-backed generated prose is semantically correct.

## Prepare without inference

```bash
python3 evaluation/shared_intelligence.py prepare \
  --cases evaluation/corpora/shared-intelligence/cases.json \
  --output evaluation-results/shared-prepared
```

Preparation copies original sources and records their exact fingerprints. It
performs no model calls and executes no candidate implementation. The default
cases, when `--cases` is omitted, are the existing payment, ledger and release
coding fixtures. The optional shared-intelligence cases add a distinct,
same-project transfer task. Both sets remain synthetic development fixtures.

## Collect the real CLI experience

Build Lore and make the selected model available, then use a fresh output path:

```bash
python3 evaluation/shared_intelligence.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_PINNED_LOCAL_MODEL \
  --allow-inspection \
  --cases evaluation/corpora/shared-intelligence/cases.json \
  --output evaluation-results/shared-local-01
```

The collector initializes each copied project, performs an unchanged update,
and invokes each of these experiences twice:

| Experience | Actual invocation |
| --- | --- |
| Legacy fast | `lore --json context TASK --fast` |
| Legacy intelligent | `lore --json context TASK --schema-version 3` |
| Legacy decision | `lore --json context TASK --schema-version 4` |
| Shared agent | `lore --json context TASK --schema-version 5` |
| Shared human | `lore --json onboard --topic TASK` |
| Agent restriction | Schema 5 with `--no-inspect` |
| Human restriction | Onboarding with `--no-inspect` |

`--allow-inspection` supplies a standing `LORE_INSPECTION_ROOT` grant limited to
the copied project through each subprocess's environment. It never writes a
grant into project documentation. Omitting this option produces a documentary
run. A restriction remains effective even after earlier queries populated
permitted derived caches.

Hosted inference requires `--allow-hosted`. Fresh checkout egress separately
requires `--allow-checkout-egress`. These options set the corresponding host
environment grants and configuration for the explicitly requested collection.
Ambient grants are cleared from the child environment before the chosen
envelope is applied; unrelated credentials remain private environment values
and are not copied into reports. The parent environment is unchanged.

The human and agent queries intentionally share project state to test their
common intelligence and restriction behavior. Their execution order is fixed.
Reported per-call latency and cache status are diagnostic measurements from
that sequence, not a randomized performance comparison. Use the existing
independent coding-task experiments for comparative implementation outcomes.

## Hard contracts

The assessor recomputes checks from full records and the copied source bytes:

- The outer human/agent schemas and nested schema-4 contract remain explicit.
- Both shared experiences identify the same project and registry revision.
- Every human evidence reference belongs to the nested shared evidence or the
  validated schema-5 `source_relationships` manifest. Source relationships must
  keep their endpoint identities, knowledge/native revisions, complete evidence,
  discrepancy status and qualifications. Orphan records, duplicate identities,
  substituted support and unbound review dispositions fail the gate.
- The relationship manifest's exact documentary excerpts and source provenance,
  and its native endpoint scope, source metadata and locators, must match the
  originals returned by the existing `lore evidence` resolver. Native metadata
  and quoted example identifiers are source data; they cannot mint citations.
  Every static observation separately has its original file hash, exact line
  range, verbatim bytes and content-derived identity verified independently.
- Source-registry and wiki bytes stay unchanged during context collection.
  Source hashes and modification times also stay unchanged.
- `--fast` makes no model calls and repeats identically.
- Repeated intelligent core guidance reports a cache hit without another core
  model call. Human presentation calls are counted separately and included in
  its total, so presentation overhead cannot disappear from measurements.
- `--no-inspect` yields no checkout observations, file reads or checkout-bearing
  model calls. No response claims an execution or source-writing capability.
- A genuine unchanged update reports no-op, zero model calls and unchanged
  registry/wiki bytes.
- Returned budget metadata must be internally valid. Exact tokenizer-budget
  conformance remains independently exercised by Rust output-contract tests.

```bash
python3 evaluation/shared_intelligence.py assess evaluation-results/shared-local-01
```

Assessment rechecks source snapshots, outputs, observations, current checkout
and registry state, configuration and review bindings. Changed source bytes,
fabricated observations, mismatched snapshots, stale review bindings and a
forged saved `checks` boolean cannot pass.

Response permission fields are contract assertions. They do not replace the
independent process/network/filesystem audit described in
[DECISION_INTELLIGENCE.md](DECISION_INTELLIGENCE.md). Missing audit evidence
remains explicitly unmeasured.

## Independent human source review

Each collection creates two initially empty review slots per orientation in
`reviews/`. Give reviewers the original sources, the corresponding orientation
and its exact evidence. Reviewers assess purpose, concepts, architecture,
workflow, critical conditions, attribution and a useful next step. Each score
requires concrete source-backed notes; severe unsupported claims are recorded
separately. Two distinct attributed reviewers and scores of at least 2/3 are
required for the orientation-review gate.

This gate assesses the output, not the learner. It does not become a learning
result even when every review is complete. The JSON reports these separately:

| Field | Meaning |
| --- | --- |
| `mechanical_contracts_passed` | Full source/protocol checks passed |
| `orientation_review_status` | Reviews are complete or still pending |
| `orientation_review_passed` | Complete source reviews meet their rubric |
| `human_learning_outcome` | Unmeasured by this collector |
| `agent_productivity_outcome` | Unmeasured by this collector |
| `independent_integrity_audit` | Unmeasured by this collector |
| `product_acceptance_passed` | Always false here; this collector cannot establish the full product outcome |

Reviewer identity and interpretation remain accountable human attestations.
Fingerprints detect later changes; they cannot authenticate a reviewer's
identity or establish whether someone actually read the sources.

## A distinct transfer task with an important exception

The shared-intelligence corpus pairs two tasks in the same payment project:

1. Complete the existing idempotent payment retry adapter. Preserve the exact
   original token, retry only transient transport failures and retain the
   distinction between accepted policy and implementation history.
2. Complete the legacy refund adapter. That provider has no idempotency support.
   An ambiguous submission must be reconciled through one read-only lookup,
   never resubmitted. A missing receipt means unknown outcome; an unavailable
   lookup is not proof that no refund occurred.

The transfer checker remains outside both input snapshots. It verifies a
single submission, exact token and receipt identity, false-valued receipt
objects, unresolved outcomes, definitive declines and unavailable status
lookups. The checked-in regression tests actually execute it against an
unimplemented adapter, a correct scoped implementation and an incorrectly
copied retry loop. A superficially successful retry pattern therefore cannot
pass the second task merely by resembling the first solution.

Use the manifest with the existing `coding_tasks.py` or `decision_tasks.py`
runner to obtain real coding attempts and independent executable results. Keep
the second task, its source snapshot, answers and checker outside first-task
participant inputs. These public fixtures are recognized by source fingerprint
even when copied into an external manifest; changing a label does not make
them independent held-out projects.

For a genuine human study, separately record the participant's first and
second patches, explanation, applicable-condition reasoning, assistance and
mentor interventions, total active time, source versions and authorship.
Declare the second-task assistance allowance before scoring. An agent-authored
patch is evaluated in the agent track and cannot establish human competence.
No participant outcome is recorded by the checked-in regression fixtures.

## Milestone gates and current evidence

| Scope | Concrete evaluation gate | Evidence supplied here |
| --- | --- | --- |
| M0 / R0, A1–A2, G1 | Shared project revision, exact manifests, schema compatibility, no-op, inspection veto, bounded honest fallback | Executable collector and adversarial assessor fixtures; real inference must be collected separately |
| M1 / O1–O3 | Accurate orientation plus correct first task, accurate explanation and distinct less-assisted transfer | Independent orientation review packets and executable same-project transfer counterexample; human outcomes remain unmeasured |
| M2 / A3, R4 | New ADR invalidates reused advice even if old files are unchanged; deep same-size edits/deletions and narrower grants invalidate support | Existing Rust mutation/permission tests plus schema-5 snapshot checks; real reuse-efficiency and correctness comparison remains separate |
| M3 / R1–R3 | Exact direct retrieval, multiple-parent/depth-over-five DAG, and rare exceptions survive views and retrieval | Transfer table/exception source supports retrieval and summarization tests; hierarchy benefit needs its own flat/direct ablation |
| M4 / R5–R7 | Explicit historical baseline, consequential findings, irrelevant-change false positives, no fabricated replay or mastery | Existing policy/scope corpora support counterexamples; longitudinal guardian burden and optional runner validation remain separate |

The detailed AT, ON and AG scenarios stay authoritative in the design documents.
The transfer corpus records its precise ON/AG mappings. Do not describe a
milestone as accepted merely because a row, fixture or module exists.

## Offline regression checks

```bash
python3 -m unittest discover -s evaluation/tests -v
```

The pre-change evaluation baseline at `4197ba345de5ff7ec1bad17ab1f8349803b47783`
was 79 passing Python tests. New tests exercise per-process grant isolation,
exact human reference closure, source/output tampering, legacy routing,
shared revision identity, complete human call accounting, review binding,
fixture provenance and the distinct executable transfer conditions. Offline
provider/annotation fixtures are labeled as such and cannot pass product
acceptance. No live-model quality score, real human learning improvement,
coding-agent productivity improvement or billed-cost saving is claimed here.
