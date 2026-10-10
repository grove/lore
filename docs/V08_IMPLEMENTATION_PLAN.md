# Lore 0.8 — Implementation Plan

**Release:** 0.8.0  
**Theme:** From project intelligence to proven practical value  
**Repository:** <https://github.com/grove/lore>  
**Baseline:** `main` containing merged 0.7.0 release PR #39 (merge commit `0ba8b17f7e63726a8d81def2b60af18573d4640c`; verify the actual working head before editing)  
**Status:** Implementation specification, not a claim that 0.8 is implemented  
**Primary audience:** An autonomous coding agent working in this repository

## 0. Agent execution instructions

Implement Lore 0.8 as a sequence of small, independently testable changes. Read this specification, `VISION.md`, `docs/V07.md`, `docs/SHARED_INTELLIGENCE.md`, `docs/PROJECT_COMPANION.md`, `docs/IMPLEMENTATION_TRACKER.md`, and `evaluation/PRODUCT_QUALITY_07.md` before editing. Inspect current code rather than assuming every suggested signature remains exact. Work from the current `main`, not from the historical 0.6 descriptions in some design files.

**Execute useful work, don't stop at planning.** Make implementation choices consistent with the contracts below, fix discovered regressions, run applicable tests, and complete as much of the work as existing permissions and local capabilities allow. Do not ask the user routine implementation questions. Do not fabricate real-model results, executed checks, independent reviews, user consent, sandboxing, provider bills, or missing capabilities. For externally dependent studies, implement the complete reproducible harness, run them only if the required models, permissions, isolation and review inputs actually exist, and record `not_run` with the exact missing gate otherwise. Do not initiate hosted inference or incur API charges merely because credentials happen to be present: respect explicit existing egress grants.

Use the existing Rust 2024 core, SQLite registry, Python evaluation infrastructure, and Clap CLI. **Do not rewrite Lore in C for this release:** that is a separate architectural migration, not a prerequisite for practical improvements. Do not create a second knowledge registry, another general-purpose agent runtime, an always-running watcher, or a graphical UI. Preserve the existing OpenWiki/Engram/Beads adapters.

Deliver one pull request per package below (or equivalent reviewable commits if PR creation is unavailable). Rebase in dependency order, keep `main` green, and update documentation/tests in the same PR as the behavior. Never silently weaken a source-integrity test, independent validation requirement, source binding, or permission check to get a green result. Record the exact baseline and final commit IDs, test results, known failures and evidence paths.

## 1. What 0.7 gives us — and what it does not

Lore 0.7 already has:

- Local Markdown and optional OpenWiki, Engram, Beads intake, evidence/version history, review workflow, SQLite storage, incremental compilation and Markdown publication.
- `lore context` schema 4 by default, explicit schema 5 adaptive intelligence, deterministic schema 2 `--fast`, optional checkout inspection, host grants and revalidated investigation reuse.
- `lore onboard`, Knowledge Zoom/exploration, decision lenses and cases, explicit project baselines, `lore changes` and advisory `lore guard`.
- Six-arm coding-agent evaluation with independent executable checker *processes*, bounded repair loops, different-task reuse sequences, a 30-candidate public repair corpus, a 12-participant human protocol, and a 60-transition Guardian protocol.

The 0.7 scorecard reports 427 passing Rust tests, 174 passing Python tests, 24/24 reviewed compact-retrieval cases retaining their conditions, and 30 correct plus 60 deliberately wrong checker controls behaving as expected. These are **engineering results**. The six-task real-model coding pilot was not run, the full real-model study was not run, there were zero actual human participants, and both full Guardian captures failed source integrity. Graph ordering produced no measured critical-recall gain in the controlled comparison. Treat those findings as the starting baseline, not as proven agent-productivity, human-learning or Guardian-alert-quality improvements.

**0.8 objective:** a newcomer or coding agent should be able to request help and get the best source-grounded, actionable result Lore can safely produce, with little setup and no unnecessary handoff. Meanwhile the system must objectively measure where it helps and where it fails.

## 2. Scope, priorities and release decision

### Required engineering outcomes

