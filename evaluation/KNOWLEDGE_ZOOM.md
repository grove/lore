# Measure direct retrieval against Knowledge Zoom

`tests/knowledge_zoom_comparison.rs` executes both existing retrieval entry points against the same compiled SQLite snapshot and the same query and token budget:

| Arm | Executed code | Included retrieval work |
| --- | --- | --- |
| Flat/direct | `context::build_context` | Deterministic lexical and recorded-relationship retrieval, source resolution, complete-response packing |
| Knowledge Zoom | `knowledge::explore` | The same direct candidate substrate, a fresh documentary graph build and validation, cross-level selection, source resolution, complete-response packing |

This is an actual retrieval comparison. The default corpus is a **synthetic diagnostic fixture** compiled through the ordinary update engine with `tests/common::FakeModel`. It does not measure inference quality, human comprehension, learning transfer or coding outcomes. The 0.7 regression gate requires compact Zoom to preserve the critical conditions that flat retrieval retains and all three documented constrained-budget failures. It does not require graph superiority or lower latency.

The synthetic test creates twelve documents in a temporary project, compiles the registry, and runs eight hand-authored query cases at 1,500 and 8,000 tokens. Each arm runs three times per case and budget: the original operational comparison still makes 96 measured calls. The 0.7 extension adds 48 public compact calls and 96 controlled-ablation calls, plus separate graph-cache measurements and two unmeasured validation retrievals. Cases cover an exact original ID, exact symbol, exact source path, capacity changes with a rare exception, conceptual orientation, an explicitly proposed change, a documented disagreement with its original relationship witness, and an identifier absent from the fixture. The same test target also runs the separate 24-case, three-repository source corpus described below.

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

Before saving a synthetic report, the harness corrupts a real returned source-revision digest, flips an embedded evidence activity flag, invents a relation witness and endpoint, invents a summary reference, changes lifecycle and scope, shortens a qualified statement, removes the relation manifest, and substitutes an unrelated historical revision. All ten manifest mutations must fail. An eleventh probe inflates an actual compact response while falsely understating its used tokens. The whole-output budget check must reject it. These probes deliberately catch validator panics; with `--nocapture`, an expected caught assertion can appear in the log while the test still passes. The report records the probes and the relation witnesses, embedded evidence records, source revisions and summary references actually checked.

Recall is a **retention measurement for the supplied gold set**, not semantic truth. The fixture model mechanically assigns its declared type and lifecycle and uses a default `production` scope. Literal staging and future-intent qualifications remain in the original statements. This controlled fixture does not test whether a real extraction model chose the right labels.

An empty gold set has a `null` recall fraction, not a perfect score. The absent-identifier case exposes any additional returned records. Missing records or complete-condition groups are recorded even when an implementation correctly reports `partial`, `empty` or `budget_limited`. A retrieval error is retained as an error with no invented recall or latency-success result. The synthetic 8,000-token cases must actually execute both entry points successfully; they need not achieve a prescribed recall advantage.

## Interpret timing and negative results

Timing starts immediately before each retrieval call and stops when it returns. It excludes fixture compilation, independent citation scoring, report construction and report-file writing. The entry points' own token counting and packing remain included. Both arms bypass derived caches. Zoom's fresh full graph build and validation are therefore part of its time, and the comparison does not claim incremental graph-cache speed.

The original `measurements` section compares **complete public entry points**. The entry points differ in identifier handling, candidate selection, condition grouping, manifests and presentation overhead. A recall difference there cannot be attributed solely to DAG structure. For example, an exact stored ID may be supported by one entry point without being a searchable task term in the other. The separate `graph_ablation` section in 0.7 controls candidates, obligation closure, compact schema and packing; its narrower interpretation is described below.

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

The harness rejects unknown gold IDs, duplicated case names or expected IDs, and critical text or qualifications that disagree with the selected registry. It accepts at most 96 cases, five budgets per case, 128 required knowledge IDs and evidence IDs per case, 5,000 current registry records and 32 MiB of retained evidence text. The case file is capped at 256 KiB. All calls run under one SQLite read snapshot, so both arms see the same compiled inputs even if another process later publishes new project data.

