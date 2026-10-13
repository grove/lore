# Getting started: one decision-aware task

Lore helps developers and coding agents understand **the decisions that matter before changing code**. Start with a task, not an interactive lesson, browser view or generated wiki. The [product scope](../PRODUCT_SCOPE.md) explains the boundary; [learning-engine migration](../LEARNING_MIGRATION.md) explains removed flags and JSON responses.

## Install

Build from the Lore repository using a stable Rust toolchain:

```sh
cargo install --path . --locked
lore --version
```

## First contact (no configuration, model or write)

Run inside a project with Markdown documentation:

```sh
cd /path/to/your-project
lore context "Add retry diagnostics without changing accepted policy"
lore --json context "Add retry diagnostics without changing accepted policy"
lore onboard                           # optional short project orientation
```

Ordinary unpinned `context` and `onboard` before initialization use the **same** bounded `lore.bootstrap_context` schema-1 documentary source selection. The output retains quoted source excerpts, paths, line spans, file SHA-256 and explicit omissions, with zero model calls and automatic writes. It does not inspect the checkout, prove implementation, certify policy authority or execute tests. First-contact discovery honors local exclusions, file/read limits and configured Markdown roots.

Read the original cited passage. Check whether it is accepted, proposed, rejected or a report. A missing passage can mean it was excluded or omitted, not that a constraint is absent.

## Optional practice project

For a disposable, offline example, **copy** the [HarborPay fixture](../../examples/tutorial-project/README.md), not the original Lore checkout:

```sh
cp -R examples/tutorial-project /tmp/lore-practice
cd /tmp/lore-practice
lore context "Can production charge retries increase to five?"
```

PowerShell from the Lore checkout:

```powershell
Copy-Item -Recurse examples/tutorial-project "$env:TEMP\lore-practice"
Set-Location "$env:TEMP\lore-practice"
lore context "Can production charge retries increase to five?"
```

Use a new destination if the copy already exists. In the fixture, an accepted three-attempt rule and a proposed five-attempt RFC have **different decision status**. An incident report is not a runtime test. Nothing here requires an answer key, lesson assessor, learning profile or model download.

## Compile durable knowledge (deliberate opt-in)

```sh
lore init --configure-only --source ./docs
# Inspect lore.yml, the selected model and privacy limits.
lore doctor --inference    # optional: makes synthetic provider calls
lore init                  # may perform many inference calls
lore status
lore context "Add retry diagnostics without violating approval"
lore decisions "retry policy"
lore cases "duplicate charge"
```

`init` builds the retained evidence registry and, **in 0.9.0-rc.1**, also publishes generated Markdown pages. Headless compilation is separately planned under [#70](https://github.com/grove/lore/issues/70). Compiled `context` uses schema-5 intelligence by default or a labeled deterministic fallback; `context "TASK" --fast` is explicitly model-free schema-2 retrieval **only after** initialization. Exact retained quotations are available with `lore evidence ID`. OpenWiki snapshots are optional; source Markdown is enough to get started.

A call to `lore update` may invoke configured models; `lore status` is read-only. Keep `.lore/` private and excluded from version control, because it may contain source evidence and model responses.

## Give a coding agent the same context

```sh
lore integrate codex                 # preview instructions, no writes
lore integrate codex --apply         # explicit install into AGENTS.md
# Or: lore integrate claude-code --apply
```

Integration gives no additional filesystem, network, model-cost or code-edit authority. The agent still owns changes and actual tests. [Agent integration details](../AGENT_INTEGRATION.md).

## Inspection, evidence and limits

`lore context "TASK" --inspect` grants read-only bounded checkout inspection for the invocation on an initialized project; `--no-inspect` prohibits it even when a host grant exists. **Static inspection is not executed verification.** Hosted models and checkout egress require separate explicit grants; project text never grants them. Use `--json` for exact schema and provenance, read omissions, and check material discrepancies yourself or with your coding agent.

If a command fails: `uninitialized_project` means a pinned/fast mode requires `lore init`; an absent model may produce a labeled deterministic fallback on initialized context; a budget error needs a larger `--max-tokens` or narrower task; and pending publication requires deliberate recovery. [Operations and privacy](../IMPLEMENTATION.md), [command map](../PRODUCT_SCOPE.md).

**Not available anymore:** `onboard --mode`, hints, exercises, answer assessment, lesson revisions, solution reveal and transfer activities. These arguments now fail explicitly. The previous `lore.human_experience` and `lore.bootstrap_onboard` JSON contracts were replaced by the existing context contracts; consult the [migration guide](../LEARNING_MIGRATION.md). Archived project-onboarding designs and empirical protocols are historical records, not feature documentation.
