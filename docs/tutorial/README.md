# Start with one task — Lore first-run guide

**For developers and coding agents about to change code:** use `lore context TASK` first. Lore returns source-backed decisions, constraints, relevant history and a next defensible action; your agent or developer still owns edits, tests and delivery. See the [authoritative product scope](../PRODUCT_SCOPE.md).

**No setup needed:** from your project's root run `lore context "Change retry handling without violating accepted policy"`. This uninitialized response is bounded, source-only guidance with original quoted evidence—not a complete compiled decision assessment, verified implementation, or a mandatory wiki. To install optional agent instructions, preview `lore integrate codex` or `lore integrate claude-code`, then explicitly `--apply`. To compile durable knowledge, configure `lore init` deliberately with a reviewed model and permission budget. OpenWiki is optional.

**Detailed learning material below is a *tool-usage course*, not the primary product workflow.** Lore 0.9.0-rc.1 still exposes interactive lessons, a browser view and Guardian advice; they are scheduled for separate changes ([#66](https://github.com/grove/lore/issues/66), [#67](https://github.com/grove/lore/issues/67), [#68](https://github.com/grove/lore/issues/68)). This guide is not announcing command removals.

**Audience:** New CLI users with terminal and Git familiarity. The optional seven lessons take roughly 60–90 minutes. First contact needs no model, account, key or network service after installation; compiled guidance may invoke explicitly configured inference.

## Optional in-depth lessons

1. [What Lore is](01-meet-lore.md) — its job, source authority and limits.
2. [Your first five minutes](02-first-contact.md) — run zero-setup Lore against a complete fictional project.
3. [Compile lasting project knowledge](03-initialize.md) — choose source roots, inference provider, cost boundary, initialization and updating.
4. [Find answers and evidence](04-explore-and-decide.md) — context, search, explore, decisions, cases and citations.
5. [Work with Codex or Claude Code](05-coding-agents.md) — safe opt-in instructions and task walkthrough.
6. [Keep understanding current](06-keep-knowledge-current.md) — snapshots, Guardian, reviews, adjudication and feedback.
7. [Everyday workflows and troubleshooting](07-workflows.md) — repeatable workflows, practical decision trees and safe cleanup.

## Set up the classroom

From your original Lore clone, build/install once:

```sh
cargo install --path . --locked
lore --version
```

Then **copy** the standalone practice project to a disposable directory, rather than experimenting inside Lore's own source checkout. POSIX example:

```sh
cp -R examples/tutorial-project /tmp/lore-classroom
cd /tmp/lore-classroom
lore context "Can we raise the production charge limit to five?"
```

Windows PowerShell equivalent (from the Lore clone):

```powershell
Copy-Item -Recurse examples/tutorial-project "$env:TEMP\lore-classroom"
Set-Location "$env:TEMP\lore-classroom"
lore --json onboard
lore context "Can we raise the production charge limit to five?"
```

If the destination exists, choose a new empty one; do **not** blindly overwrite previous work. The example project contains only Markdown, Python and standard-library tests. Before `lore init`, results are **ephemeral documentary navigation**, not compiled intelligence, verification of code or production facts.

**Answer key:** [Expected concepts and limitations](answers.md). Avoid reading it until you've tried the exercises. Exact response ordering, IDs, wording and model output vary. Use stated *invariants*, not copied screenshots, as expected results.

**Documentation scope:** This course reflects Lore **0.9.0-rc.1**. The release is an engineering candidate; genuine coding-agent productivity and human-onboarding outcomes have not yet been measured. [0.9 release status](../../evaluation/PRODUCT_QUALITY_09.md).

**Need a shortcut?** If you're using Lore only for code changes, complete lessons 1, 2, 3 and 5. For maintainers, focus on the task workflow, decisions and evidence; consult lessons 1–4 and 6–7 for detail. The [command cheat sheet](07-workflows.md#cheat-sheet) is at the end.
