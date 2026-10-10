# Accepted rollback contract

A rollback is a deployment. Its artifact must still have an active deployment
authorization. A cryptographically valid signature establishes origin; it does
not make a withdrawn or pending authorization active.

For production, an active authorization and a valid signature are both required.
For staging, the same rule applies except that a staging fixture may omit its
signature while its authorization is active. Withdrawn, pending and unknown
authorizations are denied in every environment. Unknown environments are denied.

The previous exercise's staging fixture exception concerned signatures. It did
not exempt an artifact from a later security withdrawal. A proposal to let an
operator override a withdrawal is under discussion and has not been accepted.
