# Lore 0.8 real coding study

The experiment_08.py wrapper prepares, seals, checks and launches the existing
adaptive_tasks.py runner. It adds no coding agent, scoring algorithm or model
simulation. Its six arms remain baseline, fast, lore05, lore06,
adaptive_no_reuse and adaptive_reuse.

**No real coding-productivity result is checked in.** The local readiness
receipt records **not_run**: coding/Lore models and their revisions were not
configured, independent task/checker authorship and gold reviews were not
supplied, and independently enforced isolation, provider availability and
authenticated external runtime/egress captures were unavailable. No hosted
inference or paid study was started.

## Prepare and pin an experiment

The public six-task subset is useful for development. It spans three pinned
upstream projects and remains recognizable as published candidate input, even
if an operator changes its labels. It cannot establish a held-out result.

~~~bash
python3 evaluation/real_coding_corpus.py prepare --pilot \
  --output /ABSOLUTE/PATH/pilot-inputs

python3 evaluation/experiment_08.py prepare \
  --cases /ABSOLUTE/PATH/pilot-inputs/cases.json \
  --output /ABSOLUTE/PATH/pilot-study

python3 evaluation/experiment_08.py preflight /ABSOLUTE/PATH/pilot-study
~~~

Preparation copies source snapshots and pins the external checker programs.
It writes incomplete EXPERIMENT.json and PREREGISTRATION.json records in a
private directory. Preparation, pins, seal, preflight and assess make zero
subprocess or provider calls. Checker programs are hashed, not executed
by preflight. Actual checker controls remain available through the
[real-source corpus tool](REAL_CODING_TASKS.md).

Configure EXPERIMENT.json before obtaining independent review:

| Field | Required input |
| --- | --- |
| mode | pilot for exactly six tasks; full for at least 30 distinct held-out tasks. |
| run.lore_binary | Absolute path to the built, executable, frozen Lore binary. |
| run.provider, run.model | Available Lore provider and exact model identity. Optional decision/embedding models and endpoints use the existing adaptive CLI options. |
| lore_revisions | Revision identifiers for every enabled Lore role. Revisions inside a provider need externally authenticated evidence. |
| run.agent_command | JSON argument array with an absolute executable; identical in every arm. The command and referenced files are pinned. No shell expansion is performed by the harness. |
| run.agent_id, run.agent_location | Declared coding model/tools/settings identity and local or hosted execution. |
| agent | Provider, model, revision, reasoning, endpoint and exact prompt artifact. A relative prompt_file is resolved within the private study directory. |
| run.seed | Fixed unsigned integer used by the existing matched-arm order. |
| run.max_attempts, run.attempt_budget_seconds | Identical repair and complete coding/checking-loop limits; the existing 1–5 attempt limit remains. |
| run.timeout, run.max_tokens | Per-command wall limit and whole Lore-context output budget. The context budget does not bound an arbitrary external agent's source input or tools. |
| run.allow_inspection, run.allow_hosted, run.allow_checkout_egress | Explicit grants; checkout egress needs both hosted egress and inspection. Ambient credentials do not provide consent. |
| study_wall_time_seconds | Bound for the entire study, including preparation, context, coding, verification and observed waits. |
| output_retention_days, publish_raw_outputs | Private retention plan, 1–365 days, with raw-output publication disabled. The harness does not silently delete expired research records. |
| pilot_bundle | For a full study, the separate completed, independently reviewed/audited real-model pilot whose source and task identities must not overlap the holdout. |

The bundled coding_agent.py adapter uses its INSTRUCTIONS string as the system
prompt. Save those exact UTF-8 bytes as the prompt artifact. Preflight checks
its known provider/model/endpoint/reasoning arguments, requires the study's
hosted grant for hosted adapter use, and rejects an adapter argument that
substitutes its own usage-ledger path. A different adapter needs the same exact
command pin and external model/tool/prompt enforcement evidence.

After configuring these fields, compute the pins without contacting a model:

~~~bash
python3 evaluation/experiment_08.py pins /ABSOLUTE/PATH/pilot-study
~~~

This returns the exact command digest, prompt digest, per-case generated Lore
configuration digests and enabled provider roles. Use them in the existing
preregistration record. It does not fill reviewer identities, claim a model is
available or mark a registration complete.

## Independent registration and external readiness

The shared static preregistration validator checks the same source, task,
checker, author, gold-review, model, tool and budget declarations used by
outcome assessment. The post-run validator **also** checks the models actually
reported by every attempt and the configurations actually retained in every
arm. Preflight contains no invented observations to satisfy those checks.

