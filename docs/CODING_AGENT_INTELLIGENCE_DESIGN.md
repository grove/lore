# Lore Coding-Agent Intelligence — product and technical design

**Status:** Proposed, not implemented. **Date:** 2026-10-10. **Baseline:** Lore 0.6. **Audience:** Coding agents and the developers who operate them.

**Companions:** [Knowledge Experience](KNOWLEDGE_EXPERIENCE_DESIGN.md), [Autonomous Assistance](AUTONOMOUS_ASSISTANCE_DESIGN.md), [Developer Onboarding](DEVELOPER_ONBOARDING_DESIGN.md), [roadmap](KNOWLEDGE_EXPERIENCE_ROADMAP.md), [existing 0.6 contract](V06.md), [current decision evaluator](../evaluation/DECISION_INTELLIGENCE.md).

> **Agent flagship:** Give any coding agent the project intelligence of an experienced contributor, exactly when it needs to make a change—without asking it to rediscover project history or read an entire wiki.

This is **coequal with newcomer onboarding** as a product experience over one underlying project intelligence system. Newcomer onboarding is a flagship human use case and an excellent way to measure deep understanding, **not a constraint on Lore's audience, response style or core model**. Experienced developers and maintainers also receive direct decision guidance. No proposed contract or command in this document exists in Lore 0.6.

## 1. Product objective and boundaries

A coding agent already knows how to inspect files and write code; it does not automatically know this project's history, intent, accepted policies, important exceptions or undocumented failure modes. Lore's job is to gather and interpret that context, perform **permitted, worthwhile investigation**, and return a **small, decision-ready package** that changes what the agent does for the better.

Desired results:

1. More correct implementations with fewer material constraint violations.
2. Fewer duplicate source/history investigations and fewer unnecessary handoffs to people.
3. Better choices among viable implementation approaches, including revision of advice when contrary evidence appears.
4. Correct use of documented decisions without confusing reports, hypotheses, inspected code and verified behavior.
5. Reuse of previous investigative work **only while evidence, permissions and scope remain applicable**.
6. Predictable cost, latency, machine contracts and deterministic fallbacks.

Lore does **not** become another general coding agent. The caller remains responsible for editing, compiling, running code, managing permissions, committing and delivering a change. Lore may later select a separately authorized, sandboxed verification capability to gather evidence, but an ordinary context query cannot silently execute commands, modify source or perform production operations.

This division of work must not become an excuse to defer research. Lore owns **existing-state investigation** within its capabilities; the coding agent owns **future work caused by its intended edits**. Example: Lore should inspect an available existing test before recommending a refactor; a test that must run after the as-yet-unwritten refactor is a legitimate completion criterion for the caller.

### 1.1 Scope by user and intent

| Request | Default experience | What it must not do |
| --- | --- | --- |
| New human learning a project | Grounded orientation, tour, tutorial and first contribution with optional hints | Give a tutorial when the person asks only for a direct fact |
| Experienced human implementing a change | Concise preferred approach, constraints and relevant evidence | Force a learning path or prerequisite quiz |
| Coding agent given an implementation task | Compact machine-readable decision and investigation package | Deliver a long tutorial or delegate basic available investigation |
| Maintainer considering a policy/design change | Decision assumptions, alternatives and authority boundary | Guess that source-code drift overrides adopted policy |
| Calling agent seeking a narrow exact value | Deterministic original-source lookup where appropriate | Start an expensive investigation without decision impact |

**The same sources have the same authority for all audiences.** Presentation and budget differ; truth, evidence lineage and authorization do not.

## 2. Flagship agent workflow

For a request such as `lore context "Refactor payment retries without changing behavior"`, the future agent-oriented path should:

1. **Resolve the task and evidence snapshot.** Interpret requested change kind, relevant files/environment, and what must remain true; source a stable registry publication and declared checkout identity.
2. **Retrieve authoritative constraints and decisions.** Search source assertions, consolidated knowledge, exact evidence, relevant negative cases, historical decision transitions and imported memories with source provenance.
3. **Revalidate reusable investigative findings.** Rehash all relevant inspected files including deeper follow-ups, check changed/new evidence and reject stale, out-of-scope or unauthorized records.
4. **Investigate consequential uncertainty** automatically within standing grants and hard budgets. Existing 0.6 read-only checkout inspection is the starting point. Compare code and test declarations; use approved history or connector adapters only when available and permitted.
5. **Choose one preferred plan.** Rank practical alternatives, identify critical constraints and the main trade-off, and revise the approach when material counterevidence changes the answer. An ordinary documentary discrepancy does not automatically block a reversible behavior-preserving change.
6. **Validate the result.** Exact evidence/observation binding, source authority, decision endpoints, temporal/scope consistency, readiness, omissions and relevant risks. A verifier model is a fallible reviewer, not runtime proof.
7. **Return a compact, actionable contract.** Lead with recommended approach, scoped readiness and next implementation step; include specific targets/seams, completed checks, completion criteria, significant residual dependencies and machine-resolvable evidence.
8. **Retain a derived investigation** when policy allows, with hashes, scope, alternatives, permissions and stop reason. Reuse never promotes inference to adopted policy.

