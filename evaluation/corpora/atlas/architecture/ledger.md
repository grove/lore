# Ledger service architecture

## Persistence

The ledger service writes payment transactions to MySQL in the production environment. Reads that require transactional consistency go to the primary database rather than an eventually consistent analytics replica. This page describes the documented design and does not independently verify a running production deployment.

## Boundaries

The analytics warehouse is a separate subsystem. A change to reporting technology does not change the payment ledger's selected database.
