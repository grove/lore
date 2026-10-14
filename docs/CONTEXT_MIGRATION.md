# Context contracts in Lore 0.10.0

Issue [#72](https://github.com/grove/lore/issues/72) removes public context
schemas 3 and 4. **0.10.0 is an intentional breaking release.** The last
supporting release candidate is **0.9.0-rc.1**. Stay on that
version while migrating a legacy consumer. This release removes the selectors
immediately; it does not ship an intermediate warning-only deprecation release.
The earlier schema-3/4 compatibility paths described in the 0.6–0.9 guides are
historical. No schema 6 is introduced.

## Compatibility matrix

| Request | Through 0.9.0-rc.1 | 0.10.0 | Initialization, reads and inference |
| --- | --- | --- | --- |
| Unpinned `context TASK`, initialized | Schema 5 | Same schema 5 | Registry snapshot; optional models/cache and bounded checkout reads within caller grants |
| `context TASK --schema-version 5` | Schema 5 | Same schema 5 | Requires initialization; never silently bootstraps |
| `context TASK --fast` | Schema 2 retrieval | Same schema 2 retrieval | Requires initialization; zero model calls, no checkout inspection or cache writes |
| Unpinned `context TASK`, uninitialized | `lore.bootstrap_context`, schema 1, `bootstrap_source_only` | Same first-contact contract | Bounded local documentary reads; no models, initialization, registry or cache writes |
| `context TASK --schema-version 3` | Schema-3 brief or explicit fallback | Exit 2, `invalid_arguments`, migration guidance | Rejected before config, source, registry, usage ledger or model access |
| `context TASK --schema-version 4` | Schema-4 decision or explicit fallback | Exit 2, `invalid_arguments`, migration guidance | Rejected before config, source, registry, usage ledger or model access |
| Schema 5 pin or `--fast`, uninitialized | `uninitialized_project` | Same error | Explicit initialized contracts never silently switch shape |
| `onboard` / `onboard --topic TOPIC` | Schema-5 context, or source-only first contact on the focused development line | Same retained context pipeline | Same caller grants and budgets; no separate reasoning engine |

Schema numbers on configuration, evidence storage, usage and other named
contracts are unrelated and unchanged. `--fast` still conflicts with a schema
pin and model/inspection flags. Unsupported schema numbers also fail argument
validation. Markdown follows the same selection rules as JSON.

## Migrating consumers

Replace either legacy invocation with:

```sh
lore --json context "Implement payment retries" --schema-version 5
# Or model-free retrieval from the initialized registry:
lore --json context "Implement payment retries" --fast
```

For former schema-4 consumers, read the decision/fallback under `intelligence`;
read the complete-output `budget`, `snapshot`, `capabilities`, `usage` and
optional `source_relationships` on the outer envelope. The nested
`intelligence.schema_version` remains **4** as part of the existing schema-5
contract. Removing top-level selection does not remove that nested structure.
Markdown retains schema 5's action-first ordering.

Schema-3 consumers must migrate their parser: `known` becomes retained
`intelligence.brief.facts`, `inferred` becomes qualified `hypotheses`,
`constraint_checks` becomes `constraints`, and `next_steps` becomes `next_action`
plus prioritized `checks`. These are semantic adaptations, not field aliases.
Always branch on `intelligence.mode`: a small-budget deterministic fallback can
contain retrieval fields directly rather than a `brief`; `reference` is also a
zero-call mode. A fallback is not fresh model assessment. An empty result or
omitted group does not establish safety or absence of constraints.

Schema 5 requires **caller-owned** grants. Configuration alone no longer
suffices for integrations migrating from schema 3/4: hosted documentary
inference needs `LORE_ALLOW_HOSTED_EGRESS=1` and `privacy.local_only: false`.
Use `LORE_INSPECTION_ROOT` for bounded automatic checkout reads or
`--inspect`/`--investigate` for one invocation; `--no-inspect` is an absolute
veto. Hosted checkout content needs its separate grant within the privacy
ceiling. See [all grant rules](SHARED_INTELLIGENCE.md#standing-grants).
No mode executes repository code or grants source writes.

The complete JSON and Markdown, including citations, warnings, metadata and
relationship groups, remain bounded by `--max-tokens`. Model input and attempted
calls keep separate bounds. Required qualifications stay together; output that
cannot fit returns a typed budget error. Privacy ceilings, exact evidence and
revision validation, historical authority and omission reporting are retained.

## Consumer inventory and maintenance reduction

| Consumer / integration | Disposition |
| --- | --- |
| `src/main.rs` context dispatch | Removes schema-3 orchestration and schema-4 direct dispatch; explicit early errors replace both selectors |
| `src/context/intelligence.rs` | Removes schema-3 response types, prompts, generation, verification, budget packing, renderer and guidance cache; retains shared evidence catalog, validation and types used by schema 5 |
| `src/context/decision/runtime.rs`, `decision.rs` | Removes unused direct schema-4 runner and alternate Markdown order; retains synthesis, fallback, retrieval, inspection, investigation, decision cache and nested result |
| `tests/intelligent_context.rs`, `tests/context_intelligent_cli.rs` | Removes obsolete schema-3-only suites; shared decision/evidence, retrieval and schema-5 suites remain |
| `tests/context_decision_cli.rs`, `bootstrap_cli.rs`, `context_migration.rs` | Moves CLI cache/privacy/inspection coverage to schema 5; keeps fast/bootstrap budgets and adds removed-selector/side-effect regressions |
| `tests/decision_runtime.rs`, `decision_contract.rs`, `adaptive_context.rs`, `source_relationship_manifest.rs`, `hybrid_retrieval.rs` | Retained shared-engine evidence, citation, constraints, privacy, budgeting and cache coverage; nested schema 4 remains intentional |
| README, `docs/AGENTS.example.md`, `SHARED_INTELLIGENCE.md`, `PRODUCT_SCOPE.md` | Current instructions use schema 5, explicit fast or first contact; link this matrix |
| `src/integrations.rs`, `tests/agent_integration_cli.rs`, tutorial, provider-usage guide | Already use unpinned context or schema 5; no legacy selector to migrate |
| `evaluation/shared_intelligence.py` and tests | Current collector drops schema-3/4 arms; protocol v2 distinguishes the reduced collection from historical v1 captures |
| `evaluation/coding_tasks.py`, `decision_tasks.py`, `adaptive_tasks.py`, downstream `adaptive_sequences.py` / `experiment_08.py` and their tests | Historical schema-3/4 comparison arms remain explicitly pinned. Full old comparisons require the 0.9.0-rc.1 binary and matching harness; they fail clearly on 0.10.0 rather than relabel schema 5 as an old arm |
| Current `evaluation/decision_work_09.py`, `human_onboarding.py`, `guardian_longitudinal.py` / `guardian_integrity_gate.py` and CI | Already consume schema 5 (or fast); pinned historical Guardian baseline keeps its own binary. Source-integrity gates remain mandatory |
| `evaluation/capture_contracts_08.py`, frozen `evaluation/results/`, corpus manifests | Historical 0.8 capture/verification and immutable results remain bound to their original binary/source hashes; not current-binary compatibility tests |
| `DESIGN.md`, `docs/V05.md`, `V06.md`, `V08.md`, `V09.md`, historical design/implementation plans, evaluation protocol docs | Historical specifications retained for traceability; this migration and current CLI govern supported selection |

Rust embedders lose `IntelligentContextResult`, `IntelligentBriefResult`,
`FastFallbackResult`, `build_intelligent_context`, `fast_fallback`,
`render_intelligent_context`, schema-3-only brief helper types and the direct `decision::runtime::run` entry
point. Use `context::adaptive::run` (or `build` with explicit grants/model),
`adaptive::render`, or deterministic `context::build_context` as appropriate.
Shared evidence types keep their module paths.

Upgrading rewrites no original sources, persisted evidence, revisions, decisions,
baselines or exported pages. Old schema-3 `.lore/context-cache/<hash>.json`
files are no longer read or created. That directory is still used by schema 5
for `decision-<hash>.json` findings; preserve those when removing obsolete
schema-3 files, and check for user-added files first. Other caches remain
disposable and keep their own revision/grant validation. There is no automatic
data cleanup or conversion.