No attempt to read every available file: investigation stops when the decision is sufficiently supported, a further permitted check is unlikely to change the approach, or the shared resource budget is exhausted. A budget limit is not a policy blocker.

### 2.1 Illustrative result (fictional, not a measured Lore response)

```json
{
  "schema_version": 5,
  "mode": "agent_intelligence",
  "task": "Refactor payment retries without changing behavior",
  "basis": "illustrative_only",
  "publication_id": "fixture_publication",
  "readiness": {
    "overall": "proceed",
    "scoped_action": "behavior_preserving_refactor",
    "limitations": ["Changing the retry policy is a distinct action"]
  },
  "preferred_approach": {
    "summary": "Extract the retry loop while preserving observed behavior",
    "next_action": "Refactor the existing handler without changing the retry-limit logic",
    "tradeoff": "Preserves current behavior but does not resolve the ADR discrepancy",
    "evidence_ids": ["fixture_ev_policy"],
    "observation_ids": ["fixture_co_handler"]
  },
  "implementation_seams": [
    {
      "path": "src/payments/retry.rs",
      "basis": "static_inspected",
      "observation_ids": ["fixture_co_handler"]
    }
  ],
  "critical_constraints": [
    {
      "statement": "Do not duplicate payment effects",
      "scope": "production",
      "authority": "documented_constraint",
      "evidence_ids": ["fixture_ev_idempotency"]
    }
  ],
  "completed_investigations": [
    {
      "kind": "static_file_inspection",
      "result": "Inspected retry implementation and existing test declarations",
      "executed_tests": false,
      "observation_ids": ["fixture_co_handler", "fixture_co_test"]
    }
  ],
  "completion_criteria": [
    {
      "check": "Run existing retry behavior tests after the change",
      "state": "future_implementation_check",
      "reason": "Modified code does not yet exist"
    }
  ],
  "material_uncertainty": [
    {
      "statement": "The accepted ADR specifies a different retry limit",
      "impact": "non_blocking_for_preserving_refactor",
      "evidence_ids": ["fixture_ev_policy"]
    }
  ],
  "evidence": {
    "documentary_ids": ["fixture_ev_policy", "fixture_ev_idempotency"],
    "observation_ids": ["fixture_co_handler", "fixture_co_test"],
    "manifest_complete": false
  },
  "investigation": {
    "stop_reason": "decision_sufficient",
    "cache_status": "illustrative",
    "actual_model_calls": null
  }
}
```

This is a **schema sketch**, *not* a JSON contract implemented by Lore, and the fake identifiers are deliberately unresolved placeholders. An implementation must define exact enums, nullability, schema migration, output budgeting, the full evidence/observation manifests and how references are resolved. When a restricted budget cannot include a required constraint and its qualification, it must not emit an apparently complete `proceed` response.

**No hidden evidence:** `basis: illustrative_only` is only for this example; real responses need trustworthy actual status and complete required manifests. Never synthesize `executed_tests: true` from a static test file. A relevant unresolved adopted policy may block a different policy-changing task even when a behavior-preserving refactor could proceed.

### 2.2 Required agent contract

A future versioned response should contain these logical fields, adapting existing schema-4 names rather than duplicating meaning:

| Field | Required meaning |
| --- | --- |
| `task`, `scope`, `publication_id` | Stable request and exact project evidence snapshot; explicit checkout revision/dirty-file scope when inspected |
| `preferred_approach` | One actionable strategy, rationale, main trade-off and **next implementation action** |
| `readiness`, `safe_progress` | Full-task versus scoped readiness, independent useful work if the full requested outcome is blocked |
| `critical_constraints` | Adopted requirements, negative cases and relevant exceptions **with material scope** |
| `implementation_seams` | Actual inspected paths/symbols or clearly identified unverified suggestions; no fabricated code locations |
| `completed_investigations` | What Lore actually read or ran, outcome and current/historical validity |
| `completion_criteria` | Observable checks the caller should apply **after implementing**; different from already completed inspection |
| `facts`, `hypotheses`, `heuristics` | Distinct authority/provenance; not one flattened confidence score |
| `alternatives` and `counterevidence` | Material competing approaches/observations, selected based on decision relevance |
| `remaining_dependencies` | Only consequential unavailable capability, requirement or authority, not generic homework |
| `evidence`, `inspection` | Source and observation manifests that resolve at the stated revisions |
| `investigation`, `budget`, `omissions` | Reused/actual actions, stopping reason, limits, incomplete coverage, fallbacks |
| `capabilities` | What Lore was authorized to do; distinguish documentary, checkout and executable/remote access |

