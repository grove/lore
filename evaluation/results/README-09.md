# 0.9 evidence

[Acceptance ledger](acceptance-09.json) distinguishes passed, failed, not_run,
unmeasured and blocked_external. [Baseline captures](baseline-09/) retain fresh
0.8 controls and exact source/command identities. These are engineering evidence;
they do not establish real-model efficacy or human benefit.

Use Python 3.10 or newer (this run uses 3.14); pinned upstream Python sources
contain structural pattern matching. Preserve failed attempts when correcting
environment prerequisites. Reproduce contract captures with
`evaluation/capture_contracts_08.py`; use fresh output directories.

[Integrated RC captures](release-09/README.md) contain full local Rust/Python
results, exact code identities, optimized CLI smoke, frozen schemas and bootstrap
comparison. [Decision controls](decision-work-09/controls.json) and
[zero-enrollment human readiness](human-onboarding-09/README.md) remain explicitly
separate from product efficacy. Final CI/merge identities live on the eight linked
pull requests and their exact-head check records, avoiding circular commit hashes
inside artifacts committed to those same hashes.
