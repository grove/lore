# PR47 platform matrix — retained failure at a55cfeead567

The [matrix workflow run 38092630287](https://github.com/grove/lore/actions/runs/38092630287) completed with conclusion **failure** at PR47 head `a55cfeead56775af4e0ae9d171cbdcf191af2007`. The Linux and macOS jobs passed; Windows failed its Python evaluator-test step after passing all Rust tests and release compilation. This receipt preserves the failed candidate result.

All three logs report checkout `cc9c520f15d6cba5b5b1c8dca428383fda401487`. Its Git tree `a0ebeecb3970a14abd10a4bcf15ea96d4bbc2049` equals the PR head’s tree. The source-derived inventory contains exactly 57 Rust targets. The verifier reconciles each target, each declared test count and every suite summary, including ignored tests and filters.

| Platform | Job | Rust passed | Rust ignored | Rust failed | Python discovered | Python passed | Python skipped | Python failure outcomes |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Linux | 114332014048 | 472 | 3 | 0 | 236 | 236 | 0 | 0 |
| macOS | 114332014210 | 472 | 3 | 0 | 236 | 235 | 1 | 0 |
| Windows | 114332014292 | 461 | 3 | 0 | 236 | unknown | 10 | 2 |

The Windows failures are two subtest outcomes in one independent-transfer fixture test. A failed unittest summary can count several outcomes within one discovered testcase, so the verifier leaves its pass count `null` instead of subtracting two failures as though they were two separate discovered testcases. There are no reported Python error outcomes. This is a test-fixture failure, not measured human participation or transfer performance.

All platforms completed `cargo build --release --locked`. Linux and macOS each completed all 11 allowlisted release CLI invocations and printed `lore 0.8.0`. Windows skipped those CLI smoke steps after its Python failure. Formatting and strict Clippy passed everywhere. The separate Linux frozen-registry comparison passed one test, with three others filtered; it remains separate from the all-targets totals.

## Observed environment

| Platform | Runner image | Image version | Python directly observed | rustc / Cargo directly observed |
|---|---|---|---|---|
| Linux | ubuntu-24.04 | 20261004.327.1 | 3.12.3, from benchmark metadata | unknown / unknown |
| macOS | macos-26-arm64 | 20260907.0351.1 | unknown | unknown / unknown |
| Windows | windows-2025-vs2026 | 20260925.250.1 | unknown | unknown / unknown |

All report GitHub runner version `2.337.0`. Exact compiler/interpreter values that were not printed remain `null`; image defaults are not substituted for observations.

## Retention and reproduction

`selected-matrix-artifacts.json` lists the exact bounded matrix receipt, source/run metadata, reproduction verifier and this summary. It excludes all raw CI logs. The receipt retains raw-log byte counts and SHA-256 bindings only. Unexpected command arguments are never copied into it; only allowlisted help/version command identifiers are published, with count/hash fields for any unexpected invocation.

To recount independently, obtain the authorized job logs from GitHub while they remain available, save them as `<job-id>.log` in a separate local directory, and run:

```sh
python3 verify_release_matrix.py \
  --metadata matrix-metadata.json \
  --logs /path/to/local-only-job-logs \
  --output /path/to/new-matrix-verification.json
```

The selected public files do not preserve enough raw data to recount test output after GitHub log expiry; they retain the source-bound result and verifier instead. No raw log is included or uploaded. The verifier launches no tests, CI runs, model requests or publication. A later corrected head must receive its own platform checks.