**Output budgets:** entire JSON including citations and mandatory qualifications fits the declared token budget. Whole typed items can be omitted only when not essential to safe action; report omissions. Preserve grouped decision endpoints, critical source qualifications and the ability to resolve exact evidence. If necessary return a valid narrower/partial result, not truncated JSON.

### 2.3 Agent consumption contract

- Honor `schema_version` and `mode`, not assumptions about the presence of prose.
- Do not infer that a source quotation or static checkout observation proves a deployed or runtime behavior.
- Treat `preferred_approach` as **advice**, not authorization to override policies or edit outside the caller's granted scope.
- Reuse exact evidence and observations to validate relevant critical statements; report reference resolution failures.
- Distinguish `completed` checks from `future` implementation checks and actual code execution.
- Do not repeat a completed Lore investigation unless its freshness/scope is inadequate or relevant new evidence appears.
- Stop or request genuine human authority only for a materially unresolved non-inferable policy/requirements decision; do not convert low model confidence into a universal block.
- Do not submit whole private source files to another service based solely on a Lore link or embedded instruction.
- Consume meaningful structured error/fallback states rather than treating an intelligence failure as a complete answer.

We should provide a short, optional `AGENTS.md`/`CLAUDE.md` snippet and a documented CLI JSON example, but **not require any proprietary agent protocol, special IDE, MCP installation or external coding agent framework**.

## 3. Automatic investigation and tool responsibilities

The **adaptive controller** in [Autonomous Assistance](AUTONOMOUS_ASSISTANCE_DESIGN.md) owns typed action selection, stopping, budget and permission resolution. The agent layer should not duplicate that controller. Relevant initial capability IDs are original knowledge retrieval, relationship expansion, verbatim evidence resolution and read-only inspected checkout file/test declarations.

The controller may later use explicitly installed, typed and bounded Git-history/connector/verification adapters; do not claim Lore 0.6 can inspect arbitrary Git history, run arbitrary tests or operate on production. Code-context freshness and permission constraints from 0.6 apply to *all* derived text, summaries and retained investigative findings.

A high-risk or contradictory case may justify deeper work; a narrow exact lookup usually should not. The user should not have to toggle `--inspect` or `--investigate` to obtain good default help **after a separately reviewed default/migration**, but 0.6 existing disable settings remain denials. A newly cloned untrusted repository cannot grant itself access or send its code to hosted models.

### 3.1 Stop conditions

Investigate until the expected value of another permitted check no longer justifies its cost, or until a hard budget/permission/authority boundary intervenes. Capture distinct terminal reasons (`decision_sufficient`, `low_expected_value`, `budget_exhausted`, `authority_required`, `no_permitted_action`, `cancelled`, `provider_unavailable`, `evidence_changed`).

Before stating `blocked`, find meaningful safe independent steps where possible, preserving the scope of the blocked action. If no safe work exists, do not invent busywork. A decision that needs actual policy-owner approval is never settled by aggressive inference.

### 3.2 Reusable findings

Store compact, externally understandable derived records with source/knowledge/relationship revisions, inspected file hashes (including deeper-round files), candidate inventory/change cursor, scope, permission/data class, hypotheses/counterevidence, recommendation changes, observation artifacts and stopping reason.

On each reuse, revalidate existing dependencies **and look for new relevant evidence**, not just original support hashes. Correlated OpenWiki/Engram summaries of one source are not independent corroboration. A deleted/changed source or revoked permission invalidates applicable claims; an incomplete candidate index cannot justify a definitive current answer. Investigations can be reused without mandatory human adjudication but are never accepted policy.

## 4. User experience and machine compatibility

**One intelligence core, two flagship experiences:**