Reports include the selected source inventory, original IDs and full gold conditions. Keep the output in a location appropriate for the selected project. Native import observations may occupy space in direct context, but the current Zoom graph organizes documentary knowledge; this scorer evaluates documentary records only. Include that difference when designing real tasks and interpreting recall or output size. Projects without independently reviewed gold cases can supply descriptive timing and output-size observations, but not a reviewed completeness score.

No real-project, real-model or human-learning result is implied by the bundled synthetic run. Preserve separate reports for any such measurements, with the reviewed case manifest and exact build identity.

## 0.7 controlled comparison and reviewed regression corpus

The 0.7 harness retains the original flat/schema-1 comparison and adds two
separate measurements. `compact_operational` calls the public schema-2 compact
entry point. `graph_ablation` supplies **every original candidate**, never the
gold answer set, to `select_compact_controlled` under `DirectOnly` and
`GraphGuided`. Both modes share the exact candidate digest, required documentary
closure, compact output schema, token limit and whole-response packer. The
controlled change is graph ordering and optional navigation.

The legacy flat/schema-1 arms alternate within their measurement block. The
public compact arm runs in a separate later block, followed by the controlled
ablation, whose two arms also alternate. Compact-versus-legacy elapsed times are
therefore descriptive comparisons of separate blocks, not an interleaved
three-arm timing experiment. The paired graph/direct timing deltas below come
from the controlled ablation.

Both controlled arms build and validate the same graph as common setup. This
isolates the effect of ordering/navigation; it is **not** a graph-free total-cost
comparison. The operational measurements separately charge full uncached graph
construction. `graph_cache_cost_samples` retains three cold population and
validated replay samples, their sum, and the actual reuse/validation counters.
Graph construction is never hidden by timing only selection. Each compact arm
retains all three samples, p50 and p95. With three observations the reported p95
is the sample maximum, not a reliable population tail estimate. Filesystem and
SQLite page caches remain uncontrolled.

The independent `mandatory_v1_payload_measurements` constructs exact gold
critical originals, all required documentary relation endpoints and witnesses,
and original revision provenance. It removes optional navigation and records
compact JSON, pretty JSON, portable Markdown and schema-1 reservation costs.
Metadata numeric widths are reserved. This measures a **canonical schema-1
payload containing mandatory evidence** with optional summaries and warnings
removed; it is not a mathematical lower bound for every possible encoding.
In particular, normalized schema 2 can fit evidence that the repeated schema-1
envelope cannot. An impossible schema-1 payload remains a limitation; it is never
scored as a retained condition.

### Source pins and review

The [manifest](corpora/zoom-reviewed-v1/manifest.json) contains eight exact
documentary cases from each of three public projects, with upstream paths,
line spans, full excerpts, source digests and explicit critical qualifications:

| Project | Pinned commit | Cases |
| --- | --- | ---: |
| ripgrep | `3fce3b5bb0236da2df6d99672afb8a719642eca7` | 8 |
| fd | `14dcd92fb76ca0ebc2e82671a275f67c790d25fc` | 8 |
| jq | `b904884b94ca48e025a12697ab172711e6c134d8` | 8 |

The manifest SHA-256 is
`8ea3a80daa703deced518119261810a215cb295826c2279865aa06a079bc0088`.
The [review capture](corpora/zoom-reviewed-v1/review.json) binds that exact
manifest. A separate AI engineering reviewer checked every quotation, upstream
locator and query qualification without inspecting selector outputs. This is
an independent source review within the implementation session, **not** an
independent human study. Its findings were corrected before final measurement.

These cases exposed option/path tokenization, acronym dependency and relevance
ranking defects, so the final corpus is **development/regression gold**, not an
unseen holdout. The 24 source cases principally cover documented current rules,
exceptions and unrelated noise. Separate synthetic and Rust tests cover exact
original IDs, proposals, historical transitions, relation witnesses, withdrawals
and multiple-parent navigation; those dimensions are not all independently
represented in the three-project documentary set.

Exact excerpts are compiled through the ordinary update engine by a deterministic
capture adapter. Provider requests are zero. The test checks the bundled excerpt
digests, declared commit format and binding between the manifest and review
receipt, then validates returned records/revisions directly against the compiled
registry. Verification against complete upstream files and line spans is recorded
in the separate source-review receipt; the test does not fetch upstream commits.
It does not test a real extraction model, upstream executable behavior, human
navigation utility or coding success.

