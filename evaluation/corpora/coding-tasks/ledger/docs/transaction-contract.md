# Transaction adapter contract

The existing PostgreSQL adapter exposes `begin()`, `debit(account, amount)`,
`credit(account, amount)`, `commit()` and `rollback()`. A transfer must debit
and credit inside one transaction. It must roll back and propagate the
original exception if either write or commit fails after the transaction
has begun.

Reject amounts less than or equal to zero with `ValueError` before beginning
the transaction or touching either account. Return `None` after a successful
commit. This task uses the existing adapter, without introducing a second
database backend. The replacement ADR is the current architectural intent;
the old MySQL memory describes the earlier implementation.
