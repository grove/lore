# Human first-contribution and transfer pilot

`human_onboarding.py` prepares, checks readiness for, runs, records and assesses an opt-in human study
of `lore onboard`. The default preparation creates 12 counterbalanced **slots**
and zero enrolled participants. It runs no models and creates no human outcomes.
Real participants, consent, contributions, assistance ledgers, independent
reviews and an accountable human-participation audit must be collected before
any measured human result is reported.

The 0.8 release has **zero real participants**, no completed human sessions and
no independent human assessments. First-correct contribution time and independent
learning transfer are **unmeasured**. The checked-in
[`human-onboarding-08-not-run.json`](evidence/human-onboarding-08-not-run.json)
records an actual zero-enrollment preparation and read-only readiness check.
Its twelve planned slots are not twelve people. Fixture tests below verify
mechanics with explicitly synthetic consent and review records; none are
included in the release's human-outcome denominator.

This protocol is separate from the product's normal task flow. It adds no
mandatory questionnaire, test, watcher or learning requirement to Lore. The
existing `lore onboard --task ...` bypass remains available. Product UX changes
must follow observed stumbling points; simulated learners do not justify
claims about real learning.

## Counterbalanced design

Each participant declares both selected projects unfamiliar and receives two
rounds: one project with original files and no Lore intervention, and a different
project with original files plus an actual `lore onboard` response. Each round
contains a first contribution followed by a different transfer task. Four
combinations of starting project and starting condition balance project, order
and intervention. A recorded seed shuffles the slot allocation. Twelve slots
provide three of each combination; a completed participant contributes four
task stages.

Transfer tests require adapting a principle to a new exception. They receive
no automatically supplied Lore or AI context, no solved first-task patch and
no first-task checker results. Every requested hint, mentor intervention or
external AI assistance must be logged. A participant may stop without passing;
that outcome remains failed, with null time to first correct completion.
Correctness alone does not establish an independent explanation or mastery.

The bundled development corpus contains two journeys:

| Journey | First contribution | Different transfer condition |
| --- | --- | --- |
| Payments | Preserve safe payment retries and a stable idempotency token. | A legacy refund provider does not deduplicate writes: an ambiguous result requires read-only reconciliation, never another submission. |
| Releases | Enforce production signatures and the scoped staging-fixture exception. | A signature or fixture origin cannot override withdrawn or missing current rollback authorization. |

The payment/refund fixture reuses the existing shared-intelligence sources and
external checkers. The new rollback fixture rejects copying the first task's
signature/fixture decision without checking active authorization. The unit
tests execute correct and known-wrong transfer implementations. Both journeys
are public synthetic debugging fixtures, not held-out projects or human data.
Supply independently selected unfamiliar projects via `--cases` for a real
pilot. Each schema-1 case has an exact source snapshot, task, permitted edits,
constraints and external checker; `learning_journeys` names distinct first and
transfer cases and the source-derived explanation criteria. Preparation pins
the checker/helper/executable bytes as well as source and manifest hashes.
Every load verifies the exact complete case set and task text against that
manifest, the prepared source/checker/eligibility record, and the counterbalanced
assignment against the original journeys and seed. Editing a task or a slot in
`study.json` does not create an eligible replacement study.
Preparation and every later load reject declared checker/helper paths within
any selected participant source root and exact checker/helper bytes copied into
any source snapshot. This catches a hidden checker renamed or copied into a
different task's source. Enforcing participants' access to other files remains
the operator's independently audited responsibility.

## Prepare without enrolling anyone

```bash
python3 evaluation/human_onboarding.py prepare \
  --participants 12 --seed 7 \
  --output evaluation-results/onboarding-pilot
```

Use a fresh output directory. `--participants` accepts 4–100 planned slots in
multiples of four; fewer than 12 completed participants cannot satisfy the
pilot outcome gate. The output contains `study.json`, source snapshots,
`CONSENT_TEMPLATE.json`, `WITHDRAWAL_TEMPLATE.json` and an incomplete `HUMAN_AUDIT.json`. The template's
false consent fields are intentional. They must not be filled by an agent
impersonating participants.