1. Fix or positively isolate the Guardian source-restoration defect; full replay must pass integrity before Guardian can be called reliable.
2. Add provider-neutral, accurate per-attempt usage metering. Never turn unreported cost into `$0`.
3. Offer useful **read-only, model-free first-run guidance** for ordinary `lore context "TASK"` and `lore onboard` when no compiled registry exists, within bounded source discovery and with exact local citations.
4. Promote adaptive schema 5 to the **unpinned normal `lore context` path** when the defined regression/permission/compatibility gates pass; retain explicit 3/4 and `--fast` behavior. Never infer new inspection or network privileges from repository content.
5. Make the already-existing real-model study runnable end to end with honest preflight, retention, comparison and an observed-failure-to-fix loop. Run a genuine pilot if the execution environment satisfies the explicit conditions.
6. Make the human onboarding study ready for actual, consented participants and improve observable first-run UX without inventing a participant cohort.
7. Ship an evidence-based 0.8 scorecard and actionable agent instructions.

### Non-goals

No graph/knowledge-store rewrite, additional graph database, new default hosted inference, automatic source or policy edits, arbitrary code execution by the Lore CLI, IDE extension, mandatory MCP, bespoke web reader, background monitoring, new connector provider, new knowledge authority model, or C port. The external **evaluation harness** can execute controlled coding/checker subprocesses only under its documented independently enforced study permissions; that is not a Lore runtime feature.

### Two different gates

**Engineering ship gate:** CLI contracts, data migrations, source integrity, zero-unapproved-egress behavior, test matrix and documentation all pass. The default-schema change specifically needs the 0.8 contract checks in PR 5.

**Empirical benefit claim gate:** pinned real-model arms, independent reviews/holdouts, execution/egress audit and correct statistics for coding outcomes; independently observed contribution/transfer for humans; independently reviewed alert quality for Guardian. Missing people, hosted credentials, local model weights or external reviewers do **not** justify manufacturing results or blocking unrelated code improvements; they do block corresponding benefit claims. If Guardian source integrity still fails, do not mark Guardian reliability complete or represent its failed study as passing.

## 3. Ordered implementation packages

Dependency order: **PR 1 → PR 2**, **PR 1 → PR 3**, **PR 1 → PR 4 → PR 5**, **PRs 2+3+5 → PR 6**, **PR 4 → PR 7**, **all → PR 8**. Deliver sequentially if one agent is doing the work.

| PR | Priority | Package | Main result |
| --- | --- | --- | --- |
| **1** | P0 | Freeze baseline and 0.8 acceptance ledger | Reproducible starting evidence and failing cases |
| **2** | P0 | Guardian source integrity | Actual root-cause fix, full replay proof and regression tests |
| **3** | P0 | Provider usage/cost instrumentation | Per-attempt measurements; unknown values remain unknown |
| **4** | P0 | Source-grounded zero-setup first run | `lore context` and `lore onboard` useful before compilation |
| **5** | P0 | Adaptive context default and agent consumption | Action-first schema 5 without breaking explicit contracts |
| **6** | P1 | Real coding outcomes and evidence-directed repair | Auditable six-arm pilot, diagnosis and measured changes |
| **7** | P1 | Human first-contribution workflow | Better first-contact UX, study readiness and honest outcomes |
| **8** | P0 | Integration, hardening and release | Version 0.8.0, validated CLI and truthful scorecard |

### PR 1 — Freeze the baseline, acceptance ledger and reproductions

**Files:** `docs/IMPLEMENTATION_TRACKER.md`, new `docs/V08_IMPLEMENTATION_PLAN.md` (copy this document), new `evaluation/PRODUCT_QUALITY_08.md`, new `evaluation/results/README-08.md` or the repository's existing provenance convention; small additions to tests only as needed.

**Tasks**

1. Verify current branch/commit and ensure 0.7 is merged. Capture `cargo --version`, `rustc --version`, Python version, OS, existing model and environment availability *without printing credentials*.
2. Run the current CI gates unchanged. Record all results, including ignored tests; do not copy 0.7 counts as new test execution.
3. Re-run/reproduce the 0.7 Guardian source-integrity failure using the archived corpus and the documented commands on a fresh, exclusive working directory. Retain event IDs, original/after SHA-256, command phases and exact response/status artifacts. If it cannot reproduce, record that explicitly; do not assume the defect is gone.
4. Freeze existing schema 2/3/4/5 golden behavior; add a source/permission/cost benchmark manifest for 0.8. Record exact command, source revision, grants, model identity or lack of model, elapsed time and result status for each.
5. Create a machine-readable 0.8 scorecard template with three distinct states per gate: `passed`, `failed`, and `unmeasured`/`not_run`, plus precise artifact hashes and scope.

**Acceptance:** deterministic baseline artifacts can be independently regenerated, their SHA-256 bindings are valid, and no failed or missing empirical result appears as success. No default behavior changes in this PR.

