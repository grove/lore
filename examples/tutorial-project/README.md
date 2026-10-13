# HarborPay — a fictional first-contact project

HarborPay is a **small, invented project for learning Lore**. It is not a payment service. Its Python code makes no network requests and handles no real payments.

## What to explore

- [ADR-001](docs/ADR-001.md) records the accepted charge retry rule.
- [RFC-002](docs/RFC-002.md) proposes a different limit, but has not been approved.
- [The runbook](docs/runbook.md) describes the documented operational procedure and its exceptions.
- [INC-014](docs/INC-014.md) is a *reported* historical incident, not an independently verified test result.
- [The sample implementation](src/policy.py) and [tests](tests/test_policy.py) show one possible implementation state. Lore does not execute this code.

**Learning question:** Does RFC-002 authorize five production charge attempts? Which historical problem makes blind refund retries dangerous?

## Run the sample tests separately from Lore

From this directory, with Python 3.10+:

```sh
python3 -m unittest discover -s tests -v
```

The tests use only Python's standard library. Running them is a separate action; a Lore inspection or suggestion does not mean these tests ran.

Return to the [static first-run guide](../../docs/tutorial/README.md) from your original Lore checkout. If you copied this directory elsewhere, follow that original guide for the supported CLI workflow.
