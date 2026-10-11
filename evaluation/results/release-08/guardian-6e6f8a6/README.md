# Corrected PR47 Guardian verification — 6e6f8a6af871

The [Guardian workflow run 38094627173](https://github.com/grove/lore/actions/runs/38094627173) completed successfully for PR47 head `6e6f8a6af871bdeca53dcc79b63a60fd2af52743`, source tree `1e77e84d9f7e69b89968c6b60a6beef7f9afec82`. This is a new source-bound verification; the earlier a55 Guardian capture and failed a55 platform matrix remain separately retained.

Official artifact `11685921786` is 4,568,649 bytes. Its downloaded outer ZIP matches the official SHA-256 `d0d0ca8890f3998d1e25aec19851f75b790c8a6baf2bfb07d5df61e61f5554f4`. The retained inner synthetic ZIP is 4,813,894 bytes, SHA-256 `7d74e13097c6fe201f589e21b4bc92861b18c3ff157f434c0053a7000a082301`. All 259 bound manifest members were verified, covering 260 ZIP entries including the manifest.

## Recomputed source-integrity results

| Cohort | Events | CLI command intervals | Source preserved | Source observer complete | All-tree observer complete |
|---|---:|---:|---:|---:|---:|
| Pinned 0.7 baseline | 60 | 602 | 602 | 602 | 599 |
| 0.8 candidate | 60 | 601 | 601 | 601 | 598 |
| Candidate repeated | 60 | 601 | 601 | 601 | 598 |
| Focused payments | 10 | 71 | 71 | 71 | 70 |
| No-Lore control | 60 | 240 non-CLI intervals | 240 | 240 | — |

Every one of the 190 replay events passes all recomputed event checks. The 1,875 actual CLI intervals retain source-hash comparisons, inventory/metadata effects and filesystem observations. All 200 authorized snapshot intervals also pass internal hash validation. The baseline issues one additional evidence resolution in `ledger-18`: ten reads versus the candidate’s nine. Counts reflect this run’s captures; they are not copied from an earlier run.

All 1,875 commands explicitly record `error: null` and a JSON response. The pinned collector produces that pair only after process return code zero. Nonzero exit, timeout, cancellation, unavailable-process, invalid-JSON and unknown-outcome counts are all zero. The ten broader observer gaps apply only to newly created Lore-owned state directories; source observers remain complete. The no-Lore control covers four intervals for each of 60 events and launches no processes.

Logical model calls are independently recomputed from every initialization, event materialization and nested Guardian response, then reconciled with the recorded aggregates: zero in every cohort. Independent physical provider-attempt counts remain `null` because provider ledgers are not retained in this synthetic archive. Guardian alert quality remains unmeasured; the execution/egress syscall audit was not run. This clean hosted run does not identify the writer responsible for the retained historical local source-restoration failures.

## Commands, profiles, binaries and printed versions

| Procedure | Command family | Profile and observation |
|---|---|---|
| Baseline and candidate compilation | `cargo build --locked` | Two completed `dev [unoptimized]` builds; `CARGO_PROFILE_DEV_DEBUG=0` |
| Baseline synthetic registry export | `cargo test --locked --test guardian_corpus_export -- --nocapture` | One completed `test [unoptimized]` build; `CARGO_PROFILE_TEST_DEBUG=0` |
| Guardian replay | Frozen copies of the two resulting Lore binaries | Uses those dev-profile binaries; no release-profile claim |
| Platform Rust and release validation | Separate matrix workflow | Counted independently in its matrix receipt |

`CARGO_INCREMENTAL=0` is set by the Guardian workflow. The baseline is source `0ba8b17f7e63726a8d81def2b60af18573d4640c`, binary SHA-256 `f03f54e0affacf3ac623634cfbb2fc8c992b78e9e881f8b16af8e8f5de98b127`. The candidate binary SHA-256 is `59f20591127dbd04ac350ae0bd7e9651be0f33daf7a24d3dbcd9bfd05c6c83f8`, identical to the separately verified a55 candidate binary. Prepared-input, compiled-registry, event-manifest and collector hashes are retained in the original and derived receipts.

The successful source/version step directly printed:

| Item | Observed value |
|---|---|
| rustc | `1.99.0 (b940084d7 2026-09-28)` |
| Full compiler commit | `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4` |
| Compiler host | `x86_64-unknown-linux-gnu` |
| LLVM | `23.1.1` |
| Cargo | `1.99.0 (5f94df478 2026-08-27)` |
| Python | `3.12.3` |
| Runner image | `ubuntu-24.04`, image `20261004.327.1` |
| GitHub runner | `2.337.0` |

The observed source SHA matches the stated candidate. The bounded version/profile observations carry their local raw-log byte/hash binding; no raw log is included.

## Permanent retained verification

`selected-artifacts.json` is an explicit copy allowlist. It contains the original synthetic inner archive and receipt, bounded official metadata and retained-member bindings, both derived receipts, this summary and the unchanged reviewed verifier. It excludes raw CI logs, signed download URLs and the redundant outer ZIP.

`guardian-verification.json` records successful official-download verification, including the outer digest. `retained-inner-verification.json` records a separate successful verification using only the retained files. The latter explicitly sets `official_digest_verified: false` and `outer_digest_revalidation: not_run_retained_inner_only`: the recorded official outer digest remains provenance and is not freshly revalidated or synthesized.

```sh
python3 verify_guardian_release.py \
  --retained-inner \
  --input /path/to/retained-selected-files \
  --output /path/to/new-verification-output \
  --collector /path/to/exact-6e6f8a6af871-checkout/evaluation
```

Before official artifact expiry on 2026-11-09, the additional outer-origin check can be repeated by placing the original download at `github-artifact.zip` beside the supplied metadata and omitting `--retained-inner`. Both modes verify every retained member and all source/model-count gates. Neither launches Lore, tests, CI, providers or publication.

The verifier is unchanged from its peer-reviewed implementation, which passed ten labeled engineering controls including corrupt retained ZIP rejection and coherent model-count tampering. Those controls are not empirical model or human outcomes. The separate corrected-head matrix must complete before this release is described as passing every platform gate.
