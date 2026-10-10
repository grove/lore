"""Public context-probe input; the sequence never scores a coding outcome."""

MAX_RETRIES = 5


class TransientError(Exception):
    """Transport failed before a definitive result was received."""


class PaymentDeclined(Exception):
    """Provider rejected the payment."""


def retry_payment(send, idempotency_token):
    """Return send(token)'s result or propagate its final exception."""
    raise NotImplementedError("Implement the retry loop")
