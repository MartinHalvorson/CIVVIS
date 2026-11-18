"""The Science-lane out-of-reach retire rule (lane rule, 2026-11-17; off without a policy share).

`science_out_of_reach_reading` reads our tech count over the best met rival's
from every agent `state` record (`tech_standing`, the Domination rule's own
reading) and retires a Science-lane game once that share has stood under the
policy's line for three game turns running, from Online turn 150. The host
policy file's CIVVIS_RESTART_SCIENCE_BEHIND_TECHS turns it on and
CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN moves the first turn.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402


def tech_record(turn, ours, *rivals, **extra) -> dict:
    """An agent `state` record holding `ours` techs against each met rival's count."""
    return {"kind": "state", "ctx": "agent", "turn": turn,
            "techs": [f"TECH_{index}" for index in range(ours)],
            "rivals": [{"civ": f"CIVILIZATION_{index}", "techs": count}
                       for index, count in enumerate(rivals)], **extra}


def read(records, share=0.70, from_turn=150) -> tuple[list, dict]:
    state: dict = {}
    return [civ6_play.science_out_of_reach_reading(state, record, share, from_turn)
            for record in records], state


def play(series: list[tuple[int, int, int]], last_turn: int, share: float = 0.70,
         from_turn: int = 150) -> tuple[dict | None, dict]:
    """Fold a recorded game, given as (turn, ours, best-rival-from-that-turn)
    steps, one record a turn; the first verdict and the carried state."""
    state: dict = {}
    steps = {turn: (ours, best) for turn, ours, best in series}
    ours = best = 0
    for turn in range(series[0][0], last_turn + 1):
        ours, best = steps.get(turn, (ours, best))
        verdict = civ6_play.science_out_of_reach_reading(
            state, tech_record(turn, ours, best), share, from_turn)
        if verdict is not None:
            return verdict, state
    return None, state


#: Our tech count against the best met rival's from turn 150 in three live
#: Online games, as the first `state` record of each turn had it (each entry is
#: the turn a new pair starts), and the game's last turn.
#: Deity, Science lane, 2026-11-18: played to t189 and lost. The rival held
#: the whole tree (77) from t163.
LOST_045850Z = ([(150, 40, 70), (151, 41, 71), (154, 41, 72), (155, 42, 73), (157, 42, 74),
                 (158, 42, 75), (160, 43, 76), (163, 44, 77), (167, 45, 77), (169, 46, 77),
                 (170, 47, 77), (173, 48, 77), (176, 49, 77), (180, 50, 77), (182, 51, 77),
                 (183, 52, 77), (184, 53, 77), (185, 54, 77), (187, 55, 77)], 189)
#: The Science win that stood lowest at turn 150 (Emperor, 2026-11-02, won at
#: t218): 46 against 56, 82%, the lowest it or any Science winner stood from there.
WON_185903Z = ([(150, 46, 56), (151, 47, 56), (153, 47, 57), (155, 48, 57), (156, 48, 58),
                (158, 49, 58), (159, 49, 59), (160, 50, 59), (161, 51, 60), (162, 52, 60),
                (165, 53, 61), (166, 54, 61), (167, 54, 62), (169, 55, 63), (170, 56, 64),
                (171, 57, 65), (172, 58, 65), (173, 59, 66), (175, 59, 67), (176, 60, 68),
                (178, 61, 68), (179, 62, 68), (180, 62, 69), (181, 63, 69), (183, 64, 69),
                (184, 64, 70), (186, 65, 70), (187, 65, 71), (189, 66, 71), (190, 67, 72),
                (192, 68, 72), (193, 68, 73), (195, 69, 73), (196, 69, 74), (197, 70, 74),
                (198, 70, 75), (199, 71, 75), (200, 71, 76), (201, 72, 77), (204, 73, 77),
                (207, 74, 77), (210, 75, 77), (211, 76, 77), (214, 77, 77)], 218)
#: The Deity Science win that stood lowest (2026-11-06, won at t187): 57
#: against 68, 84%.
WON_050334Z = ([(150, 57, 68), (151, 58, 69), (152, 59, 69), (154, 60, 69), (155, 60, 70),
                (156, 61, 71), (158, 62, 71), (160, 62, 72), (161, 63, 72), (162, 63, 73),
                (163, 64, 73), (164, 65, 73), (165, 65, 74), (166, 66, 74), (169, 67, 75),
                (172, 68, 75), (173, 69, 76), (175, 70, 77), (178, 71, 77), (180, 72, 77),
                (182, 73, 77), (185, 74, 77)], 186)


