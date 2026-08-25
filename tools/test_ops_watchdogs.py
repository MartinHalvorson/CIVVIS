"""Regression coverage for the unattended GUI-lane stall guards."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
OPS = TOOLS / "ops"
STATE_PATH = OPS / "civvis_watchdog_state.py"
SPEC = importlib.util.spec_from_file_location("civvis_watchdog_state", STATE_PATH)
assert SPEC is not None and SPEC.loader is not None
watchdog_state = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(watchdog_state)


class RepeatingUnitBlockerTest(unittest.TestCase):
    def test_counts_only_the_latest_unit_blocker_on_its_turn(self) -> None:
        events = [
            {"kind": "turn", "turn": 152},
            {"kind": "blocked", "turn": 152,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "turn", "turn": 153},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "dismissed", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
        ]
        self.assertEqual(
            watchdog_state.repeating_unit_blocker(events),
            (153, "ENDTURN_BLOCKING_UNITS", 3),
        )

    def test_non_unit_blockers_do_not_trigger_the_unit_recovery(self) -> None:
        events = [
            {"kind": "turn", "turn": 153},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_PRODUCTION"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_PRODUCTION"},
        ]
        self.assertIsNone(watchdog_state.repeating_unit_blocker(events))

    def test_a_later_turn_or_outcome_clears_an_old_blocker_signal(self) -> None:
        self.assertIsNone(watchdog_state.repeating_unit_blocker([
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "turn", "turn": 154},
        ]))
        self.assertIsNone(watchdog_state.repeating_unit_blocker([
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "outcome", "turn": 153},
        ]))


class WatchdogWiringTest(unittest.TestCase):
    def test_agent_watchdog_escalates_an_explicit_repeating_unit_blocker(self) -> None:
        source = (OPS / "civvis-agent-wedge-watchdog.sh").read_text()
        self.assertIn("CIVVIS_WEDGE_BLOCKER_STREAK", source)
        self.assertIn("civvis_watchdog_state.py", source)
        self.assertIn("repeating unit blocker", source)
        self.assertLess(
            source.index("repeating unit blocker"),
            source.index("mirror_turn=$(curl"),
        )

    def test_interactive_host_keeps_the_agent_watchdog_alive(self) -> None:
        source = (OPS / "civvis-interactive-host.sh").read_text()
        self.assertIn("WEDGE_WATCHDOG", source)
        self.assertIn("start_wedge_watchdog", source)
        self.assertIn("wedge_watchdog_owned", source)


if __name__ == "__main__":
    unittest.main()
