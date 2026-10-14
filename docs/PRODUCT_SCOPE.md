# Lore product scope — decision-aware task guidance

> **Status:** This is the authoritative *product-scope contract* for the focus/reduction milestone ([#65](https://github.com/grove/lore/issues/65)). It describes what Lore should retain and what belongs elsewhere. **It is not a declaration that planned removals have shipped.** The current implementation is 0.9.0-rc.1; use [the 0.9 guide](V09.md) and [engineering scorecard](../evaluation/PRODUCT_QUALITY_09.md) for implemented interfaces and validation status. Historical design proposals are not competing product contracts.

## One job, one primary workflow

**Primary users:** Developers and coding agents about to make a code or architecture change; maintainers reviewing the implications of a change use the same evidence.

**Question Lore owns:** *Given this task, which documented decisions and constraints apply, why do they exist, which alternatives or negative cases matter, how does the observed implementation compare, and what is the next defensible action?*

**Primary promise:** A source-backed, task-specific recommendation, not a knowledge portal or code-writing agent. Prefer one actionable next step, with readiness/blockers, exact citations, relevant history and rejected approaches, material conflicts, bounded observations and verification still required. State when evidence is incomplete. A recommendation is **advice**, not approval, authorization, or proof that code behaves as documented.

### One-task happy path

Run in the project directory, before editing code:

```sh
lore context "Add payment retry diagnostics without changing approved retry limits"
lore --json context "Add payment retry diagnostics without changing approved retry limits"
```

Unpinned `context` works **without** configuration or a model: first contact is bounded, read-only, source-only documentary navigation, not a compiled decision assessment or code inspection. An initialized project gets default schema-5 task intelligence from its retained registry and, where permitted, a model. Follow the cited original quotations, check scope/negative cases/omissions and inspect the actual code and tests before changing it. If an initialized response returns IDs, drill down with `lore evidence ID`, `lore decisions "retry policy"` and `lore cases "duplicate charge"`. These registry commands require initialization.

For coding agents, `lore integrate codex` **previews** a bounded `AGENTS.md` instruction block; `lore integrate codex --apply` explicitly installs it. Use `claude-code` for `CLAUDE.md`. No agent integration, model, hosted account, native import or published wiki is required for first contact. Integration does not run an agent or authorize source edits. See [agent integration](AGENT_INTEGRATION.md).

For durable, versioned evidence, deliberately configure and compile Markdown sources:

```sh
lore init --configure-only --source ./docs
# Review lore.yml, model configuration, privacy and cost first.
lore init
lore --json context "Add payment retry diagnostics"
```

**Focused-release behavior:** New CLI-initialized projects use registry-only compilation by default. Generated Markdown requires explicit `output.mode: markdown` or new-project `init --publish-markdown`. Existing wiki projects keep publication until an explicit `update --migrate-output`; no existing pages are silently deleted. See [headless migration](HEADLESS_MIGRATION.md) and [first-run guide](tutorial/README.md). Historical 0.9.0-rc.1 always published pages.

## Ownership boundaries

| Owner | In scope | Not an implied capability |
| --- | --- | --- |
| **Lore** | Ingest primary Markdown decisions, architecture, plans and reviewed work history; keep revisioned source evidence and exact original quotes; represent lifecycle, scope, accepted/rejected/superseded positions and conflicts; ingest *optional* OpenWiki observations with separate authority; perform bounded permitted static checkout inspection; assemble task guidance; expose evidence, decisions, cases, review, safety, privacy, audit and provenance | Editing code; executing tests or arbitrary commands; authorizing policy; continuously monitoring a repository; verifying production deployments |
| **OpenWiki and source-producing tools** | Generate, maintain and optionally present implementation/code-wiki observations; export local snapshots that Lore may import | A mandatory hidden dependency of `lore context`, source of accepted architectural policy by default, or a tool Lore secretly invokes |
| **Calling coding agent / developer** | Choose/implement edits, inspect current checkout, execute tests, validate runtime effects, deliver changes and report actual results | Treat Lore advice, source hashes or static inspection as a passing test or implementation permission |
| **Project maintainers** | Approve decisions and changes, review sources and adjudication, set inference/inspection/egress permissions and disclosure policy | Delegating decision authority or privacy grants to imported reports or model-generated hypotheses |

A document may describe an accepted rule; code may appear to implement another rule; an OpenWiki export may summarize a third. Lore should present the discrepancy and qualified next check, not declare the newest text authoritative or silently repair code. OpenWiki snapshots are **optional implementation evidence**, not accepted decisions or independent runtime proof. Ordinary Markdown remains sufficient. Native Engram intake is retired; [retained evidence and rollback](ENGRAM_MIGRATION.md) preserve its original recollection authority and revision history; Beads work history remains in scope where it helps explain decisions and rejected paths.

## Supported core versus product surfaces to reduce

The **retained core** is: (1) document and decision ingestion; (2) exact quotations, provenance and versioned history; (3) decision lifecycle, scope, relationships and conflict review; (4) optional OpenWiki implementation observations; (5) bounded, permission-checked inspection; (6) task recommendations with checks/limitations; (7) evidence, decisions, cases, search and review drill-down; and (8) update, audit, recovery, retention, privacy and cost controls.

The **non-core product surfaces** include the interactive lessons and assessments **retired by [#66](https://github.com/grove/lore/issues/66)**, the embedded browser/graph viewer **retired by [#67](https://github.com/grove/lore/issues/67)**, separate Guardian task advice **retired under [#68](https://github.com/grove/lore/issues/68)**, specialized task-feedback receipt drafting **retired in [#69](https://github.com/grove/lore/issues/69)**, mandatory wiki publication **retired under [#70](https://github.com/grove/lore/issues/70)**, redundant browse/read navigation and long-lived legacy context schemas. The remaining non-core surfaces stay available **where implemented today** until their linked removal issue changes the CLI. The retained `onboard` command now delegates to task context without learning-specific output; see the [#66 migration](LEARNING_MIGRATION.md). Do not replace them with a new tutorial engine, agent framework, website, generic navigation browser or code analyzer.

### Current command map (0.9.0-rc.1)

Every listed command has a core purpose or an explicit migration owner. This maps the **focused development line**; the historical 0.9.0-rc.1 release includes additional learning flags/contracts documented as retired in [the migration guide](LEARNING_MIGRATION.md).

| Command | Core use, maintenance reason, or planned migration |
| --- | --- |
| `context TASK` | **Primary:** task-specific guidance, source-only first contact or compiled default schema 5; deterministic `--fast` remains an explicit option |
| `integrate codex\|claude-code` | Optional safe install/preview/removal of instructions that invoke `context`; agent workflow |
| `evidence ID` | Resolve exact retained documentary/native quotations and provenance |
| `decisions QUERY` | Why a choice was made, alternatives, applicability, scope and decision history |
| `cases QUERY` | Documented negative cases, procedures, rejected paths and reported outcomes |
| `review` | Inspect/adjudicate conflicts and record dispositions without silently changing source authority |
| `search QUERY` | Find retained facts and drill-down IDs; not a separate browsing product |
| `init`, `update` | Explicit versioned evidence compilation; new projects are **registry_only**. Markdown export is optional via `output.mode: markdown`; migrations require `update --migrate-output` ([guide](HEADLESS_MIGRATION.md)) |
| `status` | Read-only drift and freshness check before update |
| `audit` | Evidence/integrity review; explicit `--deep` can invoke models |
| `doctor` | Provider/configuration diagnostics; `--inference` makes synthetic provider requests |
| `baseline`, `changes` | Deterministic retained-evidence history and A→B→A comparisons; kept after [#68](https://github.com/grove/lore/issues/68) |
| `restore` | Reinstall stored pages only for **active Markdown** projects; returns a clear error in `registry_only` mode. Headless crash recovery happens automatically on `update` |
| `memory` | Inspect/clear disposable investigation leads; retention and privacy housekeeping, not a second source of authority |
| `purge` | Explicit managed-data deletion; never deletes original sources |
| `onboard` | **Thin source-grounded orientation using the context pipeline**; former interactive lessons, exercise flags, separate schemas and caches retired in [#66](https://github.com/grove/lore/issues/66); [migration](LEARNING_MIGRATION.md) |

**Retired:** `lore explore` and `lore read`, their browsing schemas, rendering and navigation caches under [#71](https://github.com/grove/lore/issues/71) ([navigation migration](NAVIGATION_MIGRATION.md)); `lore visualize`, its HTTP routes and browser assets under [#67](https://github.com/grove/lore/issues/67), standalone `lore guard` advice under [#68](https://github.com/grove/lore/issues/68), and `lore feedback draft` receipts/output/write path under [#69](https://github.com/grove/lore/issues/69). See [visualizer migration](VISUALIZER_MIGRATION.md), [Guardian migration](GUARDIAN_MIGRATION.md) and [task-feedback migration](FEEDBACK_MIGRATION.md). The source-integrity/Guardian **CI** gate remains mandatory and independent; no new advice or UI path was created.

**Context contracts:** uninitialized bootstrap schema 1, model-free `--fast` schema 2 and initialized default/explicit schema 5 are retained. Explicit schemas 3/4 were removed in the breaking 0.10.0 release under [#72](https://github.com/grove/lore/issues/72); see the [complete compatibility matrix, consumer inventory and migration](CONTEXT_MIGRATION.md). No new response framework or schema is introduced. `--json`, `--config` and task flags are options, not separate products.

## What is enforced, and what is not yet measured

**Implemented mechanics (conditional on the selected path and grants):**

- First-contact unpinned `context` selects bounded local documentary passages, preserves quoted source path/line/hash and makes no model call or automatic write. It cannot promise complete policy coverage.
- Compiled evidence retains revisions and exact source quotations; imported OpenWiki or work reports are labeled as observations, not accepted rules. Automatic/assisted relationships and review are qualified, with failures and omissions surfaced.
- Initialized `context` defaults to schema 5, supports bounded source reads under caller-owned grants and separates checked observations from hypotheses, reported outcomes and post-edit checks. `--fast` is model-free, read-only retrieval.
- `init`/`update` are explicit, potentially costly operations; the staged registry always has recovery protection and optional Markdown publication retains its two-slot safety checks. Privacy limits, permission ceilings, source integrity checks, audit, and deliberate retention/purge apply. Agent instruction installation is preview-first and writes only on `--apply`.

**Not established by these mechanics:** causal improvement in coding correctness, time saved or fewer regressions on real tasks; human learning/transfer gains; semantic accuracy of every model response; quality of Guardian alerts; independent execution or deployment verification; real-agent compliance with installed instructions; universal completeness when evidence is missing or budgets truncate selection. The independent decision-work pilot remains unmeasured; see [#2](https://github.com/grove/lore/issues/2), [evaluation status](../evaluation/PRODUCT_QUALITY_09.md), and the [current 0.9 guide](V09.md). Passing fixture/CI contracts does not prove user outcomes.

## Removal and migration policy

1. **#65 was documentation-only.** Follow-ups #66–#69 remove interactive learning, embedded browser UI, separate Guardian advice and receipt drafting with explicit migration. No source material, persisted evidence, privacy grant, authority rule or OpenWiki opt-in status changes.
2. **Inventory consumers and stored data first.** Each linked issue owns its actual CLI/code removal, tests and migration path; preserve historical original evidence and its authority/revision labels. Never silently promote imported reports into policy, rewrite user sources, or delete generated pages on upgrade.
3. **Announce intentional breaking changes.** Document the last supported version, replacement command and affected flags/JSON contracts; use explicit errors after removal, not silent aliasing to a different authority or semantics. Maintain old wiki projects' output until an explicit `--migrate-output`; no silent removal or overwrite ([#70](https://github.com/grove/lore/issues/70)). Preserve model-free first contact and deterministic fast access per [#72](https://github.com/grove/lore/issues/72).
4. **Preserve integrity and grants.** Keep read/egress ceilings, review boundaries, source-digest/revision checks, A→B→A history, recovery, cancellation and cross-platform regression coverage. [#68](https://github.com/grove/lore/issues/68) removes Guardian *advice*, **not** the independent Guardian CI source-integrity harness and restoration tests.
5. **Measure after simplification.** Do not promote source-gold fixtures, provider probes or pending study protocols to user benefit claims. Complete the existing independently checked real-world evaluation in [#2](https://github.com/grove/lore/issues/2) after the reduced surface stabilizes.

For implementation detail and retained historical designs, consult [the implementation guide](IMPLEMENTATION.md) and [DESIGN.md](../DESIGN.md). Where those older texts promote another flagship product, **this contract governs current product priorities**; implementation and published CLI contracts remain authoritative for what actually runs.
