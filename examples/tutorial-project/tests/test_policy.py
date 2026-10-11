"""Independent, local example checks; Lore does not run these automatically."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from src.policy import MAX_CHARGE_ATTEMPTS, charge_attempt_keys, refund_next_step


class PolicyTests(unittest.TestCase):
    def test_accepted_attempt_limit_and_key_reuse(self):
        self.assertEqual(MAX_CHARGE_ATTEMPTS, 3)
        self.assertEqual(charge_attempt_keys("charge-42", 3), ("charge-42",) * 3)

    def test_proposal_is_not_implemented(self):
        with self.assertRaises(ValueError):
            charge_attempt_keys("charge-42", 5)

    def test_unknown_refund_is_not_resubmitted(self):
        self.assertEqual(refund_next_step("unknown"), "read_only_reconciliation")
        self.assertEqual(refund_next_step("confirmed_failure"), "request_authorized_decision")

    def test_invalid_requests_are_rejected(self):
        for key, attempts in [("", 1), ("charge-42", 0), ("charge-42", True)]:
            with self.subTest(key=key, attempts=attempts):
                with self.assertRaises(ValueError):
                    charge_attempt_keys(key, attempts)
        with self.assertRaises(ValueError):
            refund_next_step("maybe")


if __name__ == "__main__":
    unittest.main()