class ScienceOutOfReachLiveGames(unittest.TestCase):
    def test_the_live_loss_is_retired_on_its_third_turn_from_150(self):
        verdict, _ = play(*LOST_045850Z)
        self.assertEqual(verdict, {"rule": "science_out_of_reach", "turn": 152,
                                   "since": 150, "from_turn": 150, "turns": 3,
                                   "techs": 41, "rival_techs": 71, "share": 0.5775,
                                   "line": 0.70, "fires": True})
        # Thirty-seven turns before the game's own end at t189.
        self.assertEqual(LOST_045850Z[1] - verdict["turn"], 37)

    def test_the_science_winners_that_stood_lowest_play_on(self):
        for name, game in (("185903Z", WON_185903Z), ("050334Z", WON_050334Z)):
            for share in (0.70, 0.75, 0.80, 0.82):
                with self.subTest(game=name, share=share):
                    verdict, state = play(*game, share=share)
                    self.assertIsNone(verdict)
                    self.assertNotIn("science_out_of_reach", state)
        self.assertEqual(min(round(ours / best, 2) for _, ours, best in WON_185903Z[0]), 0.82)
        self.assertEqual(min(round(ours / best, 2) for _, ours, best in WON_050334Z[0]), 0.84)


class ScienceOutOfReachSparesTheCongressRoad(unittest.TestCase):
    """`CIVVIS_RESTART_SCIENCE_BEHIND_SPARE_DVP`: a seat on 10 or more
    Diplomatic Victory points plays its Congress road out (live Deity
    2026-11-18: civvis-20261118T214750Z, America, retired at t152 on 43 of
    68 techs with 10 points against the best rival's 7)."""

    @staticmethod
    def read(records, spare):
        state: dict = {}
        return [civ6_play.science_out_of_reach_reading(state, record, 0.70, 150, spare)
                for record in records], state

    def test_ten_points_waits_and_nine_retire(self):
        held, state = self.read([tech_record(t, 43, 68, dvp=10) for t in (150, 151, 152, 153)], 10)
        self.assertEqual(held, [None] * 4)
        self.assertNotIn("science_out_of_reach", state)
        short, _ = self.read([tech_record(t, 43, 68, dvp=9) for t in (150, 151, 152)], 10)
        self.assertEqual(short[2]["rule"], "science_out_of_reach")

    def test_points_falling_under_the_spare_start_the_count_again(self):
        records = ([tech_record(150, 43, 68, dvp=9), tech_record(151, 43, 68, dvp=9),
                    tech_record(152, 43, 68, dvp=11)]
                   + [tech_record(t, 43, 68, dvp=8) for t in (153, 154, 155)])
        results, _ = self.read(records, 10)
        self.assertEqual(results[:5], [None] * 5)
        self.assertEqual((results[5]["since"], results[5]["turn"]), (153, 155))

    def test_off_or_unread_points_change_nothing(self):
        for spare, extra in ((0, {"dvp": 15}), (10, {}), (10, {"dvp": None}), (10, {"dvp": True})):
            with self.subTest(spare=spare, extra=extra):
                results, _ = self.read([tech_record(t, 43, 68, **extra) for t in (150, 151, 152)], spare)
                self.assertEqual(results[2]["rule"], "science_out_of_reach")

    def test_the_policy_reads_a_count_of_points(self):
        self.assertEqual(civ6_play.parse_science_behind_spare_dvp(None), 0)
        self.assertEqual(civ6_play.parse_science_behind_spare_dvp(""), 0)
        self.assertEqual(civ6_play.parse_science_behind_spare_dvp("10"), 10)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(civ6_play.parse_science_behind_spare_dvp("ten"), 0)
            self.assertEqual(civ6_play.parse_science_behind_spare_dvp("-3"), 0)


