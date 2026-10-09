# Lore vision

> **Give your project a memory that grows with it.**

**We want every developer and coding agent to understand a software project as though they have worked on it for years.**

Lore should make project knowledge easy to discover, easy to trust appropriately, and useful when making engineering decisions. The goal is not more documentation. It is better understanding, better decisions and greater ability to act.

## The problem

Project intent is scattered across architecture documents, ADRs, plans, issue trackers, investigations and conversations. The code shows much of what was implemented, but often cannot explain why an approach was chosen, which alternatives were rejected, whether a proposal was approved or what changed since an earlier decision.

Humans spend time reconstructing that context. Coding agents may miss it even when they can read every source file. A pile of summaries is not enough: it can hide disagreements, flatten history and turn a reported outcome into an unwarranted fact. Merely identifying those problems and asking the user to investigate them also falls short. Lore should do the useful work it is capable and permitted to do, then deliver the best defensible help.

## The promise

Lore helps answer four questions:

1. **What?** What does this project do, and how do its parts fit together?
2. **Why?** Which decisions, constraints and trade-offs explain the design?
3. **Now what?** What is the current documented understanding, what changed and what remains uncertain?
4. **So what?** What matters for my task, and what is the best useful action I can take?

The fourth question is the ambition. Lore should make someone more capable of acting on project understanding, not merely better at browsing generated pages.

## Maximum useful autonomy. Minimum user burden.

The user describes the goal. Lore takes responsibility for retrieving available evidence, investigating consequential uncertainty, evaluating alternatives and recommending the strongest defensible approach within its permissions.

Lore should not ask people or calling agents to investigate, interpret evidence or perform routine checks it can usefully complete itself. It should choose investigative effort automatically rather than require users to discover flags, tune reasoning effort or classify their request. It should stop when it has enough evidence for the scoped decision or further investigation is unlikely to change the result—not when every uncertainty has disappeared.

Incomplete knowledge does not necessarily prevent a good next action. Lore should state the best available recommendation, its decisive boundary and any consequential remaining assumption. When a genuine authority, requirement or capability dependency blocks part of a task, it should still identify meaningful independent work that can proceed. It must never invent authorization or hide a critical precondition just to sound decisive.

**Initiative is automatic; access is not.** A standing, understandable permission envelope authorizes routine work without repeated prompts. Existing denials remain denials, users can narrow or cancel work, and untrusted project content cannot grant tools, network access or policy-changing authority.

These are product commitments for future development, not claims that Lore 0.6 already implements the adaptive default. The [autonomous assistance design](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) defines the proposed controller, answer contract, permissions, stopping rules and acceptance tests.

## One knowledge core, two experiences

### For humans

Opening Lore should feel like consulting a knowledgeable teammate who has already done the useful homework. Lead with an understandable answer, a clear mental model or the preferred next action. Let the reader expand naturally into systems, concepts, decisions, history, examples and exact evidence.

Important conditions stay visible; detailed investigation logs and source manifests are available on demand. Show what materially changed without requiring someone to reread the collection. Make assumptions easy to correct, alternatives easy to compare and scope easy to narrow. A learning experience can deliberately invite practice; an ordinary request for help should not become a questionnaire.

The reading experience should be welcoming and polished without requiring a special viewer. Portable Markdown and ordinary source links remain first-class outputs. Knowledge Zoom and Diátaxis modes are ways to deliver useful understanding, not navigation work the user must complete before receiving help.

### For coding agents

A coding agent should be able to ask: What matters before changing this part of the system, and what should I do next?

Lore should return a focused machine-readable package: preferred approach, applicable constraints, decisive evidence, observations already collected, safe progress, completion criteria and exact unresolved dependencies. It should distinguish documentary intent, reported behavior, static inspection, actual executed checks and inference. Do not make the calling agent repeat investigation Lore already performed or could reasonably have completed.

The agent still owns implementation and verification of code it subsequently changes. A test to run after a future edit is different from an existing-source inspection Lore could perform now. Context should make that distinction explicit.

Agents should not need to load an entire wiki, rely on a proprietary protocol or invoke another coding agent merely to retrieve project understanding. Markdown and CLI/JSON are foundations; an optional MCP adapter can improve compatibility without becoming mandatory.

**The human guide and the agent context are views of the same underlying knowledge**, not independently maintained answers.

## Product principles

**Empowerment over output.** Optimize for correct progress, understanding and user control—not pages, warnings, model calls or a high proceed rate in isolation. Include reading, correction, repeated investigation and waiting in the cost of an answer.

**Understanding over summarization.** Connect concepts, decisions, motivations, dependencies, conditions and changes rather than reproducing one summary per file.

**Own useful investigation.** Resolve what can materially change the answer using available permitted evidence. Do not delegate routine investigative work, perform exhaustive low-value exploration or turn non-blocking uncertainty into mandatory review.

**Evidence over confident presentation.** Source-dependent conclusions remain traceable. A citation establishes what a source said, not necessarily what runs in production. Useful hypotheses and general heuristics are allowed, but keep their basis and applicability distinct from accepted knowledge.

