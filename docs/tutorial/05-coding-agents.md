# Lesson 5 — Give a coding agent project context

**Goal:** Safely integrate Lore with Codex or Claude Code and understand what integration actually does.

Work from the **root of your disposable classroom project**. You do not need `lore init` to preview/install the instruction block.

```sh
lore --json integrate codex
lore integrate codex --apply
lore --json integrate codex --status
```

This adds an exact Lore-owned block to `AGENTS.md`; for Claude Code use `lore integrate claude-code --apply`, which targets `CLAUDE.md`. The first command previews a change and does not write. `--apply` is an **explicit write to the instruction file**; it never initializes the Lore knowledge registry or starts a model.

The managed instructions tell an agent to consult `lore context` once per meaningful code/architecture task, handle source-only vs initialized output, respect permissions and verify changed code. These are **instructions**, not executable hooks or a guarantee a real agent followed them.

## Worked example

Ask your authorized coding agent:

> Read Lore's guidance first. Add diagnostic handling for ambiguous charge responses without raising the three-attempt limit or changing the logical charge's idempotency key. Inspect the original ADR and run the existing tests after any approved edit.

Before accepting its work, inspect its source changes and **real test results**. Lore only recommends checks; the agent owns changes and execution within its own grants. With no compiled registry, Lore may return source-only guidance, so the agent must independently inspect code.

Try `lore integrate codex --remove --apply` only when you want to remove the unmodified Lore-managed block. This preserves unrelated instructions. For a safe preview of removal, omit `--apply`.

**Exercise:** What happens if Lore is unavailable? The installed instructions tell the agent to work from original sources and disclose the limitation, not fabricate context or run `lore init` automatically.

[Detailed integration contract](../AGENT_INTEGRATION.md).

**Next:** [Keeping knowledge current](06-keep-knowledge-current.md).
