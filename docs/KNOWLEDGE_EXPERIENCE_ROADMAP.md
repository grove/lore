# Lore Knowledge Experience — shared-intelligence roadmap

**Status:** Proposed implementation plan, not shipped functionality. **Roadmap revision:** 4. **Date:** 2026-10-09. **Baseline:** Lore 0.6.

**Designs:** [Developer Onboarding](DEVELOPER_ONBOARDING_DESIGN.md) (**flagship human learning journey**), [Coding-Agent Intelligence](CODING_AGENT_INTELLIGENCE_DESIGN.md) (**coequal agent flagship**), [Autonomous Assistance](AUTONOMOUS_ASSISTANCE_DESIGN.md), [Knowledge Experience](KNOWLEDGE_EXPERIENCE_DESIGN.md).

> **Understand any project. Work with confidence.**
>
> **Maximum useful autonomy, minimum user burden.** One project intelligence core should empower newcomers, experienced developers, maintainers and coding agents. Learning through practice is the flagship human onboarding experience; compact decision-ready context is the flagship agent experience.

Revision 4 preserves newcomer onboarding as the most demanding **human flagship**, but makes coding agents a **coequal flagship** and retains direct support for experienced developers and maintainers. The earliest engineering milestone is a *small, shared* vertical slice: permitted knowledge/inspection plus a short human orientation/tour/exercise and a scoped agent recommendation package. The full first-contribution/transfer journey, investigation reuse and broad Knowledge Zoom follow as independently evaluated phases. Existing R0–R7, A1–A3 and O1–O3 names remain traceable; G1 defines the agent path. All commands and schemas described as future work are **proposals**, not existing Lore 0.6 behavior.

## 1. Scope, priorities and release policy

This is a capability roadmap, not a version-number or calendar promise. **The first engineering milestone (M0) is a minimal shared-intelligence slice serving one human learning request and one coding-agent task on the same evidence core.** It is deliberately smaller than completing O1/O2/O3, all of A1/A2 or a full new schema/storage layer. The *later full human outcome* remains a correct and understood first contribution plus independent transfer; the agent outcome is a correct, constraint-preserving change with less unnecessary effort. Assign release numbers and numerical targets only after evaluation. No staffing, dates, prices or gains are promised.

The proposed newcomer experience begins with `lore onboard`: an immediate grounded orientation and a small source-linked workflow tour, with optional exploration and practice. Existing `lore context` remains the direct task entry for experienced people and agents. The future agent contract provides compact JSON (recommended action, constraints, inspected seams and completion checks). No mandatory learner profile, custom UI, agent runtime or giant wiki index. Adaptive investigation performs the research; deliberate learner exercises support learning. Advanced restrictive flags and existing schema contracts remain available.

### 1.1 Delivery order

| Stage | User-visible value | Dependency |
| --- | --- | --- |
| R0 | Two independent baseline tracks: newcomer learning **and** agent coding-task quality; shared source/permission contracts | Existing 0.6 benchmark and knowledge registry |
| **M0** | **One common source-bound intelligence path with two tiny outputs:** guided human orientation plus decision-ready agent JSON | Small subset of A1/A2, O1/O2 and G1; no new runtime needed |
| A1 / A2 (complete) | Broader automatic permitted investigation, action-first responses and scoped safe progress | R0/M0 outcomes; can improve incrementally |
| **O1 / O2** | Project tours and meaningful tutorial/learning paths | M0 human path, measured expansion |
| **G1** | Full coding-agent package, mature readiness and validation | M0 agent path, measured expansion |
| **O3** | Supported first contribution and **different-task independent transfer** | O1/O2 and A1/A2; this is the complete human onboarding outcome |
| A3 | Revalidated investigation reuse benefiting humans and agents | A1/A2; not a dependency for M0 |
| R1 | Better contextualized segment/retrieval quality | R0, optional in M0 if necessary |
| R2 | Adaptive-depth Knowledge Zoom for all audiences | M0 interfaces; no fixed 4- or 5-level ontology |
| R3 | Full Diátaxis explain/how-to/tutorial/reference renderers | M0/O2; no tutorial requirement for agent or expert task requests |
| R4 | Decisions, negative cases and investigate-first gaps | Shared knowledge intelligence, not onboarding-only |
| R5A / R5B | Worked cases and separately approved sandbox replay | Explicit safety/product gates; no dependency for M0 |
| R6 | Change-impact insights, guardian and optional reader | Useful to maintainers, newcomers and agents |
| R7 | Opt-in learner preferences and cross-project transfer research | Permission/quality evidence |

**M0: smallest shared-intelligence implementation target**

- **Common substrate:** select one stable current knowledge/evidence snapshot; retrieve constraints and relevant decisions; perform **at most a bounded permitted static inspection** when useful; preserve the current source/authority and privacy boundaries. Reuse current 0.6 mechanisms rather than building another evidence registry.
- **Human demonstration:** from a single unfamiliar project, produce one accurate project essence, a coherent short path through actual source landmarks and one optional source-grounded prediction or worked exercise. No full learning graph or persistence requirement.
- **Agent demonstration:** for one real coding task in the **same project**, produce one compact JSON recommendation with readiness, critical constraints, actual inspected implementation seam, completed-vs-future check distinction and evidence manifests. No new coding agent, shell executor or mandatory integration.
- **Review:** deterministic contract/invalidation/privacy tests and at least one independently evaluated human comprehension exercise **and** one independently evaluated agent implementation. Report both as exploratory until held-out validation; do not falsely claim improved learning or coding task quality.
- **Do not bundle:** a complete learner curriculum, first/second-task evaluation infrastructure, all Diátaxis renderers, reusable investigation database, whole-corpus hierarchy, live connector, hosted service or reader GUI.

**The next product evaluations are independent:** O3 validates the complete first-contribution/second-task human journey; G1/A1–A3 validate agent task correctness/efficiency. Shipping or benchmarking one does not confer success on the other.

### 1.2 Common release gates

