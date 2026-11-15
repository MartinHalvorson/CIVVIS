"""The city-collapse retire rule (lane rule, 2026-11-15; off unless the policy names a count).

`city_collapse_reading` folds every agent `state` record's `cities` list into
the most cities held this run, and retires the game once the count has fallen
by at least N from that peak AND stands at or below half of it, on records of
three game turns running. The host policy file's
CIVVIS_RESTART_ON_CITY_COLLAPSE turns it on.
"""

from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402


def city(index: int) -> dict:
    return {"id": 65536 * (index + 1) + index, "name": f"City {index}",
            "original_owner": 0}


def state_record(turn: int, count: int, ctx: str = "agent") -> dict:
    return {"kind": "state", "ctx": ctx, "turn": turn,
            "cities": [city(index) for index in range(count)]}


def play(series: list[tuple[int, int]], last_turn: int, lost: int = 3,
         frames: int = 1) -> tuple[dict | None, dict]:
    """Fold a recorded game, given as (turn, count-from-that-turn) steps, one
    or more records a turn; the first verdict and the carried state."""
    state: dict = {}
    steps = dict(series)
    count = 0
    for turn in range(series[0][0], last_turn + 1):
        count = steps.get(turn, count)
        for _ in range(frames):
            verdict = civ6_play.city_collapse_reading(state, state_record(turn, count), lost)
            if verdict is not None:
                return verdict, state
    return None, state


#: Our city count by turn in four live games, as their `turn` records had it
#: (each pair is the turn a new count starts).
#: Pericles, 2026-11-15: five of six cities lost to a joint war, t112-123.
COLLAPSED_075925Z = ([(1, 0), (2, 1), (18, 2), (22, 3), (37, 4), (46, 3), (55, 4), (70, 5),
                      (72, 6), (112, 5), (113, 4), (117, 3), (120, 2), (123, 1)], 129)
#: Trajan, 2026-11-13: six cities, three by t127, played on to t194 and lost.
COLLAPSED_162518Z = ([(1, 0), (2, 1), (22, 2), (32, 3), (65, 2), (86, 3), (91, 4), (98, 5),
                      (106, 6), (122, 5), (126, 4), (127, 3), (150, 4), (157, 3),
                      (188, 2)], 194)
#: The two WON games that came closest: a King win that fell from four cities
#: to three at t58 (75% of its peak, the lowest any winner stood) ...
WON_071051Z = ([(1, 0), (2, 1), (25, 2), (41, 3), (44, 4), (58, 3), (74, 4), (88, 5),
                (92, 6), (110, 7), (131, 8), (142, 9), (147, 10)], 222)
#: ... and the only winner that ever stood two cities under its peak.
WON_105550Z = ([(1, 0), (2, 1), (24, 2), (54, 3), (62, 4), (71, 5), (75, 6), (83, 7),
                (100, 8), (103, 9), (109, 10), (180, 11), (185, 12), (201, 13), (203, 12),
                (204, 11), (210, 12), (211, 13), (212, 12), (216, 13), (219, 12),
                (221, 13), (228, 15), (231, 17), (233, 18), (234, 19), (237, 18),
                (238, 19), (241, 20), (242, 21), (243, 20), (246, 21), (249, 20)], 249)


