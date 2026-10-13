# Migration: retire standalone Guardian advice

[Issue #68](https://github.com/grove/lore/issues/68) removes the public `lore guard` advisory command and its dedicated `GuardianReport`/advisory aggregation layer. This is an intentional breaking change after the 0.9.0-rc.1 release candidate. Lore has **one** task-guidance path, `lore context TASK`, and retains a separate, deterministic versioned-history path, `lore baseline` / `lore changes`.

## How to replace a Guardian workflow

Old command (no longer supported):

```sh
lore guard --since joined --task "Improve retry diagnostics" --no-inspect
```

Use **two distinct** existing operations instead:

```sh
lore changes --since joined
lore --json context "Improve retry diagnostics without changing the accepted retry policy" --no-inspect
```

For scoped change discovery use `lore changes --since joined --task "Improve retry diagnostics"`. To compare against a named checkpoint, first save it with `lore baseline save joined` **before** the source changes and explicitly run `lore update` after authorized document changes. Baselines capture compiled knowledge; neither `changes` nor `context` silently runs `update`. `lore status` helps find source drift.

**Do not pass `--since` to `lore context`.** Context assesses the *current retained knowledge* relevant to the supplied task. It does not read the named baseline or automatically incorporate a before/after comparison. Read the `changes` output and its exact sources when the historical delta matters, then ask a task question explicitly. This is a change of workflow, not a 1:1 alias.

## Removed API and JSON contracts

- The `guard` subcommand and its `--since`, `--task`, `--inspect`, `--no-inspect`, `--no-cache`, `--allow-checkout-egress` and `--max-tokens` options **under that subcommand** no longer parse. The normal CLI reports exit **2** and `--json` reports `code: "invalid_arguments"` before loading the project or creating a model. Supported equivalents are options on `context`, where documented, with a separate `changes` request for checkpoint comparison.
- The old schema-1 Guardian advisory result (`assessment_status`, `advisories`, `advisories_in_shared_guidance`, `warnings` and optional nested `intelligence`) is **not** silently remapped. Parse `lore --json changes --since NAME` as the existing schema-1 `ChangeReport`; parse `lore --json context TASK` as the existing initialized schema-5 context result (or source-only bootstrap schema 1 when uninitialized).
- The former Rust `lore::companion::guard`, `GuardianReport`, `Advisory` and `render_guard` interfaces are removed. There is no Guardian-specific inference runner, renderer, advisory grouping or new cache. Applications should call the already supported `lore::context::adaptive` API or invoke `context` from the CLI.
- Empty changes, budget omissions or an empty risk list do **not** certify safety. `changes` reports document/revision membership and historical evidence, **not** current runtime behavior. `context` distinguishes checked static observations, model guidance and deterministic fallback; none are executed tests.

## What remains unchanged

`lore baseline save/remove` and `lore changes --since` remain deterministic, with named snapshots, source-owned scope/lifecycle, current and historical evidence membership, imported observations, cross-source interpretations and exact before/after revisions. The A→B→A case, withdrawn evidence, accepted-versus-proposed intent, whole-group omissions and review qualifications remain covered by regression tests. Existing named `.lore/baselines/*.json` files remain readable; the migration does not rewrite source documents, delete stored evidence or delete published wiki pages. `--json`, budgets and read-only access behave as before.

`lore context TASK` continues to provide source-backed risks, material blockers, constraints, permitted bounded investigation and verification still required. Static checkout reads require caller grants; `--no-inspect` remains an absolute veto. Hosted inference and checkout egress remain separate authorizations. Running `changes` is not a grant to run or edit code.

## Guardian *CI* is deliberately retained

The independent [Guardian source-integrity workflow](../.github/workflows/guardian-integrity.yml), [replay gate](../evaluation/guardian_integrity_gate.py), oracle, exact-head binary pinning, source-restoration detection, per-command read-only/source integrity checks, cancellation regression tests, and no-Lore controls are **not** retired with the advice product.

The gate continues to replay the historical **pinned 0.7 baseline binary** using its original `guard` contract. Replays of the **current candidate binary** use `context` with its real schema-5 result and explicit inspection veto. The integrity oracle checks both modes without claiming Guardian alert accuracy or reducing the complete source-event coverage requirements. Historical evaluation and reported alert studies remain archival and are not current product features.

See [project change history](PROJECT_COMPANION.md), [product scope](PRODUCT_SCOPE.md), and [the current task-context guide](tutorial/README.md).
