# Lore 0.8 evidence

The baseline is merged Lore 0.7.0 commit
`0ba8b17f7e63726a8d81def2b60af18573d4640c`. The attached implementation plan is
preserved verbatim in `docs/V08_IMPLEMENTATION_PLAN.md`.

`baseline-08/` retains the actual offline command receipt, complete output hashes,
source/permission/cost scenarios and frozen public-registry contracts. The
machine-readable `acceptance-08.json` records missing empirical gates explicitly.
Logs and source fixtures are engineering controls; they contain no real model
results or participant records.

Reproduce contract captures with a separately frozen binary from the baseline:

```sh
python3 evaluation/capture_contracts_08.py capture \
  --binary /absolute/path/to/baseline-lore \
  --source-revision 0ba8b17f7e63726a8d81def2b60af18573d4640c \
  --output /absolute/new-contract-capture
python3 evaluation/capture_contracts_08.py verify /absolute/new-contract-capture
```

Run the unchanged CI commands recorded in the implementation plan to regenerate
the engineering results. Exact elapsed times vary; archived response/log hashes
bind the bytes of a particular execution. Golden baseline responses preserve
their original schema and do not get rewritten when a future default changes.

The archived 0.7 Guardian failures remain in `guardian-debug-2026-10-10/`.
New command-level observations, full replay receipts and their precise limits
will be retained separately. A successful hash check verifies retained bytes,
not authorship, external sandboxing, product benefit or an unobserved source writer.

## Retained 0.8 release evidence

| Capture | Scope and status |
| --- | --- |
| [Fresh baseline CI](release-08/baseline-ci.json) | Standard Linux/macOS/Windows tests on the unchanged 0.7 runtime; the condensed receipt retains exact log hashes and GitHub run/job links. |
| [Final local release and contracts](release-08/local-final/summary.json) | Public source `a55cfeead56775af4e0ae9d171cbdcf191af2007`, release 0.8.0, 11 smoke commands and five source-preserving contract captures; prior focused test/permission receipts remain explicitly scoped. |
| [Integrated Python suite](release-08/integrated-python.json) | 236 offline tests passed, with exact start/end source revision, command and full log hash. Fake models and loopback servers do not constitute real-provider study attempts. |
| [Completed local Rust rerun](release-08/integrated-rust.json) | 470 offline Rust tests passed, zero failed and three ignored at the recorded pre-final-fix revision; final platform CI remains required. |
| [Initial local release commands](release-08/initial-local.json) | Formatting, strict Clippy driver and release build passed; the standard Rust run failed because a generated test executable lost execute permission. The failed run remains retained. |
| [Merged Guardian package proof](release-08/guardian-pr41/verified-summary.json) | Verified GitHub artifact and all inner manifest members: baseline/candidate/repeat/focused190 events and1889 source-preserving observed commands, plus240 zero-Lore control intervals. Final integrated0.8 proof remains a separate gate. |
| [Integrated Guardian proof](release-08/guardian-a55cfee/README.md) | Public source a55cfee: all 259 members verified, 190 replay events, 1,891 preserved and source-observer-complete commands, 240 no-Lore intervals and permanent retained-inner verification. Physical provider attempts remain unknown without sidecar ledgers. |
| [Corrected integrated Guardian proof](release-08/guardian-6e6f8a6/README.md) | Public source 6e6f8a6: all 259 members verified, 190 events, 1,875 source-preserving and source-observer-complete commands, 240 no-Lore intervals; direct compiler/profile measurements and permanent retained-inner verification. |
| [Corrected source binding](release-08/corrected-source/corrected-head-binding.json) | All 70 runtime and 145 Guardian inputs are identical to a55; the sole human fixture change matches the reviewed correction. This historical binding makes no later CI claim. |
| [Completed integrated platform matrix](release-08/matrix-6e6f8a6/MATRIX.md) | Exact source 6e6f8a6: all 57 Rust targets and enabled Python cases passed on Linux/macOS/Windows; optimized release builds, 11 CLI checks per platform and `lore 0.8.0` confirmed. |
| [Retained integrated matrix failure](release-08/matrix-a55cfee-failed/MATRIX.md) | All 57 Rust targets and release compilation passed on Linux/macOS/Windows. Windows failed two human-workflow fixture subtests; the failed run remains distinct from corrected-head CI. |
| [Native runtime regression history](release-08/pr42-runtime-regression-history.json) | Failed main-stack and owned-fixture revisions, exact corrections and the final passing three-platform PR42 matrix. |
| [Human fixture correction](release-08/pr46-pr47-human-fixture-regression-v3.json) | Exact two Windows fixture failures, canonical POSIX path correction and preserved rejection boundary; failed attempts remain separate from the completed corrected-head verification. |
| [Corrected human-package native verification](release-08/pr46-corrected-native-verification.json) | Source 76826b68 and merge 7b242d25: all 57 Rust targets and enabled Python tests passed on Linux/macOS/Windows, including the previously failing transfer fixture; 11 debug CLI checks per platform. Release-profile validation is separately recorded for PR47. |
| [Coding preflight](experiment-08-readiness/not-run.json) | Actual preflight refusal, six public tasks and36 planned assignments, no launched study or real providers; exact missing prerequisites retained. |
| [Human readiness](../evidence/human-onboarding-08-not-run.json) | Actual prepare/readiness/assess commands,12 planned slots and zero people or sessions; contribution and transfer remain unmeasured. |

