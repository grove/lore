# Migration: retired interactive onboarding

Issue [#66](https://github.com/grove/lore/issues/66) removes the separate learning engine in the focused post-0.9.0-rc.1 development line. **This is an intentional breaking change**, not a silent compatibility alias. Refer to the release's published notes before upgrading a pinned consumer; 0.9.0-rc.1 remains the last documented release candidate with the old `lore.human_experience` and interactive lesson features.

## What still works

- `lore context "TASK"` is unchanged: model-free, source-only first contact when uninitialized; the schema-5 context contract when compiled. Explicit legacy context schemas and `--fast` keep their current behavior under separate [#72](https://github.com/grove/lore/issues/72).
- `lore onboard`, `lore onboard GOAL`, `lore onboard --topic TOPIC` and `lore onboard --task TASK` remain convenient ways to request **source-grounded orientation**; they now use the same context assembler, privacy ceiling and evidence/inspection pipeline as `context`. `--path`, `--max-tokens`, `--no-cache`, `--inspect`, `--no-inspect` and `--allow-checkout-egress` remain. No learning graph, lesson cache, assessor, synthesized exercises or tutorial model calls remain.
- `lore integrate codex` / `claude-code` remain optional agent entry points. Source files and retained versioned evidence, reviews and authority are untouched.

## Removed commands/options

The following former `lore onboard` flags are **errors**, not ignored or mapped to a different format:

| Old argument | Replacement |
| --- | --- |
| `--mode tutorial`, `--mode explanation`, `--mode how-to`, `--mode reference` (and `auto`) | Use `lore context "YOUR QUESTION"` for a task, or `lore onboard --topic "TOPIC"` for orientation. For exact source support, use `lore evidence ID`, `lore decisions QUERY` and `lore cases QUERY` after initialization |
| `--hint N`, `--show-solution`, `--activity primary\|transfer` | Interactive activities and hint levels no longer exist. The [static getting-started guide](tutorial/README.md) shows real work without exercise generation |
| `--answer TEXT`, `--lesson blake3:DIGEST` | No answer assessment, revision-pinned lesson or feedback model. Keep answers and any learning records in your own files; Lore does not store or automatically migrate a learner profile |

Clap exits with argument error **2**; `--json` returns `code: "invalid_arguments"`. `lore onboard --help` explains the migration. There is no silent fallback to a different lesson or curriculum.

## JSON clients

| Path | Before (0.9.0-rc.1) | Focused behavior after #66 |
| --- | --- | --- |
| Uninitialized, unpinned `lore --json onboard` | `contract: "lore.bootstrap_onboard"`, `schema_version: 1` | `contract: "lore.bootstrap_context"`, `schema_version: 1`, `mode: "bootstrap_source_only"` |
| Initialized `lore --json onboard` | `schema_version: 1` `lore.human_experience`-shaped result: `orientation`, `references`, `tutorial`, `feedback`, `first_task`, `revision_key`, `presentation_revalidation`, `intelligence` | The same **schema-5** top-level context envelope as `lore --json context TASK`: `snapshot`, `capabilities`, `intelligence`, source relationships, usage and budget |
| `lore --json context TASK` | Bootstrap / schema 5 / explicitly pinned existing contracts | **Unchanged** |

Do not parse `tutorial`, `feedback`, `lesson_revision`, `first_task`, `revision_key` or `presentation_status` on new `onboard` results. Instead, pass a concrete task, inspect `intelligence` and its evidence/omission and readiness fields, and query `evidence` or `decisions` for details. Do not assert that a source-only orientation verified code or established policy.

The old *human-experience* provider-neutral embedding API (`lore::experience`) was also removed; embed the already supported `lore::context::adaptive` run/build contract instead. Remove learning-mode/assessment deserializers and cached lesson fixtures. No state migration is necessary because the lesson cache and generated lesson output were disposable, not documentary evidence. Existing source registries, revisions, evidence IDs and approved privacy grants must not be changed by this migration. Existing `.lore/` caches may contain historical response data; handle their retention under normal local privacy policy or an explicit full-project purge (which also removes evidence), not as automatic source deletion.

## First-contact and safety checks

Without `lore.yml`, use `lore context "TASK"` or `lore onboard --task "TASK"` for the same capped, read-only documentary scan. In a compiled project, both use the existing schema-5 intelligence pipeline. `--inspect` remains a bounded explicit read grant; `--no-inspect` blocks it. Hosted inference and checkout egress still require the caller's authorization. Neither path edits original project sources, executes code, runs tests or silently compiles the registry.

Consult [product scope](PRODUCT_SCOPE.md) and [getting started](tutorial/README.md) for the supported workflow. Historic [0.9 release candidate guide](V09.md), [human experience design record](HUMAN_EXPERIENCE.md) and archived evaluation protocols describe the **former** feature set and are not present-feature promises.
