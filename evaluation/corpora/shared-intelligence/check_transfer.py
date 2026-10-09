#!/usr/bin/env python3
"""External behavioral checks for the distinct refund transfer task.

Never include this file in a learner or coding-agent input snapshot. Candidate
code runs with the invoking process's privileges, as in check_tasks.py; these
checks are not a sandbox and do not contact any payment service.
"""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def load(workspace: Path):
    spec = importlib.util.spec_from_file_location("refund_candidate", workspace / "src" / "refund_adapter.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def checks_for(module):
    def confirmed_submission():
        token, receipt, events = object(), object(), []
        def send(actual):
            events.append(("send", actual))
            return receipt
        def lookup(actual):
            events.append(("lookup", actual))
        return module.refund_or_reconcile(send, lookup, token) is receipt and events == [("send", token)]

    def lookup_receipt():
        for receipt in (object(), False, 0, "", {}):
            token, events = object(), []
            def send(actual):
                events.append(("send", actual))
                raise module.TransientError("Unknown result")
            def lookup(actual):
                events.append(("lookup", actual))
                return receipt
            if module.refund_or_reconcile(send, lookup, token) is not receipt or events != [("send", token), ("lookup", token)]:
                return False
        return True

    def unresolved_outcome():
        token, events = object(), []
        def send(actual):
            events.append(("send", actual))
            raise module.TransientError("Unknown result")
        def lookup(actual):
            events.append(("lookup", actual))
            return None
        try:
            module.refund_or_reconcile(send, lookup, token)
        except module.ReconciliationRequired as error:
            return error.request_token is token and events == [("send", token), ("lookup", token)]
        return False

    def definitive_error():
        for error in (module.RefundDeclined("Declined"), ValueError("Malformed submission")):
            token, events = object(), []
            def send(actual):
                events.append(("send", actual))
                raise error
            def lookup(actual):
                events.append(("lookup", actual))
            try:
                module.refund_or_reconcile(send, lookup, token)
            except Exception as actual:
                if actual is not error or events != [("send", token)]:
                    return False
            else:
                return False
        return True

    def lookup_failure():
        token, events, error = object(), [], RuntimeError("Status service unavailable")
        def send(actual):
            events.append(("send", actual))
            raise module.TransientError("Unknown result")
        def lookup(actual):
            events.append(("lookup", actual))
            raise error
        try:
            module.refund_or_reconcile(send, lookup, token)
        except RuntimeError as actual:
            return actual is error and events == [("send", token), ("lookup", token)]
        return False

    return [("confirmed_submission_returns_original_receipt", "correctness", confirmed_submission),
            ("ambiguous_submission_looks_up_once_without_resending", "constraint", lookup_receipt),
            ("unknown_outcome_requires_reconciliation", "constraint", unresolved_outcome),
            ("definitive_errors_propagate_without_lookup", "constraint", definitive_error),
            ("unavailable_status_is_not_negative_evidence", "constraint", lookup_failure)]


def main(argv=None):
    module = load(Path((argv or sys.argv[1:])[0]))
    results = []
    for identity, kind, check in checks_for(module):
        try:
            passed, error = check() is True, None
        except Exception as exception:
            passed, error = False, type(exception).__name__
        results.append({"id": identity, "kind": kind, "passed": passed, "error_type": error})
    print(json.dumps({"schema_version": 1, "checks": results}))


if __name__ == "__main__":
    main()