- Preserve explicit schema-2/3/4 contracts, `--fast`, `--no-inspect`, no-cache, source evidence, review history and no-op behavior. A new default has explicit version/migration notes, not silent changes behind an old schema.
- Source-dependent assertions retain exact evidence, revision, scope, modality and authority. Derived findings never become accepted policies through reuse.
- Expand initiative inside accepted grants, not access. Existing denials survive upgrades; a repository's own configuration cannot authorize host execution or egress.
- No generic investigative handoffs when a suitable permitted capability and budget exist. Remaining dependencies identify what is missing and why it affects action.
- Resource exhaustion, capability unavailability and policy/requirements blockers remain distinct. Safe partial work cannot disguise a blocked full task.
- Validate both caution and initiative: fewer false blocks must not increase unsafe proceeding or hidden preconditions.
- Every stateful feature has versioning, bounded storage, invalidation, purge, privacy and crash recovery. Persistent citations outlive disposable cache eviction.
- No secret/unauthorized egress, untrusted process execution, source mutation, hidden telemetry or background continuation by default.
- Every quality claim has an actual reviewed experiment. Synthetic fixtures establish mechanics, not product superiority. Features can ship experimentally with honest status while evidence is collected.
- **Two independent product quality gates:** onboarding claims require a verified, understood first task and a different-task transfer test; agent claims require externally checked implementation outcomes, preserved constraints and measured total effort. Neither substitutes for the other. An agent-written patch or read page is not human mastery.
- Exercises, hints, progress and state are explicit and optional; no mandatory quizzes, invasive tracking or inferred competence from page views.
- Tours, starter tasks and lessons must preserve source scope/revisions; no invented real issues, runtime traces or passing tests.
- An agent query is **not** a learner session. No default tutorial text, progress profile, forced practice or special agent framework; experienced users also retain direct guidance.

## 2. Backlog priorities

| Priority | Workstream | Smallest useful result | Why it matters |
| --- | --- | --- | --- |
| **P0** | **Common source-bound intelligence substrate** | One validated evidence/permission/inspection path feeding human and agent renderers | No duplicated truth or repeated research |
| **P0** | **Human orientation/tour/one activity** | Useful source-grounded first-minute understanding | Tests quality of the conceptual model |
| **P0** | **Coding-agent decision JSON** | One concrete recommendation, constraints, inspected seam and completion checks | Enables agent task work immediately |
| **P0** | **Two matched evaluation tracks** | Human comprehension probe and externally checked agent task | Prevents one successful audience masking another |
| P1 | Full onboarding contribution/transfer | Correct first human change plus distinct less-assisted second task | Proves skill transfer |
| P1 | Mature adaptive investigation and safe progress | Correct decisions without routine handoffs or unnecessary reads | Helps all audiences |
| P1 | Revalidated investigative reuse | Reapply actual findings only while still relevant | Compounds experience |
| P1 | Contextual retrieval and rare-case preservation | Better exact and concept retrieval | Supports both learning and code changes |
| P1 | Knowledge Zoom | Navigate from essential concepts to precise evidence at any meaningful depth | Reduce overload without a fixed depth |
| P1 | Diátaxis experiences | Tutorial when learning; how-to, explanation and reference when needed | Correct help for the actual intent |
| P1 | Decision conditions and negative cases | Applicable constraints and past failures | Teaches human judgment and improves agent decisions |
| P2 | Optional learning state/resume | Minimal explicit progress with inspect/export/reset | No hidden user profiling |
| P2 | Change-aware briefings and advisory guardian | Relevant consequences for maintainers/returners/agents | More than a new wiki |
| P3 | Approved isolated verification | Narrow test execution inside a reviewed sandbox grant | More evidence, higher risk |
| P3 | Optional visual reader | Accessible tours, semantic zoom, evidence drill-down | Usability without mandatory UI |
| Research | Cross-project analogy | Namespace- and permission-safe comparisons | Transfer only if relevant |

First-stage non-goals: mandatory MCP, graph database, hosted account, daemon, automatic external tracker, unrestricted agent runtime, arbitrary model-authored commands, autonomous code editing/production action, hidden profiling, an eager whole-wiki distillation tree or unreviewed policy acceptance.

## 3. R0 — baseline, contracts and evaluation foundation

**Purpose:** define the experience and test it without making all research a prerequisite for writing the first useful implementation.

### Work items

- **R0.1 Pin the baseline:** source/build/config/model identities; current 0.6 default versus explicit inspection/investigation, including actual grants and resource ceilings.
- **R0.2 Two outcome tracks:** human orientation, correct bounded first change, accurate mental-model explanation and **different-task transfer**; independently, a coding agent making a correct change under project constraints. Include exact lookup, policy changes, partial progress and document-only assistance; balance participant familiarity.
- **R0.3 Adversarial fixtures:** preserve provenance, chronology, table, exception, deletion, scope, injection and no-op; cover AT-01–AT-26 autonomy, ON-01–ON-24 onboarding **and AG-01–AG-18 coding-agent** scenarios. Include false code flow, fabricated issue/trace, stale investigation and denied inspection.
- **R0.4 Extend measurement:** human learning and second-task transfer, hints/mentor burden and learner authorship; **separately** held-out agent implementation correctness, missed constraints, repeated investigation, rework and full task cost. Also record false blocking, unsafe proceeding, permission grants, provider attempts and billing when known.
- **R0.5 Versioned shared/agent contracts:** define evidence/permission scope, adaptive action limits, response-mode negotiation and explicit schema-2/3/4 compatibility. Specify **small** onboarding/tour/learning-activity and agent decision-package views; defer complete learning/view schemas until later.
- **R0.6 Matched study and budgets:** preregister separate human and agent correctness/safety/burden thresholds after baselines. Record cold and amortized preparation; prevent prior solved tasks or generated tutorials leaking held-out outcomes. Expert/coding-agent entry must skip onboarding.
- **R0.7 Threat review:** distinguish trusted host grants, project preferences, caller restrictions, data classes and source-derived egress; document missing adapter capabilities honestly.

### Deliverables and gate

A shared evidence/permissions contract RFC, both benchmark manifests, independent scoring guides and small executable fixtures. Contract/fixture readiness unlocks a **bounded M0 spike** across human and agent requests. Broader claims depend on later held-out research; do not hold a clearly labeled prototype hostage to an exhaustive study. Broad quality/default-on claims require reviewed evidence, but lack of a completed study does not indefinitely block an explicitly experimental build. Reviewers must distinguish static inspection, executed checks, source reports, inferences and authority.

## 4. A1 — adaptive read-only assistance

**Purpose:** make Lore own useful investigation immediately using existing foundations.

### PR-sized work packages

