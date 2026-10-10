# Lore 0.7 release scorecard

Recorded evidence: **2026-10-10**. This scorecard accompanies the
[0.7 product guide](../docs/V07.md) and [implementation tracker](../docs/IMPLEMENTATION_TRACKER.md).
It separates implemented capabilities, measured regressions, and product outcomes.

## Ship decision

**Release individually validated fixes and evaluation infrastructure under the
plan's honest ship rule, after every final pull-request head passes the required
Linux, macOS and Windows checks.** The available evidence supports improved
retention of the documented tight-budget conditions and a targeted guardian
source-reversion fix. It does not establish general graph benefit, coding-agent
productivity, human learning, or independently reviewed guardian alert quality.

A release may describe these bounded improvements and the runnable protocols.
It must keep unmeasured outcomes explicit, retain failed captures, and preserve
experimental opt-in interfaces. This decision does not waive privacy, provenance,
or execution-audit requirements for a completed product study. In particular,
the full guardian integrity captures and its product-quality gate have **not passed**.

| Planned work package | Implemented deliverable | Evidence and acceptance status |
| --- | --- | --- |
| [#33](https://github.com/grove/lore/pull/33) Knowledge Zoom reliability | Mandatory-condition closure, complete evidence before navigation, explicit compact schema 2 and reversible resolver | Three frozen 1,500-token failures repaired through the explicit compact path; legacy schema 1 limitations retained. |
| [#34](https://github.com/grove/lore/pull/34) Retrieval evaluation | Operational comparison, controlled ordering/navigation ablation, 24 source-pinned cases and exact artifact provenance | Reviewed regression retention passes the measured cases; graph-induced critical-recall delta is zero. |
| [#35](https://github.com/grove/lore/pull/35) Coding-agent benchmarks | Thirty public repair candidates, six-arm runner, bounded repairs, independent checker process, outcome and cost ledgers | 30 correct controls pass; 60 wrong controls fail. Independent authorship/review and real-model pilot/full study remain unperformed. |
| [#36](https://github.com/grove/lore/pull/36) Agent intelligence improvements | Failure triage and change-validation gates bound to actual pilot and holdout artifacts | Infrastructure implemented. No new adaptive runtime algorithm or prompt is justified by an unrun study; measured task improvement remains unmeasured. |
| [#37](https://github.com/grove/lore/pull/37) Human onboarding | Counterbalanced contribution/transfer protocol, consent, assistance records and independent review bindings | Twelve planned slots; zero actual participants. Human outcomes and evidence-directed UX changes remain unmeasured. |
| [#38](https://github.com/grove/lore/pull/38) Project guardian | Current-support checkpoint tracking, material change prioritization, grouped actionable advice, 60-event replay/review protocol | Targeted regressions pass; 140 actual debug event captures retained. Both full cohorts fail source integrity; precision/recall remain unmeasured. |
| [#39](https://github.com/grove/lore/pull/39) Release validation | Product guide, scorecard, dated evidence, compatibility and release gates | Local integrated engineering gates pass; all final platform checks are required before merge. |

The linked pull requests implement the seven work packages. Their final head
identities, review records, check results and merge receipts are authoritative
on GitHub; the outcome-study limitations in this scorecard remain unchanged by
a successful engineering merge.

## Final engineering gates

The [dated engineering receipt](results/release-07-engineering-2026-10-10.json)
records the local 0.7.0 gates, exact counts, ignored tests and log hashes. Final
cross-platform status and commit identities are the checks attached to the
release pull requests; each final head must pass before merge. Focused tests or
an earlier head's successful check cannot substitute for that requirement.

| Gate | Required command or evidence | Final integrated status |
| --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | Passed through the matching local formatter, including the later macOS test setup correction; standard command remains in CI |
| Strict lint | `cargo clippy --all-targets --locked -- -D warnings` | Passed through the matching Clippy driver with `-D warnings`; standard command remains in CI |
| Rust suites and CLI contracts | `cargo test --all-targets --locked` | **427 passed, 0 failed, 3 ignored**, across 55 test binaries |
| Evaluation protocol suites | `python3 -m unittest discover -s evaluation/tests -v` | **174 passed**, 0 failed |
| Retrieval comparison and frozen replay | Full comparator and explicit replay below | Full comparator and exact frozen replay passed; post-correction synthetic comparator passed under a real symlinked temporary root |
| No-inference preparations and checker controls | All seven existing preparations plus 0.7 additions in [CI](../.github/workflows/ci.yml) | **13/13 passed**; fresh controls again accepted 30 correct originals and rejected 60 wrong implementations |
| Actual CLI version/help | Rebuilt binary and ten help surfaces | **11/11 passed**, including `lore 0.7.0` |
| Linux, macOS, Windows | Passing matrix on every final release PR head | Required before merge; authoritative receipts are attached to the release PRs |
| Independent execution/egress audit for external studies | Retained trace and independently reviewed permitted execution/network destinations | Unavailable in this environment; not passed |

Compatibility includes default context schema 4, deterministic `--fast` schema 2,
optional schema 3, explicit shared-intelligence schema 5, and plain exploration
schema 1. Compact exploration schema 2 requires `--compact`. Exact evidence,
relationship closure, old checkpoint checksums, bounded output/work, grants,
no-op behavior and cache invalidation remain engineering requirements.
The ignored tests are the explicit local Ollama check, live OpenAI check and
selected existing-project replay. The latter was also run separately against the
frozen fixture. Ignored provider tests remain unperformed model experiments.

The local formatter and Clippy frontends could not resolve `/proc/self/exe`, so
the matching Rust 1.90 formatter and Clippy driver ran the checks directly. This
was actual strict Clippy across all targets, not a plain compiler substitute.
The full Rust gate took 380.07 seconds; its retained output hash is
`b3668542a6e2826a592bdbd1bfd14afc013150a123e89fc32e667cbf8c02cf73`.
The subsequent macOS-style path regression passed in 57.86 seconds. All 28 local
gate logs were re-read and checked against their recorded byte counts and hashes.
Windows CI subsequently exposed two fixture portability defects: repair input
was normalized from CRLF to LF before byte-bound verification, and a UTF-8 corpus
assertion used the system decoder. The tests now preserve exact UTF-8 source bytes,
exercise both line endings, and select UTF-8 explicitly. The restoration test also
closes its SQLite handle before temporary-directory cleanup. These changes preserve
the production verifier and all frozen evidence bytes; final platform CI covers
the corrected fixtures. The integrated Python suite passed again after these
changes: 174 tests in 127.700 seconds; the dated receipt retains both runs.

## #33 — Frozen tight-budget regressions

The [baseline](results/knowledge-zoom-07-baseline-frozen-2026-10-10.json) and
[candidate replay](results/knowledge-zoom-07-after-frozen-2026-10-10.json) use the
same 12-original SQLite registry, source inventory, gold IDs and registry revision.
The production baseline is `6f4898bf1d6e0dc135a24ef792921c3d98af2c24`; its only
comparator modification is the retained fixture-capture hook. See the
[frozen fixture](corpora/zoom-frozen-v1/README.md) and [provenance](results/knowledge-zoom-07-provenance-2026-10-10.json).

Database SHA-256: `6ff2b2b328b65af775902512350ce3125083d2e6441849258ca7757b8248da55`.
Registry revision: `blake3:6c2f2d17988d44ee58c654dc236cb972e0743c7309f35ea79373d0b88e74a5b2`.
The replay used a read-only connection, recorded zero SQLite changes and zero new
logged model calls, and retained byte-identical database content afterward.

Each fraction below is **complete critical conditions retained / required** at
**1,500 tokens**. Tokens count the larger complete emitted JSON/Markdown response
with the established `cl100k_base` tokenizer, including metadata and omissions.

| Frozen query | Flat, unchanged | Baseline Zoom schema 1 | Candidate schema 1 | Candidate compact schema 2 | Compact tokens | Compact status |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Capacity and rare exception | 2 / 3 | 0 / 3 | 0 / 3 | **3 / 3** | 1,405 | `partial` |
| Proposed staging scope | 2 / 2 | 0 / 2 | 1 / 2 | **2 / 2** | 1,406 | `partial` |
| Documented disagreement | 2 / 2 | 0 / 2 | 0 / 2 | **2 / 2** | 1,445 | `complete` |

The capacity and staging results remain partial because other relevant originals
are omitted. Full critical-condition retention does not erase that limitation.
The independent canonical schema-1 payload reservations for these three cases
are 2,864, 1,991 and 2,334 tokens. They include mandatory originals, qualifications,
relation endpoints/witnesses and provenance, with optional navigation removed.
This is a measured canonical envelope, not a proof of the smallest possible
encoding; the normalized schema-2 response demonstrates another feasible encoding.

The [new synthetic diagnostic](results/knowledge-zoom-07-synthetic-2026-10-10.json)
also rejects ten corrupted manifests and one oversized response with understated
token use. The [October 9 baseline](results/knowledge-zoom-synthetic-2026-10-09.json)
is preserved separately: its generated IDs differ and its flat capacity result
was 3 / 3. Substituting it for this exact-registry baseline would be misleading.

## #34 — Reviewed retrieval and graph contribution

The [reviewed report](results/knowledge-zoom-07-reviewed-2026-10-10.json) contains
24 exact documentary queries, eight from each pinned source below. A separate
AI engineering reviewer checked source bytes, line spans, complete qualifications,
queries and documentary scope/lifecycle without inspecting selector outputs.
The [review receipt](corpora/zoom-reviewed-v1/review.json) records its correction,
role and limits; this was **not an independent human outcome study**.

| Documentary project | Pinned upstream commit | Cases |
| --- | --- | ---: |
| ripgrep | `3fce3b5bb0236da2df6d99672afb8a719642eca7` | 8 |
| fd | `14dcd92fb76ca0ebc2e82671a275f67c790d25fc` | 8 |
| jq | `b904884b94ca48e025a12697ab172711e6c134d8` | 8 |

| Budget | Flat complete critical cases | Zoom schema 1 | Compact Zoom schema 2 |
| --- | ---: | ---: | ---: |
| 1,500 | 23 / 24 | 17 / 24 | **24 / 24** |
| 8,000 | 24 / 24 | 24 / 24 | **24 / 24** |

These cases exposed option/path tokenization, acronym dependency and ranking
failures that informed fixes. The final set is **development/regression gold**,
not an unseen holdout. Its 24 current documentary rules do not independently cover
every proposal, withdrawal, historic transition or multiple-parent navigation case;
those dimensions have separate synthetic and Rust regressions. Exact excerpts
were compiled by a deterministic capture adapter with zero provider requests.
The tests verify captured source bindings; the separate review checked complete
upstream files. Neither step executes upstream projects or tests real extraction.

### Controlled ordering/navigation ablation

Both arms receive every original candidate and share the candidate digest,
mandatory evidence closure, compact schema, budget and packer. The changed variable
is `DirectOnly` versus `GraphGuided` ordering/navigation. No gold answer IDs are
used to restrict candidates. All **48 case/budget pairs** retain the same critical
conditions; the per-pair graph-induced critical-retention delta is **zero**.

| Budget | Pairs | Median added whole-output tokens | Median added retrieval latency |
| --- | ---: | ---: | ---: |
| 1,500 | 24 | +41.5 | +34.1425 ms |
| 8,000 | 24 | +1,933.5 | +146.674 ms |

These are medians of paired per-case differences, using three local debug repeats
on Linux x86-64. Both controlled arms build and validate the same graph: this
measures use of graph ordering/navigation, **not a graph-free architecture's
total cost**. Full operational measurements include uncached graph construction;
three separate cold-population/validated-replay samples retain cache costs.

The report contains 720 timed retrieval calls plus separate cache measurements.
Arms alternate within each comparison block; public compact and legacy timings
come from separate blocks. SQLite/OS caches are uncontrolled. With three repeats,
p95 is a sample maximum, not an established population tail. No general latency
claim follows. Human navigation utility remains `null`. The result supports
condition closure, ranking and compact packing; it supplies no graph-induced
critical-recall advantage and does not promote Zoom over the default context path.
See [methods and interpretation](KNOWLEDGE_ZOOM.md).

## #35 and #36 — Coding outcomes and evidence-directed changes

The [candidate manifest](corpora/coding-real-v1/manifest.json) binds 30 constructed
function/method repair tasks, ten per project. Each task removes one body from a
pinned public Python module while retaining its docstring and surrounding source.
These are bounded source slices with licenses, not complete repository builds or
three distinct programming-language domains. Public upstream answers may already
be known to a provider. The same implementation agent authored tasks and checkers;
independent authorship, gold review and held-out status are **pending**.

| Coding project | Pinned upstream commit | Candidate tasks |
| --- | --- | ---: |
| boltons | `4e5faa3d7e4008d89e0d8bf1ea87b6d9a061a16d` | 10 |
| more-itertools | `b21ec4ee8e139ebe297e2221980a694bebb69523` | 10 |
| python-dotenv | `0b2880591780a426b0551436f47a17e7a76d954f` | 10 |

The [actual checker-control report](results/coding-controls-2026-10-10.json)
records **30 original implementations passing and 60 wrong implementations
rejected**: one missing body and one constant-`None` body per task. Those are
90 executed checker controls with zero inference calls, not coding-agent attempts.
The report pins Python 3.12.14, the executable, checker, harness, source and results.
Accepting originals and rejecting these two controls does not prove arbitrary
checker completeness, independent authorship, or a productivity improvement.

The six matched arms are `baseline`, `fast`, `lore05`, `lore06`,
`adaptive_no_reuse` and `adaptive_reuse`. The runner retains each repair and
interruption, stops at first independently correct patch, keeps sanitized feedback
separate from hidden checks, and charges initialization, warm-up, context,
investigation, agent attempts and verification. Separate sequences exercise task B
after task A, related/unrelated revisions and narrower grants. Reuse-disabled
comparison combines cache/investigation reuse; it is not a memory-only ablation.

**Actual six-task real-model pilot: unperformed. Full 30-task/six-arm study:
unperformed; zero of the required 180 initial model attempts are claimed.**
No coding-model version, weights/API revision or reasoning/tool policy was used
for an outcome study here. Task success, critical violations, end-to-end time,
tokens, dollars and paired confidence intervals remain unmeasured. The proposed
target is +10 percentage points in success, or 20% less time/cost with non-inferior
success and no increase in critical violations; neither target has been measured.

[Failure triage](FAILURE_TRIAGE.md) now binds proposals to failing samples,
exact pilot metrics, charged cost, independent reviews, captured audit and distinct
prepared holdout content. A plan, renamed fixture or unverified review cannot make
`algorithm_tuning_ready` true. No new shared adaptive runtime algorithm or prompt
is attributed to these unrun studies. Measured improvement requires a subsequent
matched intervention and independent holdout. Commands and study requirements are
in [real coding tasks](REAL_CODING_TASKS.md) and [the six-arm protocol](ADAPTIVE_TASKS.md).

## #37 — Human first contribution and transfer

The [human protocol](HUMAN_ONBOARDING.md) prepares 12 counterbalanced slots,
with an original-source and `lore onboard` condition on different unfamiliar
projects. Each round includes first contribution and a distinct transfer task.
The public payments/refunds and releases/withdrawals exercises debug exceptions
that make copying the first solution incorrect; they are not held-out human tasks.

**Actual participants: 0 of 12; completed human contributions and transfers: 0.**
There are no actual consent, assistance, explanation or independent human-review
outcomes to score. Preparing pseudonyms or filling an agent-authored test record
cannot establish participation. Verified human count remains unavailable without
the separately bound participation audit.

The protocol retains study-bound opt-in consent, active time and pauses, hints,
mentor/AI assistance, exact submitted patches, independent checks and blinded
review captures where feasible. Participant work and hidden checks stay outside
Lore's navigation cache. It reports first-correct time, transfer pass, missed
exceptions and explanation quality separately. These effects and their uncertainty
remain unmeasured. The existing direct `lore onboard --task ...` flow and optional
tutorial remain available; no observed-human UX improvement is claimed.

## #38 — Guardian regressions and failed full captures

The [60-transition corpus](corpora/guardian/events.json) contains 20 consequential,
20 benign and 20 ambiguous events across three synthetic projects. It explicitly
labels scope, current/history status, alert necessity and a safe next action.
Sources, extraction records and labels were authored together; they have no
independent human labeling or held-out status. The harness actually ran named
baseline saves, changes and guard commands on successive retained snapshots.

The [capture summary](results/guardian-debug-2026-10-10/summary.json) and
[retained archive](results/guardian-debug-2026-10-10/captures.zip) contain **140
event captures**, including both full cohorts and two targeted ten-event runs.

| Capture | Events | No documented change | Partial static guidance | Events failing source hashes | Engineering capture pass |
| --- | ---: | ---: | ---: | ---: | --- |
| Full baseline | 60 | 20 | 40 | 32 | **False** |
| Full candidate | 60 | 17 | 43 | 15 | **False** |
| Payments baseline segment | 10 | 5 | 5 | 2 | **False** |
| Payments candidate segment | 10 | 4 | 6 | 0 | True, for this segment only |

Deleted source paths reappeared with earlier bytes during read-command intervals.
One short run retained an intermediate `docs/.rsync-tmp/history.md` before the
original path returned. No-Lore deletion controls did not reproduce restoration.
**The writer was not independently identified.** The
[exact staged differences](results/guardian-debug-2026-10-10/source-integrity-failures.json)
remain failures; no source check was removed or relabeled as passing. Unchanged
queried database/configuration bytes do not cure failed source integrity.

The A-to-B-to-A regression is narrower: old checkpoints used the accumulated
historical evidence set, hiding a restored current exception. Candidate checkpoints
record current support separately while retaining complete history and old checksum
compatibility. The clean targeted event comparison changes from zero to one
included change group; the candidate's ten-event segment passes mechanical checks.
Other focused tests cover material accepted/scope priority, overlapping alert
grouping, preserved citations, and distinct or negated action text. See the
[failure log and focused test counts](results/guardian-debug-2026-10-10/README.md).

All four captures made zero model calls. A `partial_static_guidance` result is
an unavailable model assessment, not a rated alert or a successful quiet guardian.
High-severity precision, consequential recall, actionability and developer burden
are `null`/unmeasured; reviewed required-event coverage is zero. The proposed
90% precision, 85% recall, no benign high-severity alerts and one advisory per
material change targets have not passed. On-demand operation, named baselines
and existing grant ceilings remain; no watcher or repository execution was added.

The [publication content audit](results/guardian-debug-2026-10-10/publication-audit.json)
checks the exact retained archive and sidecars. Its 150 regular UTF-8 JSON members
contain the synthetic captures and protocol metadata; all captured source passages
bind to the authored fixture. An implementation agent performed the audit and a
separate AI engineering reviewer inspected and reran it. Neither role is a human
outcome reviewer. The [read-only verifier](audit_guardian_publication.py) reproduces
the fixture, hashes, evidence bindings and content classification without extracting
the archive or invoking Lore. The two deletion-control records retain opaque file
hashes without their original preimages; they contain no source text or command
output. This publication audit does not establish runtime execution/egress behavior,
identify the source writer, or change the failed product results above.

## Reproduce and complete the missing evidence

Run from a fresh checkout with supported Rust and Python toolchains and new output
directories. Exact frozen retrieval replay performs no update or provider call:

```sh
python3 evaluation/restore_zoom_fixture.py --output /tmp/lore-07-frozen
LORE_ZOOM_COMPARISON_CONFIG=/tmp/lore-07-frozen/lore.yml \
LORE_ZOOM_COMPARISON_CASES=/tmp/lore-07-frozen/cases.json \
LORE_ZOOM_COMPARISON_OUTPUT=/tmp/lore-07-replay.json \
  cargo test --locked --test knowledge_zoom_comparison \
  measured_existing_compiled_project -- --ignored --exact
LORE_ZOOM_REVIEWED_OUTPUT=/tmp/lore-07-reviewed.json \
  cargo test --locked --test knowledge_zoom_comparison \
  source_corpus::measured_source_pinned_three_repository_cases -- --exact
python3 evaluation/real_coding_corpus.py prepare --pilot --output /tmp/lore-07-pilot
python3 evaluation/real_coding_corpus.py validate --output /tmp/lore-07-controls
python3 evaluation/adaptive_sequences.py prepare --output /tmp/lore-07-sequences
python3 evaluation/human_onboarding.py prepare --output /tmp/lore-07-human
python3 evaluation/guardian_longitudinal.py prepare --output /tmp/lore-07-guardian
python3 evaluation/audit_guardian_publication.py
```

Checker validation intentionally executes candidate Python code. The
[coding guide](REAL_CODING_TASKS.md), [actual model-run commands](ADAPTIVE_TASKS.md),
[human run/record/assess guide](HUMAN_ONBOARDING.md) and
[guardian capture/review/assess guide](GUARDIAN_LONGITUDINAL.md) document the
additional inputs and grants. Preparation alone runs no outcome experiment.
Independent authors, source reviewers, pinned actual models, isolated agent/checker
environments and bound review captures are still required for those studies.

This environment denied `ptrace`, so a syscall execution/egress audit was
**unavailable**, not passed. Permissions were not escalated and traces were not
fabricated. Harness assertions and subprocess grant flags do not authenticate
actual external execution or enforce an operating-system sandbox. Preserve unknown
provider usage and billing as `null`; zero is supported only for captured zero-call
controls. No productivity, human-transfer or guardian-quality confidence interval
can be computed from missing outcomes.

## Artifact identities and build limits

These SHA-256 values bind the retained evidence bytes, not a broader success claim.
The [provenance record](results/knowledge-zoom-07-provenance-2026-10-10.json) also
pins comparator/helper and production source digests. Reviewed retrieval used
selector commit `6798ddd2b6bb165d3902e95ec2f514fa1879872c`; frozen replay used
`15bd446c4f67f24b0d8bc1acb02f1e77e2ed3149`. Reports accurately retain package version
0.6.0 because measurements preceded the release-only version bump. The reviewed
capture preceded the two malformed coverage/cache rejection fixes; the frozen
replay includes them. A later portability correction canonicalizes the test's
new temporary cache root before the timed operation so macOS's `/var` alias does
not trigger the production symlink rejection. It changes test setup only, leaves
the source and token scorer intact, and does not rewrite these measured reports.

| Artifact | SHA-256 |
| --- | --- |
| [Frozen baseline](results/knowledge-zoom-07-baseline-frozen-2026-10-10.json) | `b20ae1a20442c445d11a3737b6c659cba62e369769a97698fc5c756eb5e5d534` |
| [Frozen candidate](results/knowledge-zoom-07-after-frozen-2026-10-10.json) | `ce6e0676eda0374457364afdee013a3c195fc32db9a579224d11f18192ff96c4` |
| [Reviewed retrieval](results/knowledge-zoom-07-reviewed-2026-10-10.json) | `4def29ab7ce61e6355e6ef9ee106b7ded34f00103fd205ddf934dbae15959e71` |
| [New synthetic retrieval](results/knowledge-zoom-07-synthetic-2026-10-10.json) | `45e848ab8bfb9d8a0adff30d0d16f6c0396fada97d1d4d6055538afb1a7cc05a` |
| [Retrieval provenance](results/knowledge-zoom-07-provenance-2026-10-10.json) | `c4929578869d9fafb0562747e0ba7541675e12225d04f8dfba01032276bc0e3b` |
| [Reviewed source manifest](corpora/zoom-reviewed-v1/manifest.json) | `8ea3a80daa703deced518119261810a215cb295826c2279865aa06a079bc0088` |
| [Separate AI source review](corpora/zoom-reviewed-v1/review.json) | `ca5447bc08acfd06b9171dd65ee16948b031f2fbca3b62b1bf38d60b99b79779` |
| [Coding candidate manifest](corpora/coding-real-v1/manifest.json) | `81982c50ecd835d63f23f17ada2f0d24c0f64763ac83ac8b182939a9d29cd4f2` |
| [Executed coding controls](results/coding-controls-2026-10-10.json) | `36f2441bf3d4fb76dd792ddcfeeaff8d840a4684b5c3b1e13724ff49bcda521a` |
| [Guardian event corpus](corpora/guardian/events.json) | `462a2e5b6d95fd4dbac8f1640274e74977c4d80dd0a3be2341c05a3cb1a82461` |
| [Guardian summary](results/guardian-debug-2026-10-10/summary.json) | `d6ba68605d684b69787c0e848533b43945cb6b3799481e89a19b66c4dcf32422` |
| [Guardian source failures](results/guardian-debug-2026-10-10/source-integrity-failures.json) | `2ed15700e8ee56a966997f912195c129029c51304859a4deddd053ce221e64b6` |
| [Guardian 140-capture archive](results/guardian-debug-2026-10-10/captures.zip) | `79abc6eb8809010d46b2dc4ed9d3e00320b4adff3a804f896515f5d10cac06cf` |

Guardian captures pin baseline binary `fbfc52db67252f50952248a0b60ac0dbe77756f48c363d01433bee7520d40f3b`
and candidate binary `5022a2efb31662d639c703870c783e7d4b633d1757d4b6c940da733c84a5b90b`.
The candidate capture preceded the final distinct/negated-advice deduplication
regression; provider-disabled captures do not measure model advisory text quality.
The archive retains responses, source/registry captures, failures and run digests;
it contains no executable binaries or SQLite snapshots. Its integrity as an archive
does not convert its failed source-integrity observations into successful runs.
| [Publication-content audit](results/guardian-debug-2026-10-10/publication-audit.json) | `c27517499bba32eb45ab4d4c0f78a78e547bf015e3b1dfb1fde89bb7b3af2e1f` |
| [Publication verifier](audit_guardian_publication.py) | `45fa47ad014015ac2fa84c07310d0f4e4cb287f65a4f7829cc0586e06d1ecfa4` |
| [Integrated engineering receipt](results/release-07-engineering-2026-10-10.json) | `56c92041c784da06797b68298dd8caea2a6e304cb72bdac5e99b7922b06f5045` |
