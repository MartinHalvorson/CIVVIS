"""The clock margin: how far our lane was from winning when a live game ended."""
import gzip
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import clock_margin  # noqa: E402
import live_ledger  # noqa: E402

FAITH = "RELIGION_BUDDHISM"
OTHER = "RELIGION_JUDAISM"
LAUNCHES = ["PROJECT_LAUNCH_EARTH_SATELLITE", "PROJECT_LAUNCH_MOON_LANDING",
            "PROJECT_LAUNCH_MARS_BASE", "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"]


def city(religion):
    return {"religion": religion}


def rival(player, *, cities=0, ours=0, majority=OTHER, stages=0, svp=0, rate=0,
          culture_turns=-1):
    known = [city(FAITH)] * ours + [city(OTHER)] * (cities - ours)
    return {"player": player, "civ": f"CIVILIZATION_{player}", "cities": known,
            "public_stats": {"city_count": cities}, "religion": majority,
            "science_projects": LAUNCHES[:stages], "science_victory_points": svp,
            "science_victory_points_per_turn": rate,
            "culture_turns_to_victory": culture_turns, "at_war": False}


def board(turn, rivals, *, own=(3, 3), stages=0, svp=0, rate=0, dvp=None):
    points = [{"player": 0, "points": dvp}] if dvp is not None else []
    return {"kind": "state", "turn": turn, "founded_religion": FAITH,
            "cities": [city(FAITH)] * own[1] + [city(OTHER)] * (own[0] - own[1]),
            "science_projects": LAUNCHES[:stages], "science_victory_points": svp,
            "science_victory_points_needed": 25, "science_victory_points_per_turn": rate,
            "congress_dvp": {"points": points}, "rivals": rivals}


SEAT = {"kind": "seat", "local_player": 0}


def lost_to(player, turn, victory="VICTORY_TECHNOLOGY"):
    return {"kind": "victory", "turn": turn, "won": False, "team": player,
            "victory_type": victory}


class ReligiousLane(unittest.TestCase):
    def game(self):
        # Rival 1 is converted; rival 2 holds 6 cities, 0 of ours at t150 and
        # 2 at t170, so 2 more must turn (a majority of 6 is 4) at 2 per 20.
        early = board(150, [rival(1, cities=4, ours=4, majority=FAITH),
                            rival(2, cities=6, ours=0)])
        late = board(170, [rival(1, cities=4, ours=4, majority=FAITH),
                           rival(2, cities=6, ours=2, stages=4, svp=25, rate=3)])
        return [SEAT, early, late, lost_to(2, 170)]

    def test_shortfall_pace_and_margin(self):
        reading = clock_margin.clock_margin(self.game(), "religious")
        lane = reading["lanes"]["religious"]
        self.assertEqual(lane["majors_short"], 1)
        self.assertEqual(lane["cities_short"], 2)
        self.assertEqual(lane["turns_short"], 20.0)
        self.assertEqual(reading["rival_clock"]["turns"], 0.0)
        self.assertEqual(reading["rival_clock"]["player"], 2)
        self.assertEqual(lane["margin"], -20.0)
        self.assertEqual((lane["finish_turn"], reading["rival_clock"]["finish_turn"]), (190.0, 170.0))
        # Ourselves and rival 1 whole, rival 2 two of the four it needs: 2.5 of 3.
        self.assertAlmostEqual(lane["progress"], round((1 + 1 + 2 / 4) / 3, 3))

    def test_no_progress_is_no_estimate(self):
        events = self.game()
        events[1] = board(150, [rival(1, cities=4, ours=4, majority=FAITH),
                                rival(2, cities=6, ours=2)])
        lane = clock_margin.clock_margin(events, "religious")["lanes"]["religious"]
        self.assertIsNone(lane["turns_short"])
        self.assertIsNone(lane["margin"])
        self.assertEqual(lane["cities_short"], 2)

    def test_a_majority_the_host_denies_is_at_least_one_city_short(self):
        # Every known city follows us but the host reads the majority as not
        # ours (cities we have not seen): one city, not zero.
        reading = clock_margin.religious_short(clock_margin.board_reading(
            board(100, [rival(1, cities=5, ours=3, majority=OTHER)]), 0))
        self.assertEqual(reading["cities_short"], 1)

    def test_our_own_cities_count(self):
        reading = clock_margin.religious_short(clock_margin.board_reading(
            board(100, [rival(1, cities=4, ours=4, majority=FAITH)], own=(4, 2)), 0))
        self.assertEqual((reading["majors_short"], reading["cities_short"]), (1, 1))

    def test_a_win_is_zero_short_and_the_rival_clock_is_estimated(self):
        events = self.game()
        events[-1] = {"kind": "victory", "turn": 170, "won": True, "team": 0,
                      "victory_type": "VICTORY_RELIGIOUS"}
        events[2]["rivals"][1].update(science_victory_points=10)
        reading = clock_margin.clock_margin(events, "religious")
        lane = reading["lanes"]["religious"]
        self.assertEqual((lane["turns_short"], lane["progress"]), (0.0, 1.0))
        # Rival 2 is flying: 15 light years at 3 a turn.
        self.assertEqual(reading["rival_clock"]["turns"], 5.0)
        self.assertEqual(lane["margin"], 5.0)