- **A1.1 Effective policy resolver:** intersect trusted operator grants, project/root identity, caller access and request restrictions. Preserve old denials and data-class egress restrictions. Expose available capability IDs, not arbitrary tool strings.
- **A1.2 Controller state:** goal/change kind, provisional answer, applicable constraints, material uncertainties, competing hypotheses, completed observations, candidate actions, shared budget and stop reason. No hidden chain-of-thought storage.
- **A1.3 Typed read capabilities:** wrap existing registry/evidence/relation retrieval and no-follow bounded inspection. History reads initially use available snapshots; live history/connectors require an approved adapter, not a promised general capability.
- **A1.4 Action selection:** rank narrow permitted checks by plausible impact on the answer, risk/reversibility and cost. Explicit requested identifiers/modes override routing; a decision model is advisory, never a relevance veto over critical constraints.
- **A1.5 Recommendation revision:** incorporate new evidence and change the action when the premise fails. Do not merely append a warning to an unchanged bad recommendation.
- **A1.6 Stopping:** decision sufficiency, low expected value, no information gain, capability absence, budget exhaustion, authority/requirement dependency, evidence mutation, cancellation and provider failure. No exhaustive-search requirement.
- **A1.7 Resource ledger:** count all model attempts/retries, reads/rereads, indexes, waiting and verification/repair under aggregate bounds; reserve final validation. Independent parallel reads share budgets and cancellation.
- **A1.8 Capability ergonomics:** one understandable standing envelope during trusted setup; no new grant through configure-only/untrusted repo settings; no repeated prompts within the same grant; useful restricted operation without setup nagging.
- **A1.9 Regression scenarios:** AT-01–AT-10, AT-14–AT-16, AT-20–AT-21 and AT-24–AT-26; include no unnecessary reads on an exact lookup and a policy change that must not be authorized by inference.

### Deliverables and exit gate

A future versioned/experimental adaptive command path produces a useful scoped answer with actual completed reads and truthful static-only observations. It works with configured documents alone when checkout access is unavailable. Contract tests prove limits, permissions, legacy behavior and counterevidence revision. Compared with a capability/budget-matched 0.6 inspected arm, measure whether adaptive selection/stopping improves burden or outcome without increasing material errors. Do not attribute gains solely to larger budgets or extra data.

## 5. A2 — action-first UX, safe progress and minimal handoffs

**Purpose:** make the answer empowering rather than an evidence dump or a list of homework.

### PR-sized work packages

- **A2.1 Answer composer:** direct answer/preferred approach first, decisive rationale, main trade-off and next useful action. Do not force every explanation or exact lookup into six compulsory headings.
- **A2.2 Scoped readiness:** distinguish full requested outcome from independent actions; expose completion criteria and material remaining dependencies. A genuine blocker is specific to an action.
- **A2.3 Delegation policy:** complete relevant available checks before handoff. Identify why an unavailable observable is material; distinguish future tests on code not yet changed from checks Lore could perform now.
- **A2.4 Noninteractive behavior:** never wait for stdin to resolve a missing requirement; return structured dependency and useful work. Interactive escalation is one focused request only after meaningful assistance and only when necessary.
- **A2.5 Progressive disclosure:** keep critical constraints visible; place detailed evidence, alternatives, history and budgets behind CLI detail options or reader expansion. The machine contract retains full required manifests.
- **A2.6 Useful degradation:** repair/withdraw an invalid component while retaining independent supported findings. Never detach a material qualifier to make an answer fit. No stale answer relabeled current and no pretend model reasoning on deterministic fallback.
- **A2.7 Agency and feedback:** allow scope narrowing, correction, cancellation and evidence inspection. Occasional synchronous progress explains useful findings, not raw tool chatter. No covert background continuation.
- **A2.8 Golden UX examples:** useful one-value reference; three-versus-five refactor; genuine policy dependency with preparation work; unavailable runner; provider outage; document-only request; optional tutorial exercise; no meaningful independent work.

### G1 — coding-agent flagship (built on A1/A2; not a second intelligence engine)

Implement the [Coding-Agent Intelligence design](CODING_AGENT_INTELLIGENCE_DESIGN.md) in small slices:

- **G1.1 Compact versioned JSON:** preferred approach, scoped readiness, material constraints/negative cases, actual inspected implementation seams, completed investigation, future completion criteria, uncertainty and exact manifests. Explicit schema-2/3/4 behavior remains intact.
- **G1.2 Task-context orchestration:** use the same selected source/knowledge snapshot and bounded automatic permitted reads as the human experience. No redundant search agent, wiki regeneration or new unconstrained tool loop.
- **G1.3 Agent consumption examples:** optional Codex/Claude/other agent guidance and correct fallback handling. No required MCP server, IDE plugin or provider-specific agent runtime.
- **G1.4 Agent outcome tests:** AG-01–AG-18, real held-out implementation/constraint checks against original sources, current Lore 0.6 and OpenWiki (when available) at matched capabilities and comparable budgets.
- **G1.5 Experienced human/maintainer routing:** answer direct questions and decisions without forcing learning steps, agent-only JSON, learner state or an onboarding questionnaire.

**M0 scope of G1:** one bounded task, one evidence-bound recommended change, one actually inspected seam when permitted, one future verification criterion and an explicitly documented fallback. Mature revalidated investigation reuse, broad integration and performance claims come later.

### Deliverables and exit gate

Humans can identify the answer, next action and decisive boundary without reading an investigation log; coding agents receive the same decisive knowledge in a **compact, source-bound, versioned machine contract** without a tutorial. Reviewers judge meaningful safe progress rather than filler. Tests reject generic delegation when the recorded capability was available and affordable, while permitting honest future implementation checks. Evaluate comprehension, correction effort, avoidable questions, false blocking and unsafe proceeding jointly. The shortest answer or fewest questions alone is not the objective.

## 6. A3 — reusable investigative knowledge

**Purpose:** let each useful investigation reduce future work without turning guesses into accepted facts.

### PR-sized work packages

- **A3.1 Record/schema:** immutable ID/revision, task family/scope/applicability, original evidence, observations, hypotheses/alternatives, counterevidence, action changes, completed checks, validation/coverage, stop reason and concise rationale.
- **A3.2 Derived store:** separate from accepted registry; bounded retention, schema migration, access/data-class labels, no-cache/read-only semantics and transparent actual writes. Routine reuse needs no human approval.
- **A3.3 Revalidation:** bind all supporting source/unit/relationship revisions and inspected file hashes, including deeper follow-ups and dirty files. Check candidate inventories and newly relevant evidence, not only old support hashes.
- **A3.4 Safe partial reuse:** a preserving-change finding cannot authorize a policy change. Reuse unaffected applicable observations while revising dependent recommendations; incomplete indexes become leads, not complete current answers.
- **A3.5 Negative results:** empty bounded search is not global absence; permission denial/refusal/timeout is not domain evidence. Reconsider when capabilities or evidence change.
- **A3.6 Publication lifecycle:** durable views retain referenced observation manifests/artifacts even after answer-cache eviction. Add dependency indexes, invalidation events, journal recovery and purge coverage.
- **A3.7 Retrieval and cost:** use original unit retrieval alongside investigation candidates; avoid correlated-evidence double counting; record reused versus newly performed actions and revalidation cost.
- **A3.8 Tests:** AT-11–AT-13, AT-18–AT-19, AT-21–AT-22 and AT-24; new ADR with unchanged older files, same-size edit, deleted deeper-round file, reduced permission, partial inventory and evicted record cited by a persistent view.

