# Lore

**Understand your project. Make better changes.**

Projects collect their history in architecture notes, ADRs, plans, issue exports, investigations, and previous agent experiences. Understanding a subject often means piecing together several sources written at different times for different reasons. Lore is a Rust command-line application that reads your project's Markdown directories and optional native knowledge snapshots, maintains evidence-backed project understanding, and uses the decisions, constraints, history, and open questions to recommend an approach for your next task. The same knowledge powers a linked, topic-oriented Markdown wiki.

**Lore 0.6 provides decision-ready project intelligence.** `lore context "Implement payment retries"` leads with one preferred approach and says whether to proceed, perform a specific check first, or resolve a material policy decision. It distinguishes exact project evidence, hypotheses, and general engineering principles. Opt-in `--inspect` reads relevant local source and tests; `--investigate` performs bounded follow-up inspections and revises the recommendation when evidence contradicts the original hypothesis. `--fast` preserves deterministic schema 2, and `--schema-version 3` retains the 0.5 contract. See [the 0.6 guide](docs/V06.md) for privacy, limits, caching, and evaluation status.

### Experimental shared intelligence

`lore context TASK --schema-version 5` selects automatic investigation within
caller-owned standing grants, while existing schemas retain their behavior.
Human and agent experiences share a complete retained-registry snapshot and the
existing evidence/decision runtime. Exact supported symbolic lookups avoid
unnecessary model calls. Investigative findings are revision-bound, revalidated
and retention-limited; `lore memory` lists available leads and `lore memory
--clear` discards them. See [the shared intelligence guide](docs/SHARED_INTELLIGENCE.md)
and [implementation tracker](docs/IMPLEMENTATION_TRACKER.md) for grants, contracts
and the distinction between verified mechanics and unmeasured product outcomes.

### Understand and learn a project

`lore onboard` gives an immediate source-grounded orientation. It uses the same
evidence, decisions, static observations and permission boundary as adaptive
agent context. You can start with a workflow or go directly to a contribution:

```bash
lore onboard
lore onboard --topic "Explain payment dispatch"
lore onboard --task "Improve dispatch diagnostics without changing ordering"
lore onboard --topic "Teach me dispatch queues" --mode tutorial
lore onboard --topic "Exact dispatch limits" --mode reference
lore --json onboard --no-inspect
```

Explanation, how-to, tutorial and reference modes follow the request's intent;
an explicit `--mode` overrides it. Tutorials offer optional hints, a lesson-bound
answer comparison and a distinct practice activity. Ordinary task guidance does
not require a lesson, account or learner profile. Complete source conditions and
honest provider-unavailable fallbacks remain available. Read the
[human experience guide](docs/HUMAN_EXPERIENCE.md) and the separate
[human/agent evaluation protocol](evaluation/SHARED_INTELLIGENCE.md).

### Understand decisions and documented cases

`lore decisions QUERY` organizes retained choices, rationale, alternatives,
conditions and historical transitions. `lore cases QUERY` shows source-reported
outcomes, documented procedures, rejected approaches and relevant work history:

```bash
lore decisions "payment retry policy"
lore --json decisions "payment retry policy" --max-tokens 12000
lore cases "payment retries during failover"
```

Both commands use the existing registry without a model provider. Each passage
keeps its original source, lifecycle and scope; missing rationale remains
unknown, and a reported outcome is not a runtime verification. Complete views
are omitted with an explanation when they exceed the output budget. See the
[decision lenses and cases guide](docs/DECISION_LENSES.md).

## Vision

**Lore's long-term goal is to make everyone working in a project more capable—newcomers, experienced developers, maintainers and coding agents.**

Lore's goal is not more documentation. It is to connect evidence, code observations, decisions and historical experience; investigate what matters; and present the right understanding or practical guidance for each user's goal. **Newcomer onboarding is the flagship human learning experience. Decision-ready, automatically investigated context is the flagship coding-agent experience.** Experienced developers and maintainers get direct answers, applicable constraints and informed trade-offs without entering a tutorial.

