# Lore vision

> **Understand any project. Work with confidence.**

**Lore is a shared project intelligence system for everyone working on a project—newcomers, experienced developers, maintainers, and coding agents.** It distills the project's collective knowledge and experience, investigates what matters, and helps each user understand and act with confidence.

Lore should make knowledge easy to discover, learn, appropriately trust and apply. Its ambition is not more documentation but **better understanding, better decisions, and more capable people and agents**. Newcomer onboarding is the **flagship human learning experience and initial deep-understanding benchmark**, not the product's only purpose. **Decision-ready, automatically investigated task context is the flagship coding-agent experience**; experienced humans and maintainers also receive direct practical intelligence.

## The problem

An unfamiliar project is hard to understand, and that difficulty does not vanish after onboarding. Newcomers need a coherent path from purpose to workflows and first contributions; experienced engineers need reliable constraints and reasons before changing systems; maintainers need decision history and emerging risks; coding agents need the same project intelligence without loading an entire wiki or duplicating investigation. More pages and a search box alone cannot solve these problems.

Project intent is scattered across architecture documents, ADRs, plans, issue trackers, investigations and conversations. The code shows much of what was implemented, but often cannot explain why an approach was chosen, which alternatives were rejected, whether a proposal was approved or what changed since an earlier decision.

Humans spend time reconstructing that context. Coding agents may miss it even when they can read every source file. A pile of summaries is not enough: it can hide disagreements, flatten history and turn a reported outcome into an unwarranted fact. Merely identifying those problems and asking the user to investigate them also falls short. Lore should do the useful work it is capable and permitted to do, then deliver the best defensible help.

## The promise

Lore helps answer four questions:

1. **What?** What does this project do, and how do its parts fit together?
2. **Why?** Which decisions, constraints and trade-offs explain the design?
3. **Now what?** What is the current documented understanding, what changed and what remains uncertain?
4. **So what?** What matters for my task, and what is the best useful action I can take?

**Two flagship experiences share one intelligence engine:** newcomers explore the project essence, a real workflow, deliberate practice, a first correct contribution, and independent transfer; coding agents obtain a concise, machine-readable, automatically investigated task recommendation. Experienced developers and maintainers get direct answers, applicable decisions, history, trade-offs and change consequences. No audience must traverse a wiki hierarchy or complete a tutorial before receiving help.

## Flagship experiences: learning and action

**Human onboarding flagship:** the first experience should be welcoming and immediately useful. A proposed `lore onboard` shows the project's purpose, a few pivotal concepts and one evidenced workflow **without a setup questionnaire**. A short source-linked tour follows a request through the important boundaries, decisions and failure cases. The developer can dive into any concept or skip straight to a real task.

A tailored learning path follows **conceptual prerequisites**, not directories or a fixed-depth summary tree. **Adaptive Knowledge Zoom** shows an overview or the exact evidence at the depth needed. **Diátaxis** gives each interaction its proper shape: Tutorials lead the learning journey; Explanation clarifies why; Reference answers precise questions; How-to supports actual contributions.

Lore helps a learner predict behavior, understand a failure, and work through one purposeful exercise. It then supports a real bounded task, performs routine investigation itself, and offers progressively fewer hints for a new related task. The learner remains in control of their work; an agent producing code is not the same as the developer having learned. Voluntary checkpoints can demonstrate understanding, but page views cannot certify mastery.

**Coding-agent flagship:** `lore context "TASK"` should evolve into a focused, source-bound decision package: preferred approach, scoped readiness, critical constraints/negative cases, actual investigated implementation seams, completed checks, future completion criteria, alternatives and material unresolved dependencies. Lore performs suitable permitted investigation; the calling agent owns edits, tests and delivery. This is also useful directly to experienced developers who want decisions rather than lessons.

These are proposed product directions, not claims of implementation in 0.6. See the [Developer Onboarding design](docs/DEVELOPER_ONBOARDING_DESIGN.md) and [Coding-Agent Intelligence design](docs/CODING_AGENT_INTELLIGENCE_DESIGN.md).

## Maximum useful autonomy. Minimum user burden.

The user describes the goal. Lore takes responsibility for retrieving available evidence, investigating consequential uncertainty, evaluating alternatives and recommending the strongest defensible approach within its permissions.

Lore should not ask people or calling agents to investigate, interpret evidence or perform routine checks it can usefully complete itself. It should choose investigative effort automatically rather than require users to discover flags, tune reasoning effort or classify their request. It should stop when it has enough evidence for the scoped decision or further investigation is unlikely to change the result—not when every uncertainty has disappeared.

Incomplete knowledge does not necessarily prevent a good next action. Lore should state the best available recommendation, its decisive boundary and any consequential remaining assumption. When a genuine authority, requirement or capability dependency blocks part of a task, it should still identify meaningful independent work that can proceed. It must never invent authorization or hide a critical precondition just to sound decisive.

**Initiative is automatic; access is not.** A standing, understandable permission envelope authorizes routine work without repeated prompts. Existing denials remain denials, users can narrow or cancel work, and untrusted project content cannot grant tools, network access or policy-changing authority.