### Deliverables and exit gate

A subsequent related request benefits from safely reusable findings. A new contrary source invalidates or changes the old conclusion even when original evidence remains unchanged. No current view loses its cited observations on eviction. A warm/cold comparison reports avoided work, actual revalidation overhead, stale-answer errors and cross-task leakage controls. Findings remain derived; explicit authoring is required only for new primary notes/policy, not for every reusable insight.

## 7. O1 — immediate orientation and guided project tours

**Purpose:** give a developer a truthful, inviting project mental model on first contact without a new GUI or a mandatory learner profile.

- **O1.1 Welcome:** proposed `lore onboard` displays the project purpose, a few central concepts and one suggested end-to-end workflow immediately, then optional Explore/Learn/First task paths.
- **O1.2 Workflow discovery:** identify coherent evidenced request/data flows, rank by explanatory value and constraints, not folder names or code size.
- **O1.3 Tour stops:** actual source landmarks with repository/source identity, why each matters, key decision/invariant, one failure path and a direct evidence link. Static inference must never be mislabeled as observed runtime execution.
- **O1.4 Accessibility and audience:** novices receive definitions; experienced newcomers can skip to an issue or deep code. Unknown experience must not trigger a long quiz.
- **O1.5 Document-only fallback:** tour source concepts and procedures when no code checkout is available; record coverage rather than invent code paths.
- **O1.6 Verification:** ON-01–ON-07, ON-11/12 and ON-16/20; source IDs, flow/temporal attribution, first-minute comprehension and navigation.

**Exit gate:** a newcomer can correctly identify the system's key responsibilities and locate one useful workflow/entry point with less time or better accuracy than baseline documents. Attractive prose alone is insufficient.

## 8. O2 — adaptive learning paths and tutorial-first Diátaxis

**Purpose:** convert a tour into a short project-specific learning experience.

- **O2.1 Concept prerequisites:** derive a fallible educational DAG separate from the source graph and zoom-view DAG. Model-proposed dependencies require provenance; handle cycles as co-learned modules rather than enforcing a false order.
- **O2.2 Path planner:** choose the smallest concept set needed for one meaningful flow or intended first task; allow detours, jumps and adjustable complexity. **No fixed four-level knowledge hierarchy.**
- **O2.3 Guided activity:** prompt one prediction, trace or small decision with grounded feedback and a clearly distinguished expected versus actually observed result.
- **O2.4 Scaffolded hints:** reveal definitions, source lines, rationale and worked solution only as helpful/requested; no punitive grades or forced exercises.
- **O2.5 Diátaxis transitions:** tutorial is the primary onboarding path; explanation clarifies why, reference resolves exact details, and how-to assists a real contribution.
- **O2.6 Runner-free default:** provide honest worked examples; only a separately authorized isolated runner may claim to execute tests. Source snippets never authorize commands.
- **O2.7 Validation:** ON-07–ON-11, ON-14 and ON-17–ON-19; check prediction rubrics, rare exceptions, learner control and no fake mastery claims.

**Exit gate:** the learner can explain or predict relevant unfamiliar behavior correctly; they can access original sources and switch depths directly without traversing a fixed curriculum.

## 9. O3 — first contribution and independent skill transfer

**Purpose:** turn conceptual understanding into a correct change and independent future work, not a demonstration of the assistant's coding ability.

- **O3.1 Real task entry:** prefer a user-supplied task. Suggest real starter work only from actual available work items; label generated alternatives as practice exercises, never invented issues.
- **O3.2 Task suitability:** clarity, scope, low risk, reversibility, decision constraints, testability and connection to learned concepts. Avoid dangerous “easy” migrations or approval-dependent changes.
- **O3.3 Companion:** investigate relevant code/docs/tests/history automatically within permissions; explain one viable approach, acceptance checks, common mistakes and optional hints. The learner retains authorship/control over the actual change.
- **O3.4 First-task verification:** use independent correctness/constraint checks when authorized or external reviewer evidence; static inspection is not execution.
- **O3.5 Faded guidance:** give a **different related** held-out task with fewer preselected hints; learner can still ask for help without being blocked or shamed.
- **O3.6 Judgment check:** explain why the change works and one condition where that approach would be wrong. Model-graded explanations are fallible feedback, not proof of mastery.
- **O3.7 Consent/resume:** optional minimal local learner state, inspect/export/reset, no hidden page tracking or employer reporting.
- **O3.8 Empirical study:** ON-12–ON-24; count developer-authored work separately from coding-agent-produced output, mentoring and transfer accuracy.

**Exit gate:** correctly completed first bounded change **and** demonstrated learning on a genuinely new task. Agent-only code completion does not satisfy the gate.

## 10. R1 — contextualized segments and retrieval

**Purpose:** improve the informational units where measurement shows value, without delaying A1–A3.

- **R1.1 Segmentation profile:** extend `src/sources.rs` with derived context metadata; preserve existing extraction/quote identities unless a demonstrated defect requires migration.
- **R1.2 Structures:** tables with headers/units, fences with purpose, procedures with order/rollback, ADR status/decision scope, examples with negative conditions.
- **R1.3 Retrieval headers:** deterministic/generated context is non-evidentiary metadata; preserve modality and prompt/profile fingerprint.
- **R1.4 Ablations:** original units versus contextualized units/chunks; lexical-only versus hybrid; exact identifier/path bypass and direct access to rare exceptions.
- **R1.5 Overlap/deduplication:** inherited context does not create independent corroboration or false merge across version/environment/modality.
- **R1.6 Updates/purge:** invalidate inherited context on changed/deleted/moved sources; true no-op retains bytes with zero inference.
- **R1.7 Tests:** table/procedure splits, repeated headings/quotes, large Unicode section, ADR acceptance, derived import, denied access and budget overflow.

**Exit gate:** measurable retrieval/downstream improvement without exact-reference, constraint-recall or authority regression. When improvement is negligible, keep current units and proceed with useful assistance/views rather than forcing a segmentation redesign.

## 11. R2 — evidence-preserving Knowledge Zoom

**Purpose:** let the user expand a useful answer into a working model, decision detail and exact evidence without navigating first.