A participant record uses a random `p_` identifier followed by 16–32 hexadecimal
characters. It contains no name or email. The operator handles identity and
consent verification separately and retains only the documented fields: the
pseudonym, slot, study digest, actual consent time, human participation,
informed consent, withdrawal rights and unfamiliar-project declaration.
Consent must be explicit, study-bound and not future-dated. Extra personal
fields, missing acknowledgments and duplicate slot assignments are rejected.

The operator should explain the tasks, recording, optional assistance, retention
and withdrawal process before seeking consent. A refusal is not an enrollment.
The harness does not contact or recruit people, send invitations, or authenticate
their legal identity. Independent capture review verifies real participation;
multiple pseudonyms alone cannot prove multiple humans.

## Check readiness without starting a session

```bash
python3 evaluation/human_onboarding.py readiness \
  --study evaluation-results/onboarding-pilot
```

Readiness reads the pinned sources, tasks, checker program hashes, assignments
and any existing records. It enrolls nobody, publishes no task, executes no
checker and makes no inference call. It reports exact pending requirements:
independently selected unfamiliar held-out projects, twelve actual consenting
people, four completed stages per participant, two independent reviews for
every exact submission and explanation, and an independent audit of participation,
access boundaries, assistance, timing and current withdrawal state.

To check a particular next session, add the same `--consent`, `--round`,
`--stage`, binary/model and grant arguments that you intend to pass to `run`.
All three session identifiers are required together. The previous stage must
have a valid bound submission, cumulative ledger and executable checker record
before the next task is exposed. A lone `stage_complete: true` field is insufficient.
Readiness reports a specific blocker without revealing the next task's identity
when that sequencing gate fails.

A session's `mechanically_ready: true` verifies local inputs and declarations.
Provider availability remains `not_checked`; identity and operator-enforced
isolation still need independent verification. A mechanically ready session
does not imply that the human study has run or that an outcome is measured.

## Start a contribution or transfer stage

For the assigned `without_lore` stage, the source and task are enough:

```bash
python3 evaluation/human_onboarding.py run \
  --study evaluation-results/onboarding-pilot \
  --consent /absolute/path/actual-consent.json \
  --round 1 --stage contribution
```

For an assigned `with_onboard` contribution, also supply the actual Lore binary,
available pinned provider/model and any intended grants:

```bash
python3 evaluation/human_onboarding.py run \
  --study evaluation-results/onboarding-pilot \
  --consent /absolute/path/actual-consent.json \
  --round 1 --stage contribution \
  --lore-binary target/release/lore --provider ollama --model YOUR_LORE_MODEL \
  --model-revision YOUR_EXACT_PROVIDER_MODEL_REVISION \
  --allow-inspection --max-tokens 6000 --timeout 3600
```

Use the round and condition assigned in `study.json`. Round 2 cannot begin
until round 1's transfer finishes. A transfer stage cannot begin until that
round's contribution finishes. Starting the same stage twice is rejected;
record another bounded attempt in the existing workspace instead.
The onboarding session retains the actual binary hash, provider/model, declared
model revision, complete configuration digest and captured initialization
response. The declaration is a reproducibility pin, not independent evidence
that a provider actually served that revision.

The command reports the participant's `workspace` and `TASK.json` paths. For
the onboarding contribution, `ONBOARD.json` holds the actual response.
Initialization and orientation happen in a separate `orientation-project`;
participant explanations and patches are never written into its navigation or
investigation cache. The participant's source workspace contains no Lore cache,
checker code or transfer answer keys. The operator must enforce which study
files and tools the participant can access: separate directories are not an
operating-system sandbox. The two intervention conditions must have comparable
editor, terminal, documentation and optional assistance access.

The existing explicit hosted/inspection/checkout options apply. Checkout egress
requires both hosted and inspection grants. Ambient Lore grants are removed
before creating each subprocess's environment. Configuration cannot widen the
selected grants. Do not interpret these environment variables as filesystem or
network enforcement on external programs.

An exclusive per-slot operation marker prevents overlapping stage starts or
submissions. Normal errors and interruptions release it; after an uncatchable
process exit, the operator must verify that the old process has stopped before
removing the stale marker. An interrupted initialization publishes no completed
session. An interrupted checker publishes no completed submission. Assess the
retained state before retrying; interruption never implies completion.

## Withdraw consent and remove managed participant data

