# Guardian longitudinal evaluation

The guardian protocol replays successive retained source states through actual
`lore baseline save`, `lore changes`, `lore guard`, and `lore evidence` commands.
It assesses documentary change detection and mechanical evidence integrity
separately from model alert quality. Empty alerts from an unavailable provider
are unavailable assessments. They are not evidence that a project is safe.

## Included corpus and its limits

`corpora/guardian/events.json` contains sixty ordered transitions across three
synthetic projects: payments, a balance ledger, and a release controller. There
are twenty consequential, twenty benign, and twenty ambiguous labels. Each event
names its immediate parent and successor, exact source edits or deletions,
significance, source scope, alert necessity, a safe next step, and whether runtime
behavior was independently checked. No included event claims runtime verification.

The corpus covers numeric changes without a changed extracted summary, rare
exceptions, A→B→A source reversion, cosmetic edits outside evidence, citation
location changes, proposal and historical-source removal, production versus
staging scope, accepted security constraints, a source-reported bypass, and
cross-source interpretation addition, withdrawal, and reconsideration. Exact
quotation, scope, lifecycle, original relationship reason, and qualifications
remain source-owned data.

The sources, extraction records, and labels were authored together for debugging.
`fixture_only: true`, `held_out: false`, and
`label_provenance.status: synthetic_debug` are intentional. The sixty-transition
size satisfies an engineering corpus target; it is not a completed independently
labeled real-project guardian study. Repeated revisions within a project are
correlated and must not be described as sixty independent users or experiments.

`generate_fixture.py` deterministically regenerates the checked-in event JSON.
The corpus supports two separate materialization methods:

1. **Explicit synthetic assertions.** The opt-in Rust test helper captures the
   supplied records using Lore's real immutable source, assertion, evidence,
   knowledge, and relationship storage APIs. It preserves the full preceding
   history at every successor. It makes zero model calls and does not pretend to
   run extraction. This is the reproducible provider-unavailable control.
2. **Configured real extraction.** Omit `--compiled` and supply an explicit model
   configuration template. Replay invokes `lore init` and successive `lore update`
   commands, records compilation and guardian calls separately, and charges both
   to the outcome. Debug labels still do not become independent labels merely
   because a model processed them.

The documentary adapter accepts source files under `docs/` and documentary
cross-source interpretations. Native-import integrity and source-owned vocabulary
also have existing Rust companion tests. This adapter rejects an unrelated
configuration's native imports rather than reading external import paths without
a source-bound event adapter.

## Reproduce the disabled-provider control

Use fresh output directories. Run from the repository root with the supported
Rust and Python toolchains. No third-party Python packages are needed.

```sh
python3 evaluation/guardian_longitudinal.py prepare --output /tmp/lore-guardian-prepared

LORE_GUARDIAN_PREPARED=/tmp/lore-guardian-prepared \
LORE_GUARDIAN_COMPILED=/tmp/lore-guardian-compiled \
cargo test --locked --test guardian_corpus_export -- --nocapture

cargo build --locked
python3 evaluation/guardian_longitudinal.py run \
  --prepared /tmp/lore-guardian-prepared \
  --compiled /tmp/lore-guardian-compiled \
  --binary target/debug/lore \
  --output /tmp/lore-guardian-run

python3 evaluation/guardian_longitudinal.py assess /tmp/lore-guardian-run
```

If `CARGO_TARGET_DIR` is configured, use that directory's binary path. Freeze
separate baseline and candidate binaries before a matched replay; do not rebuild
or overwrite a binary while a run is using it. Both binaries consume the exact
same prepared sources, immutable retained snapshots, token budget, inspection
policy, and provider availability. A baseline saved by an older binary has its
original checkpoint field set and checksum.

Keep the replay workspace isolated from editors, synchronization processes, and
other writers. A source edit, deleted source restored by a synchronizer, or a new
temporary file during replay fails the byte-integrity checks. The evaluator does
not ignore these failures or retrospectively declare a run clean. Use a fresh
directory and rerun after removing the external writer.

### Per-command diagnosis and supported integrity runner (0.8)

Every new replay records a `source_integrity` ledger around each actual
`baseline save`, `changes`, `guard`, and `evidence` invocation. The live-provider
adapter also surrounds `init`/`update`; deliberately applied source snapshots
have a separately labeled `snapshot_integrity` interval. Records include the
exact phase, before/after SHA-256 file maps, file device/inode/mode/timestamps,
create/delete/write/replacement or identity-bound rename effects, spawned PID,
and a bounded child-tree observation where the platform permits it. The ledger
classifies primary sources, configuration, `.lore` state, and generated output.
It contains no unrestricted environment values, process arguments from other
processes, source text, or failed provider output.

