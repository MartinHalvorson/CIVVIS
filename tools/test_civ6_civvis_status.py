"""The residual census reports three numbers, and never one.

⚠ THIS FILE EXISTS BECAUSE ONE NUMBER MISLED A CAREFUL READER WITH THE SOURCE
OPEN. On 2026-08-17 a review of fourteen live runs read the flat residual total
of 1,577 as "1,577 decisions taken by the Lua fallback instead of CIVVIS" and
had to withdraw it. The true split was 937 bounded escapes after CIVVIS had
already answered, ~350 declines where nothing decided anything, and **three**
actual leaks. Every case below is one way that conflation can come back.
"""

from __future__ import annotations

import io
import sys
import unittest
from collections import Counter
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_civvis_status as status  # noqa: E402


def report(counts: dict) -> str:
    with redirect_stdout(io.StringIO()) as out:
        status.print_residual(Counter(counts))
    return out.getvalue()


class ResidualCensus(unittest.TestCase):
    def test_the_leak_is_reported_apart_from_the_escape_hatch(self):
        text = report({
            "ENDTURN_BLOCKING_PRODUCTION": 348,
            "ENDTURN_BLOCKING_PRODUCTION!after_civvis": 348,
            "ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE": 3,
            "ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE!unasked": 3,
            "!after_civvis": 348,
            "!unasked": 3,
        })
        self.assertIn("LEAKED", text)
        self.assertRegex(text, r"LEAKED\s+3\b")
        self.assertRegex(text, r"escape\s+348\b")
        # The leak must name its prompt; a count with no name is not actionable.
        self.assertIn("ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE", text)

    def test_a_decline_is_never_counted_as_a_decision(self):
        # The ladder returning nil means NOBODY decided. Reading 164 declines
        # as 164 heuristic decisions is half of the 2026-08-17 error.
        text = report({
            "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN": 164,
            "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN!declined": 164,
            "!declined": 164,
        })
        self.assertRegex(text, r"declined\s+164\b")
        self.assertRegex(text, r"LEAKED\s+0\b")

    def test_a_clean_run_reports_a_zero_leak_rather_than_a_big_total(self):
        text = report({
            "ENDTURN_BLOCKING_RESEARCH": 294,
            "ENDTURN_BLOCKING_RESEARCH!after_civvis": 294,
            "!after_civvis": 294,
        })
        self.assertRegex(text, r"LEAKED\s+0\b")
        self.assertNotIn("leaked prompts", text)

    def test_a_run_from_before_the_census_is_unclassified_not_bucketed(self):
        # A pre-#1839 total cannot be split after the fact, and guessing which
        # way it went is exactly the error this reporting exists to prevent.
        text = report({
            "ENDTURN_BLOCKING_UNITS": 368,
            "ENDTURN_BLOCKING_PRODUCTION": 348,
            "ENDTURN_BLOCKING_PRODUCTION@civvis": 348,
        })
        self.assertIn("unclassified", text)
        self.assertIn("716", text)
        self.assertNotIn("LEAKED", text)

    def test_a_mixed_run_keeps_the_unclassified_remainder_visible(self):
        # A run that spans the deploy has both shapes; the older turns must not
        # silently vanish into the classified totals.
        text = report({
            "ENDTURN_BLOCKING_RESEARCH": 100,
            "ENDTURN_BLOCKING_RESEARCH!after_civvis": 40,
            "!after_civvis": 40,
        })
        self.assertRegex(text, r"unclassified\s+60\b")

    def test_no_residual_at_all_says_none(self):
        self.assertIn("none", report({}))

    def test_the_source_breakdown_is_not_double_counted(self):
        # `@source` keys are a second view of the same events. Summing them
        # into the flat total would inflate every number on this line.
        text = report({
            "ENDTURN_BLOCKING_RESEARCH": 10,
            "ENDTURN_BLOCKING_RESEARCH@civvis": 10,
            "ENDTURN_BLOCKING_RESEARCH!after_civvis": 10,
            "!after_civvis": 10,
        })
        self.assertNotIn("unclassified", text)


if __name__ == "__main__":
    unittest.main()
