# Lore

**Give your project a memory that grows with it.**

Projects collect their history in architecture notes, ADRs, plans, issue exports, investigations, and half-finished ideas. Those documents are valuable, but understanding a subject often means piecing together several files written at different times for different reasons. Lore is a Rust command-line application that reads your project's Markdown directories and turns them into a linked, evidence-backed wiki. It organizes knowledge by topic rather than producing another stack of file-by-file summaries.

Lore now has a working initial implementation: Markdown ingestion, live model clients, source-evidence storage, conservative reconciliation, incremental updates, and recoverable wiki publication. The test suite exercises the full compiler with deterministic models and the real CLI with local HTTP fixtures. This is early software, not a claim that every model has been evaluated for factual quality. Live hosted-account eligibility and local model behavior should be checked with `lore doctor --inference` before processing your project.

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

`status`, `update --dry-run`, search, reading, and the ordinary audit do not call a model. `audit --deep` performs a broader, model-assisted refresh and reconciliation; it can be expensive. `update --refresh` bypasses semantic caches and re-extracts all sources, which is useful after changing a model behind an unchanged model alias. `update --rebuild` explicitly allows replacement of Lore-owned generated output, including manual edits, while retaining the knowledge history. By default, Lore refuses to overwrite edited pages or unmanaged files.

The CLI also provides `lore evidence <evidence-id>` to inspect an exact archived passage and its original location, and `lore review` to list unresolved reconciliation records. Review records are retained for inspection; this release does not provide a UI for editing the knowledge graph or automatically resolving every ambiguity. `--config path/to/lore.yml` selects another configuration, and `--json` produces machine-readable results. Operational errors exit with code 1, argument errors with code 2, and audit findings with code 3.

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

## Evidence and failure safety

Lore stores exact original excerpts rather than asking a model to invent a citation. A proposed quote must resolve unambiguously in the captured source bytes before it becomes evidence. SQLite keeps source observations and assertion histories append-only during normal operation, with current support maintained separately. Generated prose includes citations and explicit qualifications for historical evidence, proposals, unresolved conflicts, and reported outcomes. An optional second generative check reviews synthesis for unsupported claims; this is a fallible quality check, not a proof of truth.

Updates run against a staged database and staged output. The existing published wiki is not rewritten piecemeal while model work is still underway. A recoverable journal handles interruptions during the final filesystem/database switch, and source changes detected during compilation cause the run to stop rather than publish a misleading successful baseline. Rerunning `update` recovers a pending publication and reuses valid cached model work where possible.

Deleting a source is not the same as deleting its retained history. To intentionally erase all Lore-managed evidence, cached responses, and generated pages, use `lore purge --all --yes`. This never modifies original sources. It is a logical deletion of managed files, not a guarantee of physical secure erasure on storage devices or removal from external backups. Treat `.lore/` and the generated wiki according to your project's confidentiality requirements.

## Development and current boundaries

Run `cargo test --all-targets --locked` for the offline compiler, provider, database, source-boundary, and CLI tests. No real API credentials or installed model weights are needed for these tests. The implementation deliberately favors correctness and inspectable history over maximum throughput: source processing is sequential, candidate comparisons are exhaustive in bounded batches, and topic pages are regenerated as coherent units. These choices can make large first-time compilations expensive. The current automatic supersession guard recognizes explicit English replacement wording and predecessor references; less explicit or differently worded cases remain reviewable instead of being guessed.

Lore reads local Markdown, including exported issues and discussions. Direct tracker connectors, automatic verification against code or production systems, a hosted service, a graph editor, and optimized large-corpus retrieval are not part of this initial implementation. The architecture and remaining trade-offs are described in [DESIGN.md](DESIGN.md), while [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md) documents the implemented behavior, recovery, provider testing, and operational limitations. Feedback grounded in a small reproducible document corpus is particularly welcome.

Lore is inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) and research on grounded generation, provenance, and incremental knowledge maintenance. It is licensed under the [Apache License 2.0](LICENSE).