Humans should get inviting project tours and explanations when learning, and concise decisions and project guidance when working. Coding agents should receive compact machine-readable task intelligence—one preferred approach, relevant constraints, actual investigations, inspected implementation seams and completion checks—instead of rediscovering intent or reading a wiki. **Both use the same versioned knowledge, evidence, permissions and applicability rules**, distinguishing accepted decisions, proposals, reports and observations.

The CLI, task-context interface, native snapshot adapters, and generated Markdown wiki are the foundation. A richer reading experience, conversational grounded questions and answers, and live source synchronization remain future directions. Read the [full vision](VISION.md) for the product principles, intended experience, and measures of success.

## Knowledge Experience direction

Lore is developing a shared project intelligence experience for everyone. Two
flagship journeys guide the implementation and its independent evaluations:

- **Human learning:** [Developer Onboarding design](docs/DEVELOPER_ONBOARDING_DESIGN.md) — a newcomer gets an immediate project orientation and source-grounded tour, practices a concept, makes a correct first change and tackles a distinct related task with less help.
- **Agent productivity:** [Coding-Agent Intelligence design](docs/CODING_AGENT_INTELLIGENCE_DESIGN.md) — any coding agent receives a compact, evidence-bound task package with automatically investigated context, explicit constraints, implementation guidance and honest completion criteria.

An experienced developer or maintainer can ask directly for advice or explanation. There is **no mandatory tutorial, learner profile, special agent framework or parallel knowledge registry**.

The [Knowledge Experience architecture](docs/KNOWLEDGE_EXPERIENCE_DESIGN.md) combines adaptive-depth Knowledge Zoom, Diátaxis (tutorial-first **when learning is requested**), decision conditions and cases. The [Autonomous Assistance contract](docs/AUTONOMOUS_ASSISTANCE_DESIGN.md) defines permitted investigation, and the [phased roadmap](docs/KNOWLEDGE_EXPERIENCE_ROADMAP.md) starts with **one shared engine and two small user journeys**, then measures learning and agent outcomes independently. The guides above describe implemented commands; the [implementation tracker](docs/IMPLEMENTATION_TRACKER.md) distinguishes those mechanics from planned work and outcomes still requiring evaluation. The existing evidence registry, privacy and CLI compatibility remain foundational.

## Evaluating Lore

Lore's first CLI implementation is available, but we are still validating how accurately **real inference models** understand heterogeneous project documents. The [evaluation toolkit](evaluation/README.md) includes a controlled, evolving project with reviewed source checkpoints, Lore's own documentation, and pinned public OpenWiki and LLM Wiki corpora. It can run local Ollama or explicitly authorized hosted OpenAI inference, report provenance and incremental-update checks, and produce a human review sheet. Automated fixture tests and source hashes cannot establish semantic correctness, so [the baseline](evaluation/BASELINE.md) clearly separates what is already measured from the quality and billing data we still need to collect.

## Why a project needs more than a summary

Suppose an accepted ADR selects MySQL, a later idea proposes PostgreSQL, and an issue asks someone to investigate migration. A summary that treats every sentence as a current fact might announce that the project is moving to PostgreSQL. Lore instead preserves the distinction between an accepted decision, a proposal, and a work item. Closing the issue does not establish that anything shipped. A later document explicitly replacing the ADR can change the documented decision, while a deployment report remains a report rather than independent verification of production.

