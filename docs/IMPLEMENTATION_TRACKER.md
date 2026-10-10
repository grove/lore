# Shared project intelligence implementation tracker

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

## Observed baseline

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

## Integrated engineering verification — 2026-10-10

The combined implementation and existing-code lint cleanup passed:

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

## Product evaluation status

Human learning and coding-agent outcomes are independent gates. Contract tests
and synthetic source fixtures establish neither learning transfer nor improved
coding productivity. Real participants, independently verified changes and
comparable model/tool budgets must be recorded before claiming those outcomes.
No full-program completion claim is made while those gates remain unmeasured.

The Knowledge Zoom comparison records a direct-ID recall gain and several losses
in complete critical-condition coverage at a 1500-token budget. The optional
exploration command does not replace the established flat retrieval path. These
results do not satisfy M3's comparative acceptance gate at constrained budgets,
and the comparison of complete entry points does not isolate a causal benefit
from the graph alone.
