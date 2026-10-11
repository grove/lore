# Learn Lore — a hands-on guide for new users

**Start here if you want to learn how to use Lore itself.** Lore's `onboard --mode tutorial` instead teaches *a project's concepts*. This course teaches the Lore tool.

**Audience:** Newcomers with a terminal and basic Git familiarity. **Time:** about 60–90 minutes, optionally spread across sessions. Lessons 1–2 and the sample Python tests require **no model, account, API key or network service** after Lore is installed. Lessons involving compiled intelligence require a configured inference provider and can consume time/tokens.

## Seven lessons

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
lore --json onboard
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

**Need a shortcut?** If you're using Lore only for code changes, complete lessons 1, 2, 3 and 5. For maintainers, focus on 1–4 and 6–7. The [command cheat sheet](07-workflows.md#cheat-sheet) is at the end.
