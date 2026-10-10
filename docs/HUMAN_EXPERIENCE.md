# Understand a project, then work on it

`lore onboard` gives a person a first view of the project. In a fresh checkout it reads bounded original documentation and returns exact source excerpts, qualifications and navigation without setup. When compiled project knowledge is available, it presents the same retained evidence and bounded investigations used by adaptive task context as an explanation: purpose, important concepts, architectural responsibilities, a documented or statically inferred workflow, and conditions that matter when changing it. There is no questionnaire or required learning profile.

```bash
lore onboard
lore onboard --topic "Explain dispatch and its failure boundaries"
lore --json onboard
```

## First contact without setup

Run the commands above in a checkout even before creating `lore.yml` or running `lore init`. The first-run `bootstrap_source_only` response uses the same scanner and complete source bundles as fresh-checkout task context. It makes zero model calls, creates no configuration or cache, and leaves original files unchanged. Each excerpt carries its original relative path, full-file content hash and exact line range. It keeps documentary text, including linked qualifications, together; an oversized or ambiguous group is omitted as a whole with explicit navigation and omission reporting.

This response explains its limited source-only status. Its JSON contract is `lore.bootstrap_onboard`, `schema_version: 1`, with `mode: bootstrap_source_only`; it is separate from the compiled human contract described below. It does not have retained evidence IDs, a compiled decision, a generated workflow or a lesson to assess. When useful evidence is unavailable or exceeds the budget, it says so. An explicit missing configuration or a malformed existing configuration is an actionable error.

Use ordinary `lore init` and `lore update` when you want the compiled orientation, task guidance and optional learning behavior below. Tutorial mode, hints, answers, lesson pins and transfer activities require compiled knowledge; first-run source excerpts cannot substitute for a pinned lesson. Other first-run intent choices still return the explicitly labeled source-only contract. Neither path writes project sources, creates a contribution or executes tests. Compiled onboarding can reuse source-derived guidance under the existing context cache policy. Learner answers and feedback are never saved.

## Choose by intent

With compiled knowledge, describe what you want. The default `auto` mode recognizes a learning request, an exact reference question, or a goal such as a refactor; otherwise it explains the project. An explicit mode overrides this routing.

An initial explanation without a task or goal presents at most three grounded concepts and counts additional concepts in `omitted_items`. Its full source conditions and relationships remain available. A targeted explanation and an optional tutorial retain their own concept material and lesson bindings.

| Mode | What it provides | Example |
| --- | --- | --- |
| `explanation` | Purpose, concepts, relationships and a source-grounded workflow | `lore onboard --topic "Explain dispatch" --mode explanation` |
| `how-to` | The shared preferred approach, readiness, constraints and future completion checks | `lore onboard --task "Refactor dispatch without changing ordering"` |
| `tutorial` | A deliberate practice activity, optional progressive hints and a different transfer activity | `lore onboard --topic "Teach me dispatch" --mode tutorial` |
| `reference` | Exact retained passages and their revisions, without presentation inference | `lore onboard --topic "Exact dispatch queue capacity" --mode reference` |

These are presentations over one intelligence engine, not four separately authoritative knowledge stores. The JSON response includes the unchanged nested schema-4 decision/evidence result and the same whole-registry snapshot identity used by schema-5 adaptive context. Existing `lore context --fast` and explicit schemas 2, 3 and 4 keep their contracts.

## A tour follows behavior

When generation is available, Lore selects a meaningful workflow and relates its entry, participating responsibilities, outcome and important failure boundary to original source evidence. A generated transition must pass an additional source-support check. Actual inspection observations include exact paths, line ranges, excerpts and complete-file hashes.

The `basis` of each claim distinguishes documentary evidence, an imported report, static inference, a broader inference, and a hypothetical practice scenario. A supported static sequence is still not a runtime trace. A source path mentioned in a document is not proof that a file was inspected.

Source freshness does not imply adoption. A newly retained proposal or plan stays explicitly qualified as future intent. The extractive fallback does not use it as the project's present purpose, an adopted workflow or a current learning rule.

When the provider is unavailable or the generated explanation fails validation, the response remains useful: Lore selects current documented concepts, copies complete source conditions, and retains a documented procedure as its own narrative. This extractive fallback does not manufacture an ordered call graph or a project motivation that the selected sources do not state. The `generation_basis` and `presentation_status` fields make this distinction explicit.

