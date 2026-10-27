import unittest

from engine.report import client_report


class ReportTests(unittest.TestCase):
    def test_versioned_report_preserves_gameplay(self):
        result = dict(confidence="high", hi=123, continues=0, game_over=False)
        gameplay = {"hi-score": 123}
        report = client_report("earthion", "1.0.0", result, gameplay)
        self.assertEqual(report["schema_version"], 1)
        self.assertEqual(report["validator_version"], "1.0.0")
        self.assertIs(report["gameplay"], gameplay)
        self.assertFalse(report["needs_review"])

    def test_uncertain_or_heuristic_results_require_review(self):
        for override in (dict(confidence="low"), dict(hi=None),
                         dict(continues=1), dict(game_over=True)):
            result = dict(confidence="high", hi=123, continues=0, game_over=False)
            result.update(override)
            self.assertTrue(client_report("earthion", "1.0.0", result, {})["needs_review"])
