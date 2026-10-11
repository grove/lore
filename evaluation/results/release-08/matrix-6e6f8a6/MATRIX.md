# PR47 platform matrix — verified success at 6e6f8a6af871

The [matrix workflow run 38094627128](https://github.com/grove/lore/actions/runs/38094627128) completed successfully at PR47 head `6e6f8a6af871bdeca53dcc79b63a60fd2af52743`. Linux, macOS and Windows passed their formatting, strict Clippy, complete offline Rust, Python evaluator, optimized release build and release CLI gates. This is the corrected candidate's result; the earlier a55 receipt separately retains its Windows Python fixture failure.

All three completed logs report checkout `26165b9daa3cd4df3b81bfce987b59e62f345c95`. Its Git tree `1e77e84d9f7e69b89968c6b60a6beef7f9afec82` equals the PR head's tree. The source-derived inventory contains exactly 57 Rust targets. The verifier reconciles every expected target, declared test count and suite summary. Each platform has zero Rust failures, measured tests or filtered tests in its all-targets run.

| Platform | Job | Rust passed | Rust ignored | Python discovered | Python passed | Python skipped | Python failures / errors |
|---|---:|---:|---:|---:|---:|---:|---:|
| Linux | 114337891311 | 472 | 3 | 236 | 236 | 0 | 0 / 0 |
| macOS | 114337891412 | 472 | 3 | 236 | 235 | 1 | 0 / 0 |
| Windows | 114337891394 | 461 | 3 | 236 | 226 | 10 | 0 / 0 |

Platform-dependent test totals remain separate. Ignored Rust tests and skipped Python tests do not count as passes. The separate Linux frozen Zoom registry comparison passed one test, with three others filtered; it remains outside the all-targets totals. That comparison was skipped on macOS and Windows.

## Commands and observed profiles

| Gate | Actual command or invocation | Observed profile | Result on each platform |
|---|---|---|---|
| Offline Rust suite | `cargo test --all-targets --locked` | `test [unoptimized + debuginfo]` | All 57 expected targets complete |
| Release compilation | `cargo build --release --locked` | `release [optimized]` | Passed |
| CLI help/version smoke | Eleven `cargo run --release --locked -- ...` invocations | `release [optimized]`, actual `target/release/lore` or `lore.exe` process | Eleven passed; version stdout `lore 0.8.0` |
| Frozen Zoom comparison | `cargo test --locked --test knowledge_zoom_comparison measured_existing_compiled_project -- --ignored --exact` | `test [unoptimized + debuginfo]` | Linux passed; macOS and Windows skipped |

The eleven release invocations cover `decisions --help`, `cases --help`, `baseline --help`, `changes --help`, `guard --help`, `--help`, `review --help`, `context --help`, `onboard --help`, `explore --help` and `--version`, in that order. The logs contain twelve completed release-profile build markers per platform: the initial build plus the eleven command invocations. The regression suite itself ran in the ordinary test profile. The separate Guardian receipt covers frozen dev-profile replay binaries.

## Directly observed toolchains

Each platform completed the source/toolchain step containing `git rev-parse HEAD`, `rustc --version --verbose`, `cargo --version` and `python --version`. The values below were printed in that step; image defaults were not substituted.

| Platform | rustc | Cargo | Python |
|---|---|---|---|
| Linux | 1.99.0 (b940084d7 2026-09-28) | 1.99.0 (5f94df478 2026-08-27) | 3.12.3 |
| macOS | 1.98.1 (48a229cea 2026-09-01) | 1.98.1 (797e8a9bc 2026-08-05) | 3.14.7 |
| Windows | 1.98.1 (48a229cea 2026-09-01) | 1.98.1 (797e8a9bc 2026-08-05) | 3.12.10 |

| Platform | Compiler host | LLVM | Runner image | Image version |
|---|---|---|---|---|
| Linux | x86_64-unknown-linux-gnu | 23.1.1 | ubuntu-24.04 | 20261004.327.1 |
| macOS | aarch64-apple-darwin | 22.1.8 | macos-26-arm64 | 20260907.0351.1 |
| Windows | x86_64-pc-windows-msvc | 22.1.8 | windows-2025-vs2026 | 20260925.250.1 |

All report GitHub runner version `2.337.0`. The verbose Linux compiler commit is `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`; macOS and Windows report `48a229ceaefd4985c50990b14116b6d856af0985`.

## Retention and reproduction

`selected-matrix-artifacts.json` lists the exact bounded matrix receipt, source/run metadata, reproduction verifier and this summary. These selected files exclude raw CI logs. The receipt retains each original log's byte length and SHA-256 binding. It publishes only allowlisted CLI command identifiers and bounded result fields; unexpected arguments are reduced to counts and hashes.

The matrix verifier recomputes the test counts, release invocation sequence, directly printed version strings and checkout identity. The additional verbose compiler fields and completed profile-marker counts above were inspected separately from the same hash-bound logs. Neither the receipt nor this summary claims a real model study, human study or independent Guardian alert-quality measurement.

To recount independently, obtain the authorized completed job logs from GitHub while they remain available, save them as `<job-id>.log` in a separate local directory, and run:

```sh
python3 verify_release_matrix.py \
  --metadata matrix-metadata.json \
  --logs /path/to/local-only-job-logs \
  --output /path/to/new-matrix-verification.json
```

The selected public files do not preserve enough raw data to recount test output after GitHub log expiry; they retain the source-bound result and verifier. No raw log is included or uploaded. This verification launches no tests, CI runs, model requests or publication. A later head requires its own checks.