Each source keeps its own assertions and provenance. Equivalent assertions may support one consolidated knowledge unit, but their original excerpts remain separate and inspectable. When a document disappears, its historical evidence is retained and no longer presented as current support. When sources disagree, Lore preserves the disagreement and records a review item. The task briefing can still propose a likely explanation and a practical provisional approach, with its supporting evidence, alternatives, and applicability. These interpretations never change an accepted decision or close a review.

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
lore context "Implement payment retries" --fast
lore context "Implement payment retries" --no-cache
lore context "Implement payment retries" --inspect
lore context "Implement payment retries" --investigate --max-tokens 8000
lore context "Implement payment retries" --no-inspect
lore --json context "Implement payment retries" --schema-version 3
```

`context` combines relevant knowledge from the last compiled registry with the configured generative model. The schema 4 briefing includes readiness, the preferred approach, rationale, trade-off, implementation seams, constraints, prioritized checks, and completion criteria. Facts are copied from retained records. Hypotheses retain their supporting evidence, alternatives, and applicability. General engineering principles are tagged separately. A second support check reviews factual support, readiness, and treatment of counterevidence. Context preserves the registry, original sources, and generated wiki.

Checkout inspection is opt-in while its real-world benefit is being evaluated. `--inspect` selects a few relevant source and test files; `--investigate` can inspect a discriminating follow-up and revise the approach. Observations include exact paths, line ranges, excerpts, and complete-file hashes. Static inspection never proves runtime behavior or runs a test. Hosted documentary permission does not permit checkout egress: code and discovered filenames require separate `privacy.allow_checkout_egress: true` or `--allow-checkout-egress`. `--no-inspect` overrides configured inspection.

The default output budget is 3,000 tokens. Lore measures the complete compact JSON and human-readable output with the embedded `cl100k_base` tokenizer and bounds both forms, including citations, metadata, and warnings. The model's evidence input is separately limited by `processing.max_context_bytes`. Whole optional brief items can be omitted with an explicit count; required risk and constraint qualifications stay with the recommendation. When a valid briefing cannot fit, inference fails, or the configured provider is unavailable, `mode: fast_fallback` identifies a deterministic result without fresh reasoning. Use `mode`, `model_calls`, and `cache_status` to distinguish those outcomes.

`lore context --fast` emits the unchanged schema 2 retrieval contract with zero model calls and no writes, credentials, model server, or embedding service required. It groups related documentary/native records with their evidence and preserves conflict endpoints. A tight budget can omit an entire connected group, with explicit omission counts. In fast mode, paths are relevance hints and captured inspection leads; no checkout inspection occurs. An empty result means no eligible knowledge was retrieved, not that no constraints apply.

Unchanged queries reuse guidance bound to the task, selected evidence revisions, endpoint/model identity, reasoning settings, privacy, prompt version, and budgets. Schema 4 also binds the checkout index and rehashes every inspected file, including deeper investigation files. Changed evidence is re-evaluated on the next query. `--no-cache` bypasses both guidance and embedding caches for one call; `context.cache: false` disables their persistence in configuration. Refresh explicitly after changing the weights behind an unchanged model alias.

### Optional semantic retrieval

Intelligent guidance works with the existing lexical and relationship retrieval immediately. To also match concepts across different wording, configure a separate embedding model you have installed or explicitly opted into:

```yaml
models:
  generative:
    provider: ollama
    model: gemma4:12b
  embedding:
    provider: ollama
    model: embeddinggemma
