# Lore

**Give your project a memory that grows with it.**

Projects collect their history in architecture notes, ADRs, plans, issue exports, investigations, and half-finished ideas. Those documents are valuable, but understanding a subject often means piecing together several files written at different times for different reasons. Lore is a Rust command-line application that reads your project's Markdown directories and maintains evidence-backed project knowledge. It retrieves the decisions, constraints, history, and open questions relevant to your next task. The same knowledge powers a linked, topic-oriented Markdown wiki.

**Lore 0.3 makes that knowledge available at the moment a change is being planned:** `lore context "Implement payment retries"` returns deterministic, source-cited task context without a model call, network access, or a database write. Retrieval combines lexical matches, path hints, topic and subject connections, and stored relationships. Results preserve documentary status, expose known conflicts, distinguish primary from derived sources, and fit an explicit output budget. See [the v0.3 guide](docs/V03.md) for the interface, upgrade behavior, and limitations. Real-model accuracy and a measurable advantage over ordinary wiki use remain evaluation goals, not claims established by fixture tests.

## Vision

**Make every developer and coding agent feel like they've been working on the project for years.**

Lore's goal is not to generate more documentation. It is to help people and agents understand **what** a project does, **how** it works, **why** decisions were made, **what has changed**, and **what matters** when making the next change.

Humans should get an inviting, navigable guide to the project's architecture, concepts, decisions, and open questions. Coding agents should be able to retrieve the relevant, evidence-backed context for a task instead of rediscovering intent from scattered files. Both experiences should draw from the same versioned knowledge, clearly distinguishing accepted decisions, proposals, historical context, and unverified reports.

The current CLI, task-context interface, and generated Markdown wiki are the foundation. A richer reading experience, conversational grounded questions and answers, and direct source integrations remain future directions. Read the [full vision](VISION.md) for the product principles, intended experience, and measures of success.

## Evaluating Lore

Lore's first CLI implementation is available, but we are still validating how accurately **real inference models** understand heterogeneous project documents. The [evaluation toolkit](evaluation/README.md) includes a controlled, evolving project with reviewed source checkpoints, Lore's own documentation, and pinned public OpenWiki and LLM Wiki corpora. It can run local Ollama or explicitly authorized hosted OpenAI inference, report provenance and incremental-update checks, and produce a human review sheet. Automated fixture tests and source hashes cannot establish semantic correctness, so [the baseline](evaluation/BASELINE.md) clearly separates what is already measured from the quality and billing data we still need to collect.

## Why a project needs more than a summary

Suppose an accepted ADR selects MySQL, a later idea proposes PostgreSQL, and an issue asks someone to investigate migration. A summary that treats every sentence as a current fact might announce that the project is moving to PostgreSQL. Lore instead preserves the distinction between an accepted decision, a proposal, and a work item. Closing the issue does not establish that anything shipped. A later document explicitly replacing the ADR can change the documented decision, while a deployment report remains a report rather than independent verification of production.

Each source keeps its own assertions and provenance. Equivalent assertions may support one consolidated knowledge unit, but their original excerpts remain separate and inspectable. When a document disappears, its historical evidence is retained and no longer presented as current support. When sources disagree, Lore preserves the disagreement and records a review item rather than choosing whichever document happened to be newest.

## Get started

Build and install from the repository using a current stable Rust toolchain. Lore bundles SQLite and uses Rust-based TLS; it does not require a separate database server, Python runtime, Node.js runtime, coding agent, or MCP server. Local inference requires Ollama and your chosen model separately; hosted inference requires your own provider credentials.

```bash
git clone https://github.com/grove/lore.git
cd lore
cargo install --path . --locked
```

In the project you want to document, create a configuration without running inference yet. You can name several source directories, including local Markdown exported from an issue tracker. Paths in the configuration are resolved relative to `lore.yml`, not whichever directory you later run the command from.

```bash
cd /path/to/your-project
lore init --configure-only --source ./docs --source ./plans --source ./issue-export
```

Edit `lore.yml` to select your source directories and models. The default is local-only Ollama with `gemma4:12b`; any suitable installed model with reliable structured JSON output can be selected instead. Start with a small representative set of documents, because first-time extraction and cross-document reconciliation can involve many model calls.

```bash
ollama pull gemma4:12b
lore doctor --inference
lore init
```

The result is ordinary Markdown under `lore/`, with an index and topic pages. Lore keeps its database, exact evidence snapshots, and reusable inference results under `.lore/`. Add `.lore/` to your project's `.gitignore`; it contains potentially sensitive copied project material. Review generated pages before publishing them, just as you would review other documentation.

## Retrieve context for your next change

```bash
lore context "Implement payment retries"
lore context "Implement payment retries" \
  --path src/payments/retry.rs \
  --path docs/payments/retry-policy.md \
  --max-tokens 3000
lore --json context "Implement payment retries"
```

