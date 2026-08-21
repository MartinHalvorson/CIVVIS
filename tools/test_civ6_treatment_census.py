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
        (tmp / "why.log").write_text(
            json.dumps({"kind": "genome", "treatments": ["one"]}) + "\n"
        )
        return [
            str(tmp),
            "--bin",
            str(tmp / "fake"),
            "--max-turns",
            "3",
            "--jobs",
            "1",
        ]

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
            with (
                mock.patch.object(census, "replay", side_effect=passes) as replay,
                mock.patch.object(
                    census,
                    "discover_treatments",
                    side_effect=[["one", "two"], ["two"]],
                ),
            ):
                self.assertEqual(census.main(argv), 0)
        self.assertEqual(replay.call_args_list[2].kwargs["without"], "one")
        self.assertIsNone(replay.call_args_list[2].kwargs["with_treatment"])
        self.assertIsNone(replay.call_args_list[3].kwargs["without"])
        self.assertEqual(replay.call_args_list[3].kwargs["with_treatment"], "two")


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

    def test_the_binary_enumerates_its_forceable_treatments(self) -> None:
        stderr = (
            'civvis-orders: unknown --with treatment "__census_probe__"; '
            "this binary can force: camp-party, siege-commitment"
        )
        with mock.patch.object(census.subprocess, "run") as run:
            run.return_value = mock.Mock(stderr=stderr, stdout="", returncode=2)
            found = census.discover_treatments(
                Path("bin"), Path("mirror"), option="--with", verb="force"
            )
        self.assertEqual(found, ["camp-party", "siege-commitment"])
        self.assertEqual(
            run.call_args.args[0][-2:], ["--with", "__census_probe__"]
        )


class LiveTreatmentsTest(unittest.TestCase):
    """The recorded genome, not the binary's superset, selects an arm."""

    def test_reads_the_recorded_active_treatments(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "why.log").write_text(
                "[why] ordinary explanation\n"
                + json.dumps({"kind": "other", "treatments": ["ignore"]})
                + "\n"
                + json.dumps(
                    {"kind": "genome", "treatments": ["live-one", "live-two"]}
                )
                + "\n"
            )
            found = census.live_treatments(tmp)
        self.assertEqual(found, {"live-one", "live-two"})

    def test_missing_or_malformed_genome_refuses_the_census(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            with self.assertRaises(census.CensusError):
                census.live_treatments(tmp)
            (tmp / "why.log").write_text('{"kind":"genome","treatments":null}\n')
            with self.assertRaises(census.CensusError):
                census.live_treatments(tmp)


class ArmSelectionTest(unittest.TestCase):
    """A treatment must be contrasted in the direction its genome permits."""

    def test_active_is_withheld_and_held_is_forced(self) -> None:
        common = {"active", "held"}
        self.assertEqual(
            census.arm_for(
                "active",
                active={"active"},
                withholdable=common,
                forceable={"held"},
            ),
            "without",
        )
        self.assertEqual(
            census.arm_for(
                "held",
                active={"active"},
                withholdable=common,
                forceable={"held"},
            ),
            "with",
        )

    def test_unarmable_names_are_not_called_inert(self) -> None:
        with self.assertRaisesRegex(census.CensusError, "no --without arm"):
            census.arm_for(
                "active-but-unarmable",
                active={"active-but-unarmable"},
                withholdable=set(),
                forceable=set(),
            )
        with self.assertRaisesRegex(census.CensusError, "already off"):
            census.arm_for(
                "held-but-unforceable",
                active=set(),
                withholdable={"held-but-unforceable"},
                forceable=set(),
            )


class ReplayArmTest(unittest.TestCase):
    def test_force_arm_reaches_the_decider_as_with(self) -> None:
        done = mock.Mock(returncode=0, stdout='{"orders":[]}\n', stderr="")
        with mock.patch.object(census.subprocess, "run", return_value=done) as run:
            census.replay(
                Path("bin"),
                Path("mirror"),
                [7],
                without=None,
                with_treatment="held",
                civ="Rome",
                victory="diplomatic",
                strategy="g56-48",
                timeout=5,
            )
        self.assertEqual(run.call_args.args[0][-2:], ["--with", "held"])

    def test_a_replay_cannot_both_force_and_withhold(self) -> None:
        with self.assertRaisesRegex(census.CensusError, "both force and withhold"):
            census.replay(
                Path("bin"),
                Path("mirror"),
                [7],
                without="live",
                with_treatment="held",
                civ="Rome",
                victory="diplomatic",
                strategy="g56-48",
                timeout=5,
            )


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


class SeatIdentityTest(unittest.TestCase):
    """A census must replay the agent that played, not a default one."""

    def _run(self, tmp: Path, *, brain: str | None, summary: dict | None) -> dict:
        if brain is not None:
            (tmp / "brain.log").write_text(brain)
        if summary is not None:
            (tmp / "summary.json").write_text(json.dumps(summary))
        return census.played_as(tmp)

    def test_the_run_records_what_it_was_played_with(self) -> None:
        """The exact shape `civ6_brain` prints and `civ6_play` writes.

        Measured consequence of getting this wrong: censusing
        `civvis-20260818T104654Z` under the old defaults (`auto` / `civvis`)
        against its recorded seat (`WildCard9` / `diplomatic`) flips the verdict
        for **8 of 74 treatments** — six read live that are inert, and two read
        inert that are live.
        """
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            found = self._run(
                tmp,
                brain=(
                    "[brain] mode=civvis run=civvis-x db=/x/orders.sqlite decider=server\n"
                    "[brain] decider server up (fresh board, persistent agent, "
                    "strategy=WildCard9 civ=CIVILIZATION_ROME, explaining into /x/why.log)\n"
                ),
                summary={"victory_target": "diplomatic", "tag": "civvis-x"},
            )
        self.assertEqual(
            found, {"strategy": "WildCard9", "civ": "CIVILIZATION_ROME", "victory": "diplomatic"}
        )

    def test_a_run_that_records_nothing_reports_nothing(self) -> None:
        """Silence must not be reported as a reading.

        The caller prints which fields it had to assume, and it can only do that
        if an unrecorded field is absent here rather than defaulted here.
        """
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            self.assertEqual(self._run(Path(raw), brain=None, summary=None), {})

    def test_an_unparseable_summary_does_not_take_the_brain_log_with_it(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "summary.json").write_text("{not json")
            found = self._run(tmp, brain="strategy=auto civ=CIVILIZATION_ROME,\n", summary=None)
        self.assertEqual(found.get("strategy"), "auto")
        self.assertNotIn("victory", found)


if __name__ == "__main__":
    unittest.main()