### PR 2 — Repair Guardian source restoration, then validate full replay

**Primary files:** `evaluation/guardian_longitudinal.py`, `evaluation/tests/test_guardian_longitudinal.py`, `src/companion.rs`, `src/companion/interpretations.rs`, `src/main.rs` only if diagnosis warrants it, `tests/project_companion.rs`, `tests/project_companion_cli.rs`, `evaluation/GUARDIAN_LONGITUDINAL.md`.

**Known failure:** during 0.7 full event replay, previously deleted synthetic source files reappeared with prior contents during supposedly read-only `baseline save`, `changes`, `guard` or `evidence` intervals. The archived full baseline had 32 source-integrity failures; the candidate had 15. One failed sequence included `docs/.rsync-tmp/history.md`. The writer has not been independently identified. Do **not** assume a cause.

**Implementation**

1. Create a **minimal differential replay** using existing `prepare`, `apply_snapshot`, `invoke` and `source_fingerprint` functions. Instrument before/after each command—not only after a whole multi-command episode. Record file create/delete/rename/write effects, file identity and hashes, exact command phase, environment classification, PID tree where available, and whether writes touch source roots, `.lore` or generated output.
2. Use a clean, private workspace and capture independent no-Lore controls. When OS support exists, use a separately configured filesystem/syscall observer; do not interpret lack of `ptrace`, a watcher, or container tooling as an audit pass. Do not log private source text, secrets, or unrestricted environment values.
3. Trace the writer. Fix the actual source mutation or harness snapshot-restoration bug, not just the evaluator's comparison. If a platform/external writer is responsible, isolate it and specify a deterministic supported runner. Keep the same source-integrity oracle. Never skip `.rsync-tmp`, delete events, or silently change expected hashes to pass.
4. Add regression coverage for A→B→A source conditions, deletion, rename, concurrent or unexpected file creation, sibling roots, symlinks, forbidden paths, current versus historical support, overlapping/negated alerts, and cancellation/retry paths. Read-only CLI commands may write permitted Lore-owned caches/baselines; they must not write primary sources.
5. Re-run both full 60-event cohorts at least once with fresh workspaces, plus the focused segment. Bind each event's source before/after hashes and record all failures. Rerun the clean candidate to check reproducibility. Run actual provider-based alert review only with a valid model, approved source/egress envelope and independent review; don't call synthetic disabled-provider output a scored advisory.

**Hard gate:** every transition in a completed claimed-clean full replay must pass source-integrity verification, with no ignored/missing events and no unclassified source writes. If it does not, retain the fail and prevent any claim that Guardian is longitudinally validated. Existing runtime permission ceilings stay intact.

### PR 3 — Instrument model usage and honest total cost

**Primary files:** `src/inference.rs`, `src/http.rs`, `src/provider_wire.rs`, `src/engine/runner.rs`, `src/storage.rs`, new `migrations/0008_provider_usage.sql` if a persistent table is needed, `src/context/decision/runtime.rs`, `src/context/semantic.rs`, `evaluation/coding_agent.py`, `evaluation/adaptive_tasks.py`, `evaluation/coding_tasks.py`, provider and migration tests.

**Current gap:** model responses and `model_calls` retain model/call/cache/latency information, but the Rust provider contract omits token-usage metadata; the evaluator correctly leaves unknown provider cost null. The 0.8 change must not convert counts into fake invoices.

**Implementation contract**

Define a provider-neutral optional usage structure (exact naming is implementation-owned) with:

```json
{
  "provider_request_count": 1,
  "input_tokens": 1200,
  "output_tokens": 250,
  "total_tokens": 1450,
  "billed_cost_usd": null,
  "billing_source": null,
  "status": "completed",
  "cache_hit": false
}
```

Token fields are nullable nonnegative integers; `total_tokens` is nullable when the API does not supply or reliably derive it. Do not confuse offline `cl100k_base` envelope tokens with provider-billed tokens. Keep per-attempt events, including retries, timeouts, refusals and validation failures; a provider error may have *unknown* usage. An actual cached response consumes **zero new provider requests**, but can retain the historical origin metadata only with a clear separate label. Do not double-count cache warm-ups or verification/repair calls.