class ScienceOutOfReachReading(unittest.TestCase):
    def test_fires_on_the_third_turn_running_under_the_line(self):
        # 38 techs against the best met rival's 60 is 63%.
        results, _ = read([tech_record(turn, 38, 44, 60, 52) for turn in (150, 151, 152)])
        self.assertEqual(results[:2], [None, None])
        self.assertEqual(results[2], {"rule": "science_out_of_reach", "turn": 152,
                                      "since": 150, "from_turn": 150, "turns": 3,
                                      "techs": 38, "rival_techs": 60, "share": 0.6333,
                                      "line": 0.70, "fires": True})

    def test_the_line_itself_and_anything_above_it_play_on(self):
        for ours, best in ((42, 60), (47, 56), (58, 61), (66, 62)):   # 70%, 84%, 95%, 106%
            with self.subTest(ours=ours, best=best):
                results, state = read([tech_record(turn, ours, best)
                                       for turn in range(150, 200)])
                self.assertEqual(results, [None] * 50)
                self.assertNotIn("science_out_of_reach", state)

    def test_just_under_the_line_fires(self):
        results, _ = read([tech_record(turn, 41, 60) for turn in (150, 151, 152)])
        self.assertEqual((results[2]["techs"], results[2]["rival_techs"],
                          results[2]["share"]), (41, 60, 0.6833))

    def test_the_best_met_rival_is_the_measure(self):
        # Level with two rivals, far behind the third.
        results, _ = read([tech_record(turn, 40, 41, 39, 66) for turn in (150, 151, 152)])
        self.assertEqual(results[2]["rival_techs"], 66)
        # Far behind nobody we have met: the same 40 techs play on.
        results, _ = read([tech_record(turn, 40, 41, 39) for turn in (150, 151, 152)])
        self.assertEqual(results, [None] * 3)

    def test_before_the_first_turn_never_fires(self):
        results, state = read([tech_record(turn, 10, 60) for turn in range(100, 150)])
        self.assertEqual(results, [None] * 50)
        self.assertEqual(state, {})

    def test_turns_before_the_first_turn_do_not_count(self):
        results, _ = read([tech_record(turn, 30, 60) for turn in (148, 149, 150, 151)])
        self.assertEqual(results, [None] * 4)
        results, _ = read([tech_record(turn, 30, 60) for turn in (148, 149, 150, 151, 152)])
        self.assertEqual((results[4]["since"], results[4]["turn"]), (150, 152))

    def test_after_the_first_turn_it_still_fires(self):
        results, _ = read([tech_record(turn, 50, 60) for turn in range(150, 180)]
                          + [tech_record(turn, 50, 75) for turn in (180, 181, 182)])
        self.assertEqual(results[:32], [None] * 32)
        self.assertEqual((results[32]["since"], results[32]["turn"], results[32]["share"]),
                         (180, 182, 0.6667))

    def test_three_turns_running_not_three_records(self):
        # Several records a turn, as a live turn's frames are: still turn 152.
        results, _ = read([tech_record(150, 30, 60)] * 5 + [tech_record(151, 30, 60)] * 5)
        self.assertEqual(results, [None] * 10)
        results, _ = read([tech_record(turn, 30, 60) for turn in (150, 151, 152)
                           for _ in range(3)])
        self.assertEqual(results[:6], [None] * 6)
        self.assertEqual((results[6]["turns"], results[6]["turn"]), (3, 152))

    def test_a_turn_back_at_the_line_starts_the_count_again(self):
        results, state = read([tech_record(150, 41, 60), tech_record(151, 41, 60),
                               tech_record(152, 42, 60),            # a tech lands: 70%
                               tech_record(153, 42, 61), tech_record(154, 42, 61)])
        self.assertEqual(results, [None] * 5)
        self.assertEqual(state["science_out_of_reach"]["since"], 153)
        results, _ = read([tech_record(150, 41, 60), tech_record(151, 41, 60),
                           tech_record(152, 42, 60),
                           tech_record(153, 42, 61), tech_record(154, 42, 61),
                           tech_record(155, 42, 61)])
        self.assertEqual((results[5]["since"], results[5]["turn"], results[5]["turns"]),
                         (153, 155, 3))

    def test_no_met_rival_never_fires(self):
        for record in (lambda turn: tech_record(turn, 12),                # nobody met
                       lambda turn: tech_record(turn, 12, 0),             # a count of 0
                       lambda turn: tech_record(turn, 12, 0, -1),         # a failed read
                       lambda turn: tech_record(turn, 12, True),
                       lambda turn: tech_record(turn, 12, rivals=None),
                       lambda turn: tech_record(turn, 12, rivals=["x", None])):
            results, state = read([record(turn) for turn in range(150, 250)])
            self.assertEqual(results, [None] * 100)
            self.assertEqual(state, {})

    def test_a_rival_that_cannot_be_read_is_left_out_of_the_field(self):
        results, _ = read([tech_record(turn, 30, -1, 60, 0) for turn in (150, 151, 152)])
        self.assertEqual(results[2]["rival_techs"], 60)

    def test_an_unreadable_record_neither_counts_nor_clears(self):
        unreadable = ({"kind": "state", "ctx": "agent", "turn": 151},
                      tech_record(151, 30),
                      tech_record(151, 30, 60, techs=None),
                      tech_record(151, 30, 60, ctx="autoclose"),
                      tech_record(151, 30, 60, kind="turn"),
                      tech_record("151", 30, 60),
                      tech_record(True, 30, 60))
        for record in unreadable:
            results, _ = read([tech_record(150, 30, 60), record,
                               tech_record(152, 30, 60), tech_record(153, 30, 60)])
            self.assertEqual(results[:3], [None] * 3, record)
            self.assertEqual((results[3]["since"], results[3]["turns"]), (150, 3))

    def test_off_reads_nothing(self):
        for share in (0, 0.0, 1.0, 1.5, -0.7, True, None, "0.70", float("nan")):
            state: dict = {}
            self.assertIsNone(civ6_play.science_out_of_reach_reading(
                state, tech_record(200, 5, 60), share, 150), share)
            self.assertEqual(state, {})
        for from_turn in (0, -1, True, None, "150"):
            state = {}
            self.assertIsNone(civ6_play.science_out_of_reach_reading(
                state, tech_record(200, 5, 60), 0.70, from_turn), from_turn)
            self.assertEqual(state, {})

    def test_the_reading_is_the_domination_rules(self):
        record = tech_record(150, 51, 54, 48, 52)
        self.assertEqual(civ6_play.tech_standing(record), (51, 54))
        self.assertAlmostEqual(civ6_play.domination_tech_share(record), 51 / 54)
        self.assertIsNone(civ6_play.tech_standing(tech_record(150, 51)))
        self.assertIsNone(civ6_play.domination_tech_share(tech_record(150, 51, 0)))


