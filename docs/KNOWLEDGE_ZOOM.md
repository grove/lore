# Explore project knowledge at the useful level

`lore explore` lets a reader begin with a project concept, follow related concepts, or retrieve an exact source-bound fact. It uses the existing compiled knowledge and evidence registry. An original record remains searchable even when the navigation budget cannot give it a dedicated node.

```bash
lore explore
lore explore "payment retries"
lore explore "MAX_RETRIES"
lore explore --node RETURNED_VIEW_ID
lore --json explore "payment retries" --max-tokens 8000 --max-nodes 24
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

## Summaries preserve the conditions of the answer

Concept summaries quote original records with their source status and scope. They are extractive views, not new independently verified claims. Each summary lists the original knowledge IDs it uses and reports representative coverage.

Before selecting an answer, Lore expands a candidate into a group containing the recognized critical records in its topic or subject and both endpoints of attached documentary relationships. This includes decisions, constraints, risks, explicit exceptions and historical transitions identified in the registry. The expansion can cross topic boundaries through the recorded relationships.

The complete group is the unit of output packing. If it cannot fit, the group is omitted and the response reports that limitation. A short navigation summary is shown as factual prose only when the selected answer also carries its original support and attached critical records. Otherwise the node shows a structural description such as its record count and a way to open it.

This conservative grouping may bring several related conditions into an exact lookup. A broad topic with many constraints can require a larger output budget. The response does not claim that unselected or unsupported records are absent from the project.

## Budgets and omissions

The default output limit is 8,000 tokens; the accepted range is 512 through 100,000. Accounting covers the complete JSON and portable Markdown response, including evidence and reporting fields. Navigation is reduced before source groups when space is limited.

| Status | Meaning |
| --- | --- |
| `complete` | All eligible selected groups fit this response |
| `partial` | Some complete groups fit and others are explicitly omitted |
| `budget_limited` | The requested complete group cannot fit within the available output budget |
| `no_matches` | No eligible source-bound record matched the current request |

Read `coverage`, `critical_groups_omitted`, `navigation_truncated` and `warnings` together. Navigation can be bounded even when the factual result is complete. An empty result or an omitted group makes no recommendation about the missing material.

The default graph build allows 2,048 navigation nodes, 8,192 containment edges and 500,000 charged grouping operations. It reserves capacity for semantic groups while keeping original records independently retrievable. When grouping work is exhausted, the graph can fall back to a connected flatter index with an explicit warning.

Loading is also bounded: the default build accepts at most 10,000 current knowledge units and 32 MiB of retained excerpt/context input. Exceeding those limits produces a specific error. Existing direct `lore context` and `lore evidence` retrieval remain separate entry points. These are resource limits, not measured guarantees of latency or retrieval quality.

## Revision-aware, disposable caching

The optional cache contains derived graph data. A cache read still rebuilds and validates the graph against the current registry. Stored summary text is reusable only when it is equal to the newly derived node; keeping an old fingerprint next to changed prose is insufficient.

A node revision depends on its source records, evidence, source status, related endpoints and navigation membership. A new or changed source inventory also changes the whole graph's snapshot revision. Unaffected nodes can retain byte-identical contents, and a truly unchanged cache snapshot is not rewritten. This is verified reuse of derived content, not a claim that the implementation avoids all graph-building work or has a measured speed advantage.

Cache snapshots are published atomically, limited to 128 MiB, and read through a capped reader. An ownership marker distinguishes the feature's directory from an unmanaged directory; its purge API refuses unrelated files and symlinks. The existing full-state purge remains the way to erase all Lore-managed state.

`--no-cache` bypasses derived-view cache reads and writes. The graph and selected result still come from the compiled registry. No learner answers, user profile, background observation or new accepted project fact is stored by Knowledge Zoom.

## JSON and embedding APIs

The version-1 selected response has these principal fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | Selected-view contract version, currently `1` |
| `snapshot_revision` | Revision of this derived documentary graph and its inputs |
| `selected_node_id` / `nodes` / `edges` | Bounded navigation and containment relationships |
| `retrieval` / `status` | Direct-reference or cross-level route and result completeness |
| `knowledge` | Original selected knowledge records with their recorded qualifications |
| `evidence` | Exact retained source excerpts, context, provenance and digests |
| `relations` | Source-backed relationships with both selected endpoints |
| `source_revisions` | Captured source identity and current/historical status |
| `coverage` / `critical_groups_omitted` | Explicit record and complete-group omissions |
| `max_tokens` / `used_tokens` | Complete output accounting |
| `model_calls` | Always zero for this deterministic feature |

The graph's `snapshot_revision` identifies the derived view and its grouping settings. It is distinct from adaptive context's whole-registry `snapshot.registry_revision`; original knowledge and evidence identities remain shared.

Embedders can use `knowledge::build`, `build_cached`, `select`, `explore`, `explore_cached` and `render_markdown`. The exploration entry points combine the existing direct retrieval candidates with navigation within one read snapshot. `ZoomOptions` controls graph-building limits; `ExploreOptions` controls a request's query, selected node and output budget. The graph validator checks source references, source manifests, dependency fingerprints, critical memberships and containment independently of the portable renderer.

## What the checks establish

The [Knowledge Zoom tests](../tests/knowledge_zoom.rs) use a real SQLite registry and retained sources. They cover depth beyond five levels, multiple semantic parents, direct lookup despite a tiny navigation budget, rare exceptions, conflict endpoints, complete-group omission, source and relationship invalidation, extractive summaries, unchanged-cache behavior and poisoned-cache rejection.

The actual [CLI checks](../tests/knowledge_zoom_cli.rs) verify original evidence, complete output budgets, provider-independent operation, exact node drill-down, unchanged-cache bytes and modification time, explicit cache bypass and an unmanaged-cache fallback that preserves user files. The [retrieval comparison](../evaluation/KNOWLEDGE_ZOOM.md) records fixture measurements against the existing flat entry point.

These checks establish structural and provenance contracts for the implementation. They do not establish that these deterministic groups are always the best conceptual model, that every important condition was correctly captured upstream, or that the feature improves human learning or coding outcomes. Those outcomes belong to the separate human and agent evaluations.