1. Parse OpenAI Responses usage and Ollama generation/embedding counts when supplied. Handle incomplete and missing provider fields, malformed numbers, invalid units, and models/providers that do not return usage. If the direct API does not give billed USD, leave billed cost null; an explicitly configured price-based estimate, if added, must use a different `estimated_cost_usd` field and a reproducible price-table version.
2. Add optional metering to generation, embedding and decision paths, using a non-sensitive per-attempt event sink (or equivalent) so error attempts are accounted for. Do not break object-safe trait contracts or expose raw prompts/credentials. Introduce a versioned SQLite table if needed rather than destructively changing historic rows; migrations must be transactional and preserve old values as unknown.
3. Aggregate usage per invocation and expose an **additive** schema-5 usage summary, charged in the complete JSON/Markdown response budget. Explicit schemas 2/3/4 retain their current public contract; a schema-5 response must not exceed its token limit because of the new metadata.
4. Update the six-arm evaluator and coding adapter to preserve provider-reported token usage, charged initialization, warm-up, cache, retries, all repair attempts, checker time, and wall time. End-to-end USD remains nullable unless genuine billing data exists. Protect study output from credentials/provider bodies.

**Tests:** mocked OpenAI/Ollama success and retry/error bodies; cached request; generated answer rejection/repair; embedding batch; hosted disabled; request timeout; migration from the 0.7 SQLite schema; zero-call deterministic fast and bootstrap paths; no increase in unauthorized egress; ledger sum equals explicitly counted attempts.

**Acceptance:** cost reports tell the truth when data exists, preserve `null` when it does not, and independently recompute every aggregate from the saved event ledger.

### PR 4 — Progressive, zero-configuration first-run experience

**Primary files:** `src/main.rs`, new `src/bootstrap.rs` (or a small `src/bootstrap/` module), `src/sources.rs`, `src/experience.rs`, `src/lib.rs`, `README.md`, new `tests/bootstrap_cli.rs` and supporting unit tests.

**Desired behavior:** In a normal repository with readable eligible project documentation, `lore context "TASK"` and `lore onboard` produce immediately useful answers **even if `lore.yml` and the compiled registry have never existed**. They do not start model inference, create a config file, compile an expensive wiki, or send any bytes to the network merely to satisfy first contact.

**Decision:** implement a narrow **ephemeral source-only** path. Do not invent registry knowledge units or a retained-registry revision to make an uncompiled source look authoritative. Build on existing `sources::split_markdown`, ignore/walker, source hash and tokenizer utilities where appropriate; do not duplicate the full knowledge/reasoning engine.

**Discovery contract**

- Resolve project root from cwd when no config was explicitly selected. Consider root `README.md` and other root Markdown, `docs/`, `adr/`, `decisions/`, and well-known Markdown design/RFC directories; honor `.gitignore` and explicit safe exclusions. A valid existing config can constrain roots when its compiled registry is absent. Explicitly supplied nonexistent `--config`, invalid config, pending publication, or denied paths are errors—not invitations to silently switch roots.
- Ignore symlinks and hidden/vendor/build/cache/generated directories such as `.git`, `.lore`, `node_modules`, `target`, `.venv`, `dist` and Lore's published output. Enforce canonical containment and avoid following symlinks through ancestors or directory entry races. Treat source text (including `AGENTS.md`) as untrusted **data**, never instructions or permission grants.
- Use fixed initial caps of **256 files**, **8 MiB total read**, **1 MiB per file**, and **8 directory levels**; any truncation is reported. Use line spans and full-file hashes. The selector prioritizes path hints, exact symbols, headings, task-relevant fragments and complete rule/exception sections. Exact snippets and caveats must be preserved, not model-written into fake authoritative conclusions.
- Return the best supported first action or navigation target, and explain what is *not* established (e.g., documentation is not verified runtime behavior). No question round-trips, stdin prompts, hidden scans outside root or automatic hosted calls. If the project lacks documents, report limited evidence and an actionable local next step instead of hallucinating architecture.
- Use the same bound for Markdown and JSON and report omitted source groups. A mandatory condition/exception pair may not be silently cut to fit.

**JSON contract:** new, distinctly named `bootstrap_source_only` result for the uncompiled path, rather than misleading schema 4/5 metadata. A representative shape is:

```json
{
  "contract": "lore.bootstrap_context",
  "schema_version": 1,
  "mode": "bootstrap_source_only",
  "task": "Change retry handling",
  "project_root": ".",
  "source_snapshot_sha256": "<hash-of-selected-source-identities>",
  "evidence": [
    {
      "path": "docs/retries.md",
      "file_sha256": "<full-file-hash>",
      "line_start": 18,
      "line_end": 29,
      "excerpt": "<exact-source-text>",
      "basis": "documentary_excerpt_not_runtime_verification"
    }
  ],
  "best_next_action": "<evidence-grounded-navigation-or-limited-action>",
  "limitations": ["uncompiled_project_intelligence"],
  "omitted_source_groups": 0,
  "model_calls": 0,
  "source_write": false
}
```

