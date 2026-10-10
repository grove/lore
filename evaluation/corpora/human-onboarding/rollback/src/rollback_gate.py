"""A valid signature does not restore a withdrawn deployment authorization."""

def may_rollback(environment, has_valid_signature, is_fixture, authorization):
    """Return whether the requested rollback satisfies current authorization."""
    raise NotImplementedError("Implement rollback authorization with scoped fixture handling")
