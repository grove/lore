# Artifact admission interface

`may_release(environment, has_valid_signature, is_fixture)` accepts the
environment strings `production` and `staging`. The other arguments are
booleans and the result is a boolean.

Production requires a valid signature even for fixtures. Signed artifacts may
be admitted to either known environment. An unsigned staging artifact may be
admitted only when it is explicitly marked as a fixture. Unknown environments
are rejected even when the artifact carries a signature. The recorded staging
exception and closed work item do not alter production's accepted constraint.
