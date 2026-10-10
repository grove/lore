# Lore 0.8 evidence

The baseline is merged Lore 0.7.0 commit
`0ba8b17f7e63726a8d81def2b60af18573d4640c`. The attached implementation plan is
preserved verbatim in `docs/V08_IMPLEMENTATION_PLAN.md`.

`baseline-08/` retains the actual offline command receipt, complete output hashes,
source/permission/cost scenarios and frozen public-registry contracts. The
machine-readable `acceptance-08.json` records missing empirical gates explicitly.
Logs and source fixtures are engineering controls; they contain no real model
results or participant records.

Reproduce contract captures with a separately frozen binary from the baseline:

```sh
python3 evaluation/capture_contracts_08.py capture \
  --binary /absolute/path/to/baseline-lore \
  --source-revision 0ba8b17f7e63726a8d81def2b60af18573d4640c \
  --output /absolute/new-contract-capture
python3 evaluation/capture_contracts_08.py verify /absolute/new-contract-capture
```

Run the unchanged CI commands recorded in the implementation plan to regenerate
the engineering results. Exact elapsed times vary; archived response/log hashes
bind the bytes of a particular execution. Golden baseline responses preserve
their original schema and do not get rewritten when a future default changes.

The archived 0.7 Guardian failures remain in `guardian-debug-2026-10-10/`.
New command-level observations, full replay receipts and their precise limits
will be retained separately. A successful hash check verifies retained bytes,
not authorship, external sandboxing, product benefit or an unobserved source writer.