`context` selects knowledge from the last compiled registry. It groups relevant constraints, decisions, documented design, historical knowledge, proposals, and items needing verification; each material assertion carries an archived evidence reference. Stored supersession and conflict relationships retain their endpoints and evidence. A tight budget can omit a complete connected group, with explicit omission counts and warnings, so callers can request more context without mistaking a partial result for complete guidance. Paths are optional relevance hints, not files to read or code changes to perform. Suggested inspection locations come from captured sources or documented references.

The default budget is 3,000 tokens. Lore counts the entire compact JSON and human-readable output with the embedded `cl100k_base` tokenizer and bounds both forms. This includes citations, metadata, and warnings; it does not include your surrounding prompt or guarantee the same count for another model's tokenizer. Whole records are selected without silently shortening their statements or evidence. See [the output and budget contract](docs/V03.md#output-and-budget-contract).

Retrieval runs locally without credentials, a running inference server, an embedding service, or MCP. It does not inspect live source changes or infer new contradictions; run `lore status` and `lore update` when the source corpus changes. A successful empty result means no eligible knowledge was retrieved, not that the task has no constraints. Lexical and relationship expansion can bridge connected concepts, but arbitrary synonyms and unstored relationships can still be missed.

For Codex, Claude Code, and other command-capable agents, use the [optional agent instruction snippet](docs/AGENTS.example.md). Agents can inspect any returned snapshot with `lore --json evidence EVIDENCE_ID` and should verify implementation details in the actual code and tests.

## Keep the wiki current

After editing your sources, run `lore update`. Lore compares file and section fingerprints, re-extracts changed sections, finds related knowledge, and regenerates affected topic pages. A genuinely unchanged run makes no model calls and leaves wiki content byte-for-byte unchanged. It can therefore complete without API credentials or a running inference server when the configuration and successful baseline are unchanged. New documents are compared with existing knowledge even when that knowledge's original source files were not edited.

```bash
lore status
lore update --dry-run
lore update
lore search "database migration"
lore read database-strategy
lore audit
```

`status`, `update --dry-run`, `context`, search, reading, and the ordinary audit do not call a model. `audit --deep` performs a broader, model-assisted refresh and reconciliation; it can be expensive. `update --refresh` bypasses semantic caches and re-extracts all sources, which is useful after changing a model behind an unchanged model alias. `update --rebuild` explicitly allows replacement of Lore-owned generated output, including manual edits, while retaining the knowledge history. By default, Lore refuses to overwrite edited pages or unmanaged files.

The CLI also provides `lore evidence <evidence-id>` for an exact archived passage and `lore review` for unresolved questions. `lore review show <review-id>` displays the history and evidence binding; `resolve`, `dismiss` and `reopen` accept an explicit `--reason` and optional `--actor`. These actions record a disposition, not a new project fact, and make no model calls. Specific relationship questions can also close automatically when active evidence from the same source revision establishes the named replacement; withdrawn evidence reopens them. `--config path/to/lore.yml` selects another configuration, and the global `--json` option produces machine-readable results, including argument errors. Operational errors exit with code 1, argument errors with code 2, and audit findings with code 3.

## An overview you can follow back to evidence

The generated index now explains the project's major systems, documented design, decisions, future initiatives and open questions in cited paragraphs. It uses a bounded, representative selection from every topic; the detailed pages preserve the full extracted knowledge. An extra generative check reviews the overview for unsupported claims when synthesis verification is enabled. Decision links name documents rather than repeating topic titles, so two decisions on the same page appear as “ADR-027 explicitly supersedes ADR-001.” The review-history page distinguishes pending questions from retained resolutions and makes the reason for each transition inspectable.

Documentary status and factual verification stay separate. An architecture specification can describe the selected design without proving what runs in production, and that lack of verification does not make it a future plan. Proposals remain future intent, reported deployments remain reports, and mandatory release rules are not confused with implementation completion. These distinctions are represented in the extraction schema and generation prompts; their accuracy should still be checked on real project documents.

## Reasoning effort by task

When using an OpenAI Responses-compatible generative model, Lore explicitly requests a **task-appropriate reasoning effort**: `low` for evidence extraction, `high` for semantic reconciliation and verification, and `medium` for topic and overview writing. These are configurable starting points, not benchmark-proven optimal levels. Set `models.reasoning` in `lore.yml` to override one or all tasks; `enabled: false` leaves the provider's reasoning level unspecified. The setting does not affect Ollama, Clef-Flash, TypeSafe Jev, or OpenAI Decisions. See [Reasoning configuration](docs/REASONING.md) for all supported levels, exact keys, and benchmark override flags.

## Local, hosted, or a combination

Generative models extract source assertions, reconcile meaning, and write explanations. Decision models answer predefined classification questions and provide advisory hints. Lore keeps the two interfaces separate: OpenAI uses Responses and Decisions, while Ollama uses Chat and System One. A decision model is optional, and an unavailable or refused decision does not cause a source document to be silently skipped. Substantive reconciliation remains generative work.

A minimal local configuration looks like this. The optional decision role uses Clef-Flash; omit it to use only the generative model. Literal loopback addresses are required for local HTTP endpoints so local-only mode does not depend on DNS resolution or configured HTTP proxies.