- **R2.1 View storage:** versioned nodes, support, coverage, publications and dependencies in disposable derived state; DAG containment checks, migrations, retention and purge.
- **R2.2 Grouping:** compare topic/relationship, binary, four-way and adaptive overlapping groups. Structural node counts are not performance claims.
- **R2.3 Leaves:** source-bound knowledge and applicable investigation findings, with original evidence/observation manifests. No summaries used as proof.
- **R2.4 Parents:** constrained fields preserve critical constraints, rare exceptions, conflict/transition endpoints, qualifiers and included/eligible/omitted counts.
- **R2.5 Cross-level retrieval:** combine views with original lexical/path/semantic/relation access. An upper summary cannot exclude the only decisive lower-level fact.
- **R2.6 Validation/degradation:** source/scope/modal/temporal checks plus fallible semantic review; preserve valid partial help before falling back to an evidence index.
- **R2.7 Incremental invalidation:** source additions, edits, deletions, changed relationships and investigation counterevidence invalidate relevant parents, including untouched original topics.
- **R2.8 CLI/Markdown:** proposed `lore view SUBJECT` infers a sensible mode/depth. Explicit mode/depth/as-of options are advanced, not a required form. Preserve stable links and source status.
- **R2.9 Limits:** adaptive knowledge depth bounded by work/resource ceilings, **not** a fixed conceptual four levels; sparse on-demand builds, bounded nodes/fan-out/context/calls/deadlines and measured rebuild amplification; coherent staged publication and recovery.
- **R2.10 Usability:** compare orientation accuracy, navigation time, rare-condition discovery and evidence drill-down against current Lore and the simpler A2 presentation.

**Exit gate:** meaningful comprehension/retrieval benefit at measured cost, no erased critical conditions or false current/historical claims. Keep direct retrieval when it outperforms hierarchical routing. A graph canvas or dedicated UI is not required.

## 12. R3 — Diátaxis experiences

**Purpose:** fit the same investigated knowledge to the user's goal, not merely relabel generic templates. **In onboarding, Tutorial is the primary learning mode; Explanation, Reference and How-to appear exactly when the learner needs them.** Other Lore queries retain intent-driven selection, not forced tutorials.

### R3.1 Explain

Provide the decisive mental model, relevant history, recorded rationale and clearly labeled useful inference. Investigate ambiguity when it affects understanding; avoid invented causal bridges or pointless deep inspection for a simple concept question.

### R3.2 Reference

Use exact identifiers, values/types, inputs/outputs, errors and scope/version conditions. Preserve table context and numeric precision. Resolve material ambiguity using available evidence without delaying a known exact answer with unrelated history.

### R3.3 How-to

Provide goal, prerequisites, branches, safe order, implementation seams, rollback and observable completion. Complete available prerequisite investigations first. Separate completed checks, future implementation verification and external dependencies. Do not invent authority to change policy.

### R3.4 Tutorial

As the **primary onboarding mode**, prepare a bounded learner goal, prerequisite concepts, meaningful code/source landmarks, purposeful predictions, scaffolded hints, feedback, cleanup and a distinct transfer task. Exercises are appropriate because learning was requested, not because Lore delegates investigation. Start with non-executing worked examples; never claim tests ran when they were only read.

### Shared work

- **R3.5 Routing:** explicit user intent wins; sensible defaults, typed uncertainty/refusal and no cheap-model veto over critical evidence.
- **R3.6 Composition:** one knowledge/observation snapshot with mode-specific schemas and validators. Investigate gaps, then provide the closest useful labeled alternative rather than fabricate or refuse all help.
- **R3.7 Switching:** mode/depth changes reuse authorized evidence and completed work where applicable, not repeat discovery needlessly.
- **R3.8 Accessibility:** meaningful Markdown/JSON, logical headings, screen-reader status and no color-only authority labels.
- **R3.9 Study:** generic versus mode-specific responses, randomized tasks and controlled models/budgets; separately measure correct **human** first-task work, grounded explanation, lookup, independent transfer and mentor burden.

**Exit gate:** each mode has its own observed usefulness and failure boundaries. A failed tutorial experiment must not block good explanation/reference or core task assistance. No polished mode can compensate for missing critical evidence.

## 13. R4 — decisions, negative cases and investigate-first gaps

### R4.1 Decision lenses

Represent a decision's recorded problem/reasons/alternatives, documented assumptions, inferred conditions, applicability, review status and original evidence. Investigate possible condition changes before generating a review suggestion. Return the strongest provisional interpretation and action; use human attention only for consequential unavailable requirements or authority. Approving an interpretation is not superseding an ADR. Measure meaningful reconsiderations, false alarms and decision errors.

### R4.2 Negative and boundary cases

Add positive examples, counterexamples, historical failures, exceptions and incidents with source/environment/version. Attach them to relevant answers, how-tos and lenses. A rare case must survive compression when it changes action. A reported resolution is not a reproduced result; a historical failure is not a permanent prohibition. Reuse its conditions, not an unconditional ban.

### R4.3 Knowledge gaps

Investigate a gap's relevance and available evidence before surfacing it. Resolve from sources, approved tools or revalidated findings where possible; retain explicit non-blocking inference when useful. Keep non-material gaps quiet in ordinary assistance. Only an explicitly requested maintenance session should expose a ranked optional capture queue. Support declines/unknown, deduplication, expiry and attributable volunteered answers. Source authoring remains a separate authorized operation; ordinary investigation reuse requires no approval.

**Exit gate:** fewer material mistakes and less unnecessary human review. A remaining question is actually consequential, unavailable to Lore and answerable; it is not work that Lore merely declined to do. Authored testimony retains scope and provenance without becoming runtime proof.

## 14. R5 — worked cases and separately authorized replay

### R5A: no-execution cases

Define case IDs, scenarios, prerequisites, source/decision revisions, inputs, expected outcomes, negative branches, success/failure conditions and cleanup. Connect them to tutorial/how-to/reference/explanation as appropriate. Lore prepares examples using permitted static evidence and clearly labels expectations versus observations. External test descriptions can be ingested without executing them. Measure transfer to a distinct task, not only recognition of the presented example.

### R5B: isolated verification capability

R5B has a separate threat-model/sandbox RFC and adversarial isolation evidence. It is not necessary to implement A1–A3.

- Bind a standing operator grant to known command/capability IDs, pinned project inputs, approved runtime and resource/network policy. Lore may select a material check automatically within that grant; no confirmation for every execution.
- Deny arbitrary commands from source/model text, credentials, production data, host mutation and network by default. Enforce filesystem, process-tree, CPU/memory/disk/output/time limits and cancellation.
- Record actual argv, input/code/test hashes, tool/runtime, policy, observed results, exit status, output artifacts and redaction. Generated expected output is never execution evidence.
- Invalidate applicability when inputs/tests/runtime/permissions change. Retain historical outcomes honestly without a generic production-verified badge.
- Persist derived observations under cache policy; new primary project notes/policy remain explicitly authored. Do not require humans to approve every result merely to reuse it.
- Compare bounded execution with equivalent text/static cases, including setup overhead, cost, failures and security attacks.