### Recorded reviewed results

The [dated raw report](results/knowledge-zoom-07-reviewed-2026-10-10.json) contains
three complete per-project reports. There are 720 timed retrieval calls across
the original comparison, public compact response and controlled ablation, plus
separate graph-cache measurements. All returned evidence and token checks pass,
and every result reports zero retrieval model calls.

| Budget | Flat complete critical cases | Schema-1 Zoom | Compact schema-2 Zoom |
| --- | ---: | ---: | ---: |
| 1,500 | 23 / 24 | 17 / 24 | 24 / 24 |
| 8,000 | 24 / 24 | 24 / 24 | 24 / 24 |

Each case has one independently specified complete statement with its original
kind, lifecycle, scope and qualifications. A full critical score does not mean
all otherwise relevant material fits: compact responses can correctly retain
the gold condition and still report partial coverage. Schema 1 is preserved
for compatibility and still has envelope/selection limitations even when the
canonical gold-only payload is feasible. `--compact` is explicit.

The controlled arms retained the same required critical conditions in all 48
case/budget pairs. Graph ordering/navigation added a median **41.5 tokens** and
**34.14 ms** at 1,500 tokens, and **1,933.5 tokens** and **146.67 ms** at 8,000.
These are medians of paired per-case differences in this debug Linux run.
There is **no measured graph-induced critical-recall advantage** here. The new
selector limits mandatory evidence closure to applicable conditions and
documentary dependencies, then packs complete support before navigation. Graph
membership is only the final relevance tie-breaker; it cannot remove original
candidates or turn topic membership into a mandatory evidence group. Callers
explicitly choose `lore explore` for navigation, and this result does not promote
it over the existing context path or change defaults. The retention improvements
support the condition grouping, ranking and compact packing changes, not a claim
that graph guidance caused the gains. Human navigation utility remains `null`.

Retain a new reviewed report with:

```sh
LORE_ZOOM_REVIEWED_OUTPUT=/absolute/new-reviewed-report.json \
  cargo test --locked --test knowledge_zoom_comparison \
  source_corpus::measured_source_pinned_three_repository_cases -- --exact
```

The [focused replay](../tests/knowledge_zoom_reviewed.rs) rechecks all 24
unchanged conditions at 1,500 tokens without timing repeats. It complements,
and does not replace, the full raw report and causal comparison.

### Reproduce the same baseline registry

The [frozen synthetic fixture](corpora/zoom-frozen-v1/README.md) includes the exact
public synthetic SQLite bytes, twelve authored sources and eight gold cases
captured at baseline commit `6f4898bf1d6e0dc135a24ef792921c3d98af2c24`.
Its decoded database SHA-256 is
`6ff2b2b328b65af775902512350ce3125083d2e6441849258ca7757b8248da55`.
The restoration tool validates every source and database digest, refuses an
existing destination, and writes a provider-disabled portable configuration.
Do not run `lore update` before comparison: the purpose is to reuse identical
compiled originals, IDs and source revisions.

```sh
python evaluation/restore_zoom_fixture.py --output /tmp/lore-07-frozen
LORE_ZOOM_COMPARISON_CONFIG=/tmp/lore-07-frozen/lore.yml \
LORE_ZOOM_COMPARISON_CASES=/tmp/lore-07-frozen/cases.json \
LORE_ZOOM_COMPARISON_OUTPUT=/tmp/new-frozen-comparison.json \
  cargo test --locked --test knowledge_zoom_comparison \
  measured_existing_compiled_project -- --ignored --exact
```

The [baseline capture](results/knowledge-zoom-07-baseline-frozen-2026-10-10.json)
and [0.7 replay](results/knowledge-zoom-07-after-frozen-2026-10-10.json) use that
same registry. The earlier October 9 report remains unchanged; its separately
generated UUIDs affect tight packing and it must not be substituted for this
matched baseline. The [release scorecard](PRODUCT_QUALITY_07.md) records the
matched outcomes, source/build provenance, artifact digests and limitations.