Fill `WITHDRAWAL_TEMPLATE.json` only from an actual participant's request. It
requires the pseudonym, original `study_sha256`, exact original `consent_sha256`,
actual `withdrawn_at` timestamp and `requested_by_participant: true`. The consent
digest is the canonical JSON digest used by the session/readiness record;
it is not the hash of a whitespace-formatted consent file. Do not add a name,
reason for leaving or other personal information.

```bash
python3 evaluation/human_onboarding.py withdraw \
  --study evaluation-results/onboarding-pilot \
  --request /absolute/path/actual-withdrawal-request.json
```

The command atomically records a minimal study/consent/pseudonym-bound revocation
marker outside the participant tree, then deletes the entire managed
`participants/<pseudonym>` tree: retained consent, authored workspaces, onboarding
copies/caches, sessions, patches, explanations, ledgers, reviews and captures
stored there. Original project snapshots are unaffected. Keep participant-specific
review captures in that managed tree so withdrawal covers them. The retained
marker contains no contribution, explanation or score; retaining it is necessary
to reject replay of old consent and to exclude the pseudonym from later outcomes.
Explain this limited retention before consent.

Withdrawal remains available if a project source or checker pin has changed.
If deletion fails, consent is already revoked and assessment excludes the data;
retry the same request to finish cleanup. Repeating a completed request is safe.
Every stage/submission checks active consent before and after slow operations and
before returning participant artifacts. A child operation already in flight may
finish its current work, but cannot publish a new retained result: after it returns,
the runner removes any managed files recreated after withdrawal. Withdrawal does
not claim to erase provider-side requests already sent.

Independent audit captures, exports, consent copies and backups outside the
managed participant tree require operator cleanup under the consent agreement.
The command does not silently assert that external copies were deleted. A new
withdrawal changes the assessment binding and invalidates the old participation
audit. No old consent or withdrawn pseudonym can start another stage or submission.

## Record an actual submission

Copy the stage's `RECORD_TEMPLATE.json` to a separate file and fill it from
observed human work. The ledger is cumulative across attempts and contains:

- Actual finish time and active seconds since the stage started.
- Ordered, nonoverlapping pause intervals. Active time must equal elapsed time
  minus pauses, within one second, and cannot exceed two hours per stage.
- Timestamped `hint`, `mentor` and `ai_assistance` events, each with an opaque
  reference. Do not include personal names or secret answer material.
- The participant's bounded factual explanation and an optional perceived
  clarity score from 1 to 5.
- `stage_complete`, which can close an unsuccessful stage, and an explicit
  acknowledgment that the response was human-authored.

Explanations are limited to 16 KB, the complete supplied record to 100 KB,
assistance/pause lists to 200 entries each, and stages to five submissions.
Later attempts cannot erase prior active time, pauses or assistance. No failed
submission is transformed into an unassisted success by dropping its ledger.

Checking deliberately executes candidate code through the declared external
checker in the operator's study environment. The required `--execute-checks`
flag makes that execution explicit. It does not run any command supplied by
the participant or inferred from project text.

```bash
python3 evaluation/human_onboarding.py record \
  --study evaluation-results/onboarding-pilot \
  --participant p_REPLACE_WITH_ACTUAL_RANDOM_HEX_ID \
  --round 1 --stage contribution \
  --record /absolute/path/actual-submission-ledger.json \
  --execute-checks
```

The placeholder above is deliberately not a valid enrolled ID. Use the actual
consented pseudonym. `record` validates the session's study, round, condition,
task, source and consent before executing checks. It copies the actual authored
workspace to `submissions/<attempt>` and executes the hidden checker against that
exact retained copy. It binds the checker evidence and timing in `records/`, and a
separate review packet in `reviews/`. Checkers must return a nonempty set of
unique, boolean correctness/constraint checks with stable coverage. Their
program/helper hashes must match the bytes frozen during preparation. The
runner rejects unauthorized edits and checker-induced source writes. A checker
that changes its submitted copy or is interrupted cannot turn its own edits
into the participant's authored work or a passing result.

`submitted_patch_sha256` is the canonical JSON digest of the original entire
source manifest hash plus the exact full-file SHA-256 values for changed allowed
files. It is a manifest-defined patch digest, not a text `git diff` hash. Assessment
recomputes it from the retained submitted snapshot and validates checker argv
against that exact snapshot path. Earlier artifacts lacking these bindings must
be independently rechecked from the real retained submission before assessment;
do not fill missing checker evidence or new audit attestations from templates.

