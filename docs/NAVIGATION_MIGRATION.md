# Navigation migration in Lore 0.9

`lore explore` and the wiki-specific `lore read` were retired after
0.9.0-rc.1. Use the maintained registry commands:

| Previous workflow | Supported workflow |
| --- | --- |
| `lore explore "payment retries"` | `lore context "Change payment retries"` for task guidance |
| Concept discovery | `lore search "payment retries"` for retained record IDs |
| `lore read payment-retries` | `lore decisions "payment retries"` for choices, rationale and history |
| Procedure or rejected approach browsing | `lore cases "payment retries"` |
| `lore explore --node VIEW_ID` | Search by the original subject/symbol, then follow returned evidence IDs |
| Exact original support | `lore evidence EVIDENCE_ID` |
| Offline task retrieval | `lore context "Change payment retries" --fast --max-tokens 3000` |

`context` also works before initialization with bounded local documentary
selection. `search`, `decisions`, `cases` and `evidence` use an initialized
registry, including registry-only projects. No published wiki is required.

## Breaking command and response changes

Both retired commands, including former options and `--help`, return exit code
**2** and explicit migration guidance before configuration, registry or source
access, model calls, usage logging or cache writes. They are hidden from the
supported command list. `--json` returns an error object:

```json
{"code":"command_retired","command":"explore","error":"lore explore was retired in 0.9. Use lore context TASK ..."}
```

The former exploration schemas 1/2 (including `--compact` normalized tables),
`--node` view IDs, `--max-nodes`, and navigation cache options have no successor
browse contract. The wiki reader's `page`/`content` JSON response is removed.
Consumers must handle `command_retired`, choose the appropriate retained
command, and follow its original record/evidence IDs and omission fields.
Commands are not silently aliased; retained context schemas keep their own
meaning and budgets.

The Rust exploration entry points, `ExploreOptions`, `ExploreResult`,
`CompactExploreResult`, selection/rendering exports and graph cache exports
are removed. Library consumers must migrate to `context::build_context`,
`insights::decisions`, `insights::cases`, `storage::search`, or the evidence
lookup APIs as appropriate. This is an intentional source compatibility break.
The knowledge containment graph, documentary relationships, revision-bound
original records and validation remain. Graph node identities are internal
grouping identities, not public drill-down handles.

## Existing projects and evidence

Upgrading deletes no project data: original sources, retained decisions/cases,
source revisions, primary/imported evidence and generated Markdown remain.
Open existing exported Markdown files directly when needed; it is a retained
publication, while registry commands provide current source-backed guidance.
Output mode changes still require the explicit [headless migration](HEADLESS_MIGRATION.md).

Old `.lore/knowledge-zoom` caches are no longer loaded or created. They can be
removed manually after preserving any user-added files; no cleanup runs on
upgrade. Current context caches, exact-symbol retrieval, relationship grouping,
evidence selection, token budgets, omissions and caller permissions keep their
existing implementation. The old browse selection, rendering schemas,
incremental navigation cache and its cache-only topology reuse are deleted;
historical evaluation captures remain for traceability.