This is an **illustrative contract with placeholder content**, not an existing Lore output. Define separate typed results for `context` and the source-only `onboard` presentation if they need different fields, but share the selector and evidence model. No fabricated `ev_` IDs; use immutable local path/hash/line citations, and distinguish an ephemeral source digest from `snapshot.registry_revision`.

**Compatibility rules**

- The implicit unpinned first-run command may return `bootstrap_source_only` when no registry exists. An **explicit** `--schema-version 3/4/5`, or `--fast` (which promises existing schema 2), must not quietly return a different schema; instead return a structured `uninitialized_project` explanation and instructions for `lore init`.
- With a compiled registry, keep all historical pinned-schema behavior and the normal evidence/memory engine. With a valid explicit `--config` path that does not exist, return a typed configuration error, not cwd auto-discovery. Implement explicit-config detection cleanly (e.g., represent the Clap option as `Option<PathBuf>` until resolution).
- `lore init --configure-only`, existing explicit source/import settings, `lore update`, `--no-inspect`, `--no-cache`, all egress settings and `--json` remain meaningful. A bootstrap request never invokes the model or writes sources; the later explicit `lore init` is the durable compilation path. Normal schema-5 invocation continues to choose useful bounded investigation *within* an existing caller grant.
- `lore onboard` source-only mode gives an immediate evidenced orientation and optional next navigation; it must not claim a verified workflow, learner mastery or completed tutorial exercise. Full learning modes still require their existing grounded implementation.

**Tests:** no files, README-only, docs+ADRs, exact symbol, preserved rare exception, large/ignored source tree, symlink escape, bad UTF-8, changed file during read, denied path, malformed config, explicit custom config missing, pending publication, uninitialized JSON/error, previously compiled project, default local-only behavior, 256/8 MiB caps, full output budget, and zero source/config/cache writes in ephemeral mode.

**Acceptance:** a new user can run both first-contact commands without configuring a provider or being asked a question, and receive either real, hash-bound, limited source assistance or an explicit evidence-limited result. This is not a substitute for full `lore init`.

### PR 5 — Make adaptive task intelligence the everyday default

**Primary files:** `src/main.rs`, `src/context/adaptive.rs`, `src/context/decision/runtime.rs`, `src/context/retrieval.rs` only when a real defect is shown, `src/experience.rs` where shared, `docs/AGENTS.example.md`, `docs/SHARED_INTELLIGENCE.md`, `README.md`, `tests/adaptive_context.rs`, `tests/context_cli.rs`, `tests/context_decision_cli.rs`, `tests/source_relationship_manifest.rs`, `tests/investigation_memory.rs`.

**Behavior with an initialized registry**

```bash
lore context "Refactor retries without changing behavior"
lore --json context "Refactor retries without changing behavior"
# Both now use schema 5 unless an explicit schema is pinned.

lore --json context "Refactor retries" --schema-version 4 # exact legacy schema 4
lore --json context "Refactor retries" --schema-version 3 # exact legacy schema 3
lore --json context "Refactor retries" --fast             # unchanged schema 2

LORE_INSPECTION_ROOT="$PWD" lore --json context "Refactor retries"
lore --json context "Refactor retries" --no-inspect
```

**Implementation**

1. Make the unpinned initialized-project branch in `src/main.rs` call the existing `context::adaptive::run`. Do not implement a second autonomous controller. `--schema-version 4` stays on `context::decision::runtime::run`; explicit 3 and `--fast` stay on their current paths. Reserve a consistent source snapshot and preserve the complete `source_relationships` manifest.
2. Keep automatic evidence selection and permitted investigation. The default must never infer inspection authority from config or a file. `LORE_INSPECTION_ROOT`/explicit `--inspect` are the read grants; explicit `--no-inspect` denies. Hosted and checkout egress retain their **separate** existing grants; no fallback to hosted models during local provider failure.
3. Make the first screen/CLI output **action-first**: preferred strategy, next action, critical constraints and scope, decisive uncertainty, then expandable detail, citations, actual completed observations and future completion checks. Avoid generic "investigate further" when a bounded permitted read can answer it. In `--json`, preserve all evidence/provenance/omission information for consuming agents, including exact observation hashes, fallback reasons and provider-usage metadata when present.
4. Distinguish documentary intent, source-reported behavior, static code observations, execution-independent inference, and future tests. Preserve accepted constraints and rare exceptions. Recompute an invalidated cache when a new relevant source arrives, a deep inspected file changes, or a grant is reduced. A denied capability is not an evidence-based technical blocker; a genuine missing policy authority remains explicit.
5. Update `docs/AGENTS.example.md` with the **version-pinned** schema-5 invocation and handling for `intelligence` brief/fallback, `source_relationships`, `budget`, `capabilities` and `snapshot`. Show a correct consuming-agent flow: retrieve context, honor qualifications, inspect/implement under the coding agent's separate grants, then perform the agent's own changed-code checks. Do not require installing an agent integration.

