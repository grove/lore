# Lore visualizer — local project knowledge reader

Run from an initialized Lore project:

```sh
lore visualize
lore visualize --port 4400 --no-open
```

Open `http://127.0.0.1:4321` when using the default port and `--no-open`. Stop with Ctrl+C; Lore closes the local listener and exits. The server binds **IPv4 loopback only**. It serves bundled HTML, CSS and JavaScript from the Lore binary, without external CDN calls, model inference, code execution, registry migrations or source writes. A local browser may still have extensions or network access outside Lore's control. The server is for a trusted local user; **do not expose or forward the port to a remote network**.

If startup reports an edited or missing generated page, preserve any edits you need outside the wiki directory, then run `lore restore --discard-edits` followed by `lore visualize`. Restoration republishes stored pages without model calls; it does not refresh knowledge from changed sources. Unmanaged files must be moved elsewhere first. Use `lore update --rebuild` only when you intend to regenerate pages, potentially with model calls.

The three browser views are:

- **Wiki:** source-checked generated Markdown pages, represented as connected page links. The reader displays citations as numbered notes with readable source locations, suppresses raw HTML and images, and only makes recognized internal Markdown page links clickable.
- **Concepts:** reuses Lore's existing Knowledge Zoom navigation DAG, hiding low-signal term groupings without deleting underlying records. Groupings are derived, not accepted architectural decisions. Select a concept to inspect bounded underlying original records and their quoted evidence.
- **Decisions:** accepted/proposed/withdrawn decision and constraint records, grouped by topic in the sidebar. Each source-specific record remains individually selectable, with its scope, support status, evidence and documentary relationship edges. Relationship edges preserve current vs historical status. An incident report never becomes runtime verification.

The reader opens with the graph hidden. Use the graph button in the top bar to show or hide it. Search matches titles and bounded wiki body text, displays matching excerpts, and handles simple variants such as "retry" and "retries". The query stays in place when switching views. Nodes can be selected through the sidebar (accessible without a mouse), or by click in the interactive graph. Drag to pan; wheel/buttons zoom. The graph is a representation of a **bounded set**, not evidence of all possible project relationships. The snapshot identifier and warning/omission notices describe its version and limits.

**Freshness:** Press ↻ to reread the published wiki and registry. This does **not** run `lore update`. Run `lore status`, and an independently authorized `lore update` when source documents change. Pending publication, modified generated output, invalid registry state or unsafe paths stop the reader rather than claiming current or complete knowledge. Output preserves documentary/proposal/derived distinctions.

**Limits:** The viewer caps displayed wiki pages at 600, navigation concepts at 400, and decision records at 300; the canvas draws the first 200 per view. A concept's evidence pane shows up to 24 records and reports the number omitted. Knowledge Zoom's existing input/graph limits apply independently, including its record/byte budgets. There is no filesystem URL: endpoints are fixed paths, and page/record IDs must match the previously validated registry. Additional detail is available through `lore explore`, `lore evidence`, `lore decisions`, and the original wiki files.

Wiki search includes at most 64,000 text characters per page and 2,000,000 across the site. A warning reports omitted search content. This limits search coverage, not the pages available to read. Evidence IDs remain available in expandable details.

On the next page regeneration, topics link to other records from the same source documents, workflow-named pages without assigned procedure records report that coverage gap, and identical paragraphs combine their citations. These presentation changes do not merge knowledge records or refresh an existing publication when the viewer reloads.

**No automatic watcher:** The reader reloads on request, rather than polling or running a background filesystem watcher. A read-only public static export or hosted deployment is not implemented.

See [Knowledge Zoom](KNOWLEDGE_ZOOM.md) and [the 0.9 guide](V09.md) for current evidence contracts.