**Exit gate:** enforceable isolation plus actual learning/task benefit, source/observation binding and zero unauthorized execution/egress. If isolation is absent, ship R5A and useful static assistance; do not substitute an unsafe shell or burden users with infrastructure setup to get an answer.

## 15. R6 — impact briefings, advisory guardian and optional reader

### R6.1 Consequential changes

Compare named publication/source baselines or optional explicitly acknowledged revisions. Explain what the user should now understand/do differently about decisions, assumptions, procedures, cases and uncertainties. Investigate important apparent changes; suppress editorial churn. Keep source edits, reported outcomes, changed interpretations and scoped observations distinct. Never infer what someone learned from opening a page. Proposed `lore changes` remains a separate versioned command.

### R6.2 Advisory guardian

Consume a supplied or actually inspected task/patch/revision and relate it to applicable rules, decision conditions, negative cases and checks. Complete available useful investigation before emitting a warning. Provide likely consequence, specific evidence, remedy and exact outstanding dependency—not an untriaged suspicion queue. Deduplicate unchanged findings and measure interruption cost, precision and missed material errors. No source edits, hooks or merge blocking by default; CI enforcement requires a separate policy decision.

### R6.3 Optional local reader

Reuse the same result/view contract for answer-first content, mode/depth controls, breadcrumbs, why/source/changes actions, visible critical constraints and expandable detail. Support correction, narrowing/cancellation and evidence inspection without unnecessary repeated work. Preserve offline Markdown/CLI access, keyboard/screen-reader usability, responsive layout, escaping/CSP and no implicit analytics, remote images/egress or executable Markdown.

**Exit gate:** faster accurate return-to-project understanding, useful low-burden warnings and no dependency on a visual client. A simpler reader that helps users act is preferable to a graph requiring them to reconstruct the answer.

## 16. R7 — research extensions

- **Cross-project analogies:** preserve namespace, authority, privacy, source correlation and conditions under which a lesson will not transfer. A candidate analogy cannot become merged policy.
- **Personalized explanations:** explicit local preferences/acknowledged baselines, portable/deletable; no inferred beliefs or hidden activity tracking.
- **Proactive workflows:** checks may be triggered by explicitly configured update/review workflows. Ordinary requests do not silently install background agents or notifications.
- **More efficient routing:** evaluate learned decision-value selection against bounded heuristics, including false-negative recall and high-cost mistakes. Scores never replace evidence or permission.
- **Alternative presentation:** diagrams, timelines or other media remain views of the same evidence and limitations. No interface-only source of truth.

Advance only with demonstrated demand and earlier outcome/security gates. Research experiments must not reopen the foundational commitment to useful autonomy inside existing permissions.

## 17. PR-sized delivery sequence and traceability

Package IDs A01–A12, O01–O10 and G01–G04 extend the original 01–19 work packages. They are planning identifiers, **not actual GitHub PR numbers**. M0 uses small subsets from A01/A02/A03/A06, O02/O03/O05 and G01/G02; do not implement each full phase as a prerequisite. Split changes further as needed and track separately assessed human/agent outcomes.

| Package | Phase | Scope | Required evidence |
| --- | --- | --- | --- |
| A01 | R0 | Autonomy fixtures, burden rubric and matched 0.6 baseline | Reproducibility and no solved-task leakage |
| A02 | A1 | Capability manifest, trust resolution and legacy policy | Denied grants, untrusted config, restrictive overrides |
| A03 | A1 | Typed actions/controller and shared budgets | Simple task no-op, bounded calls and byte limits |
| A04 | A1 | Material uncertainty and counterevidence revision | Recommendation changes when premise fails |
| A05 | A1 | Stopping, cancellation and progress events | No endless loop; no background continuation |
| A06 | A2 | Action-first future response schema and composer | Exact lookup/partial progress/true blocker examples |
| A07 | A2 | Delegation/escalation and noninteractive contract | Available check completed, no stdin waiting |
| A08 | A2 | Component validation and useful fallback | Invalid clause cannot poison unrelated findings |
| A09 | A3 | Investigation records/store and permission inheritance | No-cache/read-only/purge/source authority |
| A10 | A3 | Freshness, new-evidence search and selective reuse | New ADR, dirty/deeper file, incomplete inventory |
| A11 | A3 | Durable publication dependencies and eviction | No dangling citations after cache removal |
| A12 | A1–A3 | Matched outcome evaluation and default migration | Benefit versus inspected 0.6; correctness and latency gates |
| G01 | R0/G1 | Versioned coding-agent JSON contract with schema-2/3/4 compatibility | Required source manifests, readiness, budget and fallback integrity |
| G02 | G1 | Context composer using permitted investigated observations | Actual inspected seam, no invented code/test execution |
| G03 | G1 | Optional agent consumption guidance and noninteractive errors | Different agent wrappers, no required MCP/service |
| G04 | R0/G1 | Independent agent coding-task benchmark | OpenWiki/current Lore comparisons, constraint preservation and real cost |
| O01 | R0 | First/second task benchmark and independently reviewed rubric | No leaked solutions, real novice task outcomes |
| O02 | O1 | Immediate grounded orientation and newcomer entry | ON-01/02 and document-only fallback |
| O03 | O1 | End-to-end tour with real code/source landmarks | Static-versus-runtime basis and exact refs |
| O04 | O2 | Fallible concept prerequisite DAG and planner | Cycles, small curricula, no forced gates |
| O05 | O2 | Tutorial/feedback/hints with no-execution fallback | Grounded prediction and no fabricated pass |
| O06 | O3 | First-contribution companion | Real vs practice tasks, safe readiness and scope |
| O07 | O3 | Voluntary local learner state | Opt-in, export/reset, revision status |
| O08 | O3 | Reduced-scaffolding second task evaluator | Independent correctness and user authorship |
| O09 | R2/O2 | Adaptive Knowledge Zoom concept navigation | Arbitrary meaningful depth with resource caps |
| O10 | O1–O3 | Newcomer accessibility and outcome evaluation | First contribution plus genuine transfer |
| 01 | R0 | Original knowledge adversarial gold | Provenance, chronology and exception preservation |
| 02 | R0/R2 | View schema and CLI interface contract | Version, scope, intent and budget checks |
| 03 | R1 | Contextual segments | Table, procedure, ADR and Unicode identities |
| 04 | R1 | Retrieval ablations | Exact lookup, rare constraints and false merges |
| 05 | R2 | Derived views, IDs and DAG | Migrations, rollback, purge and unknown refs |
| 06 | R2 | Evidence-closed leaf/parent synthesis | Mandatory qualifier and support checks |
| 07 | R2 | Cross-level retrieval | Direct bypass and conflict endpoints |
| 08 | R2 | Invalidation/staged publication | New source, deleted evidence and true no-op |
| 09 | R2 | Simple view/Markdown/JSON surface | Defaults, evidence drill-down and partial status |
| 10 | R3 | Explain/reference renderers | Numeric, authority and temporal correctness |
| 11 | R3 | How-to integrated with adaptive checks | Completed prerequisites versus future code tests |
| 12 | R3 | Tutorial/worked examples | Transfer task, no pretend execution |
| 13 | R4 | Investigated decision conditions | No automatic policy supersession |
| 14 | R4 | Negative/boundary cases | Rare-case retrieval and version invalidation |
| 15 | R4 | Investigate-first gaps and optional notes | No routine review burden; explicit authoring |
| 16 | R5A | Worked case browsing | Scope, learner feedback and cleanup |
| 17 | R5B | Separate safe-runner RFC/harness | Escape/egress/resource/cancel tests |
| 18 | R6 | Consequential changes/advisory guardian | Triage value and interruption burden |
| 19 | R6 | Optional reader | Accessibility, XSS/egress and offline fallback |

