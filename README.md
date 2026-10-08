# Lore

**Give your project a memory that grows with it.**

Projects accumulate knowledge in all sorts of places. Architecture documents explain how a system works, ADRs record why choices were made, plans describe what might happen next, and issues capture investigations, bugs, and unfinished work. Over time, these Markdown files become a valuable but scattered record of the project. Finding the right document is one challenge; understanding how dozens of documents fit together is another. Lore is an open-source, local-first CLI being designed to turn that collection into a coherent, evidence-backed project wiki.

Lore is not meant to produce a stack of summaries, one for each source file. Instead, it will look across documents for important concepts, decisions, proposals, relationships, and unresolved questions, then explain those subjects in pages organized around the project itself. Every meaningful conclusion should remain traceable to the material that supports it. The result is intended to be useful whether you're joining the project, working on a feature, reviewing an old decision, or giving a coding agent the context it needs.

> **Project status:** Lore is currently in the design phase. The CLI, commands, and configuration examples below describe the intended experience; they are **not yet available to install or run**. See [DESIGN.md](DESIGN.md) for the technical design and implementation plan.

## A project is more than its documentation

Imagine a project with three documents. An idea suggests moving from MySQL to PostgreSQL. An accepted architecture decision says MySQL remains the selected database. A later issue asks someone to investigate a migration. A simple summarizer could blur these documents together and mistakenly report that the migration is happening. Lore should instead explain that MySQL is the documented choice, PostgreSQL is an alternative under consideration, and the investigation does not by itself establish that a migration was approved or implemented.

This distinction is central to the project. A plan is not a decision, a closed issue is not necessarily a completed feature, and a statement in an old document is not automatically true today. Lore is designed to preserve the type, status, timing, and source of project knowledge, including disagreements and uncertainty, instead of flattening everything into seemingly authoritative prose.

## A living wiki, not a one-time report

The first run will read configured Markdown directories, discover the project's major topics, extract useful knowledge, and assemble a linked Markdown wiki. The wiki might contain an overview, explanations of important systems, decision histories, active initiatives, workflows, and open questions. Its layout should follow the project's concepts rather than mirror the original directory tree, so information about one subject can be brought together even when it is scattered across many files.

Later runs should be incremental. When a source document changes, Lore will compare it with previously observed versions, locate the knowledge connected to the changed material, and reconsider the affected topics. Unchanged material should not have to pass through an AI model again. If a source disappears or contradicts another source, Lore should record the uncertainty and investigate the affected understanding rather than silently remove or overwrite it. A periodic broader audit can catch drift or gaps that narrow incremental updates might miss.

```text
Markdown directories
       |
       v
Source discovery and change detection
       |
       v
Evidence-backed knowledge extraction
       |
       v
Reconciliation across documents and time
       |
       v
Linked Markdown wiki
```

## What using Lore could look like

Lore will be a Rust command-line application with one knowledge base per project and any number of configured Markdown source directories. The intended interface is deliberately small, so the common workflow does not require an agent framework, a graph database, or a collection of services. For example, a future project configuration might look like this:

```yaml
# lore.yml — proposed configuration, not yet implemented
project:
  name: payments

sources:
  roots:
    - id: documentation
      path: ./docs
    - id: planning
      path: ./plans
    - id: issues
      path: ./issue-export

output:
  wiki_dir: ./lore
  state_dir: ./.lore

models:
  decision:
    provider: ollama
    model: clef-flash
  generative:
    provider: ollama
    model: gemma4:12b

providers:
  ollama:
    base_url: http://127.0.0.1:11434
```

Once implemented, the main commands are planned to be:

```bash
lore init       # Set up the project and create its first wiki
lore update     # Reconcile changes and refresh affected pages
lore status     # See source changes and pending work without model calls
lore audit      # Review evidence, gaps, and possible contradictions
```

The generated wiki will be ordinary Markdown in a configurable directory, while Lore's internal state and dependency information will live separately in a local SQLite database. The wiki should be readable with a text editor, on GitHub, or by a coding agent, without a proprietary viewer. Keeping generated output separate from its original sources also prevents Lore from repeatedly summarizing its own earlier work.

## Local-first, with flexible models

Lore is planned as a native Rust CLI, with Ollama as its first inference provider. A local generative model such as [Gemma 4](https://ollama.com/library/gemma4) can extract knowledge and write explanations, while a specialized decision model such as [Clef-Flash](https://ollama.com/library/clef-flash) can help with fast classification, routing, and candidate matching. These two types of models have different interfaces: Clef-Flash makes decisions over predefined answers, whereas a generative model produces new structured information and prose. Lore will keep those responsibilities separate rather than assume one model must do everything.

Running locally should be a first-class option, not an afterthought. Model adapters are intended to make cloud providers possible later, but project content should never be sent to a remote inference service without an explicit configuration choice. The core processing logic, SQLite state, and resulting Markdown wiki should not depend on which compatible provider happens to be used.

## What Lore is — and isn't

Lore is intended to compile **documented project understanding**. It will preserve provenance and distinguish an artifact's claims from independently verified reality, but it cannot guarantee that a Markdown document is correct or that an issue's status reflects what has actually shipped. The first version will work with local Markdown files, including exported issues and discussions. Direct issue-tracker connectors, code analysis, interactive chat, a hosted service, and a sophisticated graph UI are possible future directions, not promises of the first release.

The approach draws inspiration from [OpenWiki](https://github.com/langchain-ai/openwiki), whose evidence-backed claims and incremental updates demonstrate how a generated wiki can be maintained over time. Lore applies related ideas to a broader mix of project artifacts, with special attention to the difference between decisions, plans, observations, and questions. Research on grounded generation, hierarchical synthesis, and knowledge provenance also informs the design; the references and trade-offs are documented in [DESIGN.md](DESIGN.md).

## Getting involved

Lore is at a good stage for discussing the knowledge model, incremental update behavior, evaluation fixtures, and CLI experience before the implementation hardens. If you're interested in a local-first tool that helps both people and coding agents understand the story of a project, feedback and contributions are welcome. The best starting point is the [technical design](DESIGN.md), which distinguishes the proposed MVP from later possibilities and explains the decisions still open for discussion.

Lore is licensed under the [Apache License 2.0](LICENSE).
