# Lore 0.10.0 engineering and publication checklist

Checked **2026-10-17**, against main source
`677c309e8791b39991df6cc428dced13d028adb3`. `Cargo.toml`, `Cargo.lock` and the
installed CLI agree on **0.10.0**. This is an intentional breaking source line,
not evidence of a published release. At inspection, `gh release list` and
`git ls-remote --tags origin` returned no entries. No tag, GitHub Release,
registry publication or merge is authorized by this checklist.

**Engineering status:** the checked main revision passed Linux, macOS, Windows
and Guardian CI. **Product efficacy remains unmeasured:** real-agent correctness,
time/cost benefit, instruction compliance and human outcomes require the separate
independent evaluation in [#2](https://github.com/grove/lore/issues/2), still open.
The [0.9 acceptance scorecard](../evaluation/PRODUCT_QUALITY_09.md), its dated
ledgers and frozen receipts remain historical; their counts are not 0.10 totals.

## Compatibility before upgrading

| Surface | Current 0.10.0 contract and migration |
| --- | --- |
| Task context | Uninitialized unpinned `context TASK`: model-free, source-only bootstrap schema 1. Initialized default/explicit schema 5: task guidance or labeled fallback. `--fast`: model-free schema 2, initialized registry only. Explicit schemas 3/4 fail with exit 2 and `invalid_arguments`; nested `intelligence.schema_version: 4` in schema 5 remains valid. [Matrix and consumer migration](CONTEXT_MIGRATION.md) |
| Onboarding | `onboard` uses the same context pipeline; interactive learning flags and separate learning JSON/API contracts are removed. [Migration](LEARNING_MIGRATION.md) |
| Retired commands | `visualize`, `guard`, `feedback draft`: exit 2, `invalid_arguments`. `read`/`explore`: exit 2, `command_retired`. Use `context`, `search`, `decisions`, `cases`, `evidence`, and separate `baseline`/`changes` for history. [Full command map](PRODUCT_SCOPE.md#current-command-map-0100) |
| Agent integration | `integrate codex` / `claude-code` previews `AGENTS.md` / `CLAUDE.md`; only explicit `--apply` writes. No model, initialization, agent execution or new grants. [Integration contract](AGENT_INTEGRATION.md) |
| Output | New CLI configurations explicitly select `registry_only`; `--publish-markdown` opts a new configuration into Markdown. Older configurations omitting mode retain Markdown behavior. An initialized project requires explicit `update --migrate-output`; enabling Markdown also requires `--rebuild`. Detached wiki files remain intact and can be stale. [Migration and crash recovery](HEADLESS_MIGRATION.md) |
| Original evidence | Documentary and imported evidence IDs/revisions remain readable with `evidence ID`; retirement is not erasure or promotion to policy. Native Engram intake is retired, while historical snapshots, payloads, labels and withdrawn history remain. Schema-10 rollback requires the complete pre-upgrade backup and prior binary. [Engram migration](ENGRAM_MIGRATION.md) |
| OpenWiki | Optional local imported implementation observations; ordinary Markdown is sufficient. Lore does not run OpenWiki or treat it as accepted policy or runtime proof. [Native imports](../README.md#optional-native-imports-openwiki-and-beads) |

Before updating an existing project, stop writers and back up `lore.yml`, the
complete configured state directory (normally `.lore/`) and any generated wiki
together. Do not purge evidence to change output mode. Pending publication blocks
ordinary reads; restore the matching configuration if missing and run `lore
update` to recover. Review grants, provider costs and source drift first: updates
can invoke configured models even though recovery/retirement itself needs none.
Static inspection never executes tests or authorizes edits.

## Installation and first-use receipt

Environment: macOS 26.6.2, arm64; rustc 1.99.0, Cargo 1.99.0. Source was exported
with `git archive HEAD` into a fresh temporary directory, excluding local
configuration/untracked files; install root and build target were empty.

From that source directory, the documented installation was attempted:

```sh
cargo install --path . --locked \
  --root /tmp/lore-issue-97/install --target-dir /tmp/lore-issue-97/build
```

**Failed:** the sandbox could not resolve `index.crates.io`. Retried with the
already cached locked dependencies:

```sh
cargo install --offline --path . --locked \
  --root /tmp/lore-issue-97/install --target-dir /tmp/lore-issue-97/build
```

**Passed:** optimized build completed; Cargo reported `Installed package lore
v0.10.0`. Installed binary SHA-256:
`7de89c89ee3b431c4bf8b2bb1e5c87dc8c1ed5fdc50221732567e5739502c595`.
This checks a clean source/build/install using cached dependencies, not an empty
Cargo cache, network download, crates.io publication or native installation on
Linux/Windows. For a normal install, ensure Cargo's binary directory is on `PATH`;
the checks below invoked `/tmp/lore-issue-97/install/bin/lore` directly.

First-use commands ran in a fresh copy of `examples/tutorial-project`, without
configuration or registry. Provider keys, usage-ledger and inspection/egress
grant environment variables were removed from the child environment. No model
was downloaded, invoked or assumed available. `TASK` below was
`"Can production charge retries increase to five?"`.

| Actual command | Observed result |
| --- | --- |
| `lore --version` | Exit 0, `lore 0.10.0` |
| `lore --help`; `context`, `init`, `update`, `onboard`, `integrate --help` | All exit 0; command map and schema/removal/default-mode help match the contracts above |
| `lore --json context TASK` and `lore context TASK` | Both exit 0; JSON is `lore.bootstrap_context`, schema 1, `bootstrap_source_only`, `model_calls: 0`, `source_write: false`, five evidence passages with checked original-file SHA-256 values |
| `lore --json onboard` | Exit 0, same bootstrap contract, zero model calls |
| `lore --json integrate codex`; `lore --json integrate claude-code` | Both exit 0, `lore.agent_integration` schema 1, `changed: true`, `source_write: false`; neither instruction file created |
| `lore --json context TASK --schema-version 3` / `4` | Both exit 2, `invalid_arguments` |
| `lore --json visualize`; `guard`; `feedback draft`; `onboard --mode tutorial` | Each exit 2, `invalid_arguments` |
| `lore --json read index`; `lore --json explore` | Both exit 2, `command_retired` |
| `lore --json context TASK --fast` / `--schema-version 5` | Both exit 1, `uninitialized_project`; no silent change of contract |
| `lore --json init --configure-only --source ./docs` | Exit 0, `configured: true`; only `lore.yml` created, `output.mode: registry_only`, no state or wiki |
| `lore --json context TASK` after configure-only | Exit 0, bootstrap schema 1, zero model calls; configuration alone is not initialization |
| `lore --json init --configure-only --source ./docs --publish-markdown` in another fresh copy | Exit 0, `output.mode: markdown`; no state or wiki created |

Before configure-only, the complete practice tree's entries, file hashes and
modification times were unchanged by context, onboarding, previews and rejected
commands. No `lore.yml` or `.lore/` appeared. Actual model-backed compilation and
real Codex/Claude execution were not attempted; initialized behavior below uses
existing deterministic/mock-provider regression tests.

## Compatibility verification

Existing focused tests passed, with no new product code or test harness:

```sh
cargo test --offline --locked \
  --test context_migration --test bootstrap_cli \
  --test agent_integration_cli --test agent_contract_consumer \
  --test headless_registry --test engram_retirement \
  --test navigation_migration --test retired_visualizer_cli \
  --test onboard_cli --test tutorial_classroom
cargo test --offline --locked --test context_decision_cli \
  --test native_storage --test openwiki_freshness
```

**77 passed, 0 failed, 0 ignored across 13 targets** (50 + 27). The first sandbox
attempt stopped at a mock-server loopback-listener `PermissionDenied` in
`agent_integration_cli`; the complete rerun with host loopback access passed.
These checks cover retained schema-5/default/fast behavior, removed selectors
before project access, exact historical evidence and native revisions,
Engram migration/rollback, legacy wiki preservation, explicit output transitions,
interrupted registry recovery, no-op updates, previews and thin onboarding.
They do not establish semantic model quality or real-agent efficacy. No new
compatibility implementation defect was reproduced, so no product code or
additional regression test was needed.

## CI, packaging and publish readiness

Read-only GitHub inspection on 2026-10-17 found latest main at the source revision
above, with completed successful runs:

- [Lore tests — 38644463402](https://github.com/grove/lore/actions/runs/38644463402): `test (ubuntu-latest)`, `test (macos-latest)` and `test (windows-latest)` all passed. The workflow checks formatting, strict clippy, all Rust targets, optimized build, Python tests and CLI help/version; corpus preparation runs on Linux.
- [Guardian source integrity — 38644463101](https://github.com/grove/lore/actions/runs/38644463101): complete gate job passed, including every-event/command enforcement and retained hash-bound artifacts. The pinned 0.7 baseline keeps its historical command contract; the candidate uses current context. This is integrity evidence, not alert-quality evidence.

Only [CI](../.github/workflows/ci.yml) and [Guardian](../.github/workflows/guardian-integrity.yml)
definitions exist on that main tree. Neither publishes packages, creates tags or
GitHub Releases, nor attaches distributable binaries. CI's step named `Lore 0.9
release CLI contracts` is a historical label: it executes the current binary's
version/integrate/adjudication help, not a 0.9 build. GitHub also lists historical
temporary workflow registrations whose paths are absent from main; their names
are not release automation. No package-registry availability was checked.

Main's classic protection endpoint returned `Branch not protected` (404), and
effective branch rules and repository rulesets were empty. Passing checks are
observed engineering results, not evidence of enforced merge protection. This
document does not change repository settings. Main's runs do not certify a new
PR head or later release candidate.

- [x] Source/lockfile/CLI version aligned; current and historical docs separated.
- [x] Clean-source offline installation and model-free first use exercised with the limits above.
- [x] Retained/retired contracts and migration/recovery checked by existing focused tests.
- [x] Checked main has green Linux/macOS/Windows and Guardian runs; efficacy remains unmeasured.
- [ ] Before publication, select the exact candidate SHA and require its own green platform/Guardian checks; retain the run/artifact links.
- [ ] Validate the intended distribution: network installation with an empty dependency cache on supported hosts, and packaging contents/install behavior for any planned registry or binary artifact. No registry/binary distribution is established here.
- [ ] Obtain explicit owner authorization for the exact tag, GitHub Release, package publication and any binary assets. Review compatibility notes, backups, artifact identity/checksums and release wording first.
- [ ] Publish only the authorized effects, then verify the tag target, release/assets and installation command against that distribution.

**Publish-readiness statement:** engineering preparation is documented and the
checked main is green. Publication is pending an exact candidate, distribution
validation and explicit owner authorization. No release was published or tagged;
passing CI and crate version 0.10.0 do not certify measured usefulness.
