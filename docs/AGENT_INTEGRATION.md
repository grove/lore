# Optional agent instructions

From the project root, preview a short Lore block, then explicitly apply it:

```sh
lore integrate codex
lore integrate codex --apply
lore --json integrate codex --status
lore integrate claude-code            # preview first
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
Managed version 2 also recognizes the exact, unchanged version-1 block: preview
shows its replacement, apply upgrades it, and removal can remove either version.
The integration response remains schema 1; `managed_version` identifies the text.

Apply uses an exclusive sibling lock, a synced sibling stage, repeated original
byte checks and an atomic replacement. New files use exclusive creation via a
hard link. Existing file permissions are preserved. A crash cannot expose a
partly written instruction file; a leftover `.lore-agent-context.lock` requires
confirming the old installer stopped before removing it. Preview/status create
no lock, stage, configuration or state directory. The lock coordinates Lore
installers; arbitrary editors do not participate in this lock protocol.

The block asks the agent to consult context once per meaningful task or relevant
evidence revision, within existing inspection, egress and model-spending grants.
It covers source-only bootstrap schema 1, initialized schema 5 decisions,
reference lookups and deterministic fallbacks, and model-free schema 2 retrieval
with `--fast`. It names readiness and missing-evidence flags, preserves exceptions
and source-owned qualifications, and asks for permitted original-source checks
when evidence is insufficient. A snapshot does not verify the current checkout;
static inspection is not test execution. Unavailable or unknown-contract Lore
falls back to original documents/current code. Neither installation nor the
block authorizes initialization, source edits, access or model spending. The
fuller [schema-5 example](AGENTS.example.md) documents manual pinned integrations.

[Consumer tests](../tests/agent_contract_consumer.rs) use a deterministic policy
adapter with mocked command responses for bootstrap, full context, fallback,
changed evidence, insufficient evidence, denied access and missing Lore. They
assert the corresponding policy clauses exist in both managed agent blocks,
retain evidence qualifications, and record calls to catch duplicate requests or
unauthorized effects. CLI tests separately exercise installation and actual
bootstrap/disabled-provider responses. These tests validate the **instructions
and contract**, not real Codex/Claude compliance or effectiveness. Live agent
evaluation is deferred to [issue #2](https://github.com/grove/lore/issues/2).