class CityCollapseReadingTest(unittest.TestCase):
    def test_off_reads_nothing(self):
        for lost in (0, -1, True, None, "3"):
            state: dict = {}
            self.assertIsNone(civ6_play.city_collapse_reading(
                state, state_record(120, 1), lost), lost)
            self.assertEqual(state, {})

    def test_only_agent_state_records_count(self):
        state: dict = {}
        for record in ({"kind": "turn", "ctx": "agent", "turn": 5, "cities": 6},
                       state_record(5, 6, ctx="ui"),
                       {"kind": "state", "ctx": "agent", "turn": 5},
                       {"kind": "state", "ctx": "agent", "turn": True,
                        "cities": [city(0)]},
                       {"kind": "state", "ctx": "agent", "turn": "5",
                        "cities": [city(0)]}):
            self.assertIsNone(civ6_play.city_collapse_reading(state, record, 3))
        self.assertEqual(state, {})

    def test_the_live_collapse_is_retired_on_its_third_turn_at_half(self):
        verdict, state = play(*COLLAPSED_075925Z)
        self.assertEqual(verdict, {"rule": "city_collapse", "turn": 119, "since": 117,
                                   "peak": 6, "held": 3, "lost": 3, "line": 3,
                                   "fires": True})
        self.assertEqual(state["city_peak"], 6)
        # Ten turns before the game's own end at t129, with one city left.
        self.assertEqual(COLLAPSED_075925Z[1] - verdict["turn"], 10)

    def test_a_long_lost_game_is_retired_sixty_five_turns_early(self):
        verdict, _ = play(*COLLAPSED_162518Z)
        self.assertEqual((verdict["turn"], verdict["peak"], verdict["held"]), (129, 6, 3))
        self.assertEqual(COLLAPSED_162518Z[1] - verdict["turn"], 65)

    def test_the_winners_that_came_closest_play_on(self):
        for name, game in (("071051Z", WON_071051Z), ("105550Z", WON_105550Z)):
            for lost in (3, 2, 1):
                with self.subTest(game=name, lost=lost):
                    verdict, state = play(*game, lost=lost)
                    self.assertIsNone(verdict)
                    self.assertNotIn("city_collapse_since", state)

    def test_both_halves_of_the_rule_are_needed(self):
        for peak, now, fires in (
            (6, 3, True),      # three lost, exactly half
            (5, 2, True),      # three lost, under half
            (9, 4, True),
            (10, 7, False),    # three lost, still 70% of the peak
            (7, 4, False),     # three lost, 57%
            (4, 2, False),     # half, but only two lost
            (5, 3, False),     # two lost, 60%
            (3, 1, False),     # a third of the peak, but two lost
            (3, 0, False),     # an empty list is unreadable, not a collapse
        ):
            with self.subTest(peak=peak, now=now):
                verdict, _ = play([(1, peak), (50, now)], 60)
                self.assertEqual(verdict is not None, fires)
                if fires:
                    self.assertEqual((verdict["since"], verdict["turn"]), (50, 52))

    def test_the_policy_count_is_the_cities_lost(self):
        game = ([(1, 8), (50, 4)], 60)       # four lost, half
        self.assertIsNotNone(play(*game, lost=4)[0])
        self.assertIsNotNone(play(*game, lost=3)[0])
        self.assertIsNone(play(*game, lost=5)[0])

    def test_three_turns_running_not_three_records(self):
        # Three records a turn, as a live turn's frames are: still turn 52.
        verdict, _ = play([(1, 6), (50, 3)], 60, frames=3)
        self.assertEqual((verdict["since"], verdict["turn"]), (50, 52))
        state: dict = {}
        civ6_play.city_collapse_reading(state, state_record(49, 6), 3)
        for _ in range(9):
            self.assertIsNone(civ6_play.city_collapse_reading(state, state_record(50, 3), 3))
        self.assertIsNone(civ6_play.city_collapse_reading(state, state_record(51, 3), 3))
        self.assertIsNotNone(civ6_play.city_collapse_reading(state, state_record(52, 3), 3))

    def test_a_city_won_back_starts_the_count_again(self):
        verdict, state = play([(1, 6), (50, 3), (52, 4), (60, 3)], 61)
        self.assertIsNone(verdict)
        self.assertEqual(state["city_collapse_since"], 60)
        verdict, _ = play([(1, 6), (50, 3), (52, 4), (60, 3)], 70)
        self.assertEqual((verdict["since"], verdict["turn"]), (60, 62))

    def test_the_peak_is_the_most_ever_held_captures_included(self):
        # Four founded, four captured by t80, five lost by t100.
        verdict, state = play([(1, 4), (80, 8), (100, 3)], 110)
        self.assertEqual((verdict["peak"], verdict["held"], verdict["lost"]), (8, 3, 5))
        self.assertEqual(state["city_peak"], 8)

    def test_an_unreadable_record_neither_counts_nor_clears(self):
        state: dict = {}
        civ6_play.city_collapse_reading(state, state_record(49, 6), 3)
        self.assertIsNone(civ6_play.city_collapse_reading(state, state_record(50, 3), 3))
        for record in (state_record(51, 0),
                       {"kind": "state", "ctx": "agent", "turn": 51, "cities": [{"name": "x"}]},
                       {"kind": "state", "ctx": "agent", "turn": 51, "cities": "partial"}):
            self.assertIsNone(civ6_play.city_collapse_reading(state, record, 3))
            self.assertEqual(state, {"city_peak": 6, "city_collapse_since": 50})
        self.assertIsNotNone(civ6_play.city_collapse_reading(state, state_record(52, 3), 3))

    def test_a_city_listed_twice_is_one_city(self):
        state: dict = {}
        record = state_record(10, 3)
        record["cities"] = record["cities"] + record["cities"]
        civ6_play.city_collapse_reading(state, record, 3)
        self.assertEqual(state["city_peak"], 3)


class CityCollapsePolicyTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(civ6_play.parse_city_collapse("3"), 3)
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            for value in (None, "", "0", "x", "-1", "1.5"):
                self.assertEqual(civ6_play.parse_city_collapse(value), 0, value)
        self.assertIn("ignoring CIVVIS_RESTART_ON_CITY_COLLAPSE='x'", out.getvalue())

    def test_policy_file_and_default_off(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "policy"
            path.write_text("# comment\nCIVVIS_RESTART_ON_CITY_COLLAPSE=3\n")
            self.assertEqual(civ6_play.city_collapse_policy(path), 3)
            path.write_text("CIVVIS_RESTART_ON_EARLY_CITY_LOST=3\n")
            self.assertEqual(civ6_play.city_collapse_policy(path), 0)

    def test_flag_is_offered(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), self.assertRaises(SystemExit):
            civ6_play.main(["--help"])
        self.assertIn("--restart-on-city-collapse", out.getvalue())

    def test_the_rule_is_wired_to_the_retire_path_and_off_by_default(self):
        source = Path(civ6_play.__file__).read_text()
        self.assertIn('ap.add_argument("--restart-on-city-collapse", type=int, default=None',
                      source)
        self.assertIn("city_collapse = (city_collapse_policy() if city_collapse is None",
                      source)
        self.assertIn('city_collapse = getattr(args, "restart_on_city_collapse", None) or 0',
                      source)
        applied = source.split("reading = city_collapse_reading(state, event, city_collapse)", 1)
        self.assertEqual(len(applied), 2)
        self.assertIn("if city_collapse > 0:", applied[0][-200:])
        self.assertIn("return abandon_with_retire(reading)", applied[1][:700])


if __name__ == "__main__":
    unittest.main()