Each task needs disjoint task and checker authors and two independently
captured gold reviews excluding the operator and those authors. Keep their
hash-bound artifacts below gold-review-captures/. Reviews must predate the
registered time. Keep the standard outcome list, complete denominators,
2,000 matched-task bootstrap draws, 95% intervals, 10-percentage-point success
target and 20% time/cost target. The wrapper refuses exclusions or a substituted
analysis policy.

Seal the configured and independently reviewed contract:

~~~bash
python3 evaluation/experiment_08.py seal /ABSOLUTE/PATH/pilot-study
~~~

The seal binds prepared inputs, exact source/checker bytes, command and
executable hashes, collector source files, the complete preregistration,
models, generated configurations, grants, tools, seed and limits. It cannot be
overwritten by this command. Changing any bound input requires a separately
prepared and reviewed experiment; no force, skip-audit or
assume-model-available option exists.

Sealing creates an incomplete READINESS.json. Run the actual external readiness
controls in the intended isolated study environment, then supply:

1. Availability evidence for every enabled coding/Lore role, with the exact
   provider, endpoint, model, revision and reasoning declarations.
2. Independent OS enforcement of agent/checker separation, allowed tools,
   token and wall limits, egress allowlist, private outputs and active external
   runtime/egress capture. The code checks the declared method and every
   required condition; a Python environment dictionary is insufficient.
3. Three separate hash-bound capture categories:
   provider_availability, runtime_isolation and egress_enforcement.
4. An authentication method and a bound external authentication capture.
   Human or organizational identity authentication occurs in that external
   system; this Python harness checks retained byte bindings and accountable
   independent reviews. It cannot authenticate a person's identity from a
   name or generate an authenticated audit itself.
5. Two independent reviews of that complete readiness record, excluding the
   study operator and task/checker authors. Set each review's target_sha256 to
   the canonical digest of all readiness fields except reviews and
   qualification. The pins command prints readiness_review_target_sha256 after
   the record exists.

The readiness capture must precede its reviews and launch. Its declared
validity may span at most 24 hours. Provider availability is checked from this
bound, fresh external evidence; preflight does not send an inference request
merely to test credentials.

These gates are necessary launch conditions. Completed answer reviews and the
existing INTEGRITY_AUDIT.json still have to bind the actual run afterward.
A ready status establishes neither independent coding success nor a benefit.

The supported launch environment is a private POSIX directory and process
group, with independent external isolation. Windows preparation and inspection
remain available, but preflight reports unavailable private-directory
enforcement instead of treating POSIX permission bits as Windows ACL evidence.

## Execute, retain and assess

Launch only inside the externally enforced study environment described by the
readiness evidence:

~~~bash
python3 evaluation/experiment_08.py preflight /ABSOLUTE/PATH/pilot-study
python3 evaluation/experiment_08.py run /ABSOLUTE/PATH/pilot-study
python3 evaluation/experiment_08.py assess /ABSOLUTE/PATH/pilot-study
~~~

The run command recomputes every gate immediately before launching the existing
adaptive CLI. A failed preflight writes a not_run report and starts no child
process. The output location is always the bundle's fresh run/ directory. The
wrapper never resumes an aborted run, skips an unavailable arm or substitutes
a different model.

Launch arguments are constructed from the validated in-memory input snapshot.
A second complete check immediately before child creation rejects intervening
source, command, grant or readiness changes. Any exception after a child starts,
including receipt-write failure, stops and reaps its process group.

The original runner retains warm-up and served context, all proposed
implementations, sanitized predetermined repair feedback, checker provenance,
per-attempt provider ledgers, blinded answer-review packets and a pending
post-run integrity audit. Checker programs, hidden diagnostics and answer keys
are not added to the coding request or warm-up.

The wrapper adds:

| Artifact | Purpose |
| --- | --- |
| PREFLIGHT.json | All passing and failed launch gates, with no provider calls. |
| PREFLIGHT_AT_LAUNCH.json | Frozen passing preflight; execution also binds the exact seal and readiness bytes. |
| EXECUTION.json | Started checkpoint, sealed-contract identity, parent-measured elapsed time, child return code and terminal execution status. |
| RETENTION.json | Every planned case × arm, all started/complete/failed/refused/timed-out/cancelled/interrupted invocations, usage coverage and exact artifact hashes. |
| RESULT.json | The retained denominator plus the unchanged adaptive assessment and its separate independent-review/audit gates. |
| FAILURE_TRIAGE.json | Existing failure triage for an intact completed comparison. No speculative runtime change is generated. |