Package 17 is a new capability boundary, not a prerequisite to completing investigations with already available tools. APIs/schemas should share existing 0.6 types where appropriate; avoid a framework rewrite to accommodate one extra action.

## 18. Dependencies and first release boundary

```mermaid
flowchart TD
  R0[R0 two outcome tracks and shared contract] --> B[Bounded A1/A2 common intelligence slice]
  B --> O1[O1 short grounded orientation and tour]
  O1 --> O2[O2 one optional learning activity]
  B --> G1[G1 coding-agent action JSON]
  O2 --> M0[M0 assessed shared-intelligence prototype]
  G1 --> M0
  M0 --> O3[O3 first contribution and transfer]
  M0 --> A3[A3 investigation reuse]
  R0 --> R1[R1 contextual retrieval experiments]
  R1 -. if useful .-> R2[R2 adaptive Knowledge Zoom]
  O1 --> R2
  O2 --> R3[R3 full Diataxis modes]
  A3 --> R4[R4 decision conditions and negative cases]
  R3 --> R5A[R5A worked cases]
  R5A --> S[Separate safe runner review]
  S --> R5B[R5B authorized replay]
  R4 --> R6[R6 guardian and change impact]
  R2 --> UI[Optional reader]
  R3 --> UI
  R6 --> R7[R7 future research]
```

**M0 is an assessed engineering prototype, not the complete onboarding or agent-performance product.** One human experience and one agent package use one knowledge core on a representative project. The prototype exits only after valid source/permission/provenance behavior, explicit legacy compatibility, meaningful failure paths and an independently reviewed human concept/agent coding example. Do not infer population-level benefit from these examples.

**Next product-level gates are independent:** O3/newcomer cohorts must demonstrate an understood correct first task and less-assisted transfer. G1/A1–A3/agent cohorts must demonstrate better **correctness-constrained** coding-task efficiency against comparable current tooling. Both use held-out projects, cost accounting and meaningful baseline arms.

**What waits:** full multi-level corpus compilation, a large curriculum/catalog, persistent learner profile, mandatory agent integration, runtime/sandboxed execution, new connectors, bespoke GUI and cross-project transfer. None is necessary to show that shared project intelligence provides value to both audiences.

## 19. Validation, rollout, risks and definition of done

### 19.1 Test layers

Unit tests cover typed actions, state transitions, policy intersections, source IDs, hashes, scopes, DAGs, budgets and response contracts. Property/mutation tests cover source additions/deletions/moves, changed relationships, dirty/same-size/deeper file changes, incomplete indexes, no-op, stale reuse and crash recovery. Adversarial tests cover injection, forged citations, negation, scope/policy mistakes, hidden critical exceptions, secret egress, malicious runner requests and poisoned memory.

Real-model tests independently measure **human learning** (orientation, grounded workflow explanation, first correct contribution, different-task transfer, mentor burden) and **agent task outcomes** (real implementation correctness, missed constraints, rework, duplicated investigation, time/cost), along with reasoning errors, false causal/temporal claims and inappropriate authority. Keep baseline code/model/tool/snapshot budgets comparable; report source preparation cost separately. General product tests also measure correction effort, cancellation, exact lookup, warnings and gap-capture burden. Runner/reader security tests are separate from prompt/schema tests.

### 19.2 Metrics and anti-gaming

**Two independent outcomes:** (1) human onboarding: correct and understood first contribution **plus** correct second task with less help; (2) coding agents: a correct constraint-respecting change with fewer material mistakes and less total effort. Track experienced-developer decision quality as a direct-assistance use case, not as novice learning. Agent-authored code cannot satisfy the human learning score; a strong tutorial cannot substitute for weak agent correctness. Report task completion, avoidable delegation, true external dependencies, false blocking, unsafe proceeding, unnecessary investigation, clarity/agency, reuse benefit, cold/warm p50/p95 and actual usage/billing where known.

**Human track:** compare original docs, OpenWiki where available, Lore 0.6 and adaptive onboarding with controlled newcomer tasks, mentors and assistance, including distinct held-out transfer.

**Agent track:** compare original code/docs, OpenWiki alone, Lore 0.6 fast/intelligent/inspected (where relevant), adaptive Lore without/with reused investigation, and optional OpenWiki + Lore. Fix the coding agent, tools, source snapshot, allowed privileges and independently scored real task. Record context-preparation cost separately from warm recurring use; don't silently grant one arm more source access.

**Autonomy ablations:** retain explicit inspected/investigated 0.6 and an always-investigate arm to isolate routing from larger budgets. Later add richer retrieval/views or execution separately. Keep the same coding model, source snapshots and external correctness checks; randomize and isolate caches. A reused prior solution or generated tutorial must not leak held-out answers. A first contribution completed entirely by a coding agent counts as agent completion, not demonstrated developer competence.

Pre-register improvement/non-inferiority targets and sample size after R0 baselines, before scoring candidate output. Zero questions, zero blockers, many checks or confident prose cannot be a standalone target. Hard safety/provenance gates remain mandatory; empirical task scores remain honestly unmeasured until executed.

### 19.3 Rollout and rollback

