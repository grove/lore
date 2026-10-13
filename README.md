# Lore

**Know the decisions before changing the code.**

Lore is a local-first Rust CLI for **developers and coding agents preparing a code or architecture change**. Ask about one task and get a source-backed recommendation: applicable decisions and constraints, why they exist, rejected alternatives and reported failure cases, possible discrepancies with implementation observations, and the next defensible action. Lore advises; **you or your coding agent edit code, execute tests, and verify results**.

### One task, before your first edit

From the project's root:

```sh
lore context "Add payment retry diagnostics without changing approved retry limits"
lore --json context "Add payment retry diagnostics without changing approved retry limits"
```

Without initialization, ordinary unpinned `context` returns **bounded, source-only navigation** with exact original citations, no model calls and no automatic writes. It does *not* inspect code or establish complete decision coverage. With a compiled registry, default context uses schema 5: review the preferred action, applicable scope and exceptions, readiness/blockers, exact evidence, alternatives, observed implementation seams and remaining checks. An observation is not proof of runtime behavior, and a proposal or reported outcome is not accepted policy. Use `lore evidence ID`, `lore decisions "retry policy"` and `lore cases "duplicate charge"` to inspect retained evidence **after initialization**.

### Make this a coding-agent habit (optional)

```sh
lore integrate codex               # preview AGENTS.md instructions; no writes
lore integrate codex --apply       # explicitly install the managed block
# Or: lore integrate claude-code --apply  (CLAUDE.md)
```

The installed instructions ask the agent to consult `lore context TASK` before meaningful edits. They do not run an agent, initialize Lore, grant permission to inspect or edit code, or guarantee compliance. See the [agent integration contract](docs/AGENT_INTEGRATION.md).

### Build lasting evidence only when you need it

```sh
lore init --configure-only --source ./docs
# Review lore.yml, provider costs and privacy grants first.
lore init
lore context "Change retry handling" --inspect
lore status
```

`init`/`update` can invoke the configured model. `--inspect` explicitly permits bounded read-only checkout inspection; it never executes tests. Existing compiled projects support model-free `lore context "TASK" --fast`. **OpenWiki is optional**: its local exports can provide implementation observations, but it is not a mandatory service, accepted-decision authority, or a hidden dependency. Markdown alone is enough.

