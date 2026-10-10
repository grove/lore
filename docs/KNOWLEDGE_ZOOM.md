# Explore project knowledge at the useful level

`lore explore` lets a reader begin with a project concept, follow related concepts, or retrieve an exact source-bound fact. It uses the existing compiled knowledge and evidence registry. An original record remains searchable even when the navigation budget cannot give it a dedicated node.

```bash
lore explore
lore explore "payment retries"
lore explore "MAX_RETRIES"
lore explore --node RETURNED_VIEW_ID
lore --json explore "payment retries" --max-tokens 8000 --max-nodes 24
lore --json explore "payment retries" --compact --max-tokens 1500
lore explore "payment retries" --compact --max-tokens 1500
lore explore "payment retries" --no-cache
```

The query examples are illustrative; use concepts and identifiers recorded in your project. Replace `RETURNED_VIEW_ID` with a `kv_…` identifier from a response. Run the ordinary `lore init` or `lore update` workflow before exploring a project.

Knowledge Zoom performs no model calls, network requests, live checkout inspection or code execution. Its current graph organizes retained **documentary knowledge**. Native snapshot reports remain available through shared context and `lore evidence`; this graph does not reinterpret those reports as documentary policy.

## Concepts can overlap and have different depths

The navigation structure is a directed acyclic graph. A concept can have several parents: retry behavior can belong to both payment processing and reliability, for example. Concepts become more specific through their original-record membership. The implementation does not impose a fixed number of levels or require a reader to traverse every level.

Grouping is deterministic. It uses recorded topics, subjects, shared meaningful terms and documented relationships, and forms useful intersections of overlapping memberships within a work budget. It does not infer a runtime call graph from directory layout or ask a model to invent a hierarchy. Groups with identical membership share one identity.

Each response includes the selected node, nearby parents and descendants when they fit. `--max-nodes` limits the displayed navigation; it does not restrict the independent original-record search to those displayed nodes. The default is 24 displayed nodes, with an accepted range of 1 through 256.

The implementation keeps three relationships distinct:

| Structure | What a relationship means | Authority |
| --- | --- | --- |
| Navigation containment | One view covers a more specific set of original records | A derived way to find and explain knowledge |
| Documentary evidence relationships | A source supports a contradiction, supersession, reaffirmation or other recorded relation | The original statement and its exact source evidence, with lifecycle and scope |
| Learning prerequisites | A suggested order for a particular learning activity | Optional educational guidance, maintained by the human experience separately |

Containment is cycle-checked and must stay connected to its root. Documentary relationship cycles can be meaningful and are retained separately. Neither a concept's depth nor its number of parents makes its contents more authoritative.

## Exact questions go directly to original records

A precise identifier, source path, value, knowledge ID, revision ID or evidence ID can match the retained original record directly. The record does not need to survive a parent summary before it becomes eligible. The result reports `retrieval: "direct_reference"` for that route; other requests report `cross_level`.

`--node` narrows the initial search to that view's members. Relevant critical conditions and relationship endpoints can still accompany a selected record from outside the node. This preserves context that would otherwise be lost at a presentation boundary.

For a single immutable excerpt, use the evidence endpoint:

```bash
lore evidence RETURNED_EVIDENCE_ID
```

Every selected record retains its knowledge and revision IDs, statement, kind, lifecycle, scope, support state and source evidence. The result includes exact evidence snapshots, surrounding context, excerpt digests and source revisions. The captured root and path belong to the referenced source revision, including when source-root provenance later changes.

“Current” refers to the captured registry's source status. A documented design, a proposal, a report and a historically superseded decision retain their own meanings. None of these fields proves that a deployment currently behaves as described.

## Complete evidence comes before navigation

Concept summaries quote original records with their source status and scope. They are extractive views, not new independently verified claims. Each summary lists the original knowledge IDs it uses and reports representative coverage.

Before selecting an answer, Lore builds an inspectable evidence bundle for each eligible original record. A bundle contains that record and its recognized mandatory conditions, exceptions and documentary relationship dependencies. A shared topic or an incidental word such as “file” or “input” establishes relevance without making every rule in the topic inseparable.

