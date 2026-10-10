# Lore 0.8 release acceptance

**Release decision: integration in progress.** The eight packages are implemented;
the final release candidate still requires the complete platform and Guardian
gates before the remaining PRs merge. Real-model productivity, human contribution
and transfer, and independently reviewed Guardian alert quality are **unmeasured**.

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
| 3. Metering | Physical provider attempts, nullable tokens/USD, retries/failures/cancellation, explicit invocation ledgers and additive SQLite schema 8 | [#42](https://github.com/grove/lore/pull/42); current platform checks pending. |
| 4. First contact | Ephemeral local documentary help with exact citations, complete qualified groups, fixed read caps and typed initialization/configuration errors | [#43](https://github.com/grove/lore/pull/43); no implicit model, configuration, source write or grant. |
| 5. Adaptive default | Unpinned initialized context uses schema 5 with action-first Markdown; explicit 3/4 and fast 2 stay compatible | [#44](https://github.com/grove/lore/pull/44); caller-owned inspection/hosted/checkout grants and full-output budgets remain enforced. |
| 6. Coding studies | Sealed preflight around six existing arms, complete planned/attempted denominators, independent ledger totals and cleanup after interruption | [#45](https://github.com/grove/lore/pull/45); genuine pilot and holdout outcomes remain unmeasured. |
| 7. Human workflow | Three-concept initial orientation, preserved tutorials, exact consent/session/submission/checker bindings, withdrawal and read-only readiness | [#46](https://github.com/grove/lore/pull/46); zero actual participants and sessions. |
| 8. Release | Version 0.8.0, current guides/agent instructions, release builds and CLI checks across Linux/macOS/Windows | [#47](https://github.com/grove/lore/pull/47); final integration gate pending. |

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

### Integrated candidate

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
The final usage-scope correction still requires the final platform matrix.

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
captures are not authenticated. The first local collector was a development
version, and its receipt retains that limitation.

The separate supported GitHub runner passed the baseline, candidate, repeated
candidate and focused replays: **190 replay events and 1,889 CLI-command
intervals**, plus **240 no-Lore control intervals**, with no source-integrity
failure. Source observation was complete for every claimed-clean command and
control interval. Observation of newly created Lore-owned cache directories
has separate reported gaps; this is not a claim of complete syscall/process
attribution. Final 0.8 runtime proof is a separate required candidate gate.

Windows metadata capture compares path observations and file-descriptor
observations within their respective APIs, retaining both timestamp values and
all original byte/identity checks. The CLI metering wrapper boxes dispatch
before cancellation scopes to avoid the Windows main-stack regression. The
macOS stack-test fixture resolves its own temporary directory before selecting
a ledger path; product symlink guards remain enforced. Failed CI receipts and successful corrected revisions are retained separately;
raw CI logs remain accessible through their GitHub job links.

Source integrity does not establish alert precision, recall, useful warning
burden or an externally audited inference run. Those outcomes remain unmeasured.

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
decision will be updated only after every required engineering gate passes.
