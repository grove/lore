# Lore 0.7 implementation tracker

Current implementation: **Lore 0.7 — evidence-driven usefulness**. The
[0.7 guide](V07.md) describes the available commands and compatibility boundaries;
the [release scorecard](../evaluation/PRODUCT_QUALITY_07.md) is the current record
of integrated validation, release receipts, and measured versus unmeasured
outcomes. This tracker preserves the earlier 0.6 evidence separately below.

## 0.7 work packages and evidence

The seven packages are implemented through [PRs #33–39](../evaluation/PRODUCT_QUALITY_07.md#ship-decision).
The linked PRs carry their exact head, three-platform checks and merge receipts.
Local integration passed **427 Rust tests** (3 explicitly ignored), **174 Python
tests**, **13 offline preparation/validation commands**, strict lint, formatting
and all 11 CLI checks. The scorecard separates these gates from unmeasured
model and human outcomes.

| Plan package | Implemented work | Evidence and remaining boundary |
| --- | --- | --- |
| #33 — Knowledge Zoom reliability | Select complete relevant evidence obligations before optional navigation; retain original conditions, history, citations, and relationship endpoints; explicit compact exploration response and budget limitations | Constrained-budget regression checks and complete JSON/Markdown budget coverage. Plain exploration schema 1 remains unchanged; schema 2 requires `--compact`. |
| #34 — Retrieval evaluation | Frozen source cases, independently checked source bindings, exact gold conditions, and reproducible comparison/attribution | Twenty-four pinned ripgrep/fd/jq cases have a separate AI-agent source-gold review. This is retrieval regression evidence, not independent human usefulness evidence or an isolated causal graph benefit. See [methods and results](../evaluation/KNOWLEDGE_ZOOM.md). |
| #35 — Coding-agent benchmarks | Thirty public candidate repairs, six matched configurations, independent executable checker controls, bounded repair loops, and complete cost accounting | Candidate preparation and engineering controls are available. Real-model task success, end-to-end time/cost, independent task review, and held-out outcomes remain separate gates. See [real coding tasks](../evaluation/REAL_CODING_TASKS.md). |
| #36 — Agent intelligence improvements | Source-bound failure triage and runtime-change validation; related-task, source-change, and permission-narrowing sequence protocol | Runtime improvement claims require actual pilot failures, exact evidence, cost risk, and a holdout plan. An unrun study does not justify a speculative adaptive algorithm. See [failure triage](../evaluation/FAILURE_TRIAGE.md). |
| #37 — Human onboarding | Counterbalanced twelve-participant protocol, explicit opt-in pseudonymous records, independently checked first contribution, and distinct learning transfer | Study infrastructure and debugging exercises are implemented; participants, first-contribution outcomes, learning transfer, and observed UX benefits remain unmeasured. See [human onboarding](../evaluation/HUMAN_ONBOARDING.md). |
| #38 — Project guardian | Current support separate from immutable history, A→B→A condition reversion, accepted-topic priority, complete grouped risk/blocker actions, and sixty successive source transitions | Targeted reversion and compatibility checks passed. Both full actual CLI captures failed source integrity when deleted paths reappeared; no full longitudinal quality pass or independent alert-precision claim. See [protocol](../evaluation/GUARDIAN_LONGITUDINAL.md) and [captured results](../evaluation/results/guardian-debug-2026-10-10/README.md). |
| #39 — Release validation | Integrated regression/compatibility validation, usage documentation, measured-outcome scorecard, and release preparation | Aggregate gate results and any remaining limitations are recorded in the [0.7 scorecard](../evaluation/PRODUCT_QUALITY_07.md). Engineering checks do not replace real-model or human outcomes. |

Existing context schemas 2/3/4 retain their contracts. Schema 5 remains explicitly
selected, and compact exploration is opt-in. The work reuses the retained
registry and shared intelligence engine. It grants no repository-command
execution authority to sources, imported records, or model responses.

### Guardian regression and capture status

The guardian's current-support regression preserves a restored condition after
A→B→A even when its short summary and cumulative historical quote set remain
unchanged. Older baseline checksums still load. Eighteen companion integration
tests, two actual CLI tests, three advisory grouping units, two corpus/export
tests, and twenty-eight guardian Python protocol tests passed locally. Distinct
actions with overlapping or negated text are retained rather than erased by a
substring deduplication.

The baseline and candidate full debug captures each covered all sixty events.
They reported 20 versus 17 no-documented-change results, respectively, but had
32 versus 15 failed source-integrity events. Every other captured mechanical
check passed. A candidate ten-event payments segment passed all checks; a short
baseline capture included `.rsync-tmp/history.md` before a deleted source path
reappeared. The writer was not independently identified. The full failures
remain in the archived reports and block their integrity gate. Provider calls
were zero, alert precision and recall are unmeasured, and independent syscall
audit was unavailable. These results support targeted debugging, not a claim of
validated longitudinal guardian usefulness.

## Historical 0.6 shared-intelligence implementation

Baseline: merged PR [#24](https://github.com/grove/lore/pull/24), commit
`4197ba345de5ff7ec1bad17ab1f8349803b47783` (Lore 0.6). The design documents
describe product outcomes; this file records implementation and measured status.

| Milestone / roadmap | Work package and modules | Functional gate | Dependencies | Validation | Status |
| --- | --- | --- | --- | --- | --- |
| M0 / R0 | Preserve existing CLI, evidence, ingestion, imports and evaluation baseline | Schemas 2/3/4 and `--fast`; no-op; no source mutation | Merged #24 | Existing Rust suite; 79 baseline Python tests; five baseline preparation commands | Baseline CI passed on Linux, macOS and Windows |
| M0 / A1–A2 / G1 | `context::adaptive`, existing decision runtime and inspection | Explicit schema 5; bounded automatic work within host grants; whole-registry identity independent of selected context | Existing retained registry, decision controller and static inspector | 11 adaptive tests, actual CLI contracts and decision-runtime regressions | Merged in [#25](https://github.com/grove/lore/pull/25); three-platform CI passed |
| M0 / O1–O2 | `experience` orientation, concepts and workflow | Human and agent consume the same evidence, observations and authority; immediate useful orientation | Shared core #25 | 23 human engine tests and 6 actual CLI tests; shared evidence/snapshot comparison | Merged in [#26](https://github.com/grove/lore/pull/26); three-platform CI passed |
| M1 / O1–O3 | Guided activity, hints, first task and distinct transfer activity | Purposeful optional learning, source-bound fallible feedback with pinned lessons, no inferred mastery | Shared core and human experience #25–26 | Human protocol, lesson/source mutation tests and executable distinct transfer counterexample | Implemented in [#26](https://github.com/grove/lore/pull/26); independent participant outcomes unmeasured |
| M2 / A3 / G1 | Revision-bound investigative reuse, `insights` decision lenses and source-owned cases | Revalidate old observations and newly relevant evidence; retain scope, history and negative cases without promoting reports to policy | Shared core, exact original evidence and existing immutable history | 4 retention tests, decision-runtime mutation/reuse tests, 12 decision/case tests and actual CLI coverage | Merged in [#25](https://github.com/grove/lore/pull/25) and [#27](https://github.com/grove/lore/pull/27); three-platform CI passed; productivity outcomes unmeasured |
| M0 / M2 / M4 / G1 | Shared `source_relationships` and human source adapter | Retain original qualifications, current review disposition, complete endpoint evidence and exact revisions through presentation and packing | Shared core #25, human adapter #26 and native/documentary relationship readers | 4 source-manifest tests plus engine/CLI qualification, whole-manifest deletion, tamper and full-budget regressions | Merged in [#29](https://github.com/grove/lore/pull/29); three-platform CI passed |
| M2 / A3 / AG-18 | `evaluation/adaptive_tasks.py` matched actual coding-task runner | Six isolated arms, bound grants/snapshots/requests, executable independent checks and charged warm-up/replay | Existing coding/decision protocols and source manifest #29 | 11 adaptive assessor tests within 106 passing Python tests; actual offline agent/checker subprocesses | Merged in [#30](https://github.com/grove/lore/pull/30); three-platform CI passed; live-model productivity, distinct-task reuse and correction loops unmeasured |
| M3 / R1–R4 | `knowledge` DAG, incremental derived views and human presentation modes | Variable depth, multiple parents, preserved conditions, direct evidence bypass, bounded source/topology validation | Existing documentary registry/retrieval and shared snapshot; no separate graph database | 28 graph tests, 4 actual CLI tests and 2 comparison/scorer tests; [96-call synthetic comparison](../evaluation/KNOWLEDGE_ZOOM.md) | Merged in [#28](https://github.com/grove/lore/pull/28); three-platform CI passed; constrained-budget comparative acceptance is not met |
| M4 / R5–R7 | `companion` explicit baselines, consequential changes and guardian | Original historical evidence, native qualifications and advisory investigation remain source-bound and budgeted | Shared core #25, decisions/cases #27 and source manifest #29; no graph dependency | 14 companion tests and 2 actual CLI tests; immutable old support, native status, interpretation withdrawal, permission and whole-group budget regressions | Merged in [#31](https://github.com/grove/lore/pull/31); three-platform CI passed; longitudinal alert precision and burden unmeasured |

## Historical observed baseline

- The exact merged baseline passed [three-platform CI](https://github.com/grove/lore/actions/runs/37998586894)
  at `4197ba345de5ff7ec1bad17ab1f8349803b47783`. This includes the full offline
  Rust test command (Linux: **288 passed, 2 explicitly ignored live-provider
  tests**), Python assessment tests and Linux corpus preparation.

- `python3 -m unittest discover -s evaluation/tests -v`: **79 passed** on the
  untouched baseline. This verifies assessment and fixture mechanics.
- Existing no-inference `prepare` commands for suite, benchmark `llm-wiki`,
  cross-source, coding tasks and decision tasks: **passed**.
- Initial shared-core gate: `cargo fmt --all -- --check` and
  `cargo test --test adaptive_context --test decision_runtime --test investigation_memory --locked`:
  **31 passed** using Rust 1.90 and the system linker in the local executor.
- No human onboarding study, hosted-model quality experiment or agent
  productivity comparison is implied by these results.

## Historical 0.6 integrated engineering verification — 2026-10-10

The pre-0.7 combined implementation and existing-code lint cleanup passed the
following recorded gates. These counts are retained as historical evidence;
current 0.7 aggregate counts belong in the release scorecard.

| Gate | Recorded local result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo test --all-targets --locked` | **402 passed, 0 failed, 3 explicitly ignored**, across 53 test targets |
| Strict Clippy, all targets and locked dependencies | Passed with zero warnings/errors through the exact Rust 1.90 Clippy driver |
| `python3 -m unittest discover -s evaluation/tests -v` | **106 passed** |
| No-inference `prepare` commands | All seven passed: suite, `llm-wiki` benchmark, cross-source, coding, decision, shared and adaptive protocols |
| `git diff --check` | Passed |

Two ignored tests require a live Ollama or OpenAI provider. The third requires
an explicitly selected compiled project and independently authored comparison
cases. None is counted as a passing provider or real-project experiment.

The local executor's `cargo-clippy` frontend cannot resolve `/proc/self/exe`.
The matching Clippy driver ran every target with `-D warnings`; this was an
actual Clippy gate, not a plain compiler substitute. The CI workflow uses the
standard `cargo clippy --all-targets --locked -- -D warnings` command on Linux,
macOS and Windows. It retains the existing formatting, complete Rust/Python,
corpus preparation and actual CLI help checks.

The cache replay regression verifies that optional output can be pruned while
the full draft remains reusable, mandatory source-bound premises survive, and
replay makes zero model calls within the complete output budget. It does not
require cold and cached output to have different optional packing on every run.

## Human integration checks

`human_experience` exercises real retained registry evidence with deterministic
provider doubles. `human_experience_cli` invokes the built executable and checks
the shared registry identity against schema 5, complete JSON/Markdown budgets,
exact evidence drill-down, direct task entry, denied capabilities, argument
errors before configuration loading, and preserved source/wiki/state bytes.

The source-derived presentation cache stores no learner answers or feedback.
Hints and feedback can be pinned to the returned lesson revision; changed
lessons receive an ungraded source comparison. Cancellation releases the read
snapshot. The learning protocol keeps actual human first-contribution and
distinct second-task results separate from coding-agent outcomes.

| Design scenarios | Executable coverage / remaining gate |
| --- | --- |
| AT-01, AG-01 | `adaptive_context::exact_retained_constant_bypasses_model_checkout_and_cache` |
| AT-02–04, AT-07–10, AT-12, AT-15–16, AT-19, AT-23, AG-03–10, AG-13 | Existing `decision_runtime`, `decision_contract`, `investigation`, `checkout_inspection` and CLI suites retain their decision, budget, static-observation and fallback gates |
| AT-11, AT-13, AT-20–22, AT-24–25, AG-11–12, AG-14–15, AG-17 | Whole-registry invalidation, host-grant, source mutation, restricted reuse, retention and legacy CLI regressions |
| AT-14, ON-24 | Dropped model futures release read snapshots; human cancellation and provider-unavailable fallback tests |
| AT-17, ON-01–08, ON-10–21 | `human_experience` and `human_experience_cli`: optional learning, real task bypass, references, source qualifiers, cycles, lesson binding, changed-code fallback and privacy |
| ON-09 | `knowledge_zoom` and `knowledge_zoom_cli`: direct references, multiple parents, depth beyond five levels, complete conditions, bounded navigation and source drill-down |
| ON-22–23 | `evaluation/tests/test_learning_transfer.py` tests distinct task mechanics and leakage barriers; actual participant competence and reduced assistance remain unmeasured |
| AG-02, AG-16 | Existing lexical/semantic retrieval plus isolated human/agent presentation and shared snapshot checks; real model relevance still needs evaluation |
| AG-18 | Existing coding-task experiment plus shared collector and independent assessment sheets; comparative productivity remains unmeasured |
| AT-05 | Optional isolated execution capability is not implemented; static work remains available under AT-06 |
| AT-18, AT-26 | Existing material-blocker and safe-progress contracts; alert precision and usefulness need independent scenario review |

## Engineering decisions

Use one retained SQLite registry and the existing provider, retrieval, decision,
static inspection, exact evidence and publication components. A new response
schema explicitly selects automatic investigation. Existing response schemas
retain their defaults. Derived views and findings are disposable and never
become accepted project knowledge.

The new experience separates the invoking host's grants from repository
configuration. Existing `privacy.local_only` and explicit `--no-inspect`
restrictions remain ceilings. No command starts repository processes or gives
repository text execution authority.

## Product evaluation status and historical comparison

Human learning and coding-agent outcomes are independent gates. Contract tests
and synthetic source fixtures establish neither learning transfer nor improved
coding productivity. Real participants, independently verified changes and
comparable model/tool budgets must be recorded before claiming those outcomes.
No full-program completion claim is made while those gates remain unmeasured.

The historical Knowledge Zoom comparison recorded a direct-ID recall gain and
several losses in complete critical-condition coverage at a 1500-token budget.
Those 0.6 results remain available and did not satisfy the constrained-budget
comparative gate. The 0.7 work adds complete-obligation selection, an explicit
compact envelope, frozen regressions, and the separate 24-case source-gold
review. The [current methods and results](../evaluation/KNOWLEDGE_ZOOM.md) and
[scorecard](../evaluation/PRODUCT_QUALITY_07.md) distinguish that new evidence from
the old failure record. Optional exploration still does not replace established
flat retrieval, and comparing complete entry points does not isolate a causal
benefit from the graph alone.