Condition applicability uses the recorded subject, explicit symbols or original-record references, and qualifications that name the requested multiword subject. Typed constraints and risks, and explicit language such as “must,” “unless,” “except,” “only if” and “cannot,” identify potential obligations. Numeric values alone do not establish a dependency. An explanatory consequence in a purpose statement remains optional unless it qualifies the requested object; explicit requirements and typed constraints retain their force even when followed by an explanation.

Both endpoints of a recorded documentary relationship and its exact evidence witness stay together, including contradictions and historical transitions across topic or node boundaries. Documentary relationships and explicit record qualifications close transitively. Lexically attached conditions do not recursively pull in further records merely through incidental shared words. A proposal can require the accepted decision it qualifies, while retaining its proposed lifecycle. A proposed change is not promoted into an authoritative exception to accepted guidance.

The selector recognizes production, staging, development and sandbox mentions when checking whether an environment-specific condition applies to a scoped query. This is deterministic matching over captured text, not a general policy reasoner. Original scope, lifecycle, effective time and historical source qualifications remain intact. These rules can preserve only conditions present in the captured registry and recognized by the selector.

The complete bundle is the unit of output packing. Lore ranks requested originals, measures their full records, all original support, source revisions and relationship witnesses together with final omission metadata, and packs complete bundles first. Overlapping bundles share their already selected originals. Optional navigation and summaries are added only after this evidence has been budgeted. If a complete bundle cannot fit, it is omitted and the response reports that limitation. A short navigation summary is shown as factual prose only when the selected answer also carries its original support and attached critical records. Otherwise the node shows a structural description such as its record count and a way to open it.

The response does not claim that unselected or unsupported records are absent from the project. Even one original record can exceed the available budget; related context being optional never permits clipping a required exception, source excerpt or historical qualification.

## Budgets and omissions

The default output limit is 8,000 tokens; the accepted range is 512 through 100,000. Accounting uses the embedded `cl100k_base` tokenizer without a model or network request. It covers the complete JSON and portable Markdown response, including evidence, final omission metadata and the `used_tokens` field itself.

The default schema-1 format preserves its existing conservative contract: `used_tokens` covers the larger of pretty JSON with a trailing newline and portable Markdown. Explicit `--compact` uses the larger of its actual compact schema-2 JSON with a trailing newline and the same complete Markdown rendering. Serializing an expanded schema-2 result or adding JSON indentation outside the CLI can make a larger document; that transformed output is not the compact wire budget.

| Status | Meaning |
| --- | --- |
| `complete` | All eligible requested originals and their required bundles fit this response |
| `partial` | Some complete bundles fit and other eligible coverage is explicitly omitted |
| `budget_limited` | No complete eligible bundle was selected within the output or bounded selection-work limits |
| `no_matches` | No eligible source-bound record matched the current request |

Read `coverage`, `critical_groups_omitted`, `navigation_truncated` and `warnings` together. Navigation can be bounded even when the factual result is complete. Exhausting selection work reports unknown remaining critical coverage and never reports `complete`. An empty result or an omitted bundle makes no recommendation about the missing material. If even the request and minimum reporting envelope cannot fit, the request returns an explicit budget error.

The default graph build allows 2,048 navigation nodes, 8,192 containment edges and 500,000 charged grouping operations. It reserves capacity for semantic groups while keeping original records independently retrievable. When grouping work is exhausted, the graph can fall back to a connected flatter index with an explicit warning.

Loading is also bounded: the default build accepts at most 10,000 current knowledge units and 32 MiB of retained excerpt/context input. Exceeding those limits produces a specific error. Existing direct `lore context` and `lore evidence` retrieval remain separate entry points. These are resource limits, not measured guarantees of latency or retrieval quality.

## Revision-aware, disposable caching