**Autonomy does the investigative homework; deliberate practice builds competence.** Lore must not pass routine evidence gathering back to the user, but it should not steal an explicit learning exercise by silently completing it either.

These are product commitments for future development, not claims that Lore 0.6 already implements the adaptive default. The [autonomous assistance design](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) defines the proposed controller, answer contract, permissions, stopping rules and acceptance tests.

## One knowledge core, audience-appropriate experiences

### For humans

For someone new to the project, opening Lore should feel like meeting a knowledgeable teammate who has done the investigative homework and can teach it in a sensible order. Show an immediate overview, an understandable guided tour, one small exercise and a clear route to the first meaningful contribution. Let experienced newcomers skip lessons. For an experienced developer or maintainer, Lore should give a direct, project-aware answer or recommendation, with important assumptions, historical constraints and their consequences. Lead with an understandable answer, a clear mental model or the preferred next action. Let the reader expand naturally into systems, concepts, decisions, history, examples and exact evidence.

Important conditions stay visible; detailed investigation logs and source manifests are available on demand. Show what materially changed without requiring someone to reread the collection. Make assumptions easy to correct, alternatives easy to compare and scope easy to narrow. A learning experience can deliberately invite practice; an ordinary request for help should not become a questionnaire.

The reading experience should be welcoming and polished without requiring a special viewer. Portable Markdown and ordinary source links remain first-class outputs. Knowledge Zoom and Diátaxis modes are ways to deliver useful understanding, not navigation work the user must complete before receiving help.

### For coding agents

A coding agent should be able to ask: **What matters before changing this part of the system, and what should I do next?** This is a **coequal flagship experience**, not a tutorial in machine-readable form.

Lore should return a compact versioned, machine-readable package: preferred approach, applicable constraints and exceptions, evidence and actual inspected implementation seams, observations already collected, safe progress, completion criteria and exact unresolved dependencies. It should distinguish documentary intent, reported behavior, static inspection, actual executed checks and inference. Do not make the calling agent repeat investigation Lore already performed or could reasonably have completed.

The agent still owns implementation, verification and delivery of code it subsequently changes. A test to run after a future edit is different from an existing-source inspection Lore could perform now. Context should make that distinction explicit.

Agents should not need to load an entire wiki, rely on a proprietary protocol or invoke another coding agent merely to retrieve project understanding. Markdown and CLI/JSON are foundations; an optional MCP adapter can improve compatibility without becoming mandatory.

**The human guide and the agent context are views of the same underlying knowledge**, not independently maintained answers.

## Product principles

**Shared intelligence, not an onboarding-only product.** Optimize for accurate project understanding and better decisions for newcomers, experienced contributors, maintainers and coding agents. Audience-specific views never create competing versions of project truth.

**Newcomer competence over documentation volume.** Optimize the human learning experience for a correct, understood first contribution and independent performance on a distinct follow-up task. A generated solution without human understanding is not evidence of learning.

**Agent implementation quality over context volume.** Optimize task-specific context for correctly completed changes, fewer missed constraints and less unnecessary investigation. A long answer or high `proceed` rate is not evidence of value.

**Empowerment over output.** Optimize for correct progress, understanding and user control—not pages, warnings, model calls or a high proceed rate in isolation. Include reading, correction, repeated investigation and waiting in the cost of an answer.

**Understanding over summarization.** Connect concepts, decisions, motivations, dependencies, conditions and changes rather than reproducing one summary per file.

**Own useful investigation.** Resolve what can materially change the answer using available permitted evidence. Do not delegate routine investigative work, perform exhaustive low-value exploration or turn non-blocking uncertainty into mandatory review.

**Evidence over confident presentation.** Source-dependent conclusions remain traceable. A citation establishes what a source said, not necessarily what runs in production. Useful hypotheses and general heuristics are allowed, but keep their basis and applicability distinct from accepted knowledge.

**History without confusion.** Preserve what was proposed, accepted, rejected, superseded or withdrawn. Neither the newest document nor a closed issue automatically establishes current truth or authorization.

**Experience that compounds.** Reuse structured investigation findings when their source revisions, environment, scope and permissions remain applicable. Search for new counterevidence as well as checking old support. Repetition must never promote an inference into policy.

**Tutorial-led onboarding, not forced tutorials.** Deliberate learning is a first-class experience, with source-grounded exercises, gradually fading hints and optional feedback. Ordinary question answering stays direct; anyone can skip, explore, or work on a real task.

**Answer first, detail on demand.** Start with a project essence, direct answer or recommended action and decisive conditions as appropriate to intent. Make evidence, rationale, alternatives and history easy to explore without burying the result.

**Radically simple UX.** Sophistication belongs in the engine. One request should produce useful help without choosing investigation depth, documentation mode, a graph path or model settings. Advanced controls preserve predictable restrictions and compatibility.

**Local-first, open and portable.** Keep the core usable without a hosted account, mandatory service or graphical reader. Respect where inference runs, what data leaves the machine, cache policy and explicit grants. No hidden telemetry or background continuation.