**Default-switch test gate:** compare schema-5 default versus explicitly selected schema 5 byte-equivalent semantics; preserve exact schema 2/3/4 explicit outputs; cover no grant, standing grant, explicit denial, hosted egress refusal, document-only, exact symbolic query (no unnecessary model work), material conflict with safe partial progress, provider unavailable, low budget, cancel, reuse with new counterevidence, and prompt injection in retained documents. Mark fallback honestly and count every produced byte/token within the requested envelope. If the default fails one of these, repair it before switching; do not bypass the test or silently widen access.

**Acceptance:** one ordinary initialized-project request gives the best valid available decision within grants, with no mode-selection questionnaire. No claim of productivity gain is implied by promoting a tested contract.

### PR 6 — Run honest coding-agent studies and improve from actual failures

**Primary files:** `evaluation/adaptive_tasks.py`, `evaluation/adaptive_sequences.py`, `evaluation/coding_agent.py`, `evaluation/coding_tasks.py`, `evaluation/outcome_protocol.py`, `evaluation/failure_triage.py`, `evaluation/REAL_CODING_TASKS.md`, `evaluation/ADAPTIVE_TASKS.md`, new `evaluation/experiment_08.py` **only as a thin orchestration/preflight wrapper**, associated Python tests and captured reports.

**Implementation**

1. Add a `preflight` command/report that checks pinned input snapshots, independent task/checker review records, configured model identity/revision, exact agent command digest, separate checker isolation, allowed tools, attempts/wall limits, Lore inspection/egress grants, provider availability, output privacy, and external runtime/egress audit status. Do not mistake a Python harness environment allowlist for OS sandboxing. Failed preflight cannot be overridden into a passing independent study by an arbitrary flag.
2. Keep the six existing arms: `baseline`, `fast`, `lore05`, `lore06`, `adaptive_no_reuse`, `adaptive_reuse`. Include all warm-up, context, regeneration, coding attempts, repair, verification, tool and observed wait time in total costs; failed tasks stay in success denominators. Report unknown tokens/USD as null, not zero. Preserve separately measured context and coding usage.
3. Run **six pilot tasks across three projects × six arms (36 initial attempts)** if an approved real coding model, Lore model, study workspace and isolation/audit are genuinely available. The existing public 30 tasks are *development candidates*; their original answers may be known to the model. Use a fixed seed, preregister the model/prompt/tool/attempt/grant policy and save exact hashed artifacts. Record all failed/refused/cancelled invocations.
4. Use `evaluation/failure_triage.py triage` to identify real missed constraints, weak recommendations, unnecessary investigation or unsafe blocking. For each proposed change, record exact failing sample IDs, the premise, expected change, regression scenario, cost risk and separate untouched holdout. Implement **at most the smallest justified retrieval/controller/presentation changes**. A failing test alone is not justification for tuning an unrelated algorithm.
5. Add a matched different-task reuse sequence: A then related B, after a new accepted contradictory ADR, after a deep-file mutation, and after reduced permissions. Count net revalidation overhead; do not call a cache hit "time saved" without total measured time or treat reuse-disabled as an isolated memory-only ablation.
6. Only when independently authored/reviewed held-out tasks, authenticated external audit and pinned providers actually exist, run the separate **≥30-task × six-arm** full study (≥180 initial coding attempts). Keep held-out source/tasks/checkers outside the tuning corpus and model input. Record per-task paired differences and 95% bootstrap intervals, not just aggregate averages or model preference ratings.

