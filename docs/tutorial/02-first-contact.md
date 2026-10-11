# Lesson 2 — Your first five minutes (zero setup)

**Goal:** Use Lore without initialization, understand output evidence and notice its limits.

Work from a **fresh copy** of [the practice project](../../examples/tutorial-project/README.md). Use the copy commands in the [course index](README.md). Confirm `lore --version` works.

```sh
lore onboard
lore --json onboard
lore context "Can HarborPay change production charge retries to five?"
lore --json context "What is MAX_CHARGE_ATTEMPTS?"
```

These unpinned commands read bounded local **documentation**, without creating `lore.yml`, initializing SQLite, calling an inference provider or touching original sources.

## Read the response intelligently

In JSON, look for `contract` (`lore.bootstrap_onboard` or `lore.bootstrap_context`), `schema_version: 1`, and `mode: bootstrap_source_only`. An evidence entry gives a relative path, line range, quotation and file SHA-256. `model_calls` is zero. `source_snapshot_sha256` identifies the selected documentary material, **not** a compiled registry.

Results can differ by query, size and document ranking. Confirm that returned evidence is **from the original files**, that omitted groups and limitations are explicit, and that the RFC isn't automatically promoted into accepted policy. A missing excerpt is **not** evidence the rule doesn't exist: read the sources if the output reports limitations.

## Repeat with a new question

```sh
lore context "What should we do after an ambiguous legacy refund timeout?"
```

Compare its cited runbook and incident text. A historical report isn't proof of the current adapter's implementation. Before initialization, `--inspect` does **not** turn this documentary bootstrap path into a code scanner.

**Exercise:** Locate the accepted three-attempt rule in the original Markdown using Lore's path/line citation. Explain in one sentence why `lore context --fast` cannot be used here yet: it is an explicitly selected compiled-registry contract and returns `uninitialized_project` until initialization.

**Checkpoint:** You can get useful, free first-contact navigation without mistaking it for deep model-backed investigation.

**Next:** [Compile lasting knowledge](03-initialize.md).