The pilot's denominator remains **36 initial assignments**. The full study's
denominator remains at least **180**, with repair invocations retained
separately. An unstarted assignment is distinguished from an attempted failure;
neither disappears because the process aborted. A first-attempt refusal has no
fabricated checker result. A successful earlier checked attempt cannot erase
a later started but incomplete invocation.

The wrapper gives a cancelled/timed-out runner a short interrupt window to
journal its current attempt, then terminates the process group. A hard kill or
host failure may leave started records; assess retains them as interrupted,
with unknown elapsed time or usage where no final observation exists.

Every completed arm retains full initialization charge, context-copy time,
warm-up, served context, all coding/repair/checker time and measured iteration
overhead through the existing cost assessor. Context and coding usage remain
separate. The whole-study clock additionally retains observed wait and failure
time. Incomplete all-in costs stay null.

Raw provider event IDs are deduplicated only when byte-identical initialization
ledgers were copied into the isolated arms. This physical request inventory
does not replace the intentional full preparation charge per comparison arm.
It is marked incomplete coverage and is not presented as a complete bill.
Unknown provider tokens and actual USD remain null; a provider error may still
have incurred charges.

Exit 0 means preparation/pins/sealing succeeded, preflight is ready, or a
mechanical comparison completed. Exit 2 means readiness/comparison is blocked
or incomplete. Exit 1 means malformed or tampered input. None of these exit
codes establishes a productivity benefit.

## Evidence-directed repair and the full holdout

Complete independent answer reviews (including earlier failed implementations)
and the actual post-run execution/egress audit, then rerun assessment. Triage
uses failure_triage.py and its existing change-proposal validation. Any runtime
change still needs exact failed sample IDs, the missed premise, expected
correction, a regression scenario, cost risk and untouched holdout. No real
pilot was available for this implementation, so no retrieval/controller
algorithm was tuned.

For the full study, prepare a separate independently authored and reviewed
cohort of at least 30 tasks. The wrapper validates the completed real pilot's
independent reviews and audit, then uses the existing content-based holdout
validator. Renamed task IDs, renamed source files or reused source bytes cannot
make pilot input into a holdout. Public candidates also remain ineligible.

Use the existing paired task differences and 95% bootstrap intervals. A small
pilot does not establish a general gain. The candidate target is +10 percentage
points in success, or at least 20% lower all-in time/cost with non-inferior
success and no increase in critical-constraint violations. The wrapper never
automatically turns a passing mechanical run or selected favorable statistic
into that empirical claim.

## Different-task revalidation

The additional public sequence runs the two existing adaptive policies through
task A, distinct related task B, a newly accepted contradictory ADR, an exact
same-size mutation five directories into the source tree, and reduced grants:

~~~bash
python3 evaluation/adaptive_sequences.py prepare \
  --cases evaluation/corpora/adaptive-sequences-08/cases.json \
  --output /ABSOLUTE/PATH/sequence-prepared

python3 evaluation/adaptive_sequences.py run \
  --cases evaluation/corpora/adaptive-sequences-08/cases.json \
  --lore-binary /ABSOLUTE/PATH/lore --provider ollama --model PINNED_AVAILABLE_MODEL \
  --allow-inspection --seed 7 --timeout 3600 --max-tokens 6000 \
  --output /ABSOLUTE/PATH/sequence-run
~~~

The same external provider/isolation/egress prerequisites apply before running
this context-only companion. Its CLI remains the existing explicit protocol,
not a way to claim completion of a coding study. Original policy bytes remain
present; the new ADR identifies its superseded source and replacement
statement. Assessment replays both changes and verifies all source/grant
bindings. The old 0.7 sequence phase contract remains supported.

Both arms receive identical phase sources and grants. Full initialization,
refresh, context and protocol-overhead time are charged. The report retains
the signed reuse-minus-no-reuse total elapsed-time difference, not an assumed
benefit from a cache hit. The no-cache option remains a combined
response/investigation control, not a pure investigative-memory ablation. These
source-level controls do not authenticate model understanding of a
contradiction; independent review is still necessary.

## Offline gate

~~~bash
python3 -m unittest discover -s evaluation/tests -p test_experiment_08.py -v
python3 -m unittest discover -s evaluation/tests -p test_adaptive_sequences.py -v
python3 -m unittest discover -s evaluation/tests -v
~~~

Tests label all fake model/audit/reviewer material as offline protocol doubles.
They also exercise actual failed subprocesses and real process-group timeout
cleanup. The checked-in not-run receipt contains no fabricated model outcome,
provider bill, human review or sandbox claim.