The optional cache contains derived graph data and a disposable dependency index. Every read first reloads the current authoritative records, exact evidence and source revision metadata within one SQLite read snapshot. Resource preflights, original-excerpt digest checks and the graph's source snapshot identity are recomputed. A cache cannot conceal corrupt authoritative evidence or replace that evidence with its own copy.

After that validation, Lore can reuse a stable navigation layout. Its grouping signature includes the grouping implementation version, all resource options, eligible record identities, topic and subject labels, normalized concept inputs, critical-record priority, and documentary relationship identities, kinds and endpoints. A label change that leaves the same members in a group still changes the signature, because labels determine a node's title and grouping explanation.

For a matching signature, the reader rederives the complete canonical navigation plan under the same work and edge limits. This validation includes source groups, overlap intersections, all selected nearest-parent edges, the critical-priority leaf set, and the exact construction warnings and truncation status. It compares that plan with the cached layout before reusing any view values. Membership, labels, source references, critical scope and adjacency must also agree. Inputs or options that change, an incompatible cache format, or invalid cached content require a full rebuild. If validation already derived a fresh plan, the rebuild uses that plan without charging or performing the grouping work a second time.

Each node's dependency revision binds its original records, exact evidence, source status and provenance, incident documentary relationships, and the original records at both relationship endpoints. Dependency indexing hashes each original source object once and reuses those digests across overlapping nodes; separate snapshot and integrity checks still process the source input. A changed opposite endpoint can therefore refresh a node even when its own knowledge revision did not change.

Only nodes whose complete dependencies changed regenerate their summary strings and revision values on the stable-layout path. Reused summaries still undergo exact validation against the deterministic original-source plan, including the same token selection, complete critical statements and coverage. That validation streams the expected source fragments and checks their content; it does not accept a stored fingerprint as proof of the prose. A cache checksum catches corruption early, but it is not authentication. Recomputing the checksum after changing a summary, title, critical scope, report, derived intersection or parent edge does not make those changes valid.

An unchanged read regenerates no view summaries or stored dependency revision values. It still rederives the canonical topology plan, constructs source descriptions and summary-selection plans, checks their token budgets, validates compact dependency hashes, and checks all retained source input and reused node content. Reused view values are copied into the current graph; this is not a zero-allocation path. A newly captured source with no assigned eligible knowledge can change the whole graph's snapshot revision without changing any node. Unaffected nodes retain identical contents, and a truly unchanged cache publication is not rewritten.

The `build_cached` API exposes this distinction through `CacheUpdate.refresh`:

| Field | Work recorded for this invocation |
| --- | --- |
| `topology_reused` | Whether the published graph used a validated existing layout |
| `layout_generation_work` | Charged canonical topology planning for a full build when no validation plan is available |
| `topology_validation_work` | Charged canonical topology planning to validate a candidate cache, including intersections and parent selection |
| `summaries_generated` / `dependency_revisions_generated` | Node text and revision derivations actually performed |
| `summaries_validated` / `dependency_revisions_validated` | Checks performed on node content and dependency fingerprints |
| `records_revalidated` / `evidence_revalidated` | Authoritative originals loaded and checked for this publication |
| `fallback_reason` | Why a full rebuild was required, when applicable |

Work counters include a rejected reuse attempt before a full rebuild. A successful stable-layout refresh has zero `layout_generation_work` and nonzero `topology_validation_work` when the input requires grouping work; their sum, not either field alone, is the charged topology-planning work. A fallback can also reuse the validation plan, so it can have zero generation-planning work while regenerating every node. The separate `reused_nodes` and `regenerated_nodes` fields count nodes in the final publication. The graph's existing `report.work_used` is the canonical layout's construction work, freshly derived on every invocation; it is not a measure of total cache-read cost.

This is incremental **derived-view reconstruction**. It does not make the whole request proportional to the number of changed records: source loading, snapshot hashing, canonical topology planning, graph checks and reused-summary plan validation still inspect current inputs. A change affecting a broad concept or many relationship endpoints can legitimately refresh many nodes. The implementation does not claim a measured latency improvement or skip critical conditions to improve cache or benchmark results.

