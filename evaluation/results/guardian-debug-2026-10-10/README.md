# Guardian debug captures — 2026-10-10

These are actual Lore CLI captures on explicitly synthetic source assertions.
They are engineering diagnostics. **The guardian product-quality gate did not
pass, and real-model alert precision and independent human usefulness remain
unmeasured.** All source-integrity failures are retained.

## What was run

Two frozen binaries consumed the same three-project corpus, sixty successive
transitions, sixty-three prepared source snapshots, immutable retained database
history, disabled provider, 8,000-token output budget, and disabled checkout
inspection. The source baseline was
`6f4898bf1d6e0dc135a24ef792921c3d98af2c24`. Every event used a freshly saved explicit
`transition` baseline, actual `changes`, actual `guard`, and actual `evidence`
resolution. The two full runs executed concurrently in isolated workspaces;
their wall times are not a product performance comparison.

| Actual capture | Events | No documented change | Partial static guidance | Failed source-integrity events | Other mechanical failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline binary | 60 | 20 | 40 | 32 | 0 |
| Candidate binary | 60 | 17 | 43 | 15 | 0 |
| Candidate payments events 01–10 | 10 | 4 | 6 | 0 | 0 |
| Baseline payments events 01–10 | 10 | 5 | 5 | 2 | 0 |

All model-call totals were zero. No run produced a model-assessed advisory, and
zero required events had independently reviewed available assessments.
High-severity precision and consequential alert recall are therefore `null`,
not 100%. No human review or runtime verification is implied by the synthetic
event labels.

The candidate emitted one documentary change for each event-08 restored
condition; the baseline emitted none. Each case changes a source exception
A→B→A while preserving its short extracted statement. The full-run byte-integrity
failures prevent treating the cohort as a clean validated study. The short
payments-08 baseline capture and all ten candidate payments captures did pass
their source checks and reproduce the narrowly scoped difference. The Rust
regression additionally verifies current support and complete retained history
for the same failure class.

## Integrity failure evidence

Deleted `docs/proposal.md` and `docs/history.md` paths reappeared with earlier
bytes during the read-command interval. In the short baseline run,
`payments-09` captured `docs/.rsync-tmp/history.md`; `payments-10` then captured
the restored `docs/history.md`. This temporary path is consistent with filesystem
synchronization. The writer was not independently identified. Two no-Lore
deletion controls remained deleted, so they do not establish the cause.

No previously present source file changed its content, and the queried
registries and copied configurations remained byte-identical. The evaluator
nevertheless counts unexpected files and restored deletions as source-integrity
failures. It does not filter `.rsync-tmp`, normalize the discrepancy away, or
upgrade an incomplete capture to a passing run. Independent syscall audit was
unavailable because the environment denied `ptrace`; it was not substituted with
Lore's own permission flags.

## Candidate engineering checks

- 18 Rust companion integration tests passed, including restored conditions,
  original checkpoint checksum compatibility, current-membership tamper
  rejection, source-owned interpretation history, and accepted-topic priority.
- 2 actual CLI integration tests passed.
- 3 advisory grouping unit tests passed, retaining all citations and actions
  while separating unrelated source groups.
- 2 corpus/export tests passed; the opt-in exporter created all sixty-three
  immutable debug snapshots.
- 28 Python protocol tests passed, including lost source bytes, new unexpected
  files, immutable-history replacement, unsupported endpoint substitution,
  missing or altered events, unknown provider cost, invalid independent review,
  incomplete syscall traces, and complete cohort merge requirements.

These are contract and regression checks. They do not measure model alert
precision, developer productivity, human onboarding, or runtime safety.

## Files and reproduction

`summary.json` pins the event corpus, preparation, compiled-history manifest,
binaries, individual run manifests, test counts, observed reversion cases, and
measurement limits. `source-integrity-failures.json` records every failing stage,
unexpected path and its exact SHA-256, and any missing or changed source path.

`captures.zip` contains the two complete sixty-event runs, the two ten-event
segments, and both no-Lore controls. Each run includes its original manifest,
every raw successful JSON response, every source/registry capture, the exact
resolver responses, errors, and assessment. Run manifests bind capture files by
SHA-256; the summary binds the archive itself. Executable binaries, private
credentials, and compiled databases are not included.

To reassess the archived evidence without a model, extract the archive and run:

```sh
python3 evaluation/guardian_longitudinal.py assess /path/to/extracted/baseline
python3 evaluation/guardian_longitudinal.py assess /path/to/extracted/candidate
python3 evaluation/guardian_longitudinal.py assess /path/to/extracted/short-candidate-payments
```

The results should retain the source failures above. See
[`GUARDIAN_LONGITUDINAL.md`](../../GUARDIAN_LONGITUDINAL.md) for fresh collection,
short-segment merging, real-provider configuration, and independent review.