## Learn by doing, when requested

```bash
lore onboard --topic "Teach me dispatch queues" --mode tutorial
```

A tutorial contains a learning objective, a hypothetical project-specific scenario, an activity that requires a prediction or explanation, and a distinct related transfer activity. The examples are generated practice. Lore does not claim that they are real issues, assigned work, historical incidents or passing tests.

The initial response hides example solutions and hints. The learner can request more help, skip the activity, or go straight to a real task:

```bash
lore onboard --topic "Teach me dispatch queues" --mode tutorial --hint 1
lore onboard --topic "Teach me dispatch queues" --mode tutorial --hint 2
lore onboard --topic "Teach me dispatch queues" --mode tutorial --show-solution
lore onboard --topic "Teach me dispatch queues" --mode tutorial --activity transfer
lore onboard --task "Refactor the dispatch queue without changing behavior"
```

Each tutorial returns a `tutorial.revision_key`. Add it as `--lesson` when returning to an activity or asking for feedback. The complete digest in the following example is a placeholder for the value printed by Lore:

```bash
lore onboard --topic "Teach me dispatch queues" --mode tutorial \
  --lesson 'blake3:RETURNED_LESSON_DIGEST' \
  --answer 'My explanation of the production and staging boundaries'
```

Only an answer pinned to the exact current lesson can receive model-assessed feedback. If the lesson changed, the source snapshot changed, or no pin was supplied, Lore offers a source comparison without grading the answer against a different exercise. Without inference, the comparison supplies the relevant exact project conditions and explicitly leaves correctness for review.

Hints are optional and have no punitive score. An educational prerequisite is a suggested learning order, not project policy or a mandatory skill gate. Circular prerequisites are detected as exact strongly connected groups and presented as concepts to learn together. These relationships are separate from the project's authoritative evidence relationships and from the Knowledge Zoom graph.

The transfer activity uses a different objective and action from the first activity, with hints hidden again by default. That makes another task available; it does not establish that the learner successfully transferred understanding. A generated plan, a plausible explanation and a coding agent's patch are not proof that a person learned the project.

## Work on a real first task

The supplied `--task` is the source of intent. Lore returns the shared decision-ready approach and relevant implementation observations, together with important constraints and completion checks. It does not require a tutorial first.

```bash
lore onboard --task "Improve the full-queue diagnostic without changing dispatch semantics"
```

The task is labeled `user_supplied_task`, not a discovered or assigned backlog item. Automatic starter-issue discovery is not implemented by this command. The developer retains authorship and control of the change. Completion checks describe future work; they do not imply that Lore ran a test or independently reviewed the resulting contribution.

## Permissions and source freshness

Onboarding uses the same host grants as adaptive context. Project configuration can narrow a grant, but cannot create it. `--no-inspect` is an absolute request-level restriction. No learning control grants inspection, hosted egress, execution or source-write permission.

For standing read-only checkout access, the invoking host can set `LORE_INSPECTION_ROOT` to the approved root. Hosted inference additionally requires the adaptive host's egress grant and compatible privacy configuration; checkout egress has its own grant. `privacy.local_only: true` remains a veto. See the adaptive context documentation for the complete host grant contract.

When human generation uses static observations, Lore rehashes them again after presentation generation/support checking within the remaining original byte and I/O-time budget. If a file, catalog or ignore rule changed, or revalidation cannot be completed, the response discards affected generated guidance and its nested static premises and returns coherent documentary context. `presentation_revalidation` reports its status and the actual revalidation reads. It never reports a test execution.

The whole registry revision participates in lesson identity and cache selection. A newly added rule can therefore invalidate a previous view even when its old supporting files remain unchanged. Every answer refers to a coherent captured registry snapshot; a source's `current` status means current in that snapshot, not verification of a live deployment.

## Source-derived caching and privacy

With the existing `context.cache` enabled, verified presentation drafts can be reused for the same goal, mode, source snapshot, original evidence, observations, provider/model identity, reasoning configuration and privacy envelope. Requesting a hint does not require generating a different lesson. The cache includes the lesson's source-derived example solution so it can reveal the same activity later.