Cache snapshots are published atomically, limited to 128 MiB, and read through a capped reader. An ownership marker distinguishes the feature's directory from an unmanaged directory; its purge API refuses unrelated files and symlinks. The existing full-state purge remains the way to erase all Lore-managed state.

`--no-cache` bypasses derived-view cache reads and writes. The graph and selected result still come from the compiled registry. No learner answers, user profile, background observation or new accepted project fact is stored by Knowledge Zoom.

## Versioned JSON and embedding APIs

Without `--compact`, the selected response remains schema 1 with its existing field shapes. Its principal fields are:

| Field | Meaning |
| --- | --- |
| `schema_version` | Selected-view contract version: `1` by default, `2` only with explicit compact selection |
| `snapshot_revision` | Revision of this derived documentary graph and its inputs |
| `selected_node_id` / `nodes` / `edges` | Bounded navigation and containment relationships |
| `retrieval` / `status` | Direct-reference or cross-level route and result completeness |
| `knowledge` | Original selected knowledge records with their recorded qualifications |
| `evidence` | Exact retained source excerpts, context, provenance and digests |
| `relations` | Source-backed relationships with both selected endpoints |
| `source_revisions` | Captured source identity and current/historical status |
| `coverage` / `critical_groups_omitted` | Explicit record and complete-bundle omissions |
| `max_tokens` / `used_tokens` | Complete output accounting |
| `model_calls` | Always zero for this deterministic feature |

The graph's `snapshot_revision` identifies the derived view and its grouping settings. It is distinct from adaptive context's whole-registry `snapshot.registry_revision`; original knowledge and evidence identities remain shared.

### Explicit compact schema 2

Use `--compact` when repeated JSON field names and duplicated citation metadata prevent a complete bundle from fitting. Schema 2 stores original text once in `strings` and uses versioned tuple rows for knowledge, evidence, source revisions and relationships. It retains every original field, including inactive support, historical source status, full excerpts and surrounding context. It has no database, cache or external reference-service dependency for resolution.

All indexes are zero-based. Unless a column below names another table or a literal type, its value is an index into `strings`. Nullable indexes and line numbers use JSON `null` for an absent value.

| Table | Ordered row columns |
| --- | --- |
| `knowledge` | `id`, `revision_id`, `statement`, `topic`, `topic_title`, `subject`, `kind`, `lifecycle`, `base_lifecycle`, `scope`, `effective_at`, `support_state`, evidence links, original relation strings |
| `evidence` | `id`, source-revision row index, optional `root_path`, `material`, optional `origin`, `excerpt`, `context_before`, `context_after`, `digest`, optional integer `line_start`, optional integer `line_end`, `captured_at` |
| `source_revisions` | `id`, `source_id`, `root_id`, `observed_path`, `content_digest`, boolean `current` |
| `relations` | `id`, from-knowledge row index, to-knowledge row index, `kind`, evidence row index, `assertion_revision_id`, `source_locator`, boolean `active` |

Each knowledge evidence link is `[evidence_row_index, assertion_id_string_index, active_boolean]`. Its final original-relation column is an array of string indexes; these preserve the record's original relation text separately from the source-backed `relations` table. Evidence source identity, path and revision resolve through the referenced source-revision row. A relationship's original source identity, revision and exact excerpt resolve through its witness evidence. Encoding checks that these references reproduce the original metadata exactly before using the compact format.

The remaining response fields, including navigation, status, coverage, warnings and budget accounting, keep their ordinary named-object shapes. The graph and disposable cache remain at schema 1 because their storage format has not changed. Both response formats use the same graph snapshot and view identities; a returned node ID can be opened with either format.

`CompactExploreResult::resolve()` reconstructs an `ExploreResult` with every original knowledge, citation, evidence, source-revision and relationship field. The resolved value retains `schema_version: 2`: its `used_tokens` measures the compact response and complete Markdown, not a fresh expanded JSON serialization. The resolver rejects unsupported versions, dangling table indexes, duplicate identities, invalid excerpt digests, inconsistent coverage and incomplete or extra source/evidence manifests. These checks establish structural consistency; validating the authenticity of supplied response bytes still requires comparison with authoritative source records.

