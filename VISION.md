# Lore vision

> **Give your project a memory that grows with it.**

**We want every developer and coding agent to understand a software project as though they have worked on it for years.**

Lore should make project knowledge easy to discover, easy to trust *appropriately*, and useful when making engineering decisions. The goal is not more documentation. The goal is better understanding—and, through that understanding, better decisions.

## The problem

Project intent is scattered across architecture documents, ADRs, plans, issue trackers, investigations, and conversations. The code shows much of *what* was implemented, but it often cannot explain *why* an approach was chosen, which alternatives were rejected, whether a proposal was approved, or what changed since an earlier decision.

Humans spend time reconstructing that context. Coding agents may miss it entirely, even when they can read every source file. A pile of summaries is not enough: it can hide disagreements, flatten history, and turn a reported outcome into an unwarranted statement of fact.

## The promise

Lore helps answer four questions:

1. **What?** What does this project do, and how do its parts fit together?
2. **Why?** Which decisions, constraints, and trade-offs explain the design?
3. **Now what?** What is the current *documented* understanding, what changed, and what remains uncertain?
4. **So what?** Which of those details matter for the work I am about to do?

The fourth question is the long-term ambition. Lore should help someone act on relevant project understanding, not merely browse a collection of generated pages.

## One knowledge core, two experiences

### For humans

Opening Lore should feel like opening a well-maintained guide written by a knowledgeable teammate:

- Start with a clear project overview and navigate naturally into systems, concepts, decisions, history, and open questions.
- Get a useful explanation before needing to inspect a source file.
- Follow material claims back to exact evidence, including the historical context and status of that evidence.
- See what changed since the last update without rereading the whole project.
- Correct or review ambiguous interpretations without erasing what the sources originally said.

The reading experience should be welcoming and polished without requiring a special viewer just to access the knowledge. Portable Markdown remains a first-class output.

### For coding agents

A coding agent working on a task should be able to ask, in effect: *What should I know before changing this part of the system?*

Lore should return a focused, machine-readable context package: relevant architecture, decisions, constraints, affected concepts, open questions, and evidence links. It should separate documented intent from verified implementation, expose uncertainty rather than inventing certainty, and make it easy for the agent to verify details in source code and tests.

Agents should not need to load an entire wiki, rely on a proprietary protocol, or invoke another coding agent to retrieve project understanding. Markdown and CLI/JSON are useful foundations; an optional MCP adapter could improve compatibility without becoming a requirement.

**The human guide and the agent context must be views of the same underlying knowledge**, not two independently maintained sets of answers.

## Product principles

**Understanding over summarization.** Organize knowledge by topic and meaning, connecting concepts, decisions, motivations, dependencies, and changes rather than reproducing one summary per document.

**Evidence over confidence.** Every material source-dependent conclusion should be traceable to exact evidence. A citation proves what a source said, not necessarily that the claim is true in production. Distinguish source reports, accepted decisions, and independently verified behavior.

**History without confusion.** Preserve what was believed, proposed, accepted, superseded, or withdrawn, and when the evidence supports those distinctions. Neither the newest document nor a closed issue automatically becomes the truth.

**Context at the moment of decision.** Optimize for the information needed to understand and complete a task, not for maximizing wiki pages, knowledge units, or generated words.

**Radically simple UX.** A small set of predictable entry points should cover setup, updates, reading, searching, reviewing, and task-focused guidance. Sophistication belongs in the engine, not in configuration burden.

**Local-first, open, and portable.** Keep the core usable without a hosted service or mandatory MCP server. Preserve user control over where inference runs and when project information leaves the machine. Prefer plain Markdown and stable machine-readable interfaces.

**Human judgment stays in control.** Surface ambiguity, allow auditable corrections, and never silently turn model guesses into authoritative project facts.

## Where we are today

Lore 0.6 is a local-first Rust CLI that ingests local Markdown and optional OpenWiki, Engram, and Beads snapshots, retains exact evidence and versioned knowledge, and publishes a cited Markdown wiki. Its default `lore context` command produces a preferred approach with explicit readiness, relevant evidence, constraints, prioritized checks, and completion criteria. Opt-in inspection and investigation can read relevant local source, test hypotheses, and revise the recommendation without executing or changing code. Optional semantic retrieval combines embeddings with lexical and recorded relationship signals. `--fast` preserves deterministic, budgeted, model-free retrieval.

Generated interpretations and search indexes remain separate from the knowledge registry. Current source evidence, documentary authority, reported implementation, and inferred rationale retain distinct meanings. Local inference remains the default; hosted inference requires explicit configuration. Search, reading, JSON output, provenance, native structured evidence, cross-source discrepancies, and the evidence-bound review workflow remain available.

The intelligence layer and its integrity checks are implemented, but **real-model factual quality and end-user usefulness still need empirical validation**. The coding-task evaluator compares actual proposed implementations with baseline sources, fast context, and intelligent guidance; bundled fixtures cannot establish independent model-quality gains. Lore does not yet provide a dedicated web UI, conversational grounded Q&A, direct issue-tracker synchronization, or independent verification of a running system. See [README.md](README.md), [the v0.6 guide](docs/V06.md), and [the coding-task evaluation guide](evaluation/DECISION_INTELLIGENCE.md).

## What we should build toward

The sequence matters more than any specific interface:

1. **Prove understanding.** Evaluate real model outputs with human reviewers. Measure omissions, false merges, incorrect decision timelines, evidence quality, utility, latency, and cost.
2. **Make understanding delightful to use.** Improve the project overview, navigation, explanations, links, and review experience for humans.
3. **Prove the usefulness of agent context.** Measure whether 0.6 guidance avoids mistakes and improves implementations beyond deterministic context, then improve retrieval and reasoning using those results.
4. **Make knowledge easier to ask and maintain.** Explore grounded Q&A, change awareness, and direct integrations for sources such as GitHub Wiki and YouTrack.
5. **Connect documented intent to code carefully.** Where useful, distinguish documentation from implementation observations and help flag possible mismatches without claiming that a citation alone verifies runtime behavior.

### Proposed knowledge experience

The [Knowledge Experience design](docs/KNOWLEDGE_EXPERIENCE_DESIGN.md) explores multi-resolution, evidence-preserving views and Diátaxis-oriented experiences (explain, how-to, tutorial, reference), with scoped decision assumptions, exceptions, knowledge-gap questions, worked cases and change-aware guidance. The [phased roadmap](docs/KNOWLEDGE_EXPERIENCE_ROADMAP.md) orders retrieval/zoom and practical modes before riskier agent execution or personalized experiences. **It is an unimplemented proposal**, not a change to the accepted 0.6 commands or authority model. Judge each phase by correct task outcomes and learning rather than generated page counts.

`lore context` has provided task retrieval since 0.3 and intelligent guidance since 0.5. Possible future commands such as `lore open` or `lore ask` remain **illustrative product ideas**, not current CLI commands or commitments to a particular design.

## How we will know it works

Our north star is **time to a correct, well-informed engineering decision**.

We should test whether a new developer can explain the architecture and its rationale accurately; whether an agent with Lore identifies relevant constraints and makes better task decisions than one without it; whether important answers are supported by evidence; and whether updates remove stale interpretations without erasing history.

We should also measure operational fundamentals: the cost of keeping knowledge current, the amount of unnecessary output churn, local-only privacy guarantees, and zero model calls on a genuinely unchanged update.

**Lore succeeds when people and agents make better changes because they understand the project—not because Lore produces more pages.**