class OtherLanes(unittest.TestCase):
    def test_a_win_by_another_victory_tops_the_scale(self):
        events = [SEAT, board(170, [], stages=2),
                  {"kind": "victory", "turn": 170, "won": True, "team": 0,
                   "victory_type": "VICTORY_DIPLOMATIC"}]
        lane = clock_margin.clock_margin(events, "science")["lanes"]["science"]
        self.assertEqual((lane["progress"], lane["turns_short"]), (1.0, 0.0))
        self.assertEqual(lane["launches_short"], 2)

    def test_science_in_flight(self):
        events = [SEAT, board(170, [], stages=4, svp=10, rate=3), lost_to(2, 170)]
        lane = clock_margin.clock_margin(events, "science")["lanes"]["science"]
        self.assertEqual((lane["launches_short"], lane["distance_short"]), (0, 15.0))
        self.assertEqual(lane["turns_short"], 5.0)
        self.assertAlmostEqual(lane["progress"], round((4 + 10 / 25) / 5, 3))

    def test_science_before_the_launch_uses_the_pace_of_its_launches(self):
        events = [SEAT, board(140, [], stages=1), board(150, [], stages=2),
                  board(160, [], stages=2), lost_to(2, 160)]
        lane = clock_margin.clock_margin(events, "science")["lanes"]["science"]
        # Two launches left at one per 10 turns, then 25 light years at base speed.
        self.assertEqual(lane["turns_short"], 2 * 10 + 25)

    def test_diplomatic_points(self):
        events = [SEAT, board(150, [], dvp=10), board(170, [], dvp=14), lost_to(2, 170)]
        lane = clock_margin.clock_margin(events, "diplomatic")["lanes"]["diplomatic"]
        self.assertEqual(lane["points_short"], 6)
        self.assertEqual(lane["turns_short"], 30.0)

    def test_elimination_is_a_loss_with_no_winner(self):
        events = [SEAT, board(100, [rival(1, cities=5)]),
                  {"kind": "defeat", "ours": True, "player": 0, "turn": 100}]
        reading = clock_margin.clock_margin(events)
        self.assertIs(reading["won"], False)
        self.assertTrue(reading["eliminated"])
        self.assertIsNone(reading["rival_clock"]["turns"])
        self.assertEqual(set(reading["lanes"]), set(clock_margin.LANES))


class LedgerWiring(unittest.TestCase):
    def write(self, events, gz=False) -> Path:
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp)
        path = tmp / ("events.jsonl.gz" if gz else "events.jsonl")
        text = "".join(json.dumps(event) + "\n" for event in events)
        if gz:
            with gzip.open(path, "wt") as handle:
                handle.write(text)
        else:
            path.write_text(text)
        return path

    def test_every_new_summary_carries_it(self):
        path = self.write(ReligiousLane().game())
        marks = civ6_ladder.progress_marks(path)
        self.assertIn(("clock_margin", civ6_ladder.clock_margin_marks), civ6_ladder.SUMMARY_MARKS)
        summary = {"victory_target": "religious", **marks}
        self.assertEqual(clock_margin.lane_margin(summary)["margin"], -20.0)

    def test_an_old_summary_is_read_from_its_ledger_log(self):
        path = self.write(ReligiousLane().game(), gz=True)
        (path.parent / "summary.json").write_text(json.dumps({"victory_target": "religious"}))
        margin = clock_margin.run_margin(path.parent)
        self.assertEqual((margin["lane"], margin["turns_short"]), ("religious", 20.0))
        self.assertEqual(clock_margin.batch_summary([margin])["losses_within_20"], 1)

    def test_the_live_ledger_grades_every_game_on_it(self):
        path = self.write(ReligiousLane().game())
        body = {"tag": "civvis-x", "_dir": path.parent, "victory_target": "religious",
                "last_turn": 170}
        game = live_ledger.join_game([body])
        self.assertEqual((game["clock_turns_short"], game["clock_margin"]), (20.0, -20.0))
        self.assertEqual(game["clock_rival_finish"], 170.0)
        keys = [key for key, _, _ in live_ledger.SCREEN_KPIS]
        for key in ("clock_progress", "clock_margin", "clock_rival_finish"):
            self.assertIn(key, keys)
        # A summary that carries the reading is read, not the log.
        body["clock_margin"] = {"lanes": {"religious": {"progress": 0.5, "margin": 3.0}}}
        self.assertEqual(live_ledger.join_game([body])["clock_margin"], 3.0)

    def test_no_lane_no_reading(self):
        self.assertIsNone(clock_margin.lane_margin({"clock_margin": {"lanes": {}}}))
        self.assertIsNone(clock_margin.lane_margin({"victory_target": "religious"}))


if __name__ == "__main__":
    unittest.main()