### Rust entry points

Embedders can use the following APIs under `lore::knowledge`:

| Purpose | APIs |
| --- | --- |
| Build and validate documentary navigation | `build`, `build_cached`, `validate` |
| Existing schema-1 selection and exploration | `select`, `explore`, `explore_cached`, `render_markdown` |
| Explicit schema-2 selection and exploration | `select_compact`, `explore_compact`, `explore_compact_cached` |
| Resolve and render compact responses | `CompactExploreResult::resolve`, `render_compact_markdown` |
| Inspect original-record obligations before packing | `inspect_bundles`, `EvidenceBundle`, `BundleDependency` |
| Compare graph ordering under fixed input candidates | `select_controlled`, `select_compact_controlled`, `SelectionMode` |

The exploration entry points combine the existing direct retrieval candidates with navigation within one read snapshot. `ZoomOptions` controls graph-building limits; the unchanged `ExploreOptions` controls a request's query, selected node and output budget. Both compact resolution and compact Markdown rendering return `Result` because malformed references are rejected. The graph validator checks source references, source manifests, dependency fingerprints, critical memberships and containment independently of the portable renderer.

`inspect_bundles(graph, seeds)` returns each seed's complete knowledge-ID set and directed dependencies with a reason and, for documentary relationships, its original relation and evidence IDs. This diagnostic uses an empty query, so its inspection covers all recognized environment scopes rather than a particular query's scope filter. It is bounded by the graph's record and work limits and creates no new authoritative assertion.

The controlled selectors take the same externally supplied original candidate IDs in `GraphGuided` and `DirectOnly` modes. They use identical applicability rules, complete evidence closure, formats and budget accounting; only graph-derived ordering and navigation change. Both modes still validate the same source graph. This is an ordering/navigation ablation, not a separate graph-free ingestion implementation. Use an independently specified answer set to score either mode; the selector's own bundles are not evaluation gold.

## What the checks establish

The [Knowledge Zoom tests](../tests/knowledge_zoom.rs) use a real SQLite registry and retained sources. They cover depth beyond five levels, multiple semantic parents, direct lookup despite a tiny navigation budget, rare exceptions, conflict endpoints, complete-group omission, source and relationship invalidation, extractive summaries, unchanged-cache behavior and poisoned-cache rejection. Incremental regressions assert zero summary/revision generation while counting canonical topology validation on no-op reads, affected-node updates through both relationship endpoints, complete fallback for changed labels/critical priority/options, source-inventory-only changes, and honest accounting for rejected partial reuse. They compare updated cached graphs with a fresh full build and verify that even a recomputed cache checksum cannot authorize forged source-derived prose.

The 0.7 regressions add dense shared-topic constraints, unrelated rules sharing ordinary words and numeric values, real permissions with explanatory clauses, required exceptions outside a selected node, proposal/current-decision qualification, historical evidence and reversible schema-2 references. They check the documented capacity/rare-exception, future-scope and disagreement failure patterns at 1,500 tokens. Both formats are checked across constrained budgets, including explicit disclosure when a complete original cannot fit. Controlled selection tests hold input candidates and required obligations fixed across modes.

The actual [CLI checks](../tests/knowledge_zoom_cli.rs) verify original evidence, complete output budgets, provider-independent operation, exact node drill-down, unchanged-cache bytes and modification time, explicit cache bypass and an unmanaged-cache fallback that preserves user files. They also check the default schema-1 contract, opt-in schema-2 round trips, cache/no-cache equality, exact evidence lookup and compiled-registry byte immutability. The [retrieval comparison](../evaluation/KNOWLEDGE_ZOOM.md) records fixture measurements against the existing flat entry point and states their evaluation scope.

These checks establish structural and provenance contracts for the implementation. They do not establish that these deterministic groups are always the best conceptual model, that every important condition was correctly captured upstream, or that the feature improves human learning or coding outcomes. Those outcomes belong to the separate human and agent evaluations.