Prototype the new controller behind an explicit experimental switch. The intended mature product is adaptive by default inside accepted permissions, not a permanent collection of per-query flags. Ship default changes only after version/migration, security, outcome and latency review. Existing deny settings cannot become grants during migration; registry-only adaptive reasoning remains useful.

Migrate derived stores independently, retaining accepted evidence and legacy schemas. Revert a failed controller/view feature without losing source history. A previous publication is a valid current fallback only if its dependencies remain current; otherwise label historical/stale and deliver fresh eligible findings. Separate optional modes/runner/UI so their failures do not break core context or reference. Respect cancellation and no-cache across all new stores.

### 19.4 Risk register

| Risk | Mitigation |
| --- | --- |
| Caution reappears as a required human review queue | Investigate-first contract; ordinary derived reuse needs no approval; measure burden |
| Low blocker count hides unsafe action | Scoped readiness, explicit preconditions, unsafe-proceeding gate |
| Autonomy means endless expensive exploration | Decision-value actions, no-information-gain stop, shared ledger and cancellation |
| Automatic initiative becomes surprise access | Standing trusted grants, deny-preserving migration, typed capabilities |
| Reuse turns speculation into truth | Original provenance, applicability, new-evidence checks and no authority promotion |
| New ADR ignored because old hashes match | Corpus/relationship change cursor, bounded relevant-evidence scan, incomplete-index rules |
| Summary erases a rare decisive exception | Critical inclusion manifests and adversarial retrieval tests |
| Partial verification discards all useful work | Dependency-aware repair/withdrawal and truthful partial output |
| Fallback preserves advice but removes its qualification | Atomic recommendation/condition handling; narrow or withhold action |
| Runner escapes or leaks data | Distinct tested isolation, no arbitrary commands, explicit data-class egress |
| Guardian overwhelms the user | Investigate/triage first, deduplicate, severity/applicability and interruption measures |
| Source-derived summaries leak under reduced access | Restriction inheritance through views/vectors/records and permission revalidation |
| UI implies user knowledge from page views | Only explicit activities or acknowledged baselines; no inferred mastery/profiling |
| Tour teaches a fabricated code flow | Ground every stop and label static versus runtime evidence |
| Assistant silently completes learner tasks | Distinguish developer-authored work from agent execution; independent transfer study |
| Onboarding becomes a mandatory quiz | Explicit learner intent, skip/jump controls, non-punitive hints and no gates |
| Generated practice task masquerades as issue | Source-backed real work items only; clearly label exercises |
| Knowledge/learning depth fixed to four levels | Adaptive meaningful hierarchy with bounded computation, not fixed ontology depth |
| Generated output overwrites authored text | Manifest ownership, explicit rebuild and staged publication |

### 19.5 Definition of done

Every ticket has a concrete user journey; out-of-scope/failure behavior; source/derived authority boundary; versioned request/result and migration; actual permission/resource/write effects; unit/negative/adversarial tests; invalidation/no-op/purge/publication as applicable; user help and honest examples; measured result or explicit unmeasured status; and rollback without accepted-evidence loss.

Onboarding tickets demonstrate source-bound tours, intentional practice, learner control, privacy and assessed first-task or transfer outcomes. **Agent tickets** demonstrate stable machine contracts, concise task-specific output, evidence/observation resolution, valid readiness, completed-vs-future checks and independently assessed implementation/constraint outcomes. Autonomy tickets demonstrate no avoidable handoff within the permitted envelope, correct stopping, scoped partial progress, cancellation and counterevidence revision. Execution tickets require independent sandbox evidence. Model-quality tickets need a real-model evaluation plan/results and actual identities/usage; fixture success is never relabeled empirical utility.

## 20. Settled direction, remaining choices and references

| Question | Decision/default for this proposal | What remains to measure/design |
| --- | --- | --- |
| Who owns available investigation? | Lore; humans are not the default fallback | Typed adapter coverage and task-specific value |
| Everyday effort selection? | Automatic inside standing permissions | Numerical limits and calibrated routing |
| Must uncertainty be eliminated? | No; resolve what changes action, expose material residual conditions | Sufficiency/error and burden trade-offs |
| What ships first? | **M0:** shared bounded A1/A2 + small O1/O2 human path + G1 agent package | Human comprehension and agent implementation examples; no implied measured superiority |
| When is onboarding complete? | O3: first understood correct contribution plus distinct less-assisted second task | Independent study and learner authorship |
| When is agent benefit established? | G1/A1–A3 real task improvements with constraints preserved | Independent coding checks, time/cost and matched OpenWiki/Lore baselines |
| Primary audience? | Humans (newcomers, experienced developers, maintainers) **and coding agents** | Newcomer learning and agent task quality evaluated separately |
| What proves success? | Better project understanding and **correct work** with less avoidable effort, per audience | Human transfer, real agent implementations, provenance and full cost |
| Role of Diátaxis? | Tutorials lead onboarding; Explanation/Reference/How-to are just-in-time | Validate switching without forced quizzes |
| Does every finding need approval? | No; derived findings reuse automatically under policy | Explicit source-authoring review remains separate |
| How to handle true blockers? | Exact missing decision/observable plus meaningful safe work | Do not fabricate safe progress where none exists |
| Hierarchy and segmentation? | Improve when measured; never required navigation | Grouping, salience and stable identity |
| Execution? | Separate approved runner; automatic choice inside grant only | Isolation implementation, cost and actual benefit |
| Reader/personalization? | Optional; CLI/Markdown first, no hidden tracking | Demonstrated usability and privacy value |
| Cross-project transfer? | Research with strict scope and authority separation | Reliable permissions and actual usefulness |

Project references: [coding-agent intelligence design](CODING_AGENT_INTELLIGENCE_DESIGN.md), [developer onboarding design](DEVELOPER_ONBOARDING_DESIGN.md), [autonomy and UX contract](AUTONOMOUS_ASSISTANCE_DESIGN.md), [full knowledge architecture](KNOWLEDGE_EXPERIENCE_DESIGN.md), [vision](../VISION.md), [existing design](../DESIGN.md), [0.6 behavior](V06.md), [0.6 decision evaluation](../evaluation/DECISION_INTELLIGENCE.md).

Research/product context remains [Diátaxis](https://diataxis.fr/), [RAPTOR](https://arxiv.org/abs/2401.18059), [GraphRAG](https://arxiv.org/abs/2404.16130) and [OpenWiki](https://github.com/langchain-ai/openwiki). They motivate experiments, not claims of novelty or measured gains for Lore.

**Acceptance question:** Did the **same evidence-backed, permission-aware Lore engine** make a newcomer better at understanding/contributing **and** help a coding agent produce a more correct change with less wasted investigation, without substituting one success for the other?
