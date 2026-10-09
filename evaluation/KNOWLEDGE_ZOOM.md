# Measure direct retrieval against Knowledge Zoom

`tests/knowledge_zoom_comparison.rs` executes both existing retrieval entry points against the same compiled SQLite snapshot and the same query and token budget:

| Arm | Executed code | Included retrieval work |
| --- | --- | --- |
| Flat/direct | `context::build_context` | Deterministic lexical and recorded-relationship retrieval, source resolution, complete-response packing |
| Knowledge Zoom | `knowledge::explore` | The same direct candidate substrate, a fresh documentary graph build and validation, cross-level selection, source resolution, complete-response packing |

This is an actual retrieval comparison. The default corpus is a **synthetic diagnostic fixture** compiled through the ordinary update engine with `tests/common::FakeModel`. It does not measure inference quality, human comprehension, learning transfer or coding outcomes. There is no assertion that either arm must have better recall, smaller output or lower latency.

The default test creates twelve documents in a temporary project, compiles the registry, and runs eight hand-authored query cases at 1,500 and 8,000 tokens. Each arm runs three times per case and budget: 96 measured calls in total. Cases cover an exact original ID, exact symbol, exact source path, capacity changes with a rare exception, conceptual orientation, an explicitly proposed change, a documented disagreement with its original relationship witness, and an identifier absent from the fixture. One additional unmeasured retrieval supplies a real response for deliberate scorer-corruption probes.

## Run the bounded fixture

The ordinary test run performs no provider requests and creates no report outside its temporary fixture:

```bash
cargo test --locked --test knowledge_zoom_comparison
```

The optional existing-project test is ignored unless explicitly selected. To retain the synthetic comparison, choose a **new absolute file path** whose parent already exists:

```bash
mkdir -p evaluation-results
LORE_ZOOM_COMPARISON_OUTPUT="$PWD/evaluation-results/zoom-synthetic-01.json" \
  cargo test --locked --test knowledge_zoom_comparison \
  measured_flat_direct_and_zoom_on_the_same_compiled_fixture \
  -- --exact --nocapture
```

The report is written only when `LORE_ZOOM_COMPARISON_OUTPUT` is set. Existing files are refused, and the report is capped at 8 MiB. The test prints the selected report path; it does not print source passages or a JSON report by default. To compare optimized retrieval timings, run the same command with Cargo's `--release` option and a separate report path. Do not compare debug and release timings as if they were the same build.

## What is scored independently

Expected statements, critical clauses and required qualifications are authored in the fixture, before either retrieval arm is called. The harness resolves those statements to the exact original registry IDs; it never derives expected answers from graph membership, the graph's critical classifier, parent summaries or returned search results. Its small scorer test also demonstrates that keeping an ID while dropping the exception, changing the scope or changing accepted to proposed does not retain the complete condition.

| Measurement | Definition and limits |
| --- | --- |
| `exact_knowledge_id_recall` | Required original knowledge IDs present in the returned records, with missing IDs listed |
| `original_source_evidence_recall` | Required original evidence IDs present with exact source revision, locator, excerpt and captured metadata; excerpt digests are checked against the registry |
| `complete_critical_condition_recall` | Required full statement and its exact knowledge ID, kind, lifecycle and scope retained together |
| `additional_record_count` | Returned originals outside the hand-authored required set; these may be useful context, so the count is not called an error or a precision score |
| `complete_cli_tokens` | The larger of the actual compact JSON plus newline and portable Markdown token counts, including evidence and metadata |
| `pretty_json_tokens_diagnostic` | Complete pretty JSON plus newline, measured separately; Zoom conservatively reserves this format internally, while the legacy flat budget covers compact CLI JSON and Markdown |
| `reported_used_tokens` | The entry point's own accounting, checked to bound both actual CLI formats and stay within the shared requested budget |
| `elapsed_retrieval_us` | All three elapsed entry-point times, plus their median; measured without a latency pass/fail threshold |
| `model_calls` | The returned successful-result counter, required to be zero; the compiled registry's model-call log must also remain unchanged |

Each successful response is checked independently of the graph validator. Every returned knowledge ID must resolve to an original registry record, its full statement and documented qualifications must be unchanged, and each of its cited evidence IDs must both belong to that record and resolve in the returned source manifest. Zoom's complete returned evidence snapshot is compared with `storage::evidence_snapshot`; the flat response is checked against the fields carried by its schema. Invalid sources, changed original claims, dangling links, duplicate records, oversized output or any reported retrieval model call fail the test.

