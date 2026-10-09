# Shared project intelligence implementation tracker

Baseline: merged PR [#24](https://github.com/grove/lore/pull/24), commit
`4197ba345de5ff7ec1bad17ab1f8349803b47783` (Lore 0.6). The design documents
describe product outcomes; this file records implementation and measured status.

| Milestone / roadmap | Work package and modules | Functional gate | Validation | Status |
| --- | --- | --- | --- | --- |
| M0 / R0 | Preserve existing CLI, evidence, ingestion, imports and evaluation baseline | Schemas 2/3/4 and `--fast`; no-op; no source mutation | Existing Rust suite; 79 Python evaluation tests; five corpus preparation commands | Baseline CI passed on Linux, macOS and Windows |
| M0 / A1–A2 / G1 | `context::adaptive`, existing decision runtime and inspection | Explicit schema 5; bounded automatic work within host grants; source-independent snapshot identity | 7 adaptive tests plus 20 decision-runtime tests passed | Core implemented; CI pending |
| M0 / O1–O2 | `experience` orientation, concepts and workflow | Human and agent consume same evidence, observations and authority; immediate useful orientation | 23 human engine tests and 6 actual CLI tests; shared evidence/snapshot comparison | Implemented; CI integration pending |
| M1 / O1–O3 | Guided activity, hints, first task and distinct transfer activity | Purposeful optional learning, accurate source-linked feedback, no inferred mastery | Human protocol and executable distinct transfer fixture; 90 Python evaluation tests | Implemented mechanics; independent participant outcomes unmeasured |
| M2 / A3 / G1 | Revision-bound investigative reuse and decision conditions | Revalidate old observations and newly relevant evidence; never promote derived findings to policy | 4 retention tests and decision-runtime mutation/reuse tests passed | Reuse implemented; lenses in progress |
| M3 / R1–R4 | `knowledge` DAG and human presentation modes | Variable depth, multiple parents, preserved conditions, direct evidence bypass, bounded work | Diamond/deep DAG and rare-condition tests; flat retrieval comparison | Implementing |
| M4 / R5–R7 | Baseline change intelligence, advisory findings and static cases | Explicit baseline, consequential changes, useful scoped guidance without arbitrary execution | Mutation and advisory precision fixtures | Pending |

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
| ON-09 | Knowledge Zoom workstream; direct and multiple-parent traversal tests |
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
