"""The desktop rescue's capture budget: cheap declines, scheduled attempts."""
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control.capture_budget import (CHEAP_ATTEMPT_SECONDS, CaptureBudget,
                                          RESCUE_INTERVAL_SECONDS)


class FakeClock:
    def __init__(self) -> None:
        self.now = 1000.0

    def __call__(self) -> float:
        return self.now

    def advance(self, seconds: float) -> None:
        self.now += seconds


class CaptureBudgetTest(unittest.TestCase):
    def setUp(self) -> None:
        self.clock = FakeClock()
        self.budget = CaptureBudget(interval=60.0, clock=self.clock)

    def test_available_host_is_never_throttled(self):
        """A machine that can capture pays 0.06 s per ask; do not ration that."""
        for _ in range(10):
            allowed, note = self.budget.spend("DiplomacyActionView", None)
            self.assertTrue(allowed)
            self.assertEqual(note, "capture available")
            self.clock.advance(1.0)

    def test_first_ask_while_unavailable_still_attempts(self):
        """A prediction of failure is not a fact, and the pixel path is the only
        thing that can dismiss a leader conversation."""
        allowed, note = self.budget.spend(
            "DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
        self.assertTrue(allowed)
        self.assertIn("systemstatusd", note)
        self.assertIn("scheduled rescue attempt", note)

    def test_repeat_asks_inside_the_interval_are_declined(self):
        """The 23.5 s stall this exists to remove: the mod re-asks every four
        close attempts, which is many times a minute."""
        self.budget.spend("DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
        for _ in range(20):
            self.clock.advance(2.0)
            allowed, note = self.budget.spend(
                "DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
            self.assertFalse(allowed)
            self.assertIn("skipped", note)
            self.assertIn("next attempt in", note)

    def test_the_rescue_comes_back_after_the_interval(self):
        """Run civvis-20260829T093602Z died because one unreadable frame ended
        the rescue for good. Skipping must be a back-off, never a stop."""
        self.budget.spend("DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
        self.clock.advance(59.0)
        allowed, _ = self.budget.spend(
            "DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
        self.assertFalse(allowed)
        self.clock.advance(1.5)
        allowed, note = self.budget.spend(
            "DiplomacyActionView", "systemstatusd is spinning at 99% CPU")
        self.assertTrue(allowed)
        self.assertIn("scheduled rescue attempt", note)

    def test_screens_hold_separate_schedules(self):
        """A DiplomacyDealView must not inherit a DiplomacyActionView's wait."""
        self.budget.spend("DiplomacyActionView", "unavailable")
        allowed, _ = self.budget.spend("DiplomacyDealView", "unavailable")
        self.assertTrue(allowed)
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertFalse(allowed)

    def test_a_landed_click_clears_the_wait(self):
        """The pixel path working on this screen is the one thing worth being
        eager about."""
        self.budget.spend("DiplomacyActionView", "unavailable")
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertFalse(allowed)
        self.budget.record_attempt("DiplomacyActionView", 9.9, dismissed=True)
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertTrue(allowed)

    def test_a_cheap_attempt_clears_the_wait_even_without_a_click(self):
        """Measured 2026-09-02: `capture_pause_reason()` reported a 100 % CPU
        spin while captures returned in 0.07 s. Rationing a 70 ms rescue saves
        nothing and delays the only thing that can dismiss a leader screen."""
        self.budget.spend("DiplomacyActionView", "unavailable")
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertFalse(allowed)
        self.budget.record_attempt("DiplomacyActionView", 0.07, dismissed=False)
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertTrue(allowed, "a cheap attempt disproves the preflight")

    def test_an_expensive_failed_attempt_keeps_the_wait(self):
        """The whole point: 23.5 s that dismissed nothing must not repeat."""
        self.budget.spend("DiplomacyActionView", "unavailable")
        self.budget.record_attempt("DiplomacyActionView", 23.5, dismissed=False)
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertFalse(allowed)

    def test_the_cheap_threshold_separates_a_healthy_capture_from_a_doomed_one(self):
        """0.06-0.11 s healthy, 3.5 s doomed with the breaker, 11.0 s without."""
        self.assertGreater(CHEAP_ATTEMPT_SECONDS, 0.5)
        self.assertLess(CHEAP_ATTEMPT_SECONDS, 3.5)

    def test_an_attempt_on_an_unknown_screen_is_harmless(self):
        self.budget.record_attempt("NeverSeen", 0.1, dismissed=True)
        allowed, _ = self.budget.spend("NeverSeen", "unavailable")
        self.assertTrue(allowed)

    def test_a_falsy_reason_counts_as_available(self):
        """`capture_pause_reason()` returns None when it believes capture works;
        an empty string is the same answer and must not be read as a reason."""
        allowed, note = self.budget.spend("DiplomacyActionView", "")
        self.assertTrue(allowed)
        self.assertEqual(note, "capture available")

    def test_recovery_clears_the_wait_for_the_next_unavailable_run(self):
        """One healthy ask proves the host came back, so the following stall
        must not be charged the old schedule."""
        self.budget.spend("DiplomacyActionView", "unavailable")
        self.clock.advance(5.0)
        self.budget.spend("DiplomacyActionView", None)
        self.clock.advance(5.0)
        allowed, _ = self.budget.spend("DiplomacyActionView", "unavailable")
        self.assertFalse(allowed, "a healthy attempt is still an attempt")

    def test_the_shipped_interval_fits_inside_the_wedge_watchdog(self):
        """The watchdog hands off at five minutes; the rescue must get several
        real attempts before that."""
        self.assertLessEqual(RESCUE_INTERVAL_SECONDS * 3, 300.0)
        self.assertGreaterEqual(RESCUE_INTERVAL_SECONDS, 30.0)


class OnlyThePixelPathIsRationed(unittest.TestCase):
    """⚠ A BUDGET ON CAPTURES MUST NOT DELAY A DISMISSAL THAT NEEDS NONE.

    `WorldCongressBetweenTurns` is closed by clicking its shipped control at a
    computed rectangle, and the remaining blocking screens by Escape. Neither
    reads a frame. Holding a blocking Congress screen for a minute because an
    unrelated capture service is busy would be a new outage, not a saving.
    """

    def _source(self) -> str:
        return (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")

    def test_the_early_return_is_guarded_by_needs_pixels(self):
        source = self._source()
        self.assertIn('needs_pixels = screen in ("DiplomacyActionView", "LeaderView",',
                      source)
        self.assertIn("if needs_pixels and not allowed:", source)

    def test_the_attempt_is_timed_from_before_the_photograph(self):
        """Both captures are the attempt; timing only the click would call a
        23.5 s ask cheap."""
        source = self._source()
        # Scope to the handler: `screenshot(` is called from the setup code too.
        block = source[source.index('elif kind in ("autoclose_desktop", "autoclose_stuck"):'):
                       source.index('elif kind == "retired":')]
        self.assertIn("attempt_started = time.monotonic()", block)
        self.assertLess(block.index("attempt_started = time.monotonic()"),
                        block.index("screenshot(shot)"))
        self.assertIn("DESKTOP_RESCUE_BUDGET.record_attempt(", block)

    def test_the_pixel_screens_match_the_dispatch_below_them(self):
        """The two lists have to name the same screens or one of them is a lie."""
        source = self._source()
        guard = source.index("needs_pixels = screen in (")
        dispatch = source.index(
            'if screen in ("DiplomacyActionView", "LeaderView", "DiplomacyDealView"):')
        self.assertLess(guard, dispatch)
        named = set(re.findall(r'"([A-Za-z]+)"', source[guard:source.index(")", guard)]))
        self.assertEqual(named, {"DiplomacyActionView", "LeaderView", "DiplomacyDealView"})

    def test_an_unreadable_capture_state_never_ends_the_run(self):
        """`record` drives the whole game; a failed availability probe is an
        optimisation question, not a reason to stop playing."""
        source = self._source()
        block = source[source.index("capture_state = popup_clear.capture_pause_reason()"):
                       source.index("allowed, budget_note = DESKTOP_RESCUE_BUDGET.spend")]
        self.assertIn("except Exception", block)
        self.assertIn("capture_state = None", block,
                      "unknown must mean 'try it', which is the old behaviour")

    def test_the_photograph_is_skipped_rather_than_taken_blind(self):
        """A capture the host cannot take is 11 s either way; do not spend it
        just because the screen has a capture-free dismissal."""
        source = self._source()
        self.assertIn("if allowed:\n                screenshot(shot)", source)
        self.assertIn("not photographed ({budget_note})", source)


if __name__ == "__main__":
    unittest.main()
