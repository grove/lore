# Lesson 4 — Navigate, question and verify

**Goal:** Choose the right Lore command for the job.

**Requires completed `lore init`** in the classroom for commands below. The initial source-only path does not provide retained decision IDs or wiki pages.

```sh
lore explore "charge attempts"
lore decisions "charge submission"
lore cases "duplicate refund"
lore --json context "Improve retry safety without changing accepted policy"
lore --json context "Change charge attempt logic" --fast
```

- **`explore`** is a zoomable map of concepts and citations, not new authoritative policy.
- **`decisions`** gathers retained choices, rationale, scope and exceptions.
- **`cases`** shows documented examples and reported outcomes, not tests Lore executed.
- **`context`** gives one preferred next action and constraints from evidence, with qualifications for assumptions and omitted material.
- **`--fast`** uses deterministic, model-free compiled retrieval (schema 2). Initialized default context uses adaptive schema 5, possibly with model inference as permitted.

Try `lore --json explore "charge attempts" --compact` to compare the optional compact schema. Read returned IDs rather than guessing them, then resolve a returned `ev_` or `ne_` evidence ID with `lore --json evidence ID`. Native imported `ne_` evidence is optional; the classroom supplies ordinary Markdown.

## What should be trusted?

Review `intelligence.mode` (for schema 5), snapshot, capability grants, citations and omission/warning fields. A `fast_fallback` has no fresh model assessment. A result that fits a tight budget can still omit optional groups. A recommendation is advice, not an automatically accepted decision.

If you explicitly permit read-only checkout inspection, you can compare source code with documentation:

```sh
lore context "Does code preserve the charge idempotency key?" --inspect
```

This can inspect eligible checkout files within the granted boundary; it does **not** execute code. If you want to check behavior, run the [sample tests](../../examples/tutorial-project/README.md) yourself.

**Exercise:** Find ADR-001's limit and RFC-002's proposed change in source documents. Explain why neither the generated wiki nor an inspection guarantees that production matches them.

**Next:** [Coding agents](05-coding-agents.md).
