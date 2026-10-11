# 0.9 evidence

[Acceptance ledger](acceptance-09.json) distinguishes passed, failed, not_run,
unmeasured and blocked_external. [Baseline captures](baseline-09/) retain fresh
0.8 controls and exact source/command identities. These are engineering evidence;
they do not establish real-model efficacy or human benefit.

Use Python 3.10 or newer (this run uses 3.14); pinned upstream Python sources
contain structural pattern matching. Preserve failed attempts when correcting
environment prerequisites. Reproduce contract captures with
`evaluation/capture_contracts_08.py`; use fresh output directories.
