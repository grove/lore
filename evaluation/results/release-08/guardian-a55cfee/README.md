# PR47 Guardian verification — a55cfeead567

This receipt covers PR47 head `a55cfeead56775af4e0ae9d171cbdcf191af2007`, source tree `a0ebeecb3970a14abd10a4bcf15ea96d4bbc2049`. The dedicated [Guardian workflow run 38092630241](https://github.com/grove/lore/actions/runs/38092630241) completed successfully. A later source revision requires its own CI result.

The official artifact is `11684548775`, named `guardian-source-integrity-a55cfeead56775af4e0ae9d171cbdcf191af2007`. The downloaded outer ZIP is 4,552,486 bytes; its SHA-256 matches the official digest `269fd529f7d78aa8f28498c85a44d40edd9e05cf9907a6569b301532375d1822`. The original artifact expires on 2026-11-09. The retained synthetic inner ZIP is 4,803,212 bytes with SHA-256 `7435a529a96d68c3ad432ef4a13c4a47877a97cfdb96750edd2be6563310ef40`.

## Recomputed results

Every one of the 259 hash-bound manifest members was checked, covering 260 archive entries including the manifest. All event IDs, run and assessment hashes, the pinned collector source, source inventories, per-command byte/inode/metadata effects, source observer completeness, and event oracle results were independently recomputed.

| Cohort | Events | CLI command intervals | Source preserved | Source observer complete | All-tree observer complete |
|---|---:|---:|---:|---:|---:|
| Pinned 0.7 baseline | 60 | 608 | 608 | 608 | 605 |
| 0.8 candidate | 60 | 606 | 606 | 606 | 603 |
| Candidate repeated | 60 | 606 | 606 | 606 | 603 |
| Focused payments | 10 | 71 | 71 | 71 | 70 |
| No-Lore control | 60 | 240 non-CLI intervals | 240 | 240 | — |

The 190 replay events retain 1,891 CLI command intervals. The baseline makes one additional cited-evidence lookup in each of `releases-14` and `releases-18`: 11 resolution reads per event versus 10 for the candidate. These are evidence resolutions, not retries. Every recorded CLI command has `error: null` and a JSON response; the pinned collector emits this pair only after a zero process return code. Nonzero exits, timeouts, cancellations, process unavailability and invalid JSON outcomes are all zero. The original captures retain the full denominator.

All ten broader observer gaps concern a newly created Lore-owned state directory (`new_directory_watch_gap:lore_state`); none affects source observation. The 200 authorized source-snapshot intervals were also checked for internal hash consistency. The no-Lore control launches no processes and covers four intervals for each of 60 events.

Logical model-call counts are recomputed separately from every initialization, event materialization and nested Guardian adaptive response, then reconciled with all saved aggregates: zero in all four replay cohorts. Independent physical provider-attempt counts remain `null` because provider sidecar ledgers are absent from this synthetic capture. Independent Guardian alert quality remains unmeasured. The execution/egress syscall audit was not run. A clean hosted replay does not identify the writer responsible for the previously retained local source-restoration failures.

## Source, binaries and environment

The baseline is commit `0ba8b17f7e63726a8d81def2b60af18573d4640c`, binary SHA-256 `f03f54e0affacf3ac623634cfbb2fc8c992b78e9e881f8b16af8e8f5de98b127`. The candidate binary SHA-256 is `59f20591127dbd04ac350ae0bd7e9651be0f33daf7a24d3dbcd9bfd05c6c83f8`. Exact source, prepared-input, compiled-registry, event-manifest and collector hashes are in `guardian-verification.json` and the original `receipt.json`.

The observed Guardian runner image is `ubuntu-24.04`, image version `20261004.327.1`, runner version `2.337.0`. Exact rustc, Cargo and Python versions were not printed by this workflow and remain `null`. Runner image metadata is not substituted for those missing toolchain measurements.

## Reproduction and publication scope

`selected-artifacts.json` is an explicit copy allowlist. It excludes raw CI logs, signed download URLs, temporary verifier controls and the redundant outer ZIP. Raw logs stay in a separate local-only directory. The inner archive contains the authorized synthetic replay capture.

The permanent retained-file mode needs only the selected `receipt.json`, `guardian-integrity-evidence.zip` and `github-metadata.json`. It checks the receipt and inner ZIP against bindings recorded after the official download, then independently revalidates all 259 manifest members and replay/source/model-count checks. It does not create an outer ZIP. Its output explicitly sets `official_digest_verified: false` and `outer_digest_revalidation: not_run_retained_inner_only`: the original official outer digest remains provenance, without being claimed as freshly revalidated.

`retained-inner-verification.json` records an actual successful repeat in that mode, using the selected files without an outer ZIP. `guardian-verification.json` records the earlier official-download mode that did verify the outer digest.

```sh
python3 verify_guardian_release.py \
  --retained-inner \
  --input /path/to/retained-selected-files \
  --output /path/to/new-verification-output \
  --collector /path/to/exact-a55cfeead567-checkout/evaluation
```

While the official GitHub artifact remains available, the original download-verification mode additionally checks its outer ZIP. Obtain artifact `11684548775` as `github-artifact.zip`, place the supplied `github-metadata.json` beside it in an input directory, and use:

```sh
python3 verify_guardian_release.py \
  --input /path/to/official-artifact-input \
  --output /path/to/verified-copy \
  --collector /path/to/exact-a55cfeead567-checkout/evaluation
```

The official-input mode checks the outer hash before validating the original receipt and every inner member. Both modes launch no Lore command, provider request, CI run or publication. Ten labeled offline engineering controls passed, including real retained-inner verification without an outer ZIP, corrupt-inner rejection, missing or truncated test summaries, unexpected failed-job arguments, stale model-call aggregates, absent call counts and missing command-outcome fields. The coherent model-count controls rebind all affected hashes and fail at the actual call-count checks. These engineering controls are not empirical model or human results.

The separate [matrix run 38092630287](https://github.com/grove/lore/actions/runs/38092630287) completed with a Windows Python failure. Its Linux and macOS jobs passed; Rust and release compilation passed on all three platforms. The failed matrix receipt is supplied separately and does not change this Guardian source-integrity result.