class ScienceOutOfReachLine(unittest.TestCase):
    def line(self, **values):
        defaults = {"restart_science_behind_techs": 0.70,
                    "restart_science_behind_from_turn": 150,
                    "civvis_victory": "science", "speed": "GAMESPEED_ONLINE"}
        return civ6_play.science_out_of_reach_line(
            argparse.Namespace(**{**defaults, **values}))

    def test_online_turn_is_unchanged(self):
        self.assertEqual(self.line(), (0.70, 150))

    def test_the_first_turn_scales_with_game_speed(self):
        self.assertEqual(self.line(speed="GAMESPEED_QUICK"), (0.70, 201))
        self.assertEqual(self.line(speed="GAMESPEED_STANDARD"), (0.70, 300))
        self.assertEqual(self.line(speed="GAMESPEED_EPIC"), (0.70, 450))
        self.assertEqual(self.line(speed="GAMESPEED_STANDARD",
                                   restart_science_behind_from_turn=120), (0.70, 240))
        self.assertEqual(self.line(speed=None), (0.70, 150))

    def test_a_standard_speed_game_is_read_from_its_own_turn_300(self):
        share, from_turn = self.line(speed="GAMESPEED_STANDARD")
        results, _ = read([tech_record(turn, 30, 60) for turn in range(150, 300)],
                          share, from_turn)
        self.assertEqual(results, [None] * 150)
        results, _ = read([tech_record(turn, 30, 60) for turn in (300, 301, 302)],
                          share, from_turn)
        self.assertEqual((results[2]["turn"], results[2]["from_turn"]), (302, 300))

    def test_only_a_science_seat(self):
        for victory in ("domination", "religious", "culture", "diplomatic", "score",
                        "civvis", "", None):
            self.assertEqual(self.line(civvis_victory=victory), (0.0, 0), victory)

    def test_off_by_default(self):
        # No share on the args (no policy key): off whatever the lane.
        self.assertEqual(civ6_play.science_out_of_reach_line(
            argparse.Namespace(civvis_victory="science")), (0.0, 0))
        self.assertEqual(civ6_play.science_out_of_reach_line(argparse.Namespace()), (0.0, 0))
        for share in (0, 0.0, None, 1.0, 2, True, "0.70"):
            self.assertEqual(self.line(restart_science_behind_techs=share), (0.0, 0), share)

    def test_an_unset_first_turn_is_turn_150(self):
        self.assertEqual(self.line(restart_science_behind_from_turn=None), (0.70, 150))
        for from_turn in (0, -5, True, "150"):
            self.assertEqual(self.line(restart_science_behind_from_turn=from_turn),
                             (0.0, 0), from_turn)


