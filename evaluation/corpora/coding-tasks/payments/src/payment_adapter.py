"""Provider-neutral transport adapter; preserve the observed retry count."""

MAX_RETRIES = 5


class TransientError(Exception):
    """Transport failed before a definitive result was received."""


class PaymentDeclined(Exception):
    """Provider rejected the payment; this is not a transport failure."""


def retry_payment(send, idempotency_token):
    """Return send(token)'s result or propagate its final exception."""
    raise NotImplementedError("Implement the retry loop")