File capture compares path metadata before/after the read and descriptor
metadata before/after hashing, retaining both timestamp observations. It does
not equate path and descriptor `ctime`: [CPython's Windows issue
157671](https://github.com/python/cpython/issues/157671) documents that those
APIs can expose creation time and metadata-change time respectively. Identity,
mode, size and modification time still agree across the opened file and its
path. Any change within either metadata sequence fails capture, and a path
replacement after the read is rejected. The additional
`descriptor_ctime_ns` field also preserves descriptor-only metadata changes
between command intervals. No source byte hash or observer requirement changes.

The original `source_hashes` oracle remains unchanged. A new independent
`per_command_source_integrity` check additionally rejects a mutation during
baseline saving even if the next authorized snapshot copy would overwrite it.
It also rejects writes that restore the original bytes before a whole episode
ends. New run manifests cannot drop their per-command ledger and fall back to
legacy assessment. Old archived captures retain their original failed results.
Source snapshot application preflights all destinations before deleting any
old file; snapshots cannot overlap the mutable project, follow a symlink, or
replace configuration, state, generated output, or a sibling source root.

Add `--observe-filesystem` to record Linux inotify events independently of the
hash inventory. The observer sees temporary creates, writes, deletes, metadata
changes, and paired rename cookies even when final source bytes match. It
reports unsupported platforms, failed watches, overflow and newly created
directory coverage gaps explicitly. A new directory inside Lore-owned state
does not remove existing watches on source roots; overall filesystem coverage
and source-only coverage are therefore reported separately. Inotify supplies
**no writer PID** and is not an execution/egress audit. `--audit-strace` remains
a separate, optional operation with its existing permission checks.

Use the same prepared corpus for a no-Lore differential control:

```sh
python3 evaluation/guardian_longitudinal.py control \
  --prepared /absolute/lore-guardian-prepared \
  --output /absolute/lore-guardian-no-lore \
  --observe-filesystem --dwell-ms 400
```

The control applies the exact source transitions, then observes four quiet
intervals per event. It starts **zero subprocesses**. Its fixed dwell is
reported explicitly and is not described as a runtime-matched performance
comparison. `--project payments --first-event 6 --event-count 4` focuses on
deletion and reversion. No path, including `.rsync-tmp`, is ignored.

An actual 0.8 diagnostic in the managed development workspace reproduced source
restoration without running Lore: **40/60 events failed the unchanged byte
oracle, and 42/60 failed the stronger metadata/event check**. In `payments-06`,
the observer recorded a temporary proposal file being written, moved through
`docs/.rsync-tmp`, and restored as `docs/proposal.md` with its earlier hash.
`ledger-07` captured the temporary path and its same-inode rename into the
restored source. This isolates that reproduction outside Lore. The rename
pattern is consistent with synchronization; the executable and PID remain
unidentified because inotify does not provide them and `ptrace` was denied.
It does not authenticate the writer of the historical 0.7 capture. The
[diagnostic receipt](results/baseline-08/guardian-08-no-lore-reproduction.json)
and [hash-bound control archive](results/baseline-08/guardian-08-no-lore-control.zip)
retain all events, observer gaps and the development collector's provenance
limitation. These remain failed captures, not a repaired runtime result.

The dedicated [Guardian integrity workflow](../.github/workflows/guardian-integrity.yml)
runs on Ubuntu with a fresh runner temporary directory, separate frozen 0.7
and candidate binaries, and one exact synthetic retained-history preparation.
It executes both full 60-event cohorts, a second full candidate replay, the
10-event payments segment, and the full no-Lore control. Its thin
`guardian_integrity_gate.py` wrapper requires every expected event, the original
oracle, every command ledger, zero provider calls and complete kernel source
observation. A failing capture or unavailable source observer fails the job.
The job uploads exact JSON captures, artifact hashes and a receipt even on
failure; it excludes workspaces, databases and binaries from the archive.

Only an actually passing job establishes this supported runner's clean replay.
The local external-writer reproduction does not substitute for that gate, and
a dedicated directory alone is not an operating-system sandbox. The workflow
does not measure provider advisory quality or pass an independent
execution/egress audit.

For an execution host that cannot retain a long-lived process, the synthetic
adapter also supports `--project payments --first-event 1 --event-count 10` and
the corresponding segment beginning at event 11. Each segment starts from the
exact immutable compiled predecessor, including all preceding history. Collect
both segments for each of the three projects, then join them against the full
original preparation:

```sh
python3 evaluation/guardian_longitudinal.py merge \
  --prepared /tmp/lore-guardian-prepared \
  --runs /tmp/payments-01 /tmp/payments-11 \
         /tmp/ledger-01 /tmp/ledger-11 /tmp/releases-01 /tmp/releases-11 \
  --output /tmp/lore-guardian-complete
python3 evaluation/guardian_longitudinal.py assess /tmp/lore-guardian-complete
```

Merge requires every original event exactly once, the same preparation, frozen
binary, permissions, and output budget. It copies the original capture bytes and
pins component run hashes. Assessment checks every capture's exact successive
source endpoints against the full original cohort. Missing events, modified
captures, mismatched configurations, and failed source checks cannot become a
successful complete run. Segmentation itself does not prevent an external writer.

`prepared.json` pins every complete source snapshot and the original events
manifest digest. `compiled.json` binds every database snapshot to that exact
preparation digest. Replay verifies these hashes before use. `run.json` pins the
binary, options, preparation, every event capture, and initialization costs.
Each `events/EVENT/capture.json` retains the before/query/after source fingerprints,
independent read-only registry capture, raw successful CLI responses and their
hashes, resolver responses, errors, and elapsed time. The evaluator withholds
failed stdout and stderr because they can contain source or provider secrets.

## Real-provider and external-project collection

For real extraction, provide a JSON configuration template; JSON is also accepted
by Lore's YAML reader. Keep credentials in environment variables. The collector
writes an isolated project name, source root, state directory, and output
directory; it retains explicit model settings. `--allow-hosted` establishes a
caller-owned hosted grant, while the template's `privacy.local_only` veto remains
effective. `--allow-inspection` independently grants bounded static inspection.
`--allow-checkout-egress` requires inspection and the existing configured egress
permission. Without these flags, inherited environment grants are removed and
the copied configuration cannot expand the collector's permission envelope.

```sh
python3 evaluation/guardian_longitudinal.py run \
  --prepared /tmp/lore-external-guardian-prepared \
  --config-template /path/to/explicit-local-model.json \
  --binary /path/to/frozen-lore \
  --output /tmp/lore-external-guardian-run
```

External project candidates use `fixture_only: false` and
`label_provenance.status: candidate_external`, with a repository identity and
full pinned commit per project. Preserve actual successive source revisions,
licenses, materiality decisions, scope, rejected alternatives, and adjudication
notes. Do not relabel the synthetic corpus as external. The included collector
does not recruit maintainers or invent independent reviews.

Generate blank, run-bound review forms only after collection:

```sh
python3 evaluation/guardian_longitudinal.py review-template \
  /tmp/lore-external-guardian-run --output /tmp/guardian-reviewer-a.json

python3 evaluation/guardian_longitudinal.py assess \
  /tmp/lore-external-guardian-run \
  --reviews /path/to/completed-review-a.json /path/to/completed-review-b.json
```

At least two distinct reviewers must attest independence from product and corpus
authorship. They confirm each event's source identity and label against the exact
captured hash, then rate every actual advisory's current scope, materiality, and
safe concrete action. Missing reviews, disagreement, unavailable providers,
failed commands, truncated comparisons, and advisory summaries omitted for
budget all remain explicit. A blank form and an AI-authored debug label do not
satisfy this review gate.

## What assessment checks and reports

Mechanical checks require exact before/after states, current evidence membership
where captured, immutable historical support, unchanged original excerpts,
complete relationship endpoints and witnesses, resolvable source citations,
explicit named-baseline identities, versioned statuses, complete reported output
budgets, byte-identical source/configuration/registry contents during read commands,
and retained historical rows across updates. Unexpected new source paths are
included in integrity fingerprints; deleted paths must remain deleted. Hash and
endpoint checks use captured read-only SQL and `lore evidence` responses as
independent references, rather than comparing two generated prose fields.

The report retains all sixty events, including failures. It reports exact counts
and denominators for available assessments, high-severity alerts, benign-change
noise, same-reference duplicates, independently supported precision, consequential
alert recall, and independently actionable recommendations. Same-reference
duplicates are a diagnostic lower bound; semantic duplication still needs review.
Consequential changes reported absent with no disclosed omission have their event
IDs listed as a change-detection regression diagnostic.

Precision and recall are `null` without sufficiently reviewed, available model
assessments. The product-quality gate requires real external full-size evidence,
two independent reviews per event, complete required-event assessment coverage,
at least 90% high-severity precision, at least 85% consequential alert recall, no
high-severity alerts on benign changes, no observed exact-reference duplicates,
passing mechanical checks, and completed independent runtime audit. This is a
strict release evidence gate, not a claim that the included debug replay passed
a real-world quality target.

Model calls include initial compilation, every update, and complete adaptive
guardian invocation accounting. End-to-end time includes baseline saving,
materialization, change assessment, guardian work, and citation resolution;
initialization is separately reported. Provider tokens and cost remain `null`
when calls occurred without provider usage data. Whole-output tokens are Lore's
reported `cl100k_base` budget values, checked against the requested limit; Python
does not independently retokenize the output. Rust tests cover actual response
token accounting. No population efficacy interval is inferred from correlated
synthetic transitions.

## Runtime audit boundaries

`--audit-strace` requests an actual bounded `execve`, `execveat`, `connect`,
`sendto`, and `sendmsg` syscall trace for `changes` and `guard`. A complete trace
must show only the requested Lore process and a successful exit. The current
automatic network check verifies the strict no-network control: a network call
prevents this check from passing even if a broader provider grant exists. The
grant itself is never treated as evidence of the destination contacted. A live
provider's approved endpoints therefore need a separately reviewed runtime audit
before the full product release gate can pass.

An unavailable tracer, denied tracing capability, incomplete trace, or absent
capture remains `independent_runtime_audit_complete: false`. Source hash
preservation and Lore's explicit `execution: false` fields remain useful
engineering evidence but do not substitute for independent syscall observation.
This implementation session could not collect syscall traces because the
environment denied `ptrace`; no trace or permission audit was fabricated.

## Reproduced failure classes and bounded changes

| ID | Reproducible failure | Change and regression coverage |
| --- | --- | --- |
| GUARD-REVERSION-001 | A supporting exception changed A→B→A while the short statement stayed fixed. Comparing the cumulative historical quote set missed the final return to A. | Checkpoints additionally capture current support; both immutable historical quotes remain intact. Rust integration and three longitudinal event-08 cases cover the transition and legacy checkpoint compatibility. |
| GUARD-SCOPE-001 | Six alphabetically earlier proposal topics displaced an accepted security rule from the automatic six-topic assessment. | Accepted constraints and consequential scope/support transitions precede proposal noise. A seven-topic regression tests the selected query and disclosed omission. |
| GUARD-DUPLICATE-001 | One source constraint represented as both a high risk and a material blocker yielded duplicate top-level advisories. | Changed-record association groups overlapping signals and keeps all explanations, actions, and original citations; unrelated groups remain separate. |
| GUARD-ACTION-001 | The risk summary copied a broad preferred strategy instead of the already available concrete next step. | Recommendations reuse the shared engine's `next_action`; material blockers retain their explicit decision requirement. |

These are engineering failure reproductions. They do not establish improved
model precision, independent human usefulness, or the absence of other failure
classes. Run the focused Python checks with:

```sh
python3 -m unittest discover -s evaluation/tests -p test_guardian_longitudinal.py -v
cargo test --locked --test project_companion --test project_companion_cli
cargo test --locked --lib guardian_signal_tests
```

## Recorded implementation-session results

[`results/guardian-debug-2026-10-10/README.md`](results/guardian-debug-2026-10-10/README.md)
records the actual matched CLI captures, the passing candidate Rust/Python checks,
and the limits of the observed comparison. Both full sixty-event runs completed
without command, contract, endpoint, history, or budget failures. Their strict
source-integrity checks failed when previously deleted files reappeared. One
short capture recorded `.rsync-tmp/history.md` before the original source path was
restored. The writer was not independently identified, and the no-Lore controls
did not reproduce that behavior. These captures remain failed integrity runs.

The complete successful JSON responses, all failed checks, and both no-Lore
controls are retained in a hash-bound archive. A candidate ten-event source
segment passed every mechanical check. None of these disabled-provider runs
measured alert precision, actionable real-model recommendations, or independently
reviewed real-project usefulness, and none passed the product release-quality
gate.
