# Migration: retiring `lore visualize`

[Issue #67](https://github.com/grove/lore/issues/67) removes the **embedded browser visualizer** from Lore's focused development line. This is an intentional public-command removal after the 0.9.0-rc.1 release candidate, not an alias or a silent no-op. The visualizer's local HTTP server, routes, embedded HTML/CSS/JavaScript, graph viewer, and visualizer-specific tests are deleted. **No replacement web UI or code analyzer is introduced.**

## Replace browser actions with the existing CLI

| Old browser task | Supported CLI workflow |
| --- | --- |
| Start or reopen the Lore viewer | `lore context "TASK"` for direct source-backed guidance, or `lore --json context "TASK"` for agent use |
| Find a concept or document | `lore search "QUERY"`; use the returned record/evidence IDs, not a generated browser route |
| Inspect a documented design choice | `lore decisions "QUERY"` to see scope, lifecycle, rationale, rejected alternatives and historical transitions |
| Inspect reported examples or negative cases | `lore cases "QUERY"` (a reported outcome does not independently verify code) |
| Inspect original citations | `lore evidence EVIDENCE_ID` (including native `ne_...` IDs) for retained original evidence and provenance |
| Review unresolved contradictions | `lore review list`, `lore review show REVIEW_ID` and supported adjudication/status commands |
| Check current retained knowledge and safety | `lore status`, `lore changes --since BASELINE` and `lore audit` |
| Browse the *optional* code wiki visually | Use **OpenWiki** separately, if desired; explicitly import any local OpenWiki snapshot as optional implementation evidence, never as accepted architectural policy or a mandatory Lore dependency |

`lore context "TASK"` before initialization still returns bounded, **source-only** documentary guidance with original quotations, no model calls, and no automatic writes. Commands querying retained evidence or project state require an initialized project. `lore read` and `lore explore` are also retired under [#71](https://github.com/grove/lore/issues/71); use the retained commands above and see [navigation migration](NAVIGATION_MIGRATION.md).

## Removed interface and compatibility

- `lore visualize`, `lore visualize --port PORT` and `lore visualize --no-open` are **unsupported**. With `--json`, the CLI returns `code: "invalid_arguments"`; otherwise it prints the normal unknown-command error. Both use exit code **2**, before reading project configuration, starting a server, calling a model, opening a browser, or writing files.
- The former `lore::visualize` Rust library module (`Viewer` and its HTTP routes `/api/site`, `/api/page`, `/api/concept`, and `/api/record`) is **removed**, not versioned or automatically replaced by a new API. Consumers must use the established CLI/JSON contracts and original citation IDs.
- Browser-only HTML/CSS/JS and their in-binary `include_str!` references are removed. There is no listener, asset packaging, browser launch, or alternative browser-facing route left in the Lore binary.
- **Nothing is deleted from an initialized user's project.** The evidence registry `.lore/`, original sources, source hashes, permission grants, relationship graph and generated Markdown `lore/` are unchanged. New projects now default to registry-only compilation; existing Markdown projects require an explicit [output migration](HEADLESS_MIGRATION.md) to change mode. Do not purge project state to migrate from the viewer.

## Ownership and security boundaries

Lore owns source-backed task recommendations, versioned documentary evidence, conflicts, review, and bounded permitted inspection; it does **not** execute repository code or certify deployed behavior. Keep document and imported observation authority distinct. An OpenWiki code-wiki view is optional and external to Lore: Lore does not start OpenWiki, fetch it automatically, require its presence, or treat a reported implementation observation as approval.

Existing read-only commands, citation and relationship preservation, source-integrity/Guardian CI checks, and explicit inspection/hosted-egress grants are retained. Deleting the browser server does **not** relax them. Run cross-platform CLI and regression tests after upgrading.

For the recommended one-task workflow and the full supported command map, see [getting started](tutorial/README.md) and [product scope](PRODUCT_SCOPE.md). Older release and design records describe the former visualizer only as history.