```

Install the selected embedding model in Ollama, then run `lore doctor --inference` to check it. [The complete local example](examples/lore.intelligent.yml) includes the default privacy and reasoning settings. Hosted embeddings require `privacy.local_only: false`, an explicit embedding provider/model, and that provider's credentials; enabling a chat model never implicitly enables embeddings.

Lore stores embeddings for original knowledge units and imported observations in a separate SQLite cache, combines semantic and lexical ranks, and retrieves their recorded relationship groups. No vector extension, Node.js process, or separate search service is required. The first semantic query builds the index; later queries reuse unchanged record and query vectors. Schema 4 limits each query to two embedding calls while reserving synthesis and verification capacity; larger indexes can populate incrementally across queries. Schema 3 retains its previous indexing behavior. Indexing, record text, and candidate counts are bounded and reported when truncated. If embeddings are unavailable, guidance can still use lexical retrieval.

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

`status`, `update --dry-run`, `context --fast`, search, reading, and the ordinary audit do not call a model. `audit --deep` performs a broader, model-assisted refresh and reconciliation; it can be expensive. `update --refresh` bypasses semantic caches and re-extracts all sources, which is useful after changing a model behind an unchanged model alias. `update --rebuild` explicitly allows replacement of Lore-owned generated output, including manual edits, while retaining the knowledge history. By default, Lore refuses to overwrite edited pages or unmanaged files.

The CLI also provides `lore evidence <evidence-id>` for an exact archived passage and `lore review` for unresolved questions. `lore review show <review-id>` displays the history and evidence binding; `resolve`, `dismiss` and `reopen` accept an explicit `--reason` and optional `--actor`. These actions record a disposition, not a new project fact, and make no model calls. Specific relationship questions can also close automatically when active evidence from the same source revision establishes the named replacement; withdrawn evidence reopens them. `--config path/to/lore.yml` selects another configuration, and the global `--json` option produces machine-readable results, including argument errors. Operational errors exit with code 1, argument errors with code 2, and audit findings with code 3.

## An overview you can follow back to evidence

The generated index now explains the project's major systems, documented design, decisions, future initiatives and open questions in cited paragraphs. It uses a bounded, representative selection from every topic; the detailed pages preserve the full extracted knowledge. An extra generative check reviews the overview for unsupported claims when synthesis verification is enabled. Decision links name documents rather than repeating topic titles, so two decisions on the same page appear as “ADR-027 explicitly supersedes ADR-001.” The review-history page distinguishes pending questions from retained resolutions and makes the reason for each transition inspectable.

Documentary status and factual verification stay separate. An architecture specification can describe the selected design without proving what runs in production, and that lack of verification does not make it a future plan. Proposals remain future intent, reported deployments remain reports, and mandatory release rules are not confused with implementation completion. These distinctions are represented in the extraction schema and generation prompts; their accuracy should still be checked on real project documents.

## Reasoning effort by task

When using an OpenAI Responses-compatible generative model, Lore explicitly requests a **task-appropriate reasoning effort**: `low` for evidence extraction, `high` for semantic reconciliation and verification, and `medium` for topic writing, overview writing, and task-context synthesis. Context verification defaults to `high`. These are configurable starting points, not benchmark-proven optimal levels. Set `models.reasoning` in `lore.yml` to override one or all tasks; `enabled: false` leaves the provider's reasoning level unspecified. The setting does not affect Ollama, Clef-Flash, TypeSafe Jev, or OpenAI Decisions. See [Reasoning configuration](docs/REASONING.md) for all supported levels, exact keys, and benchmark override flags.

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

Roots default to `material: primary`. Mark generated or secondary summaries as `derived`; `origin` is optional descriptive provenance supplied by you, not a fetched or verified upstream source. Imported OpenWiki Markdown uses the ordinary source pipeline. Lore preserves its origin and derived status, and context does not treat derived-only evidence as independent verification of code behavior. For native OpenWiki Claims, use the 0.4 import configuration below. Keep the Markdown-only path when the upstream format is unsupported. Each input must have one owner: do not configure the same OpenWiki directory as both a Markdown root and a native import.

## Connect OpenWiki, Engram, and Beads

Set `schema_version: 2` and add only the imports you use:

```yaml
schema_version: 2
project:
  name: payments
sources:
  roots:
    - id: decisions
      path: ./docs/decisions
      material: primary
imports:
  - id: implementation
    kind: openwiki
    path: ./openwiki
  - id: agent-memory
    kind: engram
    path: ./imports/engram.json
    project: payments
  - id: work-history
    kind: beads
    path: ./imports/beads.jsonl
    include_memories: false
