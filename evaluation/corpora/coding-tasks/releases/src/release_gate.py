"""Artifact admission policy shared by production and staging."""


def may_release(environment, has_valid_signature, is_fixture):
    """Return a boolean admission decision for a single artifact."""
    raise NotImplementedError("Implement admission policy")
