"""Legacy refund transport: a correlation token is not an idempotency key."""


class TransientError(Exception):
    """The submission outcome is unknown after a transport failure."""


class RefundDeclined(Exception):
    """A definitive provider rejection."""


class ReconciliationRequired(Exception):
    """A human or later reconciliation process must resolve the unknown result."""

    def __init__(self, request_token):
        super().__init__("Refund outcome remains unknown")
        self.request_token = request_token


def refund_or_reconcile(send, lookup, request_token):
    """Return a confirmed receipt or preserve the applicable failure condition."""
    raise NotImplementedError("Implement one submission with scoped reconciliation")
