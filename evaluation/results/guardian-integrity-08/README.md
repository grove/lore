# Guardian 0.8 source-writer diagnosis

The development workspace reproduced source restoration with **zero Lore
subprocesses**. The [baseline receipt](../baseline-08/guardian-08-no-lore-reproduction.json)
and its [complete control archive](../baseline-08/guardian-08-no-lore-control.zip)
retain sixty transitions and the independent inotify events. Forty events fail
the unchanged source-byte oracle. Two additional events expose metadata-only
source changes. A restored proposal passes through `.rsync-tmp` before a
same-inode rename into its original source path.

This positively isolates that reproduction outside Lore. Inotify cannot
identify the executable or PID; the observed naming pattern is consistent with
a synchronizer. The historical 0.7 writer's identity is not authenticated by this
new capture. No runtime source-restoration patch is attributed to Lore.

## Actual focused CLI capture

The [focused receipt](focused-local-receipt.json) binds ten actual payments
transitions through the frozen 0.7 binary, exact synthetic immutable history,
and the development per-command collector. The
[capture archive](focused-local-captures.zip) contains all seventy-one command
observations and the original responses. It excludes source workspaces,
databases, binaries and environment values.

The run made zero model calls. Five events failed source hashes; three failed
the additional command-boundary integrity check. All other original
mechanical checks passed. These failures are retained. The capture is useful
for diagnosis and does not establish clean replay or model advisory quality.

| Artifact | SHA-256 |
| --- | --- |
| `focused-local-captures.zip` | `8c6240dee57f93ff2479d22830ee0cbb97d3a913b9f715b1af6e064741cb863d` |
| Frozen local 0.7 binary | `5eccce5338fa7acd1c99ee03a7ded6bc292bb8f54046ca31ae37bd1ec0a11efa` |
| Prepared source history | `50c2a69bf9d8bc7ac26c799f99b4a0231ed8b905c73cf9c888eedbab15f91e89` |
| Original event corpus | `462a2e5b6d95fd4dbac8f1640274e74977c4d80dd0a3be2341c05a3cb1a82461` |

The archive's internal manifest binds every retained file. Captures retain the
collector's source hashes at invocation. The per-command validator was hardened
further before commit; the receipt explicitly records this scope. No failed
source observation was rewritten to match expectations.

## Required full-cohort gate

The dedicated [Ubuntu workflow](../../../.github/workflows/guardian-integrity.yml)
uses fresh runner temporary storage with frozen baseline and candidate binaries.
It requires two full sixty-event cohorts, another complete candidate replay,
the ten-event focused segment, and a sixty-event no-Lore control. Every command
must preserve sources, configuration and generated output as applicable; every
expected source observation must be complete. Unavailable kernel observation
or any failure fails that job.

An actual successful workflow receipt is required before marking this gate
passed. Local failures above remain failures after a supported runner passes.
The synthetic study does not score precision, recall, human benefit or real
provider advice, and does not replace an independent execution/egress audit.
