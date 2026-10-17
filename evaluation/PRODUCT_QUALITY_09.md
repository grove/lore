> **Historical acceptance record — 0.9.0-rc.1.** Counts, compatibility captures and gates below describe the dated source/binary revisions, including now-retired interfaces. They are not 0.10 test totals or publication status. See the [current 0.10 engineering and publish checklist](../docs/RELEASE_010.md). Original acceptance ledgers and receipts remain unchanged.

# Lore 0.9.0-rc.1 acceptance

All eight engineering packages are implemented. This is a **release candidate**;
stable 0.9.0 remains **blocked_external** on a genuine six-task/six-arm decision
pilot. No real-agent productivity, model-quality, cost-saving, human-learning or
Guardian alert-quality gain is claimed.

The [machine ledger](results/acceptance-09.json) is a dated evidence snapshot.
[Local integrated verification](results/release-09-final/verification.json) binds source
`1d38442733a344fb3b022cea8b600257044f99ee`; the final evidence commit preserves its
code objects. GitHub's checks on each exact PR head and merge records own live CI
and merge status. Every PR requires Linux, macOS, Windows and Guardian success.
The release matrix explicitly checks out the PR head and runs optimized CLI smoke.

| Engineering observation | Actual result |
| --- | --- |
| Full local Rust suite | 498 passed, 0 failed, 3 explicitly ignored, 60 targets |
| Full Python 3.14 suite | 239 passed, 0 failed, 1 platform-specific skip |
| Formatting / strict clippy / optimized build | Passed |
| Release version and context/review/adjudication/integrate/feedback help | Passed |
| Frozen schemas 2/3 | Byte-identical to 0.8 baseline |
| Frozen schemas 4/5/default | Only investigation elapsed milliseconds differ |
| Bootstrap schema 1 | Byte-identical; sources/state unchanged; zero provider calls |
| Public decision controls | 8 cases × correct/wrong/missing implementation: 24 passed |
| Review and feedback | Stale/concurrent inputs, history, migration rollback, explicit writes, privacy and intake/withdrawal controls passed |
| Human safety/readiness | 51 Rust + 20 Python targeted controls passed; 0 real participants |

The Mac Python skip is the Linux kernel observer; Linux CI runs it. Rust ignores
are two explicitly gated live-provider tests and the selected compiled-project
comparison, which CI executes separately against its frozen fixture. Ignored and
skipped checks are not passing tests. Guardian CI replays every event/command on a
dedicated runner, including source restoration and unavailable-observer failures;
its original integrity oracle is unchanged. Passing integrity is not alert efficacy.

## Observed corrections and limitations

Two authored regression probes reproduced incorrect exact-reference behavior:
identifier substrings were accepted, and an omitted critical conflict group could
be called decision-sufficient. Both now pass, while the genuine exact-symbol
zero-call shortcut remains. The 24-case source-pinned Zoom regression also passed.
These are deterministic correctness improvements, not measured real-agent gains.
Routing formerly incorrect shortcuts through the bounded runtime may increase
inference on those requests; actual quality/cost effects are unmeasured.

A PR-04 CI failure exposed an overly narrow test assumption about fallback enum
variants. The fix checks actual non-reference mode across both valid variants;
it does not weaken the required behavior. Initial oversized/source-order fixture
attempts and a sandbox listener failure are documented alongside successful runs.
Historical 0.7/0.8 receipts and the original 0.9 baseline capture remain unchanged.

**Historical 0.9.0-rc.1 validation (pre-[#69](https://github.com/grove/lore/issues/69) removal):** Final boundary review reproduced a blocking FIFO open in optional feedback-file
hashing. The repaired implementation rejects non-regular files before opening
and uses nonblocking/no-follow Unix opens. The new regression and all seven
feedback tests pass. A fresh full suite, optimized build and contract capture
followed the repair; [before/after logs](results/release-09-final/verification.json)
retain the failure. The earlier candidate's successful CI/Guardian records are
historical and are not substituted for the repaired head's mandatory checks.

Adjudication conservatively confirms an existing documentary relation or records
advisory interpretation; it never invents a new authoritative edge. Full documentary
fingerprints invalidate advice conservatively, including unrelated source changes.
Feedback never treats supplied hashes/attestations as proof of execution or
independence. It omits private narratives/checker material and leaves disclosure-
reviewed lesson wording to the caller. No new dependency or authoritative store
was added. See [adjudication](../docs/REVIEW_ADJUDICATION.md) and
[retired task-feedback migration](../docs/FEEDBACK_MIGRATION.md).

## Empirical gates still missing

| Gate | Status and exact prerequisite |
| --- | --- |
| Stable six-task/six-arm decision pilot | `blocked_external`: six eligible independently reviewed tasks across three pinned projects; authorized pinned coding/Lore models; sealed experiment; authenticated external isolation/egress/model captures; independent checker/gold and post-run reviews/audit |
| Broader efficacy | `unmeasured`: 30 genuinely held-out independently authored/reviewed tasks, paired results and 95% intervals; public restoration/decision fixtures are ineligible |
| Human contribution and transfer | `blocked_external`: 12 real consented participants, independently unfamiliar projects, completed stages, outcome reviews and participation/assistance/withdrawal audit |
| Guardian alert quality | `unmeasured`: actual authorized inference and independent alert scoring/audit |

No hosted evaluation was run or paid for. Actual evaluation attempts are zero;
provider billing is unknown/null, never invented as zero. The unchanged six-arm,
sealed-experiment, metering and human-study protocols remain the execution handoff.
A bounded genuine pilot is the next product-validation step once those inputs and
spending authorization exist. Stable release is not implied by merging this RC.
