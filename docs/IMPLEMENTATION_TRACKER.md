# Shared project intelligence implementation tracker

Baseline: merged PR [#24](https://github.com/grove/lore/pull/24), commit
`4197ba345de5ff7ec1bad17ab1f8349803b47783` (Lore 0.6). The design documents
describe product outcomes; this file records implementation and measured status.

| Milestone / roadmap | Work package and modules | Functional gate | Validation | Status |
| --- | --- | --- | --- | --- |
| M0 / R0 | Preserve existing CLI, evidence, ingestion, imports and evaluation baseline | Schemas 2/3/4 and `--fast`; no-op; no source mutation | Existing Rust suite; 79 Python evaluation tests; five corpus preparation commands | Python baseline passed; Rust baseline environment being established |
| M0 / A1–A2 / G1 | `context::adaptive`, existing decision runtime and inspection | Explicit schema 5; bounded automatic work within host grants; source-independent snapshot identity | 7 adaptive tests plus 20 decision-runtime tests passed | Core implemented; CI pending |
| M0 / O1–O2 | `experience` orientation, concepts and workflow | Human and agent consume same evidence, observations and authority; immediate useful orientation | Human experience integration tests and shared corpus | Implementing |
| M1 / O1–O3 | Guided activity, hints, first task and distinct transfer activity | Purposeful optional learning, accurate source-linked feedback, no inferred mastery | Human protocol and fixtures; independent participant outcomes separately required | Implementing |
| M2 / A3 / G1 | Revision-bound investigative reuse and decision conditions | Revalidate old observations and newly relevant evidence; never promote derived findings to policy | 4 retention tests and decision-runtime mutation/reuse tests passed | Reuse implemented; lenses in progress |
| M3 / R1–R4 | `knowledge` DAG and human presentation modes | Variable depth, multiple parents, preserved conditions, direct evidence bypass, bounded work | Diamond/deep DAG and rare-condition tests; flat retrieval comparison | Implementing |
| M4 / R5–R7 | Baseline change intelligence, advisory findings and static cases | Explicit baseline, consequential changes, useful scoped guidance without arbitrary execution | Mutation and advisory precision fixtures | Pending |

## Observed baseline

- `python3 -m unittest discover -s evaluation/tests -v`: **79 passed** on the
  untouched baseline. This verifies assessment and fixture mechanics.
- Existing no-inference `prepare` commands for suite, benchmark `llm-wiki`,
  cross-source, coding tasks and decision tasks: **passed**.
- Initial shared-core gate: `cargo fmt --all -- --check` and
  `cargo test --test adaptive_context --test decision_runtime --test investigation_memory --locked`:
  **31 passed** using Rust 1.90 and the system linker in the local executor.
- No human onboarding study, hosted-model quality experiment or agent
  productivity comparison is implied by these results.

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