**Success decision for empirical claim:** preregister the existing 0.7 candidate target of **+10 percentage points in independently checked task success**, **or ≥20% less all-in time/cost with non-inferior success and no increase in critical constraint violations**. Interpret uncertainty intervals and power limitations honestly; a tiny pilot cannot prove a universal gain. Publish failures as failures. If unavailable, score `coding_productivity: unmeasured` while preserving a fully tested harness.

**Acceptance for code:** offline controls, audit and tamper tests pass, matched arms cannot leak checker feedback or warm-up solutions, and the first genuine pilot is completed **if prerequisites exist**. No benchmark result is invented if they do not.

### PR 7 — Remove human-first-contact friction and prepare actual learning assessment

**Primary files:** `src/experience.rs`, `src/experience/learning.rs`, `src/experience/render.rs`, `src/experience/source.rs`, `evaluation/human_onboarding.py`, `evaluation/tests/test_human_onboarding.py`, `evaluation/tests/test_learning_transfer.py`, `docs/HUMAN_EXPERIENCE.md`, `evaluation/HUMAN_ONBOARDING.md`, `README.md`.

**Tasks**

1. Reuse PR 4's evidence-bound source-only orientation so `lore onboard` works on a fresh repository without a questionnaire. With compiled intelligence, lead with project purpose, two or three crucial concepts, one genuinely grounded workflow/landmark if evidence supports it, and next navigation/first-task choices. Experienced users can jump directly to a task; no forced lesson or false learner profile.
2. Preserve the existing optional learning path, lesson revision binding, hint stages, fallible feedback and separate first-contribution and transfer activities. No implicit tests or solution reveal during an explicit human learning exercise. A generated patch by an agent is **not** human mastery.
3. Validate study consent/revocation, real participant session and pseudonym binding, time/assistance tracking, exact submitted patch hash, independent executable checks, task explanation review, transfer with reduced assistance, hidden-checker separation, source mutation and interruption. The existing planned 12 positions do not constitute participants.
4. Where actual consenting people and independent assessors exist, run the counterbalanced 12-participant study. Otherwise retain zero real participants and mark first-correct contribution time and independent learning transfer `unmeasured`. Do not generate fictional people or fill participant records from synthetic agents.
5. Fix any first-run UX issue demonstrated by deterministic integration tests or genuine participant observations; avoid claiming evidence-directed human improvements without the corresponding evidence.

**Acceptance:** `lore onboard` has truthful, helpful first-run/compiled modes, and the participant pipeline can record and independently assess real work without leaking hidden answers. Human competence remains unmeasured until the real study occurs.

### PR 8 — Release 0.8.0 with integrated tests and an honest scorecard

**Files:** `Cargo.toml`, `Cargo.lock`, `README.md`, `VISION.md`'s current-status section where appropriate, `docs/V08.md`, `docs/IMPLEMENTATION_TRACKER.md`, `evaluation/PRODUCT_QUALITY_08.md`, `evaluation/README.md`, `.github/workflows/ci.yml`, new tests and immutable release evidence.

**Tasks**

1. Raise package version to `0.8.0` without gratuitous dependency upgrades; make README start path and agent instructions accurate for the **actual** shipped behavior.
2. Add `docs/V08.md` with exact examples, first-run bootstrap contract, model/permission matrix, schema migration notes, Guardian boundaries, provider-usage semantics, evaluation status and reproducibility steps. Preserve `docs/V07.md` as a historical record.
3. Integrate and run full Rust/Python/offline CLI gates on Linux, macOS and Windows. Include first-run, schema 3/4/5 pinning, source permissions, provider responses, budget/fallback, checkers and guardian regressions in CI. Use no API keys or costly inference in regular CI. Make the full Guardian replay a separate controlled integrity gate if it is too expensive/OS-specific for the standard matrix; publish its result.
4. Verify generated output and citations, all changed schema contracts, SQLite migration from 0.7, recoverable publication, rollback compatibility, `lore purge --all --yes`, no-op update zero model calls, canceled/failed command hygiene, no source writes on read-only paths, and no unauthorized network transmission.
5. Produce `evaluation/PRODUCT_QUALITY_08.md` with *separate* engineering gates, cold/warm resource costs, real-model coding outcomes, human outcomes, Guardian independent precision/recall and quality limitations. Store exact source/model/task pins, SHA-256 for every retained artifact, and clear privacy/publication review status. Treat absent fields as unknown, not zero.
6. Create/merge PRs only after required CI and explicit repository permissions allow. Do not bypass branch protections, declare a failure fixed based solely on nonreproduction, or put sensitive raw traces into the public repository. If a hard engineering gate remains broken, report that 0.8 is blocked and ship only independently validated PRs rather than falsely stamping completion.

