"""Posting adapter for the project's accepted database architecture."""

DATABASE_ENGINE = "postgresql"


def transfer(database, debit_account, credit_account, amount):
    """Atomically debit one account and credit the other; return None."""
    raise NotImplementedError("Implement the transfer")
