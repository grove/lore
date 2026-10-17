# Registry-only compilation and Markdown publication migration

[Issue #70](https://github.com/grove/lore/issues/70) separates Lore's **source-backed decision registry** from optional generated Markdown pages. In the 0.10.0 source line, the standard `lore init` path creates **registry-only** new projects. Existing projects keep their previous Markdown publication behavior until an explicit mode change. This is a breaking-default migration; 0.9.0-rc.1 and earlier generated a Markdown wiki on every successful compile.

## Choose a mode

| Configuration | Compiling and updating | Publication and restoration |
| --- | --- | --- |
| `output.mode: registry_only` | Stage and validate complete retained evidence, source revision history, reviews, native imports and provenance; atomically commit the SQLite registry | **No** wiki synthesis, wiki directory, public pages or extra presentation model calls. `lore restore` is unavailable; earlier detached wiki directories are left intact |
| `output.mode: markdown` | Same retained evidence and source checks, **plus** optional topic/overview synthesis | Publish to `output.wiki_dir` using the existing owned-directory checks, journaled two-slot commit and safe restore/recovery |
| Mode **omitted in an older configuration** | Legacy compatibility: behaves as `markdown` | Continue the old project output contract, even if the generated directory has been damaged or removed; it is **not** silently inferred as headless |

New projects created using `lore init` or `lore init --configure-only` explicitly write `output.mode: registry_only` to their configuration. If you want Markdown publication from the start:

```sh
lore init --configure-only --source ./docs --publish-markdown
# Review lore.yml; mode is markdown.
lore init
```

For the standard new-project workflow:

```sh
lore init --configure-only --source ./docs
# Review the configured models, allowed roots and privacy grants first.
lore init
lore status
lore context "Improve queue diagnostics without changing accepted policy"
lore evidence EVIDENCE_ID
```

In both modes, `init` and `update` may invoke explicitly configured models for new evidence. A **genuine unchanged update is a no-op** and makes zero model calls. Registry-only compilation skips Markdown rendering entirely and retains no new `wiki_pages`; it still uses bounded source extraction and a recoverable staged registry transaction. Neither mode invokes OpenWiki or treats an imported code observation as accepted policy.

### Deliberate migration for existing projects

**From Markdown to headless**: First back up your current `lore.yml`, retained `.lore/` and any generated Markdown you want to keep; confirm the last published pages, including any edits. Change only the configuration's output section, preserving its existing paths:

```yaml
output:
  mode: registry_only
  state_dir: ./.lore
  wiki_dir: ./lore
```

Run `lore status`: it reports `mode_change_required: true` and an explanatory warning. An ordinary `lore init` or `lore update` **refuses** the mode switch. Authorize it explicitly:

```sh
lore update --migrate-output
lore status
```

The previous `lore/` directory is **not deleted, replaced or refreshed**. It remains in place as a detached, potentially stale export and is excluded from configured source intake. Retained historical evidence and any stored generated pages remain in the SQLite registry; task guidance always follows *current retained source evidence*, not detached pages. Later headless updates do not write to that directory. The `status` warning calls out detached wiki files without treating them as registry drift.

**From headless to Markdown**: Review any existing `lore/` directory and preserve manually changed or unmanaged files separately. Set `output.mode: markdown`, then explicitly run:

```sh
lore update --migrate-output --rebuild
```

`--rebuild` here is deliberately required; it authorizes replacing an existing *Lore-owned* wiki with a fresh presentation rather than recycling old pages from a detached export. A wiki owned by another project, a symlink or an unsafe path still fails closed. You cannot silently overwrite an unmanaged directory. A subsequent unchanged `lore update` is again a no-op. `lore restore --discard-edits` works only with an active Markdown publication containing stored pages; it is not a way to enable output mode or overwrite a headless project's old export. For a new project created with `--publish-markdown`, no migration flag is necessary.

Setting `output.mode` without the migration command is **not enough** to change an initialized registry. `lore update --migrate-output` also refuses if no mode transition is pending. `--rebuild` alone is invalid in registry-only mode. Do not use `lore purge --all --yes` to switch modes: purge explicitly removes retained state and owned output, including historical evidence.

### Safety and crash recovery

Both modes acquire the same project lock, stage SQLite under private `.lore/staging/`, commit only after extraction/reconciliation, a complete source digest rescan, SQLite integrity/foreign-key checks and a successful database transaction. The recovery journal now names **either** a database-only promotion **or** the original two-slot database+wiki promotion. Older journals without a mode retain legacy Markdown recovery. A failed model call, source mutation, cancellation or interrupted pre-commit stage never installs partly compiled evidence or changes an existing wiki. Pending journals block ordinary read-only commands until recovered; `lore update` resumes recovery before further work. The previous database backup stays available until post-commit validation succeeds.

`lore status` remains read-only, reporting `output_mode`, `mode_change_required`, pending recovery, sources/configuration drift and output changes for **actively published Markdown only**. Detached headless output is not compared to stored pages and is never automatically repaired. `lore audit` continues checking original evidence, current/historical provenance, imported observations and the actual registry; it does not require generated pages or authenticate runtime code. `lore read` and `lore explore` are retired in both modes; see [navigation migration](NAVIGATION_MIGRATION.md). Use `context`, `evidence`, `decisions`, `cases`, `search` and `review` directly in both modes.

Privacy/egress and source authority are unchanged: imported OpenWiki snapshots are optional implementation observations, not policy; no model or report can grant filesystem or source-write permissions. Static inspection is not execution. The independent Guardian source-integrity CI checks and source-restoration tests remain active.

See [product scope](PRODUCT_SCOPE.md), [the implementation guide](IMPLEMENTATION.md), and the [first-run task guide](tutorial/README.md).