class ScienceOutOfReachPolicy(unittest.TestCase):
    def test_parse_the_share(self):
        self.assertEqual(civ6_play.parse_science_behind_share("0.70"), 0.70)
        self.assertEqual(civ6_play.parse_science_behind_share(".65"), 0.65)
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            for value in (None, "", "0", "0.0"):
                self.assertEqual(civ6_play.parse_science_behind_share(value), 0.0, value)
        self.assertEqual(out.getvalue(), "", "unset and 0 are off, not mistakes")

    def test_a_malformed_share_is_ignored_with_a_warning(self):
        for value in ("lots", "70%", "1", "1.0", "1.5", "-0.7", "7/10", "nan", "inf", "0,70"):
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(civ6_play.parse_science_behind_share(value), 0.0, value)
            self.assertIn(f"[policy] ignoring CIVVIS_RESTART_SCIENCE_BEHIND_TECHS={value!r}",
                          out.getvalue())

    def test_parse_the_first_turn(self):
        self.assertEqual(civ6_play.parse_science_behind_from_turn("120"), 120)
        self.assertEqual(civ6_play.parse_science_behind_from_turn(None), 150)
        self.assertEqual(civ6_play.parse_science_behind_from_turn(""), 150)
        for value in ("soon", "0", "-5", "150.5"):
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(civ6_play.parse_science_behind_from_turn(value), 150, value)
            self.assertIn("[policy] ignoring CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN="
                          f"{value!r}", out.getvalue())

    def test_policy_file_and_default_off(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "policy"
            # No file, and a file that names other rules only: off, from turn 150.
            self.assertEqual(civ6_play.science_behind_policy(path), 0.0)
            self.assertEqual(civ6_play.science_behind_from_turn_policy(path), 150)
            path.write_text("CIVVIS_RESTART_DOMINATION_NO_CAPITAL_LEVEL_TECHS=0.85\n"
                            "CIVVIS_RESTART_ON_CITY_COLLAPSE=3\n")
            self.assertEqual(civ6_play.science_behind_policy(path), 0.0)
            self.assertEqual(civ6_play.science_behind_from_turn_policy(path), 150)
            for text, share, from_turn in (
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0.70\n", 0.70, 150),
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS = 0.70  # of the best rival\n"
                     "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=140 # Online\n", 0.70, 140),
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0\n", 0.0, 150),
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0.70\n"
                     "CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0\n", 0.0, 150),
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=most\n"
                     "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=late\n", 0.0, 150),
                    ("CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=140\n", 0.0, 140)):
                path.write_text(text)
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(civ6_play.science_behind_policy(path), share, text)
                    self.assertEqual(civ6_play.science_behind_from_turn_policy(path),
                                     from_turn, text)

    def test_the_start_of_game_line_names_the_rule_and_its_evidence(self):
        self.assertEqual(
            civ6_play.science_out_of_reach_notice(0.70, 150, "GAMESPEED_ONLINE"),
            "[policy] a science game holding under 70% of the best met rival's techs "
            "for 3 turns running from turn 150 is retired (lane rule 2026-11-17: on "
            "2026-11-17 all 16 wins by Science victory in the ledger had held 82% or "
            "more from turn 150; from turn 150 at Online, scaled to GAMESPEED_ONLINE)")
        standard = civ6_play.science_out_of_reach_notice(0.725, 140, "GAMESPEED_STANDARD")
        self.assertIn("under 72.5% of the best met rival's techs", standard)
        self.assertIn("running from turn 280 is retired", standard)
        self.assertIn("from turn 140 at Online, scaled to GAMESPEED_STANDARD)", standard)

    def start(self, policy: str, **values) -> tuple[SimpleNamespace, str]:
        """What `play` resolves and prints at game start under this policy file.

        Attach mode with the lock and the game stubbed out, as
        `AttachRunningTests` in test_civ6_play.py runs it: nothing here reaches
        a real game, lock or policy file.
        """
        args = SimpleNamespace(**{
            "tag": "saved-game", "lock_wait": 0.0, "attach_running": True,
            "restart_below_leader_ratio": 0.0, "civvis_victory": "science",
            "speed": "GAMESPEED_ONLINE", **values})
        out = io.StringIO()
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "policy"
            path.write_text(policy)
            with patch.dict(os.environ, {"CIVVIS_VERIFICATION_POLICY": str(path)}), \
                 patch.object(civ6_play, "hold_macos_awake"), \
                 patch.object(civ6_play.gamelock, "acquire", return_value=True), \
                 patch.object(civ6_play.gamelock, "release"), \
                 patch.object(civ6_play, "_attach_running_game", return_value=0), \
                 patch.object(civ6_play.vision, "available"), \
                 patch.object(civ6_play.launcher, "stop"), \
                 contextlib.redirect_stdout(out):
                self.assertEqual(civ6_play.play(args), 0)
        return args, out.getvalue()

    def test_game_start_without_the_key_changes_nothing(self):
        for policy in ("", "CIVVIS_RESTART_ON_CITY_COLLAPSE=3\n",
                       "CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0\n",
                       "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=140\n"):
            args, printed = self.start(policy)
            self.assertEqual(args.restart_science_behind_techs, 0.0, policy)
            self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.0, 0), policy)
            self.assertNotIn("a science game", printed, policy)

    def test_game_start_with_the_key_arms_a_science_seat_and_says_so(self):
        policy = ("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0.70  # of the best met rival\n"
                  "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=150\n")
        args, printed = self.start(policy)
        self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.70, 150))
        self.assertIn(civ6_play.science_out_of_reach_notice(0.70, 150, "GAMESPEED_ONLINE")
                      + "\n", printed)
        args, printed = self.start(policy, speed="GAMESPEED_STANDARD")
        self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.70, 300))
        self.assertIn("running from turn 300 is retired", printed)

    def test_game_start_on_another_lane_stays_off_and_silent(self):
        for victory in ("domination", "culture", "religious", "diplomatic", "score", "civvis"):
            args, printed = self.start("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0.70\n",
                                       civvis_victory=victory)
            self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.0, 0), victory)
            self.assertNotIn("a science game", printed, victory)

    def test_game_start_with_a_malformed_key_warns_and_plays_on(self):
        args, printed = self.start("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=seventy\n"
                                   "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=late\n")
        self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.0, 0))
        self.assertIn("[policy] ignoring CIVVIS_RESTART_SCIENCE_BEHIND_TECHS='seventy'", printed)
        self.assertIn("[policy] ignoring CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN='late'", printed)
        self.assertNotIn("a science game", printed)
        # A good share beside a bad first turn: on, from the default turn.
        args, printed = self.start("CIVVIS_RESTART_SCIENCE_BEHIND_TECHS=0.70\n"
                                   "CIVVIS_RESTART_SCIENCE_BEHIND_FROM_TURN=late\n")
        self.assertEqual(civ6_play.science_out_of_reach_line(args), (0.70, 150))

    def test_the_rule_is_wired_to_the_retire_path_and_off_by_default(self):
        source = Path(civ6_play.__file__).read_text()
        # `play` resolves both keys from the policy file and says so at game start ...
        self.assertIn("science_behind = science_behind_policy()", source)
        self.assertIn("args.restart_science_behind_techs = science_behind", source)
        self.assertIn("args.restart_science_behind_from_turn = science_behind_from", source)
        announced = source.split("args.restart_science_behind_from_turn = science_behind_from",
                                 1)[1][:400]
        self.assertIn('if science_behind > 0 and getattr(args, "civvis_victory", None) '
                      '== "science":\n'
                      "        print(science_out_of_reach_notice(science_behind, "
                      "science_behind_from,", announced)
        # ... and `_play` hands the lane's line to the reading on the retire path.
        self.assertIn("science_behind, science_behind_from = science_out_of_reach_line(args)",
                      source)
        applied = source.split("reading = science_out_of_reach_reading(", 1)
        self.assertEqual(len(applied), 2)
        self.assertIn("if science_behind > 0:", applied[0][-200:])
        self.assertIn("state, event, science_behind, science_behind_from,", applied[1][:120])
        self.assertIn('getattr(args, "restart_science_behind_spare_dvp", 0) or 0)', applied[1][:200])
        self.assertIn("return abandon_with_retire(reading)", applied[1][:700])


if __name__ == "__main__":
    unittest.main()