The cache does **not** contain submitted answers, model feedback, completion history or learner profiles. It is disposable project-derived material, stored under `.lore/human-view-cache`, with at most 128 normal entries of at most 256 KB each. Atomic publication and existing full-state purge apply. Treat this cache as potentially sensitive project material, like other `.lore` caches.

```bash
lore onboard --no-cache
```

`--no-cache` bypasses both reads and writes for this view, and `context.cache: false` disables persistence. A bounded directory inventory and nonblocking cache lock keep retention controlled; a full or contested cache, or a read-only cache failure, leaves the validated answer available. Unmanaged entries and symlinks are not eligible for retention deletion. If a stateless regenerated lesson differs from a supplied lesson pin, its answer is not graded as though it belonged to the earlier exercise.

There is no learner account, background tracking or retained mastery score. No special reset is needed for a learner profile because this command does not create one.

## Budgets, failure and JSON

The default output limit is 6,000 tokens. The complete JSON and Markdown outputs are measured with the existing `cl100k_base` tokenizer. Optional presentation items can be omitted as whole items with an explicit count. The shared evidence manifest and critical conditions remain intact; a response that cannot preserve required content within the requested limit reports that the budget is too small.

Onboarding reserves up to two attempts from the configured aggregate model-call allowance for presentation synthesis and source checking. A reused draft needs no new presentation call unless a pinned answer requests feedback. All attempts and the original request deadline remain bounded. A dropped or cancelled presentation releases its read snapshot and schedules no background continuation.

The compiled version-1 human response contains:

| Field | Meaning |
| --- | --- |
| `schema_version` | Human contract version, currently `1` |
| `mode` | Resolved explanation, how-to, tutorial or reference purpose |
| `snapshot` | Shared project and whole-registry revision identity |
| `capabilities` | The effective shared grant restrictions |
| `orientation` | Purpose, rationale, concepts, responsibilities, workflow, exact conditions and next exploration |
| `tutorial` | Optional source-bound activity, distinct transfer, suggested prerequisite graph and revealed hints |
| `first_task` | Optional user-supplied task, shared approach and future completion checks |
| `feedback` | Optional fallible assessment or ungraded source comparison |
| `references` | Exact passages for direct reference mode |
| `intelligence` | The shared independently versioned decision/evidence result |
| `model_calls` / `presentation_model_calls` | Actual total attempts and the presentation portion |
| `presentation_status` / `presentation_revalidation` | Generation/reuse/fallback and post-presentation checkout status |
| `budget` / `omitted_items` | Complete output accounting and whole optional item omissions |

Each claim's evidence identifiers resolve through the shared manifest and `lore evidence ID`. Markdown renders source drill-down commands and exact observation excerpts; no web reader is required.

## What is verified, and what remains to measure

The `human_experience` tests use real retained SQLite evidence and filesystem observations with deterministic provider doubles. They cover source binding, generated claim rejection, critical exceptions, tutorials and transfer distinction, lesson identity/reuse/invalidation, privacy, cancellation, changed-code fallback, exact references and optional prerequisite cycles.

The `human_experience_cli` tests invoke the actual binary after ordinary fixture ingestion. They check shared snapshot identity, exact evidence drill-down, complete JSON and Markdown budgets, immediate task routing, input rejection before configuration access, and unchanged source, wiki and state bytes for requests with `--no-cache`. Hosted inference is denied before client construction in these tests; no live provider is needed.

The `bootstrap_cli` tests additionally exercise the actual first-run commands, exact excerpts and budgets, symlink/configuration boundaries, changed-source handling, linked qualifications and zero model or source/cache writes. Updating the first-contact instructions here corrects the obsolete requirement to initialize before every orientation; it is not evidence of a measured human improvement.

These are executable integrity and behavior checks. They do not establish that a real model consistently produces excellent tours or that developers make better contributions. The 0.8 release records **zero real human participants**: first-correct contribution time and independent learning transfer remain **unmeasured**. The [human onboarding protocol](../evaluation/HUMAN_ONBOARDING.md) provides read-only readiness checks, actual consent withdrawal and independently bound submission/review gates. Its prepared slots and offline fixture tests are separate from human observations and from coding-agent implementation results.
