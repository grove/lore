# Lore 0.9 acceptance

Implementation is in progress from `3ba498d5ed71fc2e0051b6a2428430f484b55df5`
(Lore 0.8.0). No stable 0.9 release or productivity benefit is claimed.

The [plan](../docs/V09_IMPLEMENTATION_PLAN.md) separates engineering acceptance
from the genuine six-task/six-arm pilot required for stable release. The
[ledger](results/acceptance-09.json) records each gate without substituting
offline fixtures for model, human or independent audit evidence.

The existing public coding pilot is preparation input, not a decision-rich
held-out study. Models, independent authors/reviewers and authenticated external
isolation/egress evidence are missing. No hosted inference is authorized.
Human readiness has twelve planned slots and zero enrolled participants.
Guardian source preservation and independently scored alert quality are separate
gates. Historical 0.7/0.8 evidence remains unchanged.

Baseline formatting, strict Clippy and optimized build passed on macOS.
Python 3.14 passed 235 tests with one explicit skip. The sandboxed Rust run
failed binding a mock loopback listener; a host run and three-platform CI are
pending. Failed network/Python-version/sandbox attempts remain in the receipt.

Fresh schema 2/3 responses are byte-identical to retained 0.8 responses;
schemas 4/5/default differ only in investigation elapsed milliseconds.
The bootstrap schema-1 capture preserves its only source and makes no writes
or provider calls. See [baseline evidence](results/baseline-09/).