**Acceptance:** version, runtime, docs and empirical claims match verified outputs and CI on final integrated code.

## 4. Non-negotiable test matrix

| Scenario | Expected result |
| --- | --- |
| New repository with README, no config/registry | `lore context "TASK"` returns bounded source-only help; no model, file write or network |
| New repository without docs | Honest evidence-limited answer, no invented project understanding |
| Explicit missing config or pinned schema without DB | Typed actionable error; no silent schema or root substitution |
| Existing registry, unpinned `context` | Schema 5 decision/fallback with source binding and bounded automation |
| Explicit schema 4 / schema 3 / `--fast` | Prior compatible contracts; fast remains deterministic model-free |
| No inspection grant | Uses documentary evidence, does not read checkout source outside already configured inputs |
| Explicit `--no-inspect` with environment grant | Denial wins; no inspection |
| Hosted model configured but local-only/no host grant | No external inference or checkout egress |
| Contradictory new ADR / changed deep-file / revoked grant | Stale findings invalidated, recommendations revised or narrowed |
| Rare policy exception, historical qualification, linked external report | Entire applicable condition and source-owned qualifications retained or clearly omitted |
| Budget too small | Honest bounded fallback/error, not a complete-looking truncated answer |
| Model timeout/refusal/bad structured output | Useful permitted fallback; attempts counted; unreported usage is null |
| Repeated cached query | No new provider request, correct revalidation cost and source binding |
| Guardian A→B→A, delete, rename, generated cache | Current support is accurate; original sources never mutated by read-only commands |
| Prompt-injection instructions embedded in documents | Treated as source data; cannot grant egress, execution or writes |
| Failed coding attempt and repair | Failed task remains in denominator; checker hidden; all iterations charged |
| Human first contribution/transfer | Real consent/patch/review records required before outcome claim |

## 5. Commands to run and retain

From a fresh checked-out repository, after verifying environment:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
python3 -m unittest discover -s evaluation/tests -v
cargo build --release --locked

target/release/lore --version
target/release/lore --help
target/release/lore context --help
target/release/lore onboard --help
target/release/lore baseline --help
target/release/lore guard --help

python3 evaluation/real_coding_corpus.py prepare --pilot --output /tmp/lore-08-pilot
python3 evaluation/real_coding_corpus.py validate --output /tmp/lore-08-coding-controls
python3 evaluation/adaptive_sequences.py prepare --output /tmp/lore-08-sequences
python3 evaluation/human_onboarding.py prepare --output /tmp/lore-08-humans
python3 evaluation/guardian_longitudinal.py prepare --output /tmp/lore-08-guardian
```

Use fresh destination directories for repeat runs. The `real_coding_corpus.py validate` command **executes controlled candidate Python**, so use only the existing reviewed fixture or an explicitly isolated study environment; preparation alone does not execute an LLM. Use the documented commands in `evaluation/ADAPTIVE_TASKS.md` and `evaluation/GUARDIAN_LONGITUDINAL.md` for actual run/assessment. Do not invent provider/model arguments; probe permitted local availability and use real configured identifiers. Live provider tests remain separate from offline CI. Capture command exit codes, start/end revisions, log hashes and counts for each CI run and independent study.

## 6. Agent handoff/finish contract

At the end, produce a concise factual report containing:

- **Implemented:** PR links/commit hashes and the main behavior changes. Distinguish completed from partial PRs.
- **Validated:** exact commands, passing/failing test counts and OS matrix; Guardian event-integrity summary; schema/egress/resource tests; scorecard and artifact links.
- **Measured outcomes:** real model/task/reviewer/permission pins and success/time/usage/cost estimates **only if actually obtained**; otherwise `not_run` or `unmeasured` plus the precise missing dependency. Same for genuine human participants and Guardian independent alert quality.
- **Remaining defects:** reproducible failure, severity, source path and next supported fix. Never rewrite the evidence to remove a failure.
- **Release decision:** `0.8.0 ready` only when every required engineering ship gate passes; `ready with explicitly unmeasured empirical outcomes` is valid, but claiming a benefit not measured is not. If blocked, publish validated components and exact blockers instead of a false release declaration.

**Product standard:** Lore should do all useful, permitted investigative homework itself, recommend the strongest defensible next action, and make its important limitations inspectable—without reducing reliability or shifting routine work to the user.