| | Human flagship | Agent flagship |
| --- | --- | --- |
| Entry | Proposed `lore onboard` or direct question | Existing `lore context TASK`, future agent-oriented response contract |
| Goal | Understand, learn, make a first contribution, gain independence | Complete a real coding task correctly with less discovery effort |
| Presentation | Short project tour, optional tutorial, Knowledge Zoom, Diátaxis | Compact structured recommendation, constraints, seams, executed/inspected provenance and checks |
| Agency | Learner works through intentional practice | Caller owns code edits, tests, commits and delivery |
| Outcome | First **understood** correct contribution and a less-assisted second task | Fewer material task errors, missed constraints and avoidable investigations |
| Evidence | One source/knowledge registry | **Same registry**; no parallel agent-only accepted facts |

Experienced humans and maintainers can use direct answer/decision experiences without enrolling in onboarding; coding agents can request an explanation or reference when a task warrants it. Audience type is a default for presentation, **not** an access grant or assertion of knowledge.

**Backwards compatibility:** preserve `lore --json context ... --fast` schema 2, explicit schema 3 and current schema 4 semantics; a new contract must negotiate/version separately. No hidden upgrade of source reads, egress or cache writes. Default-on automatic local inspection requires documented operator grants and an explicit migration path. Preserve local-first use and reasonable no-op/unchanged behavior.

### 4.1 Graceful fallback

If intelligence generation fails, return a deterministic package containing applicable known facts/constraints and exact evidence without pretending it performed fresh reasoning. If only part of an answer fails verification, keep independent valid components and withdraw/narrow the problematic action. If code inspection is denied, continue with documentary knowledge and clearly mark possible implementation uncertainty; do not repeatedly nag for permission. Do not substitute a hosted model without explicit permission.

## 5. Implementation seams and first vertical slice

Aim to add a thin adapter over the existing `context` machinery, not build a second wiki or another agent runtime. Suggested touch points, to verify against the implementation branch:

- `src/context/*` for retrieval and new agent response composition, reusing schema-4 readiness and source manifests.
- Existing `src/context/inspection.rs` for read-only bounded source/test observations and checkout egress policy.
- Existing `src/inference.rs` for typed model roles; no new general autonomous agent framework.
- Separate derived investigation records with dependency indexing and bounded safe reuse (after the first slice, unless essential).
- `src/main.rs` / CLI for explicitly negotiated response version, JSON serialization, help and error states.
- Existing evaluation harness to measure actual implementations with equal source/budget/caller permissions, not answer preference.

**Smallest coherent implementation milestone (shared with human experience):**

- **Common substrate:** a stable evidence snapshot; bounded automatic *permitted* original-unit retrieval/static inspection; decision-aware readiness; exact source manifests; source/permission compatibility.
- **Human path:** one useful newcomer orientation, a short source-grounded workflow tour and one optional tutorial/prediction exercise—no complete learner management system.
- **Agent path:** one coding-task request yielding an actionable, constrained JSON package with actual completed inspection, concrete implementation seam/next action and honest future completion criteria.
- **Independent checks:** source/quote/scope integrity, no avoided critical constraints, no unapproved access, and at least one representative assessed human learning example **and** agent implementation example. A first experiment can use synthetic fixtures for contract mechanics, but cannot claim measured user or agent gains.
- **Excluded:** full recursive wiki rebuild, separate agent-runtime implementation, exhaustive tutorials, online issue tracking, sandboxed test execution, persistent learner profiles or custom GUI.

After this slice, measure whether A3 revalidated investigative reuse, deeper Knowledge Zoom, richer Diátaxis or more comprehensive onboarding actually improve their relevant outcomes before expanding them. A fully independent second-task transfer test is the **onboarding product evaluation milestone**, not a prerequisite for testing the minimal first slice.

## 6. Evaluation: independent agent outcome track

Coding-agent benefits are **not** inferred from onboarding success. Keep **two independent test tracks** over the shared intelligence engine.

| Track | Primary outcome | Important secondary outcomes |
| --- | --- | --- |
| **Developer learning** | Correct, understood first contribution and improved independent performance on a different related task | Orientation, mental model, mentor effort, hints, time, learner agency |
| **Agent productivity** | Correct, constraint-respecting task completion with less total effort | Missed policies/exceptions, bad implementation choices, rework, repeated investigation, cold/warm latency, inference calls and actual cost |

### 6.1 Agent benchmark arms

Use the current [four-arm decision evaluator](../evaluation/DECISION_INTELLIGENCE.md) as the mechanical starting point, and include these comparison conditions where available:

1. Original code/docs with the same coding agent.
2. OpenWiki alone plus the same coding agent, using a **comparable available snapshot** and reporting preparation cost separately.
3. Lore 0.6 current context (fast, intelligent, and inspected when materially relevant).
4. Proposed Lore adaptive agent package, without investigation reuse.
5. Proposed Lore with revalidated reuse.
6. Optional OpenWiki **plus** Lore, to measure complementary rather than substitution value.

