#!/usr/bin/env python3
"""Tests for the live-treatment census.

The property that matters here is not that the tool prints a table. It is that
the table cannot be produced from a control that does not reproduce itself, and
that the treatment list is never a copy kept in this repository. Both are the
failure modes the census exists to avoid, so both are pinned.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_treatment_census as census  # noqa: E402


class CanonicalTest(unittest.TestCase):
    def test_order_sequence_is_not_a_decision_change(self) -> None:
        """Two turns that issue the same orders in a different sequence agree.

        The order channel's own sequence is not what a treatment decides, and
        counting a reordering as a difference would report every treatment live.
        """
        one = json.dumps({"turn": 4, "orders": [{"verb": "A"}, {"verb": "B"}]})
        two = json.dumps({"turn": 4, "orders": [{"verb": "B"}, {"verb": "A"}]})
        self.assertEqual(census.canonical(one), census.canonical(two))

    def test_a_moved_unit_is_a_decision_change(self) -> None:
        one = json.dumps({"turn": 4, "orders": [{"verb": "MOVE_TO", "x": 3}]})
        two = json.dumps({"turn": 4, "orders": [{"verb": "MOVE_TO", "x": 4}]})
        self.assertNotEqual(census.canonical(one), census.canonical(two))

    def test_an_unparseable_reply_is_not_silently_equal_to_another(self) -> None:
        """A decider that died mid-line must not read as agreement.

        Returning the same sentinel for every unparseable reply would make two
        crashed passes compare equal and report the treatment inert.
        """
        self.assertNotEqual(census.canonical("not json"), census.canonical("also not"))


class ControlReproducibilityTest(unittest.TestCase):
    """A census whose control cannot reproduce itself measures its own noise."""

    def _args(self, tmp: Path) -> list[str]:
        (tmp / "events.jsonl").write_text(
            "".join(
                json.dumps({"kind": "state", "turn": turn}) + "\n"
                for turn in (10, 11, 12)
            )
        )
        return [str(tmp), "--bin", str(tmp / "fake"), "--max-turns", "3"]

    def test_a_drifting_control_refuses_to_report(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "fake").write_text("")
            argv = self._args(tmp)
            passes = [
                {10: "a", 11: "b", 12: "c"},
                {10: "a", 11: "DIFFERENT", 12: "c"},
            ]
            with mock.patch.object(census, "replay", side_effect=passes):
                with self.assertRaises(census.CensusError) as caught:
                    census.main(argv)
        self.assertIn("does not reproduce itself", str(caught.exception))
        self.assertIn("turn 11", str(caught.exception))

    def test_a_stable_control_reports(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "fake").write_text("")
            argv = self._args(tmp) + ["--treatments", "one,two"]
            passes = [
                {10: "a", 11: "b", 12: "c"},  # control
                {10: "a", 11: "b", 12: "c"},  # control, again
                {10: "a", 11: "MOVED", 12: "c"},  # one
                {10: "a", 11: "b", 12: "c"},  # two
            ]
            with mock.patch.object(census, "replay", side_effect=passes):
                self.assertEqual(census.main(argv), 0)


class DiscoveryTest(unittest.TestCase):
    """The treatment list is asked for, never kept here."""

    def test_the_binary_enumerates_its_own_treatments(self) -> None:
        stderr = (
            'civvis-orders: unknown --without treatment "__census_probe__"; '
            "this binary can withhold: come-ashore, siege-role, war-patience"
        )
        with mock.patch.object(census.subprocess, "run") as run:
            run.return_value = mock.Mock(stderr=stderr, stdout="", returncode=2)
            found = census.discover_treatments(Path("bin"), Path("mirror"))
        self.assertEqual(found, ["come-ashore", "siege-role", "war-patience"])

    def test_a_binary_that_will_not_enumerate_is_an_error(self) -> None:
        """Better to refuse than to census a list this file invented."""
        with mock.patch.object(census.subprocess, "run") as run:
            run.return_value = mock.Mock(stderr="something else", stdout="", returncode=2)
            with self.assertRaises(census.CensusError):
                census.discover_treatments(Path("bin"), Path("mirror"))


class WindowTest(unittest.TestCase):
    def test_the_window_spreads_rather_than_taking_a_prefix(self) -> None:
        """A capped window must still reach the endgame.

        Settlement treatments act in the first fifty turns and war treatments in
        the last hundred; a prefix would report every war arm inert.
        """
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "events.jsonl").write_text(
                "".join(
                    json.dumps({"kind": "state", "turn": turn}) + "\n"
                    for turn in range(1, 201)
                )
            )
            turns = census.turn_window(tmp, None, 10)
        self.assertEqual(len(turns), 10)
        self.assertLess(turns[0], 25)
        self.assertGreater(turns[-1], 175)

    def test_a_run_directory_without_events_is_named(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            with self.assertRaises(census.CensusError):
                census.turn_window(Path(raw), None, 10)


if __name__ == "__main__":
    unittest.main()
