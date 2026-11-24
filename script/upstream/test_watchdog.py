from datetime import datetime, timedelta, timezone
import unittest
from unittest.mock import patch

from watchdog import check_freshness, inspect

NOW = datetime(2026, 11, 24, tzinfo=timezone.utc)


def run(age=1, **kwargs):
    result = dict(status="completed", conclusion="success", created_at=(NOW - timedelta(days=age)).isoformat())
    result.update(kwargs)
    return result


class WatchdogTests(unittest.TestCase):
    def test_recent_success_is_healthy(self):
        self.assertEqual(check_freshness([run(1)], NOW), NOW - timedelta(days=1))

    def test_failure_never_counts_as_a_successful_check(self):
        for runs in ([], [run(1, conclusion="failure")], [run(9), run(1, conclusion="failure")]):
            with self.subTest(runs=runs), self.assertRaises(ValueError):
                check_freshness(runs, NOW)

    def test_future_timestamp_is_not_healthy(self):
        with self.assertRaises(ValueError):
            check_freshness([run(-1)], NOW)

    def test_disabled_workflow_fails_even_with_previous_success(self):
        with patch("watchdog.request_json", return_value={"state": "disabled_inactivity"}):
            with self.assertRaises(ValueError):
                inspect("owner/fork", "main", now=NOW)

    def test_request_filters_default_branch_and_success(self):
        with patch("watchdog.request_json", side_effect=[{"state": "active"}, {"workflow_runs": [run()]}]) as api:
            inspect("owner/fork", "rust/main", now=NOW)
            url = api.call_args.args[0]
            self.assertIn("branch=rust%2Fmain", url)
            self.assertIn("status=success", url)

    def test_api_error_fails_closed(self):
        with patch("watchdog.request_json", side_effect=OSError("rate limit")):
            with self.assertRaises(OSError):
                inspect("owner/fork", "main", now=NOW)


if __name__ == "__main__":
    unittest.main()