The stage ends at the first independently passing patch, an explicit stop or
the fifth attempt. A failed task has null `first_correct_active_seconds`.
Onboarding initialization/context time and checker time are retained separately;
active human time starts when the stage is exposed. It must not be relabeled as
end-to-end system time or billed study cost. This protocol does not measure
recruitment, independent-review labor or the cost of creating upstream sources.

## Independent explanation and patch review

Every retained submission needs two blinded reviews where feasible, including
earlier failures. Review packets bind the exact submission record digest. Each
`independent_reviews` entry identifies a reviewer and timestamp, explicitly
declares independence and no conflicts, confirms blinding, and binds a distinct
retained capture by relative path and SHA-256. A participant cannot review their
own work. Captures live within the study and are bounded to 10 MB. The operator
keeps the arm mapping out of reviewer-facing material where practicable.

Each item in `scores` must identify the same reviewer and target digest as its
evidence. It records `factual_explanation_0_to_3`, explicit `missed_exceptions`
chosen from the declared constraints, and an independent `patch_correct`
judgment. Anonymous parallel arrays, duplicate reviewers, substituted targets,
reused identical captures and unbound test booleans are rejected. These are
accountable review artifacts; the tool does not authenticate human identities
or independently decide whether a reviewer actually remained blind.

Complete `HUMAN_AUDIT.json` only after independently verifying actual consent
and human participation. Its capture must bind both `study_sha256` and the
current `participant_records_sha256` reported by assessment. That second digest
includes retained active consent, source work, submissions, ledgers and review
packets together with the current revocation markers; changing any of those
artifacts invalidates the audit. The audit also requires explicit verified access
boundaries, assistance/timing and withdrawal state. The audit remains incomplete
until a real reviewer supplies evidence. Never fill it from offline test doubles.

## Assess outcomes without inference or code execution

```bash
python3 evaluation/human_onboarding.py assess evaluation-results/onboarding-pilot
```

Assessment rechecks source, task, checker, consent, session, submission, review,
assistance and timing bindings. It reports prepared slots, enrollment records,
completed participants and independently verified human count separately.
Without the audit, verified human count is null. Public fixtures remain fixtures
even if their input labels are changed.

The per-participant data include independent check completion, active time,
first-correct active time, attempts, hints, mentor interventions, AI assistance,
factual explanation, missed exceptions and secondary perceived clarity. Paired
outcomes compare the two counterbalanced conditions within each participant,
separately for first contribution and transfer, using seeded 95% paired
bootstrap intervals. The sampling unit is the participant, not individual
check results. Failed stages remain failures; missing explanation reviews remain
unknown. `independent_transfer_success` stays null until all attempts in that
transfer have bound independent reviews. It is true only for an independently
passing patch with no logged hint, mentor or AI assistance, and both final
reviewers judging the patch correct, awarding full factual explanation credit
and recording no missed exception. This narrow operational outcome does not
establish lasting mastery. Assisted correct patches remain visible as correctness
results but cannot become unassisted transfer successes.

Withdrawn participants are reported separately and excluded from enrollment,
completion, paired outcomes and human-benefit denominators. Assessment fails
closed if withdrawal changes while it is being computed; repeat it before
publishing a result. With no enrollment or withdrawal history, `phase` is
`not_run`; verified human count remains null until an independent audit exists.

At least 12 completed real consenting participants, distinct unfamiliar held-out
projects, intact artifacts, all independent reviews and a bound participation
audit are required for `independently_reviewed_counterbalanced_pilot` status.
The bundled fixtures cannot earn that status. Twelve people provide a pilot,
not definitive generalization across developers or projects.
`learning_benefit_established` remains false; report effect sizes, uncertainty,
missing data, order/project limitations and counterexamples before making any
benefit claim. If contribution time or less-assisted transfer does not improve,
retain that result and use its specific stumbling points to guide UX changes.

The checked-in tests exercise the consent, sequencing, replay, checker,
review-binding and different-exception contracts without real participants or
model inference:

```bash
python3 -m unittest discover -s evaluation/tests -p test_human_onboarding.py -v
```
