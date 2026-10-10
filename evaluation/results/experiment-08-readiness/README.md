# Lore 0.8 coding-study readiness

**Status: not_run. Coding productivity: unmeasured.**

The actual run command stopped at preflight with exit code 2. The public pilot
contains six pinned development tasks across three upstream projects and six
arms, giving 36 planned initial assignments. It produced zero coding attempts,
zero model/provider calls and zero launched study subprocesses. These are
unstarted assignments, not measured coding failures.

Configured coding/Lore models, immutable model revisions and a real agent
command were missing. Independent task/checker authorship and the two gold
reviews per task were pending. No sealed contract or externally authenticated
provider-availability, runtime-isolation or egress-enforcement readiness record
was supplied. Preflight retained these blockers; it did not substitute a model,
reviewer, audit or availability claim.

## Captures and bindings

[not-run.json](not-run.json) records the exact command, exit code, all blockers,
collector file hashes and the capture archive's SHA-256. The receipt also
distinguishes full-suite verification from the final focused regression run.

[preflight-captures.zip](preflight-captures.zip) contains the actual PREFLIGHT,
incomplete EXPERIMENT and PREREGISTRATION, prepared input metadata, public pilot
case manifest and an internal SHA-256 manifest. It contains no model responses,
checker program contents, submitted implementations or invented independent
review captures. Absolute paths preserve the capture environment's identity;
this archive is evidence, not a relocatable sealed study.

The full Python suite passed 204 tests after the launch/cohort review fixes.
The final wrapper suite passed 16 tests after adding the strict input-schema
privacy regression. These are engineering controls using explicitly labeled
offline doubles and real local cleanup subprocesses where appropriate. They
are not a coding-model experiment or an independent empirical review.

## Reproduce the blocked preflight

Use fresh private output paths:

~~~bash
python3 evaluation/real_coding_corpus.py prepare --pilot \
  --output /ABSOLUTE/PATH/pilot-inputs
python3 evaluation/experiment_08.py prepare \
  --cases /ABSOLUTE/PATH/pilot-inputs/cases.json \
  --output /ABSOLUTE/PATH/pilot-study
python3 evaluation/experiment_08.py run /ABSOLUTE/PATH/pilot-study
~~~

The final command must exit 2 until the prerequisites are actually supplied.
Its report retains missing prerequisites and makes no provider calls. Follow
[EXPERIMENT_08.md](../../EXPERIMENT_08.md) to configure, independently review,
seal and externally enforce the first genuine study. The public pilot remains
a development candidate; a separate untouched, reviewed holdout is required
for the full study. This not-run artifact justifies no retrieval or controller
tuning and no productivity-benefit claim.
