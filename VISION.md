# Lore vision

> **Get up to speed on any project.**

**Our primary mission is to help a new developer become productive in an unfamiliar project—then become more independent with every task.** We also want experienced developers and coding agents to draw on the same evolving, evidence-backed project intelligence.

Lore should make project knowledge easy to discover, learn, appropriately trust and put to work. Its most compelling achievement is not another generated wiki: **a newcomer builds an accurate mental model, makes a correct first contribution, and successfully tackles a different related task with less help**.

## The problem

Joining an existing project is hard even for an experienced engineer. Code and documents rarely provide a coherent route from **what this system is for** to **how a real request flows**, **why its constraints exist** and **where a first safe change belongs**. New developers don't know what matters yet; giving them more pages or a search box often increases the burden.

Project intent is scattered across architecture documents, ADRs, plans, issue trackers, investigations and conversations. The code shows much of what was implemented, but often cannot explain why an approach was chosen, which alternatives were rejected, whether a proposal was approved or what changed since an earlier decision.

Humans spend time reconstructing that context. Coding agents may miss it even when they can read every source file. A pile of summaries is not enough: it can hide disagreements, flatten history and turn a reported outcome into an unwarranted fact. Merely identifying those problems and asking the user to investigate them also falls short. Lore should do the useful work it is capable and permitted to do, then deliver the best defensible help.

## The promise

Lore helps answer four questions:

1. **What?** What does this project do, and how do its parts fit together?
2. **Why?** Which decisions, constraints and trade-offs explain the design?
3. **Now what?** What is the current documented understanding, what changed and what remains uncertain?
4. **So what?** What matters for my task, and what is the best useful action I can take?

For **newcomers**, these answers become a guided journey: project essence → real workflow → small practice → first correct contribution → independent second task. For other developers and coding agents, the fourth question remains a direct decision-ready answer. Neither group should be forced through a documentation hierarchy.

## Teach a new developer the project

The first experience should be welcoming and immediately useful. A proposed `lore onboard` shows the project's purpose, a few pivotal concepts and one evidenced workflow **without a setup questionnaire**. A short source-linked tour follows a request through the important boundaries, decisions and failure cases. The developer can dive into any concept or skip straight to a real task.

A tailored learning path follows **conceptual prerequisites**, not directories or a fixed-depth summary tree. **Adaptive Knowledge Zoom** shows an overview or the exact evidence at the depth needed. **Diátaxis** gives each interaction its proper shape: Tutorials lead the learning journey; Explanation clarifies why; Reference answers precise questions; How-to supports actual contributions.

Lore helps a learner predict behavior, understand a failure, and work through one purposeful exercise. It then supports a real bounded task, performs routine investigation itself, and offers progressively fewer hints for a new related task. The learner remains in control of their work; an agent producing code is not the same as the developer having learned. Voluntary checkpoints can demonstrate understanding, but page views cannot certify mastery.

This is a proposed product direction, not a claim that these capabilities are implemented in 0.6. See [Developer Onboarding design](docs/DEVELOPER_ONBOARDING_DESIGN.md).

## Maximum useful autonomy. Minimum user burden.

The user describes the goal. Lore takes responsibility for retrieving available evidence, investigating consequential uncertainty, evaluating alternatives and recommending the strongest defensible approach within its permissions.

Lore should not ask people or calling agents to investigate, interpret evidence or perform routine checks it can usefully complete itself. It should choose investigative effort automatically rather than require users to discover flags, tune reasoning effort or classify their request. It should stop when it has enough evidence for the scoped decision or further investigation is unlikely to change the result—not when every uncertainty has disappeared.

Incomplete knowledge does not necessarily prevent a good next action. Lore should state the best available recommendation, its decisive boundary and any consequential remaining assumption. When a genuine authority, requirement or capability dependency blocks part of a task, it should still identify meaningful independent work that can proceed. It must never invent authorization or hide a critical precondition just to sound decisive.

**Initiative is automatic; access is not.** A standing, understandable permission envelope authorizes routine work without repeated prompts. Existing denials remain denials, users can narrow or cancel work, and untrusted project content cannot grant tools, network access or policy-changing authority.

**Autonomy does the investigative homework; deliberate practice builds competence.** Lore must not pass routine evidence gathering back to the user, but it should not steal an explicit learning exercise by silently completing it either.

These are product commitments for future development, not claims that Lore 0.6 already implements the adaptive default. The [autonomous assistance design](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) defines the proposed controller, answer contract, permissions, stopping rules and acceptance tests.

## One knowledge core, two experiences

### For humans