```yaml
schema_version: 1
project:
  name: payments
sources:
  roots:
    - id: docs
      path: ./docs
    - id: issues
      path: ./issue-export
output:
  wiki_dir: ./lore
  state_dir: ./.lore
privacy:
  local_only: true
models:
  generative:
    provider: ollama
    model: gemma4:12b
  decision:
    provider: ollama
    model: clef-flash
providers:
  ollama:
    base_url: http://127.0.0.1:11434
```

OpenAI is also an initial provider, with separate adapters for the Responses API and the Decisions API. To use hosted inference, explicitly set `privacy.local_only: false`, select the provider for each role, and provide its API key through the environment. Replace the generative model placeholder below with a Responses model available to your account that supports Structured Outputs. `doctor --inference` sends only small synthetic probes, not project documents, but hosted probes may incur normal API charges.

```yaml
privacy:
  local_only: false
models:
  generative:
    provider: openai
    model: YOUR_RESPONSES_MODEL_ID
  decision:
    provider: openai
    model: gpt-6-luna
providers:
  openai:
    api_key_env: OPENAI_API_KEY
    base_url: https://api.openai.com/v1
```

You can mix roles—for example, Ollama for synthesis and OpenAI for decisions—but hosted decisions also receive the source context needed to answer their questions. Local-only mode forbids hosted provider selection, remote endpoints, and recognized cloud-model tags; there is no implicit cloud fallback. Lore cannot control whether a separately managed Ollama server itself proxies requests elsewhere, so use genuinely local model weights and an appropriately configured server when confidentiality requires it.

An experimental TypeSafe System One adapter is included for evaluating Jev as a decision backend: select `provider: typesafe`, a model such as `jev-latest`, and `TYPESAFE_API_KEY`. Its protocol is covered by fixtures, but it remains a candidate rather than a claim of production-validated compatibility. Official provider contracts and further configuration examples are linked in [the implementation guide](docs/IMPLEMENTATION.md).

## Combine ordinary Markdown sources

Several source roots can contribute to one knowledge registry while keeping their own stable source identities. For example, ingest local exports from two projects and a generated wiki:

```yaml
sources:
  roots:
    - id: service-docs
      path: ./docs
    - id: platform-docs
      path: ../platform/docs
    - id: generated-wiki
      path: ./imports/openwiki-markdown
      material: derived
      origin: https://github.com/example/payment-service
```

Roots default to `material: primary`. Mark generated or secondary summaries as `derived`; `origin` is optional descriptive provenance supplied by you, not a fetched or verified upstream source. Imported OpenWiki Markdown uses the ordinary source pipeline. Lore preserves its origin and derived status, and context does not treat derived-only evidence as independent verification of code behavior. A native OpenWiki state adapter, automatic wiki aggregation, and tracker connectors are outside 0.3. See [multi-source provenance](docs/V03.md#multi-source-provenance) for upgrade and trust boundaries.

## Evidence and failure safety

Lore stores exact original excerpts rather than asking a model to invent a citation. A proposed quote must resolve unambiguously in the captured source bytes before it becomes evidence. SQLite keeps source observations and assertion histories append-only during normal operation, with current support maintained separately. Generated prose includes citations and explicit qualifications for historical evidence, proposals, unresolved conflicts, and reported outcomes. An optional second generative check reviews synthesis for unsupported claims; this is a fallible quality check, not a proof of truth.

Updates run against a staged database and staged output. The existing published wiki is not rewritten piecemeal while model work is still underway. A recoverable journal handles interruptions during the final filesystem/database switch, and source changes detected during compilation cause the run to stop rather than publish a misleading successful baseline. Rerunning `update` recovers a pending publication and reuses valid cached model work where possible.

Deleting a source is not the same as deleting its retained history. To intentionally erase all Lore-managed evidence, cached responses, and generated pages, use `lore purge --all --yes`. This never modifies original sources. It is a logical deletion of managed files, not a guarantee of physical secure erasure on storage devices or removal from external backups. Treat `.lore/` and the generated wiki according to your project's confidentiality requirements.

## Development and current boundaries

Run `cargo test --all-targets --locked` for the offline compiler, provider, database, source-boundary, retrieval, context-budget, and CLI tests. No real API credentials or installed model weights are needed for these tests. The implementation deliberately favors correctness and inspectable history over maximum throughput: source processing is sequential, candidate comparisons are exhaustive in bounded batches, and topic pages are regenerated as coherent units. These choices can make large first-time compilations expensive. The current automatic supersession guard recognizes explicit English replacement wording and predecessor references; less explicit or differently worded cases remain reviewable instead of being guessed.

Lore reads local Markdown, including exported issues and discussions. Direct tracker connectors, automatic verification against code or production systems, a hosted service, a graph editor, and optimized large-corpus retrieval are not part of this initial implementation. The architecture and remaining trade-offs are described in [DESIGN.md](DESIGN.md), while [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md) documents the implemented behavior, recovery, provider testing, and operational limitations. Feedback grounded in a small reproducible document corpus is particularly welcome.

Lore is inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) and research on grounded generation, provenance, and incremental knowledge maintenance. It is licensed under the [Apache License 2.0](LICENSE).