The [artifact inventory](release-08/artifact-manifest.json) hashes every retained
0.8 baseline, release and readiness file. Its own hash is recorded in the
machine acceptance ledger, outside the inventory to avoid a circular digest.
Archive receipts additionally bind their individual members. New raw CI and local
command logs are not published in this directory; condensed receipts retain
their hashes and available GitHub links. Controlled-fixture publication review
is separate from independent empirical study review.

The integrated runtime has passed its full platform and Guardian gates. The
[scorecard](../PRODUCT_QUALITY_08.md) records readiness with explicitly unmeasured
empirical outcomes, while [PR47](https://github.com/grove/lore/pull/47) records the
current-head acceptance-commit checks and final merge. Each retained capture
keeps its actual source, successful or failed outcome, and qualification.

The [human-fixture history correction](release-08/pr46-pr47-human-fixture-regression-v3.json) supersedes the earlier derived receipt's release-build statement: PR46 had no separate release-build step, and its later debug-profile CLI smoke steps were skipped after the Python failure. PR47 did complete its explicit release build. PR46 uses `cargo run --locked`; PR47 uses `cargo run --release --locked`. The failed source revisions, outcomes and raw-log hashes are unchanged. The earlier v1/v2 summaries and the v2 copy inside the historical corrected-source handoff are superseded on this build-profile detail. The [PR42 checkout binding](release-08/pr42-tested-source-binding.json) separately confirms that its tested merge checkout and reviewed head have identical source trees.

## Reproduce the published candidate contracts

Build the published candidate `a55cfeead56775af4e0ae9d171cbdcf191af2007` in
a separate checkout, then capture into a fresh directory:

```sh
cargo build --release --locked
python3 evaluation/capture_contracts_08.py capture \
  --binary /absolute/path/to/candidate/target/release/lore \
  --source-revision a55cfeead56775af4e0ae9d171cbdcf191af2007 \
  --output /absolute/new-lore-08-contract-capture
python3 evaluation/capture_contracts_08.py verify /absolute/new-lore-08-contract-capture
```

The [runtime source manifest](release-08/local-final/runtime-source-manifest.json)
and [comparison](release-08/local-final/contract-comparison.json) distinguish
source identity, binary identity and timing-sensitive response fields. The
fresh baseline comparison leaves the original 0.7 baseline archive unchanged.

## Reproduce retained Guardian verification

The permanent inner-archive mode needs the retained original receipt, synthetic
archive and bounded metadata. It launches no Lore process and reports the
official outer ZIP digest as provenance rather than claiming to download or
revalidate that omitted ZIP:

```sh
python3 evaluation/results/release-08/guardian-6e6f8a6/verify_guardian_release.py \
  --retained-inner \
  --input evaluation/results/release-08/guardian-6e6f8a6 \
  --output /absolute/new-guardian-verification \
  --collector /absolute/exact-6e6f8a6-checkout/evaluation
```

The [verification receipt](release-08/guardian-6e6f8a6/retained-inner-verification.json)
records an actual successful execution of that mode. The [matrix verifier](release-08/matrix-a55cfee-failed/MATRIX.md)
requires separately obtained job logs; their bytes are deliberately not
republished in the release evidence.
