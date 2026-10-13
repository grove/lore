# Lesson 7 — Everyday workflows, decision tree and troubleshooting

**Goal:** Use Lore efficiently without adding unnecessary user burden.

## Which command should I use?

| I want to… | Start with… | Important boundary |
| --- | --- | --- |
| Start before setup | `lore context "TASK"` | Bounded source-only guidance if uninitialized |
| Ask what to do for a task | `lore context "TASK"` | Check citations, readiness and omissions |
| Get deterministic context | `lore context "TASK" --fast` | Initialized projects only |
| Find a technical concept | `lore search "TOPIC"` | Initialized; source-backed discovery |
| Find why a decision exists | `lore decisions "TOPIC"` | Documentary intent, not deployed proof |
| Examine historical examples | `lore cases "TOPIC"` | A reported case isn't a passing test |
| Review source freshness | `lore status` | Doesn't rebuild the registry |
| Refresh knowledge | `lore update` | May invoke provider inference |
| See what changed | `lore changes --since NAME` | Requires a previously saved baseline |
| Get task advice after changes | `lore context "TASK"` | For comparison, use separate `baseline`/`changes`; legacy `guard` is slated for retirement |
| Integrate agent instructions | `lore integrate codex` | Preview before explicit `--apply` |
| See unresolved reviews | `lore --json review list` | An empty queue doesn't prove correctness |
| Verify retained consistency | `lore audit` | Audit isn't an end-to-end real-world benchmark |

## A safe routine

**New project:** first use `lore context TASK` on a real task. Configure and initialize only after understanding the source/permission and inference-cost consequences. Then optionally integrate the agent instructions.

**Starting a task:** ask a specific `lore context "…"`, preserve source qualifications, choose the actual implementation seam and execute tests *separately*. Avoid reissuing identical expensive context queries without reason.

**After changes:** use `lore status`. Run `lore update` if your source knowledge changed and you have permitted inference. Use a prior baseline with `changes`; prefer `context` for task-specific guidance. Treat any feedback note as a proposal awaiting review.

## Troubleshooting

- **`uninitialized_project`:** `--fast`, explicitly pinned schemas and deeper tutorial/reference modes require successful `lore init`. Use ordinary unpinned `lore context` or `lore onboard` for first contact.
- **Model unavailable:** Source-only bootstrap or deterministic compiled `--fast` can still help. Check `lore doctor` and `lore.yml` before opting into a provider; `doctor --inference` actually sends probes.
- **No relevant evidence:** Check selected source directories and omissions; no retrieved fact is not proof no constraints exist.
- **Stale guidance:** `lore status` describes source drift; context doesn't automatically rebuild. Update deliberately.
- **No review item:** `review list` is observational, not a fixed tutorial exercise; do not fabricate IDs.
- **Budget error:** Increase `--max-tokens` for mandatory evidence groups, or narrow the query. Never truncate critical exceptions manually.
- **Unknown cost:** Provider-reported token/billing data may be unavailable; `null` is not zero.
- **Unsafe installation target:** Integration rejects ambiguous project roots, symlinks and modified managed blocks; fix the path rather than forcing writes.

## Cheat sheet

```sh
lore --version
lore context "Explain the next safe change"
lore init --configure-only --source ./docs
lore doctor --inference
lore init
lore status
lore search "retry policy"
lore decisions "charge attempts"
lore cases "ambiguous refund"
lore context "Improve retries" --no-inspect
lore --json context "Improve retries"
lore integrate codex
lore integrate codex --apply
lore baseline save before-change
lore update
lore changes --since before-change
lore context "Check impact of changed retry policy"
lore --json review list
lore audit
```

For the full current command inventory and migration paths, see the [authoritative product scope](../PRODUCT_SCOPE.md). Existing `onboard`, `explore`, `read`, and `guard` commands remain available in 0.9.0-rc.1 pending their own removal issues. This is a **reference collection**, **not** a script to run in sequence: initialization and updates need configured inference, `--fast` needs compiled state, and baselines must be saved before the changes they compare.

To clean up, delete only the **disposable copied classroom directory** you created, after confirming it contains nothing you want to keep. Do not use `lore purge --all --yes` casually: it erases Lore-managed project state and evidence, though not original source documents.

**Review:** [What you should now understand](answers.md). **Next step:** apply these patterns to one real, authorized project and record where Lore actually saves you work.