**History without confusion.** Preserve what was proposed, accepted, rejected, superseded or withdrawn. Neither the newest document nor a closed issue automatically establishes current truth or authorization.

**Experience that compounds.** Reuse structured investigation findings when their source revisions, environment, scope and permissions remain applicable. Search for new counterevidence as well as checking old support. Repetition must never promote an inference into policy.

**Answer first, detail on demand.** Start with the direct answer or recommended action and decisive conditions. Make evidence, rationale, alternatives and history easy to explore without burying the result.

**Radically simple UX.** Sophistication belongs in the engine. One request should produce useful help without choosing investigation depth, documentation mode, a graph path or model settings. Advanced controls preserve predictable restrictions and compatibility.

**Local-first, open and portable.** Keep the core usable without a hosted account, mandatory service or graphical reader. Respect where inference runs, what data leaves the machine, cache policy and explicit grants. No hidden telemetry or background continuation.

**Human agency and authority.** Users can inspect evidence, correct assumptions, narrow scope and cancel. Humans decide genuinely unavailable requirements and matters of authority; they are not the default investigators. Lore does not silently edit accepted policy or turn model guesses into authoritative facts.

## Where we are today

Lore 0.6 is a local-first Rust CLI that ingests local Markdown and optional OpenWiki, Engram and Beads snapshots, retains exact evidence and versioned knowledge, and publishes a cited Markdown wiki. Its default `lore context` command produces a preferred approach with explicit readiness, relevant evidence, constraints, prioritized checks and completion criteria. Opt-in inspection and investigation can read relevant local source, test hypotheses and revise recommendations without executing or changing code. Optional semantic retrieval combines embeddings with lexical and recorded relationship signals. `--fast` preserves deterministic, budgeted, model-free retrieval.

Generated interpretations and search indexes remain separate from the registry. Documentary authority, imported reports, current source evidence, static observations and inferred rationale retain different meanings. Local inference remains the default; hosted inference and checkout egress have their own explicit configuration. Search, reading, JSON output, provenance, native evidence, cross-source discrepancies and the evidence-bound review workflow remain available.

The intelligence layer and its integrity checks are implemented, but real-model factual quality and end-user usefulness still need empirical validation. The coding-task evaluator compares proposed implementations using baseline sources, fast context and intelligent guidance; bundled fixtures do not establish independent gains. Lore 0.6 does not provide the proposed adaptive default, reusable investigation store, dedicated web UI, conversational grounded Q&A, direct tracker synchronization or independent verification of a running system. See [README](README.md), [the 0.6 guide](docs/V06.md) and [decision evaluation](evaluation/DECISION_INTELLIGENCE.md).

## What we should build toward

Begin with adaptive assistance over the existing retrieval and safe read-only inspection foundation. Couple it to outcome-first responses, meaningful partial progress, precise noninteractive dependencies and evidence-bound reusable investigations. Measure complete task outcomes and user burden, not only factual warnings or first-response latency.

Next, improve contextual retrieval where experiments justify it; add expandable explanations and exact reference; deliver goal-specific how-to and learning experiences; and connect decision conditions, negative cases and consequential changes. Investigation-first gaps can improve the knowledge base without forcing every user into an approval workflow.

Optional execution requires separately approved, independently tested isolation. A local reader, live adapters and cross-project transfer follow where they add demonstrated value. None is a prerequisite for the initial experience of Lore doing the useful investigative work itself.

The [Knowledge Experience design](docs/KNOWLEDGE_EXPERIENCE_DESIGN.md), [autonomy and UX contract](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) and [empowerment-first roadmap](docs/KNOWLEDGE_EXPERIENCE_ROADMAP.md) specify these proposals. The roadmap prioritizes A1–A3 adaptive assistance, action-first UX and reusable investigations before hierarchy and visual polish. All proposed commands/configuration/schemas are unimplemented until an explicit release establishes them; current 0.6 contracts and privacy restrictions remain unchanged.

`lore context` has provided task retrieval since 0.3 and intelligent guidance since 0.5. Possible new view, changes, cases or reader commands remain product proposals, not current CLI capabilities.

## How we will know it works

Our north star is **correct, well-informed progress with less total user effort**.

Test whether a newcomer can explain the architecture and apply its lessons; whether an agent makes better changes while preserving constraints; whether Lore completes available investigation instead of handing it off; and whether important uncertainty changes the action when it should. Measure false blocking and unsafe proceeding together, so apparent decisiveness never substitutes for judgment.

Measure clarity, reading/correction burden, useful partial progress, actual completion time, learning transfer, cancellation/override behavior, cold/warm cost and latency, and safely avoided repeated work. Reuse must detect new contrary evidence, changed deeper-round source and reduced permissions. Historical evidence remains available without appearing current.

Keep operational promises measurable: exact source binding, controlled egress, bounded inference/inspection, recoverable publication, purge, portable output and zero model calls on a genuinely unchanged update.

**Lore succeeds when people feel more capable because it has done the useful work, explained what matters and helped them proceed correctly—not because it has produced more documentation.**
