"""The operator's leader-Production share restart rule (2026-10-26).

"lets auto restart if our prod is < 1/3 of the leader": `production_share_reading`
reads the mod's `major_production` map off every agent `turn` record from turn
50, `production_share_run` counts consecutive game turns below the line, and the
host policy file's CIVVIS_RESTART_BELOW_LEADER_PRODUCTION turns it on.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402

THIRD = 1 / 3


def turn_record(turn: int, production: dict | None, us: int | None = 0) -> dict:
    event = {"kind": "turn", "ctx": "agent", "turn": turn}
    if production is not None:
        event["major_production"] = production
    if us is not None:
        event["local_player"] = us
    return event


class ProductionShareReadingTest(unittest.TestCase):
    def test_off_reads_nothing(self):
        event = turn_record(80, {"0": 10, "1": 90, "2": 40, "3": 30})
        for share in (0, 0.0, -0.5, 1.0, 2, True):
            self.assertIsNone(civ6_play.production_share_reading(event, share), share)

    def test_under_a_third_of_the_leader_is_below(self):
        reading = civ6_play.production_share_reading(
            turn_record(60, {"0": 29, "1": 90, "2": 100.5, "3": 40}), THIRD)
        self.assertEqual(reading["rule"], "production_share")
        self.assertEqual(reading["turn"], 60)
        self.assertEqual(reading["ours"], 29)
        self.assertEqual(reading["leader"], "2")
        self.assertEqual(reading["leader_production"], 100.5)
        self.assertEqual(reading["share"], round(29 / 100.5, 4))
        self.assertEqual(reading["line"], 0.3333)
        self.assertTrue(reading["below"])

    def test_a_third_or_more_is_not_below(self):
        for ours in (34, 50, 120):
            reading = civ6_play.production_share_reading(
                turn_record(60, {"0": ours, "1": 100, "2": 60}), THIRD)
            self.assertFalse(reading["below"], ours)

    def test_the_leader_is_the_best_rival_not_us(self):
        reading = civ6_play.production_share_reading(
            turn_record(70, {"0": 300, "1": 20, "2": 200, "3": 10}, us=1), THIRD)
        self.assertEqual(reading["leader"], "0")
        self.assertEqual(reading["ours"], 20)
        self.assertTrue(reading["below"])

    def test_only_agent_turn_records_from_turn_50(self):
        production = {"0": 1, "1": 100, "2": 100, "3": 100}
        self.assertIsNone(civ6_play.production_share_reading(turn_record(49, production), THIRD))
        self.assertIsNotNone(civ6_play.production_share_reading(turn_record(50, production), THIRD))
        state = dict(turn_record(80, production), kind="state")
        self.assertIsNone(civ6_play.production_share_reading(state, THIRD))
        heartbeat = dict(turn_record(80, production), ctx="heartbeat")
        self.assertIsNone(civ6_play.production_share_reading(heartbeat, THIRD))

    def test_a_later_first_turn_reads_nothing_before_it(self):
        production = {"0": 1, "1": 100, "2": 100, "3": 100}
        for turn in (50, 79):
            self.assertIsNone(civ6_play.production_share_reading(
                turn_record(turn, production), 0.25, 80), turn)
        for turn in (80, 120):
            reading = civ6_play.production_share_reading(turn_record(turn, production), 0.25, 80)
            self.assertTrue(reading["below"], turn)
            self.assertEqual(reading["line"], 0.25)

    def test_a_record_without_our_standing_or_a_rival_is_unreadable(self):
        for event in (turn_record(80, None),
                      turn_record(80, {"1": 100, "2": 50}),
                      turn_record(80, {"0": 10, "1": 100}, us=None),
                      turn_record(80, {"0": -1, "1": 100}),
                      turn_record(80, {"0": 10}),
                      turn_record(80, []),
                      turn_record(80, "garbage")):
            reading = civ6_play.production_share_reading(event, THIRD)
            self.assertEqual(reading, {"rule": "production_share", "turn": 80,
                                       "unreadable": True}, event)

    def test_rivals_producing_nothing_leave_us_above(self):
        reading = civ6_play.production_share_reading(
            turn_record(60, {"0": 0, "1": 0, "2": 0}), THIRD)
        self.assertFalse(reading["below"])

    def test_unreadable_rivals_are_left_out_not_counted_as_zero(self):
        reading = civ6_play.production_share_reading(
            turn_record(60, {"0": 30, "1": None, "2": 80, "3": "x"}), THIRD)
        self.assertEqual(reading["leader"], "2")
        self.assertFalse(reading["below"])


class ProductionShareRunTest(unittest.TestCase):
    def reading(self, turn: int, below: bool) -> dict:
        return {"rule": "production_share", "turn": turn, "below": below}

    def test_three_turns_running_below(self):
        state: dict = {}
        runs = [civ6_play.production_share_run(state, self.reading(t, True))
                for t in (50, 51, 52)]
        self.assertEqual(runs, [1, 2, 3])
        self.assertGreaterEqual(runs[-1], civ6_play.PRODUCTION_SHARE_TURNS)

    def test_a_turn_at_or_above_the_line_ends_the_run(self):
        state: dict = {}
        seq = [(50, True), (51, True), (52, False), (53, True), (54, True)]
        runs = [civ6_play.production_share_run(state, self.reading(t, b)) for t, b in seq]
        self.assertEqual(runs, [1, 2, 0, 1, 2])

    def test_a_turn_counts_once_however_many_records_it_carries(self):
        state: dict = {}
        runs = [civ6_play.production_share_run(state, self.reading(t, True))
                for t in (60, 60, 60, 61, 61)]
        self.assertEqual(runs, [1, 1, 1, 2, 2])

    def test_the_live_loop_retires_on_the_third_turn(self):
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        branch = source.split("if production_share > 0:\n            reading = "
                              "production_share_reading(event, production_share,\n"
                              "                                               "
                              "production_share_from)", 1)
        self.assertEqual(len(branch), 2, "the live loop reads the share every turn")
        body = branch[1][:2500]
        self.assertIn("run = production_share_run(state, reading)", body)
        self.assertIn("if run >= PRODUCTION_SHARE_TURNS:", body)
        self.assertIn("return abandon_with_retire(reading)", body)


class ProductionSharePolicyTest(unittest.TestCase):
    def policy(self, text: str) -> Path:
        handle = tempfile.NamedTemporaryFile("w", suffix=".policy", delete=False)
        handle.write(text)
        handle.close()
        self.addCleanup(Path(handle.name).unlink)
        return Path(handle.name)

    def test_a_fraction_or_a_decimal(self):
        for text, share in (("CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1/3\n", THIRD),
                            ("CIVVIS_RESTART_BELOW_LEADER_PRODUCTION = 0.25  # try\n", 0.25),
                            ("CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=0.5\n"
                             "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=.3333\n", 0.3333)):
            self.assertAlmostEqual(
                civ6_play.production_share_policy(self.policy(text)), share, places=6, msg=text)

    def test_absent_zero_out_of_range_or_invalid_is_off(self):
        for text in ("CIVVIS_DIFFICULTY=DIFFICULTY_IMMORTAL\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=0\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1.5\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=-0.3\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1/0\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=third\n",
                     "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=\n",
                     "#CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1/3\n"):
            with mock.patch("builtins.print"):
                self.assertEqual(civ6_play.production_share_policy(self.policy(text)), 0.0, text)

    def test_a_missing_file_is_off(self):
        self.assertEqual(civ6_play.production_share_policy(Path("/nonexistent/policy")), 0.0)

    def test_the_first_turn_from_the_policy(self):
        key = civ6_play.PRODUCTION_SHARE_FROM_POLICY_KEY
        self.assertEqual(key, "CIVVIS_RESTART_BELOW_LEADER_PRODUCTION_FROM_TURN")
        for text, turn in ((f"{key}=80\n", 80),
                           (f"{key} = 100  # later\n", 100),
                           (f"{key}=60\n{key}=80\n", 80)):
            self.assertEqual(civ6_play.production_share_from_turn_policy(self.policy(text)),
                             turn, text)

    def test_an_absent_or_invalid_first_turn_is_turn_50(self):
        key = civ6_play.PRODUCTION_SHARE_FROM_POLICY_KEY
        for text in ("CIVVIS_RESTART_BELOW_LEADER_PRODUCTION=1/4\n", f"{key}=\n",
                     f"{key}=0\n", f"{key}=-5\n", f"{key}=eighty\n", f"{key}=80.5\n",
                     f"#{key}=80\n"):
            with mock.patch("builtins.print"):
                self.assertEqual(civ6_play.production_share_from_turn_policy(self.policy(text)),
                                 civ6_play.PRODUCTION_SHARE_TURN, text)
        self.assertEqual(civ6_play.production_share_from_turn_policy(
            Path("/nonexistent/policy")), civ6_play.PRODUCTION_SHARE_TURN)

    def test_the_flag_defaults_to_the_policy(self):
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        self.assertIn('"--restart-below-leader-production", default=None', source)
        self.assertIn("production_share_policy() if production_share is None", source)
        self.assertIn('args.restart_below_leader_production = production_share', source)
        self.assertIn('production_share = getattr(args, "restart_below_leader_production", None) or 0.0',
                      source)
        self.assertIn('"--restart-below-leader-production-from-turn", type=int, default=None',
                      source)
        self.assertIn("production_share_from_turn_policy()\n", source)
        self.assertIn("args.restart_below_leader_production_from_turn = production_share_from",
                      source)
        self.assertIn('getattr(args, "restart_below_leader_production_from_turn", None)\n'
                      "                             or PRODUCTION_SHARE_TURN)", source)


if __name__ == "__main__":
    unittest.main()
