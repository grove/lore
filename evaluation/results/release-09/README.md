# Integrated RC engineering evidence

[Verification](verification.json) binds the full local checks to the integrated
`cf5a1022097ce0e07853b5e49ecea3c435304936` source. Subsequent changes in this release
PR contain evidence and documentation only; code object IDs permit exact reuse
without pretending that documents were executable changes. [Inventory](inventory.json)
binds retained logs and captures. Full logs record explicit ignored/skipped cases.
[Compatibility](compatibility.json) compares fresh optimized output against the
immutable 0.9 baseline. [Bootstrap](bootstrap-receipt.json) also verifies no writes.

Exact-head platform and Guardian results are the release PR's GitHub check records.
The workflows run again for main; inspect their exact commit, not another branch's
run. Do not merge with a pending/failed required check. Guardian artifacts are
retained by the existing workflow and remain separate from independent alert
quality. These local receipts are offline engineering controls, not model/human
outcomes. The [acceptance ledger](../acceptance-09.json) identifies external blockers.