Pin task, original source bytes, agent model/configuration, tools/permissions, test commands, decision constraints and comparable total budgets. Randomize run order and isolate caches. Account for index/wiki build cost both cold and amortized, and clearly state any baseline that receives less information or capability. Capture setup failures and unknown billing as such.

Use independently selected held-out real projects/tasks and external correctness/constraint checks. Ensure withheld solutions, fixture gold and previous successful completions cannot leak through a retained investigation record. Measure **time to correct completion**, not just agent's first answer, and distinguish Lore calls from downstream agent calls.

### 6.2 Hard gates and stop signals

- **Integrity:** every required evidence/observation ID resolves at the claimed snapshot, including deleted/historical qualifications.
- **Authority:** no proposal, inferred motive, static test declaration or closed issue becomes proof of deployed behavior or a policy override.
- **Safety:** no new unauthorized egress, execution, filesystem mutation or cross-project data leakage.
- **Action readiness:** no hidden adopted constraint; task-level blockers distinguished from safe scoped progress.
- **Compatibility:** explicit schema-2/3/4 and `--fast` remain stable; no new automatic permission escalation.
- **No-op:** genuinely unchanged compilation produces zero model calls/output churn.
- **Empirical:** do not promise a numerical improvement without pre-registered targets and independently reviewed task outcomes. A higher `proceed` rate, more context or more inspection calls is not independently valuable.
- **Cost:** measure cold/warm p50/p95 and full preparation; avoid hidden latency regressions from always inspecting.

### 6.3 Named acceptance scenarios

| ID | Scenario | Required behavior |
| --- | --- | --- |
| AG-01 | Agent requests a straightforward exact constant | Exact supported value, no unnecessary model/checkout investigation |
| AG-02 | Task and file terminology differ | Find relevant concepts via lexical/semantic/relation signals, preserving original-source precision |
| AG-03 | Relevant accepted security/policy constraint | Constraint is prominent, scoped and evidence-linked before proceeding |
| AG-04 | Current code differs from an old accepted ADR | Inspect permitted evidence and recommend scoped behavior; do not invent supersession |
| AG-05 | Source says issue closed but no deployment proof | Report closure as history, not verified implementation |
| AG-06 | Significant new evidence contradicts provisional recommendation | Change implementation guidance, not merely append an unchanged disclaimer |
| AG-07 | Lore inspected existing tests but did not execute them | Keep `static_test` and `future_implementation_check` distinct |
| AG-08 | Coding agent will modify code later | Supply observable completion criteria, not pretend tests already passed |
| AG-09 | Task truly needs unresolved policy-owner approval | Block only unauthorized action, expose meaningful safe independent work |
| AG-10 | No permitted code inspection | Deliver best documentary guidance with explicit scope; no repeated permission demand |
| AG-11 | Reused investigation has changed deep file or new ADR | Reject stale advice and re-evaluate material new evidence |
| AG-12 | Previous investigation covered a different environment/task | Preserve scope, do not transfer policy authority |
| AG-13 | Model or verifier unavailable, small output budget | Valid deterministic/narrower package, preserve critical qualifiers |
| AG-14 | Prompt injection asks for command/egress | Treat input as data; no tool authority escalation |
| AG-15 | Fully unchanged source/update | Zero model calls on genuine update no-op |
| AG-16 | Human onboarding mode used previously | Subsequent agent query does not receive a tutorial or learner state |
| AG-17 | Same question repeated with current proven cache | Revalidate permission/new evidence and report reused versus fresh work |
| AG-18 | OpenWiki-only versus Lore-only versus combined evaluation | Comparable tool/model access and cost; no unsupported superiority claim |

Implement scenario fixtures in small batches; a suite using fake providers establishes contract behavior only. Independent implementation tasks and review are required to claim productivity improvement.

## 7. Open implementation choices

- How much of schema 4 can be reused unchanged versus new response fields, without losing compatibility?
- Which implementation seams can be grounded by available static checkout inspection versus requiring later symbol indexing?
- How to guarantee relevant high-impact constraints remain in a strict token budget?
- Which simple default effort budget balances time to answer and errors across agent sizes/models?
- Which agent wrappers can consume Lore CLI JSON without prompting for extra access or duplicating investigations?
- How to compare OpenWiki and Lore fairly when their content-preparation workloads differ?
- What additional evidence is necessary before opting into more history/connector/runnable-test adapters?

**Acceptance question:** Does Lore let an agent implement a *correct* change with less repeated investigation and fewer missed project constraints—while preserving source authority, permission boundaries and total cost visibility?
