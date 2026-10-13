# Lore vision: decision intelligence before code changes

**Primary user:** A developer or coding agent preparing a specific code or architecture change. Maintainers reviewing change implications have the same source-backed need.

**Primary product:** `lore context TASK`. It answers: *What documented decisions apply to this change? Why? What alternatives and negative cases matter? Does observed implementation differ, and what is the next defensible action?*

The authoritative feature and ownership boundary, retained command map, planned retirements and migration policy are defined in **[PRODUCT_SCOPE.md](docs/PRODUCT_SCOPE.md)**. This vision describes the aim; [the implementation guide](docs/IMPLEMENTATION.md) and [0.9 scorecard](evaluation/PRODUCT_QUALITY_09.md) describe what is implemented and tested. Earlier design explorations remain useful history, not parallel product commitments.

## What success looks like for one task

A human or coding agent runs:

```sh
lore context "Add retry diagnostics without changing approved retry limits"
```

The useful answer starts with a **preferred next step** and explains its scope and readiness, followed by the exact source-backed reasons, conditions/exceptions, rejected approaches or relevant cases, implementation observations/discrepancies, checks already performed and checks still required. Separate documentary intent, reported history, static source observations, actual executed verification, hypotheses and engineering heuristics. Preserve competing evidence rather than inventing agreement.

The answer should be bounded, actionable and honest about missing input. Lore should investigate consequential uncertainty using *already authorized*, bounded read-only access where it improves the decision, and stop when further investigation is unlikely to change the recommendation. It must not turn absence of retrieved evidence into absence of policy, treat recent text as approval, or ask a developer to redo inspections that Lore has already performed. If evidence is incomplete, offer safe partial progress and identify the precise outstanding authority or verification question.

`lore --json context TASK` allows a calling agent to consume the same source-backed decision. `lore integrate codex` / `lore integrate claude-code` install optional agent instructions *only after explicit* `--apply`; they do not invoke an agent or grant permissions.

## Ownership: one decision engine, not several products

**Lore owns** ingestion of primary Markdown and optional native evidence, exact original quotations and versioned evidence, source authority and lifecycle/scope, conflict and rejected-approach handling, bounded permission-checked static inspection, task recommendations and audit/review/provenance/safety facilities. `evidence`, `decisions`, `cases`, `search` and `review` support focused drill-down rather than a second mandatory user journey.

**OpenWiki owns** code wiki generation and optional presentation of implementation knowledge. Its exported observations are useful to Lore when the user configures them, but are not accepted architecture decisions, independent execution proof or a mandatory hidden dependency. Primary Markdown alone remains usable; Lore neither launches OpenWiki nor needs a published wiki to answer an uninitialized first-contact request.

**Developers and coding agents own** source edits, running checks, verifying runtime changes and delivery. **Maintainers own** policy decisions, approval, privacy authorization and evidence correction. Original sources—not model narration, imported reports, a passed source-integrity check or a task receipt—set documentary authority.

## Supported core and deliberately reduced scope

The product should retain durable document/decision history, evidence IDs and exact quotes; lifecycle/scope/conflicts; optional OpenWiki implementation observations; bounded inspections; task-oriented recommendations; direct evidence/decision/case/review retrieval; baseline/change history; safe update/recovery, audit and privacy controls.

It should **not** maintain multiple independent advice or reading products. The interactive learning engine is slated for retirement in [#66](https://github.com/grove/lore/issues/66), browser visualization in [#67](https://github.com/grove/lore/issues/67), separate Guardian advice in [#68](https://github.com/grove/lore/issues/68), task receipt drafting in [#69](https://github.com/grove/lore/issues/69), mandatory wiki publication in [#70](https://github.com/grove/lore/issues/70), redundant browse/read surfaces in [#71](https://github.com/grove/lore/issues/71), obsolete context contracts in [#72](https://github.com/grove/lore/issues/72) and the native Engram importer only after history-safe migration in [#73](https://github.com/grove/lore/issues/73). These are **future changes**, not removals implemented by this document; current CLI compatibility is unchanged. Preserve the Guardian CI source-integrity harness while removing Guardian *advice*.

Do not compensate for the reduction with a new browser UI, learning framework, project-monitoring scheduler, general-purpose agent memory, coding-agent runtime or code analyzer. A concise CLI/JSON decision package should be useful even when no separate viewer or mandatory wiki is present. Compilation is currently coupled to Markdown publication in 0.9.0-rc.1, so decoupling requires an explicit migration and compatibility release—not a documentation assertion.

## Guiding principles

- **Answer first, evidence attached.** Lead with one defensible action and constraints. Exact quotations, original lines/hashes, source scopes, exceptions, competing history and omissions stay inspectable.
- **History is not present authority.** Proposed, accepted, rejected, withdrawn and superseded decisions remain distinct. A reported work outcome is not verified behavior. A source disappearing must not silently erase its earlier evidence.
- **Optional observations, independent policy.** Compare OpenWiki snapshots and bounded checkout observations to documentary intent without making either authoritative by default.
- **Act within grants.** Local-first, no implicit hosted inference, checkout egress, code execution, source editing, or continuous background activity. A standing read permission is not a project document's right to grant access.
- **Investigation over ceremonial handoffs.** Perform bounded, useful *permitted* checks once, retain revision-bound findings when valid, surface unresolved dependencies and distinguish present inspection from future post-edit tests.
- **One workflow, few commands.** `context` for advice; `integrate` for optional agent onboarding; `evidence`/`decisions`/`cases`/`review` for support; `init`/`update`/`status`/`audit` and related maintenance for controlled lifecycle. See the full current command map and migrations in [PRODUCT_SCOPE.md](docs/PRODUCT_SCOPE.md).

## Implemented engineering contracts versus hoped-for outcomes

**Engineering behavior in 0.9.0-rc.1:** Uninitialized, unpinned `context` can read bounded Markdown with exact path/line/hash and zero model calls, but is only source navigation. Initialized schema-5 context assembles retained knowledge and permitted investigation with scoped guidance or a labeled fallback; deterministic `--fast` uses schema 2. Exact evidence, lifecycle/history qualifications, caller-owned inspection and egress rules, optional OpenWiki snapshots, review/adjudication and source-integrity/recovery fixtures exist. Integration edits managed instructions only on explicit apply. These are conditional mechanical properties with recorded test boundaries, not guarantees that every model recommendation is semantically right.

**Not demonstrated:** a causal reduction in errors, agent task time or duplicated investigation; improved human contribution/learning; universal code/production correctness; Guardian alert usefulness; production provider bill accuracy when missing; real coding-agent obedience to instructions. Fixture checks, synthetic probes and source-gold controls do not substitute for independently measured outcomes.

**Next evaluation:** Finish the *existing* [genuine decision-dependent coding study #2](https://github.com/grove/lore/issues/2) after the command surface stabilizes. Compare agents with/without Lore on pinned, independent tasks and checkers, including accurate constraint preservation, actual completed work, false blocks, unresolved assumptions and all time/cost of retrieval, inspection, correction and model calls. Keep provider-readiness and permission gates explicit. Report denominators, failures and unknowns rather than making benefit claims from a dry-run protocol.

Lore succeeds if it helps people and agents **make better, verified changes with the right decisions in view**—not if it creates more documents, lessons, browser screens or alerts.