Documentary relation objects are compared with the registry's original relation facts, including their source witnesses. Both endpoints and the witness must be present in the result. Zoom's full original-record object is compared with the retained record, including embedded evidence metadata and its current/historical flag. Source-revision digests, paths, status and roots are checked directly against stored revisions and **revision-specific captured provenance**; the manifest must exactly cover the returned evidence revisions. Summary knowledge IDs must resolve to selected originals, and the quoted original statements and qualifications must be present. Displayed navigation-edge endpoints must be present. These checks verify reference integrity and extractive retention, not the semantic quality of grouping or the truth of arbitrary prose.

Before saving a synthetic report, the harness corrupts a real returned source-revision digest, flips an embedded evidence activity flag, invents a relation witness and endpoint, and invents a summary reference. The independent checker must reject each mutation even though unaffected top-level evidence snapshots remain intact. The report records these probes and the number of relation witnesses, embedded evidence records, source revisions and summary references actually checked.

Recall is a **retention measurement for the supplied gold set**, not semantic truth. The fixture model mechanically assigns its declared type and lifecycle and uses a default `production` scope. Literal staging and future-intent qualifications remain in the original statements. This controlled fixture does not test whether a real extraction model chose the right labels.

An empty gold set has a `null` recall fraction, not a perfect score. The absent-identifier case exposes any additional returned records. Missing records or complete-condition groups are recorded even when an implementation correctly reports `partial`, `empty` or `budget_limited`. A retrieval error is retained as an error with no invented recall or latency-success result. The synthetic 8,000-token cases must actually execute both entry points successfully; they need not achieve a prescribed recall advantage.

## Interpret timing and negative results

Timing starts immediately before each retrieval call and stops when it returns. It excludes fixture compilation, independent citation scoring, report construction and report-file writing. The entry points' own token counting and packing remain included. Both arms bypass derived caches. Zoom's fresh full graph build and validation are therefore part of its time, and the comparison does not claim incremental graph-cache speed.

This compares **complete public entry points**, not an isolated graph ablation. The entry points differ in identifier handling, candidate selection, conservative condition grouping, manifests and presentation overhead. A recall difference cannot be attributed solely to DAG structure. For example, an exact stored ID may be supported by one entry point without being a searchable task term in the other.

The embedded tokenizer is initialized before timing. Arm order alternates by case and repeat. SQLite and operating-system page caches remain uncontrolled; these are not cold-cache measurements. Three repeats show local variation but do not establish a robust latency distribution. The report records the build profile, OS, architecture, relevant compiled source digests, dependency lockfile digest, retained source inventory and registry revision. Temporary paths, timestamps and generated IDs differ across independent fixture compilations; compare source content digests and case definitions before treating runs as comparable.

Interpret negative or inconclusive results directly:

- If both arms omit the expected group at a constrained budget, neither supplied the needed answer under that budget. The result does not show that the condition is absent from the project.
- If direct retrieval retains a singleton while Zoom cannot fit it, record the loss and inspect source and envelope overhead. A richer navigation result must justify its space with actual user value.
- If Zoom retains more conditions but costs more tokens or time, report both effects. Extra context and a correct source link alone do not demonstrate a better mental model.
- If both arms have the same source and condition recall, this fixture provides no recall evidence for preferring the graph.
- A faster or slower local median is a measurement of that build and fixture. It is not a general performance claim or a reason to loosen source-preservation requirements.
- A successful test with zero model calls verifies a deterministic local retrieval path. It establishes no improvement in model reasoning, user learning or task completion.

The structural [Knowledge Zoom contract tests](../tests/knowledge_zoom.rs) and [CLI tests](../tests/knowledge_zoom_cli.rs) answer different questions: graph invariants, original-evidence integrity, bounded selection, cache behavior and actual command contracts. Keep those results separate from this comparison's measured recall, size and latency.

## Recorded synthetic diagnostic

One executed run used Lore 0.6.0, the debug test profile on Linux x86-64, twelve compiled originals, eight cases, two budgets and three alternating repeats. All 96 timed calls completed without retrieval errors. Both arms reported zero model calls; the SQLite model-call log, registry revision and database bytes stayed unchanged. Returned documentary records and sources passed the independent checks, including the real disagreement witness, and all five deliberate manifest corruptions were rejected.

The selected rows below report **complete critical conditions retained / required**. Tokens are the larger actual CLI response format, and times are local medians in milliseconds. They are examples from this one synthetic run, not performance thresholds or promises for other projects.