**Product contract:** [What Lore owns, the retained command map, and migration policy](docs/PRODUCT_SCOPE.md). The focus is task guidance, not a tutorial platform, web browser, code wiki or project monitoring product. Current non-core commands still exist in 0.9.0-rc.1; [#66](https://github.com/grove/lore/issues/66) through [#73](https://github.com/grove/lore/issues/73) track staged removals and migrations. Nothing in this scope decision changes runtime contracts.

**Release reality:** 0.9.0-rc.1 is an engineering release candidate, **not** stable 0.9.0. Source binding, privacy and CLI behavior have deterministic/fixture checks; real-model coding improvements, human learning outcomes and Guardian alert usefulness are **not measured**. See the [0.9 guide](docs/V09.md), [engineering scorecard](evaluation/PRODUCT_QUALITY_09.md), and deferred [independent decision-task evaluation](https://github.com/grove/lore/issues/2).

**Learn the CLI:** [first-run task guide](docs/tutorial/README.md). **Need operational details?** Start below for installation, evidence provenance, provider configuration, existing wiki publication and current compatibility. Legacy reader, learning, Guardian advice and feedback commands mentioned below are present behavior, *not* recommended new product entry points; planned removals are mapped in [product scope](docs/PRODUCT_SCOPE.md).

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

For an uninitialized project, start with local documentary assistance:

```bash
cd /path/to/your-project
lore context "Find the first change for this task"
lore onboard
```

When ready for durable compiled intelligence, create a configuration explicitly
without running inference yet. You can name several source directories,
including local Markdown exported from an issue tracker. Configuration paths
are resolved relative to `lore.yml`, not the later command's working directory.

```bash
cd /path/to/your-project
lore init --configure-only --source ./docs --source ./plans --source ./issue-export
```

Edit `lore.yml` to select your source directories and models. The default is local-only Ollama with `gemma4:12b`; any suitable installed model with reliable structured JSON output can be selected instead. Start with a small representative set of documents, because first-time extraction and cross-document reconciliation can involve many model calls.

`project.name` is an editable display name. Changing it preserves an initialized project's stored identity and knowledge, including projects initialized by older Lore versions. The currently supported visualizer uses the new name immediately; generated wiki text and the registry's display name change on the next authorized `lore update`. Keep the state and wiki directories together: ownership checks still reject a wiki belonging to a different project. New projects with copied configuration files receive distinct identities when their resolved state directories differ.

```bash
ollama pull gemma4:12b
lore doctor --inference
lore init
```

During `init` and `update`, Lore shows a spinner and source-section progress bar
in interactive terminals. When stderr is redirected, it writes progress messages
there instead; `--json` keeps progress output disabled so stdout remains valid
JSON.

**In 0.9.0-rc.1**, the result is ordinary Markdown under `lore/`, with an index and topic pages. Registry-only initialization is planned in [#70](https://github.com/grove/lore/issues/70), not available yet. Lore keeps its database, exact evidence snapshots, and reusable inference results under `.lore/`. Add `.lore/` to your project's `.gitignore`; it contains potentially sensitive copied project material. Review generated pages before publishing them, just as you would review other documentation.

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
lore --json context "Implement payment retries" --schema-version 4
lore --json context "Implement payment retries" --schema-version 5
```

Initialized `context` combines relevant retained knowledge with the configured model when permitted. Its default schema-5 envelope includes a complete registry snapshot identity, actual capabilities, source relationship groups, and a decision or honest deterministic fallback under `intelligence`. Markdown leads with the preferred approach, next action and critical constraints. The decision retains readiness, rationale, trade-off, implementation seams, prioritized checks and future completion criteria. Facts are copied from retained records; hypotheses keep their support, alternatives and applicability, and general engineering principles remain separate. Exact supported symbolic queries can use a zero-call reference result. Context preserves the registry, original sources and generated wiki.

Checkout inspection requires a caller grant: `LORE_INSPECTION_ROOT` permits bounded automatic reads within that root, or `--inspect`/`--investigate` grants the configuration directory for one invocation. `--no-inspect` always denies it. Repository settings cannot widen the grant. Observations include exact paths, line ranges, excerpts and full-file hashes; static inspection does not execute tests or prove runtime behavior. Default schema 5 requires a caller hosted grant and `privacy.local_only: false` for hosted documentary inference. The host can set `LORE_ALLOW_HOSTED_EGRESS=1`; checkout content and discovered filenames additionally need their separate host/config grants. The explicit per-invocation `--allow-checkout-egress` flag grants both within the local-only ceiling. See [the exact grant rules](docs/SHARED_INTELLIGENCE.md#standing-grants).

The default output budget is 3,000 tokens. Lore measures the complete compact JSON and Markdown with the embedded `cl100k_base` tokenizer, including the schema-5 envelope, source relationship manifest, citations, metadata and warnings. The model's evidence input is separately limited by `processing.max_context_bytes`. Whole optional items can be omitted with a count; required risk and constraint qualifications stay together. If inference fails or the provider is unavailable, `intelligence.mode: fast_fallback` identifies a deterministic result without fresh reasoning. If the minimum complete response cannot fit, Lore returns a typed budget error. Read the nested mode, model-call count and cache status before treating a result as fresh reasoning.

`lore context --fast` emits the unchanged schema 2 retrieval contract with zero model calls and no writes, credentials, model server, or embedding service required. It groups related documentary/native records with their evidence and preserves conflict endpoints. A tight budget can omit an entire connected group, with explicit omission counts. In fast mode, paths are relevance hints and captured inspection leads; no checkout inspection occurs. An empty result means no eligible knowledge was retrieved, not that no constraints apply.

Schema 5 also reports invocation usage. Logical model calls, physical HTTP
attempts, provider tokens and billed USD remain distinct; missing counts or
bills are null. Retries, refusals, timeouts and rejected answers keep their
events. Accounting uses memory by default, with a private sidecar only when
the caller explicitly selects a fresh `LORE_USAGE_LEDGER` path. Successful
compilation stores non-sensitive accounting through SQLite schema 8 without
backfilling historical costs. See [provider usage](docs/PROVIDER_USAGE.md) and
the [0.8 migration notes](docs/V08.md#provider-usage-and-sqlite-schema-8).

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

## Maintain compiled evidence (currently also publishes a wiki)

After editing your sources, run `lore update`. Lore compares file and section fingerprints, re-extracts changed sections, finds related knowledge, and regenerates affected topic pages. A genuinely unchanged run makes no model calls and leaves wiki content byte-for-byte unchanged. It can therefore complete without API credentials or a running inference server when the configuration and successful baseline are unchanged. New documents are compared with existing knowledge even when that knowledge's original source files were not edited.

```bash
lore status
lore update --dry-run
lore update
lore search "database migration"
lore read database-strategy
lore audit
```

`status`, `update --dry-run`, `context --fast`, search, reading, and the ordinary audit do not call a model. `audit --deep` performs a broader, model-assisted refresh and reconciliation; it can be expensive. `update --refresh` bypasses semantic caches and re-extracts all sources, which is useful after changing a model behind an unchanged model alias. `update --rebuild` regenerates pages and explicitly allows replacement of Lore-owned generated output, including manual edits, while retaining the knowledge history; it can make model calls. By default, Lore refuses to overwrite edited pages or unmanaged files.

To recover edited or missing generated pages without inference, preserve any edits you need outside the wiki directory, then run `lore restore --discard-edits`. This restores the last stored publication without processing sources, migrating the registry, or refreshing knowledge. It discards generated-page edits and refuses unmanaged files or unsafe paths. Run `lore status` afterwards to check whether source or configuration changes still need an incremental `lore update`.

The CLI also provides `lore evidence <evidence-id>` for an exact archived passage and `lore review` for unresolved questions. `lore review show <review-id>` displays the history and evidence binding; `resolve`, `dismiss` and `reopen` accept an explicit `--reason` and optional `--actor`. These actions record a disposition, not a new project fact, and make no model calls. Specific relationship questions can also close automatically when active evidence from the same source revision establishes the named replacement; withdrawn evidence reopens them. `--config path/to/lore.yml` selects another configuration, and the global `--json` option produces machine-readable results, including argument errors. Operational errors exit with code 1, argument errors with code 2, and audit findings with code 3.

## Compare historical changes (legacy Guardian advice)

Use retained baselines and change reports for deterministic history. The separate `guard` advisory command is legacy and slated for [#68](https://github.com/grove/lore/issues/68); prefer task advice from `context`. Save a named comparison point, update the ordinary registry as project sources
evolve, then ask what changed or how those changes affect a task:

```bash
lore baseline save joined
lore update
lore changes --since joined
lore guard --since joined --task "Improve payment dispatch diagnostics"
lore baseline remove joined
```

Change reports preserve the original before/after evidence, conditions,
documentary relationships and imported source status. The advisory guardian
uses the shared investigation engine and your existing host grants before
offering source-linked guidance. Baselines are explicit local checkpoints;
`--replace` deliberately refreshes one. Read the
[project companion guide](docs/PROJECT_COMPANION.md) for snapshot scope, current
relationship qualifications, complete output limits and partial-result status.

New checkpoints retain current evidence membership separately from immutable
history, so a condition that changes A→B→A stays visible even when the short
summary is unchanged. Older checkpoint checksums remain readable. Automatic
advisory scope gives accepted constraints priority, and overlapping risk/blocker
summaries retain all original citations and distinct actions. The guardian is an
on-demand advisory command; an unavailable model or an empty alert list does not
establish that the project is safe.

## Existing optional published-wiki view (planned headless default)

The generated index now explains the project's major systems, documented design, decisions, future initiatives and open questions in cited paragraphs. It uses a bounded, representative selection from every topic; the detailed pages preserve the full extracted knowledge. An extra generative check reviews the overview for unsupported claims when synthesis verification is enabled. Decision links name documents rather than repeating topic titles, so two decisions on the same page appear as “ADR-027 explicitly replaces ADR-001.” The review-history page distinguishes pending questions from retained resolutions and makes the reason for each transition inspectable.

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

## Optional native imports: OpenWiki, Beads and legacy Engram

Set `schema_version: 2` and add only the imports you use. OpenWiki is an optional implementation-observation producer; the existing native Engram adapter is slated for [#73](https://github.com/grove/lore/issues/73), with historical evidence migration required before retirement:

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

Lore reads local Markdown and optional native snapshots from OpenWiki, Engram,
and Beads. Direct tracker connectors, bidirectional writes, runtime/deployment
verification, a hosted service, and a graph editor remain outside 0.7. Static
checkout inspection retains explicit limits and caller-owned egress safeguards;
Lore does not autonomously execute repository commands. Cross-source comparisons
and hybrid search use bounded candidate indexes. Large-corpus recall, real-model
recommendation quality, coding productivity, and human learning still require
independent evaluation. The [historical 0.6 evaluator](evaluation/DECISION_INTELLIGENCE.md)
and [0.7 six-arm protocol](evaluation/ADAPTIVE_TASKS.md) preserve comparable
contracts and separate fixture results from actual product outcomes. See
[DESIGN.md](DESIGN.md) for the architecture, [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md)
for operational behavior, and [the current scorecard](evaluation/PRODUCT_QUALITY_07.md)
for measured results and limitations.

Lore's primary job and future command ownership are defined in [the product contract](docs/PRODUCT_SCOPE.md). Lore is inspired by [OpenWiki](https://github.com/langchain-ai/openwiki) and research on grounded generation, provenance, and incremental knowledge maintenance. It is licensed under the [Apache License 2.0](LICENSE).