For someone new to the project, opening Lore should feel like meeting a patient, knowledgeable teammate who has already done the investigative homework and knows a sensible order in which to teach the project. Show an immediate overview, an understandable guided tour, one small exercise and a clear route to the first meaningful contribution. Let experienced newcomers skip lessons. Beyond onboarding, opening Lore should feel like consulting that same knowledgeable teammate. Lead with an understandable answer, a clear mental model or the preferred next action. Let the reader expand naturally into systems, concepts, decisions, history, examples and exact evidence.

Important conditions stay visible; detailed investigation logs and source manifests are available on demand. Show what materially changed without requiring someone to reread the collection. Make assumptions easy to correct, alternatives easy to compare and scope easy to narrow. A learning experience can deliberately invite practice; an ordinary request for help should not become a questionnaire.

The reading experience should be welcoming and polished without requiring a special viewer. Portable Markdown and ordinary source links remain first-class outputs. Knowledge Zoom and Diátaxis modes are ways to deliver useful understanding, not navigation work the user must complete before receiving help.

### For coding agents

A coding agent should be able to ask: What matters before changing this part of the system, and what should I do next?

Lore should return a focused machine-readable package: preferred approach, applicable constraints, decisive evidence, observations already collected, safe progress, completion criteria and exact unresolved dependencies. It should distinguish documentary intent, reported behavior, static inspection, actual executed checks and inference. Do not make the calling agent repeat investigation Lore already performed or could reasonably have completed.

The agent still owns implementation and verification of code it subsequently changes. A test to run after a future edit is different from an existing-source inspection Lore could perform now. Context should make that distinction explicit.

Agents should not need to load an entire wiki, rely on a proprietary protocol or invoke another coding agent merely to retrieve project understanding. Markdown and CLI/JSON are foundations; an optional MCP adapter can improve compatibility without becoming mandatory.

**The human guide and the agent context are views of the same underlying knowledge**, not independently maintained answers.

## Product principles

**Newcomer competence over documentation volume.** Optimize for an accurate mental model, correct first contribution, justified choices and less-assisted success on a distinct follow-up task. A generated solution without human understanding does not satisfy the mission.

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

**Start with the complete newcomer journey, not a separate knowledge feature.** Reuse Lore 0.6 retrieval and permitted inspection, add minimal adaptive investigation and build:

1. An immediate grounded project orientation and tour of one real workflow.
2. A short, source-bound learning path through prerequisites, with one meaningful tutorial exercise.
3. A first-contribution companion that supplies concrete context, constraints and progressively optional hints.
4. Independent assessment of a distinct second task, avoiding credit for code written entirely by a coding agent.

Only after this coherent experience should we optimize broad recursive distillation, large learning catalogs, optional personalization, elaborate reader interfaces or automatic execution. Adaptive Knowledge Zoom and the four Diátaxis modes remain vital building blocks, but **they serve competence rather than become the product goal**.

The [Developer Onboarding design](docs/DEVELOPER_ONBOARDING_DESIGN.md), [Knowledge Experience architecture](docs/KNOWLEDGE_EXPERIENCE_DESIGN.md), [Autonomous Assistance contract](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) and [newcomer-first roadmap](docs/KNOWLEDGE_EXPERIENCE_ROADMAP.md) specify these proposals. The roadmap sequences R0, minimal A1/A2 and O1–O3 into a working onboarding slice; A3 experience reuse and broader R1–R7 enrich it. These are **not current Lore 0.6 commands or features**.

`lore context` has provided task retrieval since 0.3 and intelligent guidance since 0.5. Proposed `lore onboard`, view, learning, change and reader interfaces remain unimplemented until explicitly released.

## How we will know it works

Our north star for onboarding is **time to a correct, understood first contribution—and independently correct work on a different related task with less assistance**.

Test whether a newcomer can accurately explain an end-to-end workflow, predict a relevant edge case, make a correct bounded change and transfer that understanding to a new task; distinguish their work from a coding agent's code. Compare original docs, OpenWiki when available, current Lore and the new guided experience under matched capability and budget. Also test whether an agent makes better changes while preserving constraints; whether Lore completes available investigation instead of handing it off; and whether important uncertainty changes the action when it should. Measure false blocking and unsafe proceeding together, so apparent decisiveness never substitutes for judgment.

Measure clarity, reading/correction burden, useful partial progress, actual completion time, learning transfer, cancellation/override behavior, cold/warm cost and latency, and safely avoided repeated work. Reuse must detect new contrary evidence, changed deeper-round source and reduced permissions. Historical evidence remains available without appearing current.

Keep operational promises measurable: exact source binding, controlled egress, bounded inference/inspection, recoverable publication, purge, portable output and zero model calls on a genuinely unchanged update.

**Lore succeeds when someone new to a project can understand its important ideas, make a correct first change, and approach the next change with genuine independence.** It achieves this by doing useful investigation, teaching through meaningful practice and respecting the learner's judgment—not by producing more documentation.