| Query case | Budget | Flat critical | Zoom critical | Flat / Zoom tokens | Flat / Zoom median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Exact original ID | 1,500 | 0 / 1 | 1 / 1 | 198 / 1,167 | 6.46 / 83.17 |
| Exact symbol | 1,500 | 1 / 1 | 1 / 1 | 564 / 1,154 | 14.28 / 83.24 |
| Capacity and rare exception | 1,500 | 3 / 3 | 0 / 3 | 1,483 / 432 | 152.53 / 66.02 |
| Capacity and rare exception | 8,000 | 3 / 3 | 3 / 3 | 2,699 / 3,763 | 172.65 / 189.71 |
| Conceptual dispatch | 8,000 | 3 / 3 | 3 / 3 | 2,701 / 6,242 | 168.18 / 358.18 |
| Proposed staging scope | 1,500 | 2 / 2 | 0 / 2 | 1,372 / 1,163 | 144.65 / 134.66 |
| Proposed staging scope | 8,000 | 2 / 2 | 2 / 2 | 2,516 / 5,665 | 162.91 / 339.50 |
| Documented disagreement | 1,500 | 2 / 2 | 0 / 2 | 1,394 / 1,204 | 59.16 / 114.28 |
| Documented disagreement | 8,000 | 2 / 2 | 2 / 2 | 1,671 / 4,261 | 67.69 / 239.21 |

The exact-ID result reflects different entry-point identifier handling. Exact-symbol and source-path recall were equal, with additional Zoom output and retrieval work. At 8,000 tokens both arms retained every required record, source and condition in the other positive cases. The unrecorded identifier returned no originals from either arm and had no assessable recall denominator.

The constrained results are material negatives. Zoom withheld the complete dispatch group at 1,500 tokens. In the proposal and disagreement cases it returned other matching material while reporting a partial result, without the required gold conditions. The flat capacity result retained all three critical conditions but only three of four required original records: its separate purpose record did not fit. Thus even a perfect condition-retention fraction does not imply a complete task answer. These results establish no general recall or efficiency advantage for the graph, and no measured comprehension benefit.

The [full synthetic report](results/knowledge-zoom-synthetic-2026-10-09.json) is preserved from the explicitly requested output (SHA-256 `b1c4e538cfe501df1854da16b69e65fb1b1a0705490cb4528c59481a4c024ae3`). Its harness source digest is `blake3:73453326bca4b8b1d44ac163e10056b06ba19bbc8ed87eb5977bf7bc6ebdd89c`; the report also retains source and implementation fingerprints, all samples, omissions and required IDs. Re-run the command above to produce a new local report. Fresh fixture IDs can change token cost and tie-breaking near a packing limit, so compare both arms within each recorded snapshot and do not assume these exact counts reproduce across newly generated registries.

## Run an explicitly selected, already compiled project

The ignored `measured_existing_compiled_project` test reuses the same measurement and independent scoring functions. It accepts a selected project configuration and a human-authored JSON array of query cases. It opens the existing registry read-only; it does not initialize or update the project, contact a model, inspect checkout files or create a derived graph cache.

Review original documents and `lore evidence` before authoring the required IDs and conditions. Do not copy a query's output into its expected set: that would measure agreement with the implementation. For example, replace every illustrative ID and statement below with independently checked records from that project:

```json
[
  {
    "id": "reviewed-retry-condition",
    "query": "MAX_BACKOFF_MS",
    "budgets": [1500, 8000],
    "expected_knowledge_ids": ["k_REPLACE_WITH_ORIGINAL_ID"],
    "expected_evidence_ids": ["e_REPLACE_WITH_ORIGINAL_ID"],
    "critical_conditions": [
      {
        "knowledge_id": "k_REPLACE_WITH_ORIGINAL_ID",
        "statement": "Replace this with the entire original statement, including its exception.",
        "kind": "decision",
        "lifecycle": "accepted",
        "scope": "production"
      }
    ]
  }
]
```

Then run:

```bash
LORE_ZOOM_COMPARISON_CONFIG=/absolute/project/lore.yml \
LORE_ZOOM_COMPARISON_CASES=/absolute/reviewed-cases.json \
LORE_ZOOM_COMPARISON_OUTPUT=/absolute/new-zoom-comparison.json \
  cargo test --release --locked --test knowledge_zoom_comparison \
  measured_existing_compiled_project -- --ignored --exact --nocapture
```

The harness rejects unknown gold IDs, duplicated case names or expected IDs, and critical text or qualifications that disagree with the selected registry. It accepts at most 16 cases, four budgets per case, 128 required knowledge IDs and evidence IDs per case, 5,000 current registry records and 32 MiB of retained evidence text. The case file is capped at 256 KiB. All calls run under one SQLite read snapshot, so both arms see the same compiled inputs even if another process later publishes new project data.

Reports include the selected source inventory, original IDs and full gold conditions. Keep the output in a location appropriate for the selected project. Native import observations may occupy space in direct context, but the current Zoom graph organizes documentary knowledge; this scorer evaluates documentary records only. Include that difference when designing real tasks and interpreting recall or output size. Projects without independently reviewed gold cases can supply descriptive timing and output-size observations, but not a reviewed completeness score.

No real-project, real-model or human-learning result is implied by the bundled synthetic run. Preserve separate reports for any such measurements, with the reviewed case manifest and exact build identity.