```

Prepare local snapshots with your upstream tools, then update Lore:

```bash
engram export ./imports/engram.json --project payments
bd export -o ./imports/beads.jsonl
lore update
lore context "Implement payment retries"
```

OpenWiki supplies its existing `openwiki/` directory. Lore never starts these tools, changes their records, or contacts them while importing. Ordinary documents remain sufficient; imports are optional, and an imports-only project can set `sources: { roots: [] }`.

An accepted ADR can say three retries while OpenWiki reports five at a captured evidence revision. Lore can retain a **potential discrepancy**, attach both evidence records, connect relevant work history, and recommend checking the current implementation and policy. A completed Beads task remains work history; an Engram memory remains a reported recollection. Neither becomes an accepted architectural decision or independently verified implementation. Opaque OpenWiki revision tokens are retained without assuming that they identify a Git commit or the current checkout.

Native evidence uses stable `ne_…` IDs resolved by `lore --json evidence ID`; the returned record includes its original parsed payload, content hash, source references, verification metadata, and current-import status. Human readers can start from the generated `imports.md` page. [The 0.4 guide](docs/V04.md) explains version support, authority, history, and failure behavior. [Cross-source evaluation](evaluation/CROSS_SOURCE.md) defines the four-system comparison and release gates.

## Evidence and failure safety

Lore stores exact original excerpts rather than asking a model to invent a citation. A proposed quote must resolve unambiguously in the captured source bytes before it becomes evidence. SQLite keeps source observations and assertion histories append-only during normal operation, with current support maintained separately. Generated prose includes citations and explicit qualifications for historical evidence, proposals, unresolved conflicts, and reported outcomes. An optional second generative check reviews synthesis for unsupported claims; this is a fallible quality check, not a proof of truth.

Updates run against a staged database and staged output. The existing published wiki is not rewritten piecemeal while model work is still underway. A recoverable journal handles interruptions during the final filesystem/database switch, and source changes detected during compilation cause the run to stop rather than publish a misleading successful baseline. Rerunning `update` recovers a pending publication and reuses valid cached model work where possible.

Deleting a source is not the same as deleting its retained history. To intentionally erase all Lore-managed evidence, cached responses, and generated pages, use `lore purge --all --yes`. This never modifies original sources. It is a logical deletion of managed files, not a guarantee of physical secure erasure on storage devices or removal from external backups. Treat `.lore/` and the generated wiki according to your project's confidentiality requirements.

## Development and current boundaries

Run `cargo test --all-targets --locked` for the offline compiler, provider, database, source-boundary, retrieval, context-budget, and CLI tests. No real API credentials or installed model weights are needed for these tests. The implementation deliberately favors correctness and inspectable history over maximum throughput: source processing is sequential, candidate comparisons are exhaustive in bounded batches, and topic pages are regenerated as coherent units. These choices can make large first-time compilations expensive. The current automatic supersession guard recognizes explicit English replacement wording and predecessor references; less explicit or differently worded cases remain reviewable instead of being guessed.

Lore reads local Markdown and optional native snapshots from OpenWiki, Engram, and Beads. Direct tracker connectors, bidirectional writes, runtime/deployment verification, a hosted service, and a graph editor remain outside 0.6. Optional static checkout inspection is available with explicit limits and egress safeguards. Cross-source comparisons and hybrid search use bounded candidate indexes; large-corpus recall, recommendation quality, and actual coding-task improvements still require independent evaluation. [The 0.6 coding-task evaluator](evaluation/DECISION_INTELLIGENCE.md) compares original sources, fast context, 0.5, and 0.6 using executable changes and independent review; fixture tests establish contracts, not measured model-quality gains. The architecture and remaining trade-offs are described in [DESIGN.md](DESIGN.md), while [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md) documents the implemented behavior, recovery, provider testing, and operational limitations. Feedback grounded in a small reproducible document corpus is particularly welcome.

Lore is inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) and research on grounded generation, provenance, and incremental knowledge maintenance. It is licensed under the [Apache License 2.0](LICENSE).