**Human agency and authority.** Users can inspect evidence, correct assumptions, narrow scope and cancel. Humans decide genuinely unavailable requirements and matters of authority; they are not the default investigators. Lore does not silently edit accepted policy or turn model guesses into authoritative facts.

## Where we are today

Lore 0.6 is a local-first Rust CLI that ingests local Markdown and optional OpenWiki, Engram and Beads snapshots, retains exact evidence and versioned knowledge, and publishes a cited Markdown wiki. Its default `lore context` command produces a preferred approach with explicit readiness, relevant evidence, constraints, prioritized checks and completion criteria. Opt-in inspection and investigation can read relevant local source, test hypotheses and revise recommendations without executing or changing code. Optional semantic retrieval combines embeddings with lexical and recorded relationship signals. `--fast` preserves deterministic, budgeted, model-free retrieval.

Generated interpretations and search indexes remain separate from the registry. Documentary authority, imported reports, current source evidence, static observations and inferred rationale retain different meanings. Local inference remains the default; hosted inference and checkout egress have their own explicit configuration. Search, reading, JSON output, provenance, native evidence, cross-source discrepancies and the evidence-bound review workflow remain available.

The intelligence layer and its integrity checks are implemented, but real-model factual quality and end-user usefulness still need empirical validation. The coding-task evaluator compares proposed implementations using baseline sources, fast context and intelligent guidance; bundled fixtures do not establish independent gains. Lore 0.6 does not provide the proposed adaptive default, reusable investigation store, dedicated web UI, conversational grounded Q&A, direct tracker synchronization or independent verification of a running system. See [README](README.md), [the 0.6 guide](docs/V06.md) and [decision evaluation](evaluation/DECISION_INTELLIGENCE.md).

## What we should build toward

**First, prove one shared intelligence engine across two small, real experiences.** Reuse the existing Lore 0.6 source and decision core, add bounded adaptive permitted investigation, and expose a consistent source/permission contract to both:

1. **Human slice:** a useful newcomer orientation, a short grounded workflow tour and one optional meaningful learning activity.
2. **Agent slice:** one task-specific machine-readable package containing an actionable recommendation, concrete constraints, real inspected observations and future completion checks.

This **first engineering milestone** is intentionally smaller than a complete onboarding curriculum or multi-level wiki rebuild. It excludes a bespoke GUI, mandatory agent runtime, code edits, sandboxed test execution, live tracker integrations and persistent learner profiles. It must demonstrate correct source binding and at least one independently assessed example from each experience; synthetic fixtures establish contracts, not measured superiority.

**Next, deepen each experience on its own merits.** Complete the first-contribution and independent-transfer evaluations for humans; establish held-out coding-task improvements for agents; then add revalidated investigation reuse, richer Knowledge Zoom, four Diátaxis modes, documented decision conditions, negative cases and optional reader/approved execution. These capabilities share evidence and source authority without forcing a single user flow.

The [Developer Onboarding design](docs/DEVELOPER_ONBOARDING_DESIGN.md), [Coding-Agent Intelligence design](docs/CODING_AGENT_INTELLIGENCE_DESIGN.md), [Knowledge Experience architecture](docs/KNOWLEDGE_EXPERIENCE_DESIGN.md), [Autonomous Assistance contract](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md), and [phased roadmap](docs/KNOWLEDGE_EXPERIENCE_ROADMAP.md) are future proposals. Lore 0.6's command/schema and permission behavior remain unchanged until an explicitly validated release.

## How we will know it works

**Two independent north-star outcomes:**

- **Human learning:** time to a correct, understood first contribution, plus success on a distinct related task with less assistance.
- **Coding-agent quality:** time and total effort to a correct, constraint-respecting implementation, including avoided repeated investigation and material mistakes.

Neither score substitutes for the other. Experienced developers and maintainers additionally need direct correctness, usable decision explanations and fewer avoidable handoffs.

Test newcomers on architecture understanding, first-task success and transfer; test coding agents separately on real task completion, policy/exception preservation and rework. Include original sources, OpenWiki where available, current Lore and new/combined experiences under comparable permissions, models and budgets. Record which work was performed by a human versus an agent; whether Lore completed worthwhile investigations; and whether counterevidence changed the recommendation when warranted. Measure false blocking and unsafe proceeding together, so apparent decisiveness never substitutes for judgment.

Measure clarity, reading/correction burden, useful partial progress, actual completion time, learning transfer, cancellation/override behavior, cold/warm cost and latency, and safely avoided repeated work. Reuse must detect new contrary evidence, changed deeper-round source and reduced permissions. Historical evidence remains available without appearing current.

Keep operational promises measurable: exact source binding, controlled egress, bounded inference/inspection, recoverable publication, purge, portable output and zero model calls on a genuinely unchanged update.

**Lore succeeds when everyone working in a project becomes more capable:** newcomers learn and contribute independently; experienced developers and maintainers make sounder decisions; coding agents implement better changes with less wasted effort. It achieves this through shared evidence, initiative, useful distillation and adaptable presentation—not by producing more documentation.
