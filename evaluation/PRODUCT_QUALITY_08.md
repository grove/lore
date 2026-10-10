# Lore 0.8 release acceptance

**Release decision: ready with explicitly unmeasured empirical outcomes.** The
integrated 0.8.0 runtime passed all required native platform, release CLI and
Guardian gates at `6e6f8a6af871bdeca53dcc79b63a60fd2af52743`. Real-model
productivity, human contribution and transfer, and independently reviewed
Guardian alert quality remain **unmeasured**.

The final acceptance commit changes documentation and evidence only. It must
pass the same current-head CI gates before merge; the exact accepted head and
merge identities are recorded in [PR #47](https://github.com/grove/lore/pull/47).
The retained results below keep their actual tested source pins.

Baseline runtime: `0ba8b17f7e63726a8d81def2b60af18573d4640c`, merged Lore 0.7.0.
The [implementation plan](../docs/V08_IMPLEMENTATION_PLAN.md) defines engineering
ship gates separately from empirical benefit claims. The [0.8 guide](../docs/V08.md)
documents the actual contracts, permission model and migration. The
[machine ledger](results/acceptance-08.json) and [evidence index](results/README-08.md)
bind retained records without rewriting the historical 0.7 failures.

## Implemented packages and review status

| Package | Implemented behavior | Review |
| --- | --- | --- |
| 1. Baseline | Exact source/environment pins, frozen 2/3/4/5/default-4 captures and honest acceptance states | [#40](https://github.com/grove/lore/pull/40), merged `e46c8897f38ac03b2dac6d4a24a722532faf7b75`. |
| 2. Guardian | Per-command byte/identity/event checks, observer gaps, zero-Lore controls and a dedicated full replay gate | [#41](https://github.com/grove/lore/pull/41), merged `75ab314368d2f87e790c95a1991ef33414f5432b`; all three platforms and the full Guardian gate passed. |
| 3. Metering | Physical provider attempts, nullable tokens/USD, retries/failures/cancellation, explicit invocation ledgers and additive SQLite schema 8 | [#42](https://github.com/grove/lore/pull/42), merged `2fef0a7057a1a7edcbb4c4cd7f983af1b78a24f4`; all three platforms and Guardian passed. |
| 4. First contact | Ephemeral local documentary help with exact citations, complete qualified groups, fixed read caps and typed initialization/configuration errors | [#43](https://github.com/grove/lore/pull/43), merged `7f5edff4f5f93c75e599af5f03dbcc50cd5a61fb`; all three platforms and Guardian passed. |
| 5. Adaptive default | Unpinned initialized context uses schema 5 with action-first Markdown; explicit 3/4 and fast 2 stay compatible | [#44](https://github.com/grove/lore/pull/44), merged `52f357b005e19ffa103aa501550d8ca90922fc60`; all three platforms and Guardian passed. |
| 6. Coding studies | Sealed preflight around six existing arms, complete planned/attempted denominators, independent ledger totals and cleanup after interruption | [#45](https://github.com/grove/lore/pull/45), merged `be67e368287e421b4595f3f761bb9408f5b686f6`; all three platforms and Guardian passed. Genuine pilot and holdout outcomes remain unmeasured. |
| 7. Human workflow | Three-concept initial orientation, preserved tutorials, exact consent/session/submission/checker bindings, withdrawal and read-only readiness | [#46](https://github.com/grove/lore/pull/46), merged `7b242d25b7a5bc1825c146c77657b46bd0eb73c6`; all three platforms and Guardian passed. Zero actual participants and sessions. |
| 8. Release | Version 0.8.0, current guides/agent instructions, release builds and CLI checks across Linux/macOS/Windows | [#47](https://github.com/grove/lore/pull/47); integrated runtime, release CLI and Guardian gates passed. The PR retains the final current-head checks and merge identity. |

The source-only path is bounded to 256 files, 8 MiB in total, 1 MiB per file and
eight directory levels. It preserves full-file hashes and line spans, reports
omitted documentary groups, and treats repository instructions as untrusted
source data. Initialized adaptive context uses the existing controller and
registry; it does not create another authority store or expand caller grants.

## Executed engineering evidence

### Baseline

Fresh standard CI on the unchanged 0.7 runtime passed:

| Platform | Rust tests | Python tests | Result |
| --- | --- | --- | --- |
| Linux | 429 passed, 0 failed, 3 ignored | 174 passed | Passed |
| macOS | 429 passed, 0 failed, 3 ignored | 174 passed | Passed |
| Windows | 421 passed, 0 failed, 3 ignored | 174 passed | Passed |

The Rust totals differ because of platform-specific tests. They exclude the
separate Linux invocation of the ignored frozen-registry Zoom comparison.
[The condensed receipt and CI links](results/release-08/baseline-ci.json) bind run
`38085575884` and head `644ac153072ce6fd8696d6f110b0f2da396feeaa`.

### Final integrated candidate

[Matrix run 38094627128](https://github.com/grove/lore/actions/runs/38094627128)
passed on all three native platforms at
`6e6f8a6af871bdeca53dcc79b63a60fd2af52743`. Its observed merge checkout
`26165b9daa3cd4df3b81bfce987b59e62f345c95` has exactly the same tree,
`1e77e84d9f7e69b89968c6b60a6beef7f9afec82`. The
[verified matrix receipt](results/release-08/matrix-6e6f8a6/matrix-verification.json)
reconciles all 57 source-derived Rust targets and every declared suite count.

| Platform | Rust passed / failed / ignored | Python passed / failed / skipped | Release build and CLI |
| --- | --- | --- | --- |
| Linux | 472 / 0 / 3 | 236 / 0 / 0 | Passed; 11 invocations; `lore 0.8.0` |
| macOS | 472 / 0 / 3 | 235 / 0 / 1 | Passed; 11 invocations; `lore 0.8.0` |
| Windows | 461 / 0 / 3 | 226 / 0 / 10 | Passed; 11 invocations; `lore 0.8.0` |

Platform-specific ignored/skipped tests are visible and are not counted as
passes. All platforms discovered 236 Python test cases. The separate Linux
frozen-registry Zoom comparison passed one test with three filtered; it is
excluded from the all-targets totals above. Ordinary tests run in the test
profile; release compilation and all 11 CLI invocations use the optimized
release profile. Formatting and strict all-target Clippy passed everywhere.

The exact standard matrix commands are:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
python -m unittest discover -s evaluation/tests -v
```

Release smoke uses `cargo run --release --locked -- ...` for version, top-level
help, and the `decisions`, `cases`, `baseline`, `changes`, `guard`, `review`,
`context`, `onboard` and `explore` help commands. Linux also completed the
prepared-corpus commands and controlled checker validation defined in CI.

| Platform | Directly observed Rust / Cargo | Python | Runner image |
| --- | --- | --- | --- |
| Linux | 1.99.0 / 1.99.0 | 3.12.3 | ubuntu-24.04 |
| macOS | 1.98.1 / 1.98.1 | 3.14.7 | macos-26-arm64 |
| Windows | 1.98.1 / 1.98.1 | 3.12.10 | windows-2025-vs2026 |

Exact compiler commits, image versions, source bindings, raw-log hashes and
reproduction commands are in the [matrix evidence](results/release-08/matrix-6e6f8a6/MATRIX.md).
The [corrected source binding](results/release-08/corrected-source/corrected-head-binding.json)
confirms that all 70 runtime inputs and 145 Guardian program/corpus inputs are
unchanged from the separately retained a55 compatibility and integrity captures.

### Retained earlier attempts and compatibility captures

The integrated Python suite passed **236 tests, zero failures and zero skips**
in 145.079 seconds of test-runner time, at
`e6dc56fd460742e8fdb76b64a87ff58755e62171`. The
[receipt](results/release-08/integrated-python.json) retains the exact command
and log hash. These are offline engineering controls with fake models and
loopback mock servers where required; they are not real-model study attempts.

The initial 0.8 local formatting, strict Clippy driver and release build passed.
The standard all-target Rust invocation failed when a generated test executable
lost its execute permission before launch. No failing runtime assertion was
relabelled as success. The [failed invocation and build receipts](results/release-08/initial-local.json)
remain retained with their log hashes. A later complete local rerun passed
470 Rust tests, zero failures and three ignored tests at
`aa4ad59446d87babbb5564b8ba3fd03686b21601`; its
[separate receipt](results/release-08/integrated-rust.json) preserves that scope.
The final usage-scope correction passed all three native platforms in the
[metering regression history](results/release-08/pr42-runtime-regression-history.json).
The failed overflow and macOS fixture revisions remain distinct from the
corrected passing revision.

The integrated candidate at `a55cfeead56775af4e0ae9d171cbdcf191af2007`
passed all 57 Rust targets: **472 passed and three ignored** on Linux and macOS,
and **461 passed and three ignored** on Windows. Release compilation passed
everywhere. Linux passed all 236 Python tests; macOS passed 235 with one skip.
Windows failed two subtests because a human-workflow fixture emitted native
backslashes in an evidence reference that requires a canonical POSIX path.
The [failed matrix receipt](results/release-08/matrix-a55cfee-failed/matrix-verification.json)
preserves that failure, with Windows Python passed count left unknown because
subtest outcomes are not separate discovered test cases. The
[fixture correction](results/release-08/pr46-pr47-human-fixture-regression-v3.json)
changes path serialization and adds a rejection assertion for backslash
references. The production path guard remains enforced. Corrected native CI passed for the human package at
`76826b68a6054af10cedc7fe45386e96bf20f55f`. The corrected [integrated release matrix](results/release-08/matrix-6e6f8a6/MATRIX.md)
also passed; the failed candidate receipt remains unchanged.

The [final local release receipts](results/release-08/local-final/summary.json)
bind a fresh five-case capture to published source
`a55cfeead56775af4e0ae9d171cbdcf191af2007`. The frozen release binary reports
`lore 0.8.0`; all 11 guide/CI help and version commands passed. Schemas 2 and 3
are byte-identical to the separately frozen baseline; schema 4 differs only
in elapsed time. Default and explicit schema 5 differ only in elapsed time.
All captures preserve source and registry bytes, stay within their complete
response budgets, and make zero provider calls. The local focused run passed
42 tests with one live-provider test ignored after a separately retained
generated-executable permission failure and narrow rerun.

Focused tests cover first-run and explicit contracts, complete JSON/Markdown
budgets, source/configuration race handling, grants and denial, retries and
nullable usage, migrations, publication recovery, rollback, purge, no-op update,
source revalidation, cancellation, and independent study/checker boundaries.
Default and explicitly selected schema 5 preserve equivalent semantics; timed
fields are compared within their documented measurement scope. Frozen baseline
responses remain unchanged in their original archive.

## Guardian source integrity and its limits

A local 60-transition control ran **zero Lore subprocesses** and reproduced
source restoration in 40 events under the original byte oracle, or 42 under
the stronger metadata/event checks. Kernel events retained temporary writes and
renames, including `.rsync-tmp`. This positively isolates an external writer
for that reproduction; its executable/PID and the writer in the historical 0.7
captures are not authenticated. The original failures remain retained.

The [corrected integrated Guardian proof](results/release-08/guardian-6e6f8a6/README.md)
comes from [run 38094627173](https://github.com/grove/lore/actions/runs/38094627173)
at public source `6e6f8a6af871bdeca53dcc79b63a60fd2af52743`. Independent archive
verification checked all **259 manifest members and 200 authorized source-snapshot
intervals**, in addition to every event, command outcome and source observer:

| Cohort | Events | Commands or intervals | Source preserved and source observer complete |
| --- | ---: | ---: | ---: |
| Pinned 0.7 baseline | 60 | 602 commands | 602 |
| 0.8 candidate | 60 | 601 commands | 601 |
| Candidate repeated | 60 | 601 commands | 601 |
| Focused payments | 10 | 71 commands | 71 |
| No-Lore control | 60 | 240 non-CLI intervals | 240 |

All **190 replay events and 1,875 CLI command intervals** passed. Every recorded
command explicitly retains a successful JSON response after exit code zero;
nonzero exit, timeout, cancellation, unavailable process, invalid JSON and
unknown-outcome counts are zero. The baseline makes one additional cited-evidence
resolution in `ledger-18`; it is not a retry. Ten broader observation gaps
concern newly created Lore-owned state directories, while source observation
is complete for every claimed-clean command and control interval.

Recomputed initialization, materialization and Guardian logical model calls
are zero. Independent physical provider-attempt counts remain **unknown**
because this synthetic archive has no provider sidecar ledgers. The supported
runner directly reported Rust 1.99.0, Cargo 1.99.0 and Python 3.12.3. Guardian
exercises dev-profile binaries and test-profile fixture export; the separate
platform matrix provides release-profile build and CLI evidence. Its candidate
binary SHA-256 matches the earlier a55 capture exactly.

The permanent retained-inner verifier works after GitHub artifact expiry. It
rechecks the retained original receipt, archive bindings and all members,
while explicitly leaving fresh outer-ZIP digest validation unrun. Historical
[PR41 proof](results/release-08/guardian-pr41/verified-summary.json), with 1,889
commands, and [a55 proof](results/release-08/guardian-a55cfee/README.md), with
1,891 commands, remain separately bound to their actual sources and prepared
captures. Different citation-resolution counts are not dropped failures or
measured performance improvements.

Windows metadata capture compares path and file-descriptor observations within
their respective APIs, retaining both timestamp values and all byte/identity
checks. The metering allocation correction and macOS owned-fixture corrections
passed their native regression gates; their failed attempts remain in the
[native runtime history](results/release-08/pr42-runtime-regression-history.json).

Source integrity does not establish alert precision, recall, useful warning
burden or an externally audited inference run. Those outcomes remain unmeasured.
A clean supported-runner replay does not identify the writer in the historical
local source-restoration captures.

## Resource costs

Provider usage is recorded per physical attempt. A retry, timeout, refusal,
validation failure or cancellation is retained even when its tokens or bill
are unknown. Historical records preserve unknown usage. Cache reuse consumes
zero new provider requests and keeps historical origin information separate.
Offline response-envelope token counts are distinct from provider usage.

| Resource outcome | Status and interpretation |
| --- | --- |
| Source-only and deterministic controls | Zero real provider calls; bounded local read/output costs are engineering measurements. |
| Cold initialization and warm/revalidated real-model requests | `not_run`; no pinned permitted provider is configured for a genuine comparison. |
| Coding-agent warm-up, context, repairs, verification, tools and waits | Retained by the harness when executed; no genuine study attempts were run. |
| Provider tokens and billed USD in genuine coding studies | Unknown/null until actual usage or billing data exists. No invoice is inferred from calls or estimated response tokens. |
| Net time/cost saved by reuse | `unmeasured`; cache-hit counts and synthetic timings are not a measured all-in saving. |

## Empirical outcomes

| Outcome | Status | Actual evidence and missing prerequisite |
| --- | --- | --- |
| Six-task, six-arm pilot | `not_run` | [Actual preflight receipt](results/experiment-08-readiness/not-run.json): six public tasks across three projects, 36 planned assignments, zero actual coding attempts/provider calls/subprocesses. Pinned configured coding/Lore models, independent task/checker authors and gold reviews, a sealed contract and authenticated external provider/isolation/egress readiness are missing. |
| Full held-out coding study | `not_run` | Pilot prerequisites plus independently authored/reviewed untouched held-out tasks. Public development candidates are not an untouched holdout. |
| Coding productivity | `unmeasured` | No genuine matched outcome/time/cost observations or uncertainty estimates. No retrieval/controller tuning is attributed to an unrun pilot. |
| Human first contribution and transfer | `unmeasured` | [Actual zero-enrollment receipt](evidence/human-onboarding-08-not-run.json): 12 planned positions, zero people, sessions or independent human assessments. Requires consenting participants, bound authored submissions, recorded time/assistance, independent executable checks and explanation/transfer reviews. |
| Guardian independent alert quality | `unmeasured` | Requires permitted pinned inference, independent event/advisory review and external runtime/egress audit. Quiet disabled-provider output is not a scored advisory. |

## Publication and reproduction

Retained release artifacts contain controlled source fixtures, synthetic Guardian
replays and condensed engineering receipts. New raw command and CI logs are not
republished; receipts retain their hashes and available GitHub job links. No
actual participant records or real provider responses were collected. This release review
is an engineering publication check; it is not independent task authorship,
human consent, an external runtime audit or a provider invoice.

Use the exact commands in the [0.8 guide](../docs/V08.md) and
[evidence index](results/README-08.md). Every retained file has a SHA-256
binding in its receipt or artifact inventory. Hash verification establishes
retained byte identity, not authorship or product benefit. The final release
decision is supported by the completed source-bound gates above. No unresolved
product defect remains from the reproduced native regressions. The historical
local source-restoration writer remains unidentified, and independent empirical
outcomes retain the missing prerequisites stated above.
