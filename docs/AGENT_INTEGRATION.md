# Optional agent instructions

From the project root, preview a short Lore block, then explicitly apply it:

```sh
lore integrate codex
lore integrate codex --apply
lore --json integrate codex --status
lore integrate claude-code --apply
lore integrate claude-code --remove --apply
```

Codex uses root `AGENTS.md`; Claude Code uses root `CLAUDE.md`. These are ordinary
instructions, not hooks, executable agents or a background service. Integration
works without a configuration, registry or model. It never initializes Lore.
Run from the repository/configuration root; `--config` is rejected for this
command to avoid silently selecting another instruction file.

Preview returns `lore.agent_integration`, schema 1, with agent, target, action,
changed, source_write, warnings, the exact managed block and `before_digest`.
`changed` reports the proposed byte change; `source_write` is true only after an
explicit apply actually changes the file. To bind a delayed application to that
exact preview, pass `--expect-digest DIGEST --apply`. Without it, apply computes a
new preview from current bytes. `--status` never writes.

The block's markers, version and content digest identify Lore-owned text.
Reapplication is byte-idempotent. Removal deletes only that exact block, including
its own surrounding newline; every pre-existing byte remains intact. A missing
file can be created, while an emptied file is retained after removal. Modified,
duplicated or malformed markers require manual repair and are never overwritten.
Targets must be regular, nonsymlink UTF-8 files of at most 1 MiB.

Apply uses an exclusive sibling lock, a synced sibling stage, repeated original
byte checks and an atomic replacement. New files use exclusive creation via a
hard link. Existing file permissions are preserved. A crash cannot expose a
partly written instruction file; a leftover `.lore-agent-context.lock` requires
confirming the old installer stopped before removing it. Preview/status create
no lock, stage, configuration or state directory. The lock coordinates Lore
installers; arbitrary editors do not participate in this lock protocol.

The block asks the agent to consult unpinned context once per meaningful task
or relevant source change. It handles bootstrap schema 1 and initialized schema
5, preserves exceptions and evidence qualifications, and treats stale/missing
or denied context as incomplete. Static inspection is not test execution.
Unavailable Lore falls back to original sources. It grants no inspection,
hosted/checkout egress, initialization, source edits or model spending authority.
Existing pinned integrations can retain the fuller [schema-5 example](AGENTS.example.md).

CLI tests use a deterministic command-capable adapter and disabled/fake providers.
They are engineering checks, not a real Codex or Claude effectiveness experiment.
