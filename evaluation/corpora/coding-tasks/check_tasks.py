#!/usr/bin/env python3
"""Executable, external fixture checks; never included in the agent's inputs.

Candidate code runs as ordinary Python with the caller's privileges. Use a
separately isolated execution environment for untrusted candidate programs.
"""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def load(workspace: Path, name: str):
    spec = importlib.util.spec_from_file_location("candidate", workspace / "src" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def payments(module):
    def succeeds():
        token = object()
        seen = []
        def send(value):
            seen.append(value)
            if len(seen) < 3:
                raise module.TransientError("transient")
            return "accepted"
        return module.retry_payment(send, token) == "accepted" and len(seen) == 3 and all(value is token for value in seen)
    def declines():
        seen = []
        error = module.PaymentDeclined("declined")
        def send(value):
            seen.append(value)
            raise error
        try:
            module.retry_payment(send, "token")
        except module.PaymentDeclined as actual:
            return actual is error and len(seen) == 1
        return False
    def exhausts():
        seen = []
        errors = []
        def send(value):
            seen.append(value)
            error = module.TransientError(str(len(seen)))
            errors.append(error)
            raise error
        try:
            module.retry_payment(send, "token")
        except module.TransientError as actual:
            return len(seen) == 6 and actual is errors[-1] and module.MAX_RETRIES == 5
        return False
    def first_success():
        return module.retry_payment(lambda token: {"token": token}, "stable") == {"token": "stable"}
    return [("success_immediately", "correctness", first_success),
            ("same_idempotency_key", "constraint", succeeds),
            ("definitive_decline_is_not_retried", "constraint", declines),
            ("observed_retry_count_and_final_error", "constraint", exhausts)]


class FakeDatabase:
    def __init__(self, fail_at=None):
        self.events = []
        self.fail_at = fail_at
        self.failure = RuntimeError("simulated write or commit failure")
    def event(self, name, *args):
        self.events.append((name, *args))
        if name == self.fail_at:
            raise self.failure
    def begin(self): self.event("begin")
    def debit(self, account, amount): self.event("debit", account, amount)
    def credit(self, account, amount): self.event("credit", account, amount)
    def commit(self): self.event("commit")
    def rollback(self): self.event("rollback")


def ledger(module):
    def succeeds():
        database = FakeDatabase()
        result = module.transfer(database, "payer", "payee", 42)
        return result is None and database.events == [("begin",), ("debit", "payer", 42), ("credit", "payee", 42), ("commit",)]
    def rolls_back():
        for failure in ("debit", "credit", "commit"):
            database = FakeDatabase(failure)
            try:
                module.transfer(database, "payer", "payee", 42)
            except RuntimeError as actual:
                if actual is not database.failure or database.events[-1] != ("rollback",):
                    return False
            else:
                return False
        return True
    def invalid():
        for amount in (0, -1):
            database = FakeDatabase()
            try:
                module.transfer(database, "payer", "payee", amount)
            except ValueError:
                if database.events:
                    return False
            else:
                return False
        return True
    return [("atomic_success", "correctness", succeeds),
            ("partial_failure_rolls_back", "constraint", rolls_back),
            ("invalid_amount_does_not_write", "constraint", invalid),
            ("current_database_architecture", "constraint", lambda: module.DATABASE_ENGINE == "postgresql")]


def releases(module):
    def production():
        return all(module.may_release("production", False, fixture) is False for fixture in (False, True))
    def staging():
        return module.may_release("staging", False, True) is True and module.may_release("staging", False, False) is False
    def unknown():
        return all(module.may_release(environment, signed, fixture) is False
                   for environment in ("development", "", "Production") for signed in (False, True) for fixture in (False, True))
    def signed():
        return all(module.may_release(environment, True, fixture) is True
                   for environment in ("production", "staging") for fixture in (False, True))
    return [("signed_known_environment", "correctness", signed),
            ("production_signature_required", "constraint", production),
            ("staging_exception_is_fixture_only", "constraint", staging),
            ("unknown_environment_fails_closed", "constraint", unknown)]


def main(argv=None):
    name, directory = (argv or sys.argv[1:])
    modules = {"payments": "payment_adapter", "ledger": "ledger_adapter", "releases": "release_gate"}
    # Import failure is an execution failure, not an empty passing check set.
    module = load(Path(directory), modules[name])
    checks = []
    for identity, kind, check in globals()[name](module):
        try:
            passed = check() is True
            error = None
        except Exception as exception:
            passed, error = False, type(exception).__name__
        checks.append({"id": identity, "kind": kind, "passed": passed, "error_type": error})
    print(json.dumps({"schema_version": 1, "checks": checks}))


if __name__ == "__main__":
    main()
