"""The operator's early-city restart rule (2026-11-04).

"if we lose our capital or one of our first 3 cities we should just restart the
game": `early_city_reading` folds every agent `state` record's `cities` list
into a guard of our first N founded cities, and retires the game once one of
them has been missing across two game turns. The host policy file's
CIVVIS_RESTART_ON_EARLY_CITY_LOST turns it on.
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

US = 0
CAPITAL = {"id": 65536, "name": "Seoul", "capital": True, "original_capital": True,
           "original_owner": US}
SECOND = {"id": 131073, "name": "Busan", "capital": False, "original_capital": False,
          "original_owner": US}
THIRD = {"id": 196610, "name": "Gyeongju", "capital": False, "original_capital": False,
         "original_owner": US}
FOURTH = {"id": 262147, "name": "Incheon", "capital": False, "original_capital": False,
          "original_owner": US}
TAKEN = {"id": 70000, "name": "Babylon", "capital": False, "original_capital": True,
         "original_owner": 3}


def state_record(turn: int, *cities: dict, ctx: str = "agent") -> dict:
    return {"kind": "state", "ctx": ctx, "turn": turn, "cities": list(cities)}


def fold(state: dict, *records: dict, count: int = 3) -> list:
    return [civ6_play.early_city_reading(state, record, count) for record in records]


class EarlyCityReadingTest(unittest.TestCase):
    def test_off_reads_nothing(self):
        for count in (0, -1, True, None):
            state: dict = {}
            self.assertIsNone(civ6_play.early_city_reading(
                state, state_record(5, CAPITAL), count), count)
            self.assertEqual(state, {})

    def test_only_agent_state_records_count(self):
        state: dict = {}
        self.assertIsNone(civ6_play.early_city_reading(
            state, {"kind": "turn", "ctx": "agent", "turn": 5, "cities": 1}, 3))
        self.assertIsNone(civ6_play.early_city_reading(
            state, state_record(5, CAPITAL, ctx="ui"), 3))
        self.assertEqual(state, {})

    def test_guard_takes_the_first_founded_cities_in_order(self):
        state: dict = {}
        fold(state, state_record(1, CAPITAL), state_record(20, CAPITAL, SECOND),
             state_record(40, CAPITAL, SECOND, TAKEN, THIRD, FOURTH))
        self.assertEqual([entry["id"] for entry in state["early_cities"]],
                         [CAPITAL["id"], SECOND["id"], THIRD["id"]])
        self.assertTrue(state["early_cities"][0]["capital"])
        self.assertEqual(state["early_city_owner"], US)

    def test_a_captured_city_is_never_guarded(self):
        state: dict = {}
        fold(state, state_record(30, CAPITAL, TAKEN, SECOND, THIRD))
        self.assertNotIn(TAKEN["id"], [entry["id"] for entry in state["early_cities"]])

    def test_losing_a_later_city_never_fires(self):
        state: dict = {}
        readings = fold(state, state_record(50, CAPITAL, SECOND, THIRD, FOURTH),
                        state_record(60, CAPITAL, SECOND, THIRD),
                        state_record(61, CAPITAL, SECOND, THIRD))
        self.assertEqual(readings, [None, None, None])

    def test_a_lost_capital_fires_on_the_next_turn(self):
        state: dict = {}
        readings = fold(state, state_record(50, CAPITAL, SECOND, THIRD),
                        state_record(51, SECOND, THIRD),
                        state_record(51, SECOND, THIRD),
                        state_record(52, SECOND, THIRD))
        self.assertEqual(readings[:3], [None, None, None])
        verdict = readings[3]
        self.assertTrue(verdict["fires"])
        self.assertEqual(verdict["rule"], "early_city_lost")
        self.assertEqual(verdict["turn"], 52)
        self.assertEqual(verdict["lost_turn"], 51)
        self.assertEqual([entry["name"] for entry in verdict["lost"]], ["Seoul"])

    def test_losing_the_third_city_fires(self):
        state: dict = {}
        readings = fold(state, state_record(80, CAPITAL, SECOND, THIRD, FOURTH),
                        state_record(81, CAPITAL, SECOND, FOURTH),
                        state_record(82, CAPITAL, SECOND, FOURTH))
        self.assertEqual([entry["id"] for entry in readings[2]["lost"]], [THIRD["id"]])

    def test_a_loss_before_the_guard_is_full_fires(self):
        state: dict = {}
        readings = fold(state, state_record(10, CAPITAL), state_record(25, CAPITAL, SECOND),
                        state_record(30, CAPITAL), state_record(31, CAPITAL))
        self.assertTrue(readings[3]["fires"])

    def test_a_city_back_before_the_next_turn_clears_the_loss(self):
        state: dict = {}
        readings = fold(state, state_record(50, CAPITAL, SECOND, THIRD),
                        state_record(51, CAPITAL, THIRD),
                        state_record(52, CAPITAL, SECOND, THIRD),
                        state_record(53, CAPITAL, SECOND, THIRD))
        self.assertEqual(readings, [None, None, None, None])
        self.assertNotIn("early_city_loss", state)

    def test_an_empty_or_unreadable_list_is_no_loss(self):
        state: dict = {}
        readings = fold(state, state_record(50, CAPITAL, SECOND, THIRD),
                        state_record(51), {"kind": "state", "ctx": "agent", "turn": 52},
                        state_record(53, {"name": "no id"}),
                        state_record(54, CAPITAL, SECOND, THIRD))
        self.assertEqual(readings, [None] * 5)
        self.assertNotIn("early_city_loss", state)

    def test_no_founder_until_the_capital_is_seen(self):
        state: dict = {}
        self.assertIsNone(civ6_play.early_city_reading(state, state_record(5, SECOND), 3))
        self.assertNotIn("early_city_owner", state)

    def test_a_guard_of_one_is_the_capital_alone(self):
        state: dict = {}
        readings = fold(state, state_record(50, CAPITAL, SECOND),
                        state_record(51, CAPITAL), state_record(52, CAPITAL), count=1)
        self.assertEqual(readings, [None, None, None])


class EarlyCityPolicyTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(civ6_play.parse_early_city_count("3"), 3)
        for value in (None, "", "0", "x", "-1", "1.5"):
            self.assertEqual(civ6_play.parse_early_city_count(value), 0, value)

    def test_policy_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "policy"
            path.write_text("# comment\nCIVVIS_RESTART_ON_EARLY_CITY_LOST=3\n")
            self.assertEqual(civ6_play.early_city_count_policy(path), 3)
            path.write_text("CIVVIS_SPEED=GAMESPEED_ONLINE\n")
            self.assertEqual(civ6_play.early_city_count_policy(path), 0)

    def test_flag_is_offered(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), self.assertRaises(SystemExit):
            civ6_play.main(["--help"])
        self.assertIn("--restart-on-early-city-lost", out.getvalue())



def religion_record(turn: int, followers, religion="RELIGION_BUDDHISM", **extra) -> dict:
    return {"kind": "state", "ctx": "agent", "turn": turn, "founded_religion": religion,
            "cities_following_religion": followers, **extra}


class ReligionLostReading(unittest.TestCase):
    def read(self, records):
        state: dict = {}
        return [civ6_play.religion_lost_reading(state, record) for record in records]

    def test_fires_after_three_turns_with_no_followers(self):
        results = self.read([religion_record(40, 2), religion_record(50, 0),
                             religion_record(51, 0), religion_record(52, 0)])
        self.assertEqual(results[:3], [None, None, None])
        self.assertEqual(results[3]["rule"], "religion_lost")
        self.assertEqual(results[3]["turn"], 52)
        self.assertEqual(results[3]["lost_turn"], 50)
        self.assertEqual(results[3]["religion"], "RELIGION_BUDDHISM")

    def test_several_records_in_one_turn_count_once(self):
        results = self.read([religion_record(40, 1)] + [religion_record(50, 0)] * 5
                            + [religion_record(51, 0)] * 5)
        self.assertEqual(results, [None] * 11)

    def test_a_returning_follower_clears_the_count(self):
        results = self.read([religion_record(40, 1), religion_record(50, 0),
                             religion_record(51, 0), religion_record(52, 1),
                             religion_record(53, 0), religion_record(54, 0)])
        self.assertEqual(results, [None] * 6)

    def test_founding_lag_does_not_fire(self):
        self.assertEqual(self.read([religion_record(t, 0) for t in range(30, 40)]), [None] * 10)

    def test_no_religion_never_fires(self):
        self.assertEqual(self.read([religion_record(t, 0, religion=None) for t in range(30, 40)]),
                         [None] * 10)

    def test_unreadable_records_are_skipped(self):
        state: dict = {"religion_followed": True}
        for record in ({"kind": "turn", "ctx": "agent", "turn": 60},
                       religion_record(60, 0, ctx="autoclose"),
                       religion_record(60, None), religion_record(60, True),
                       religion_record("60", 0)):
            self.assertIsNone(civ6_play.religion_lost_reading(state, record))
        self.assertNotIn("religion_lost_since", state)

    def test_policy_key_reads_one_as_on(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "policy"
            for text, expected in (("CIVVIS_RESTART_ON_RELIGION_LOST=1\n", True),
                                   ("CIVVIS_RESTART_ON_RELIGION_LOST=0\n", False),
                                   ("OTHER=1\n", False)):
                path.write_text(text)
                self.assertIs(civ6_play.religion_lost_policy(path), expected)



def founding_record(turn: int, religion=None, pending=False,
                    exhausted=("GREAT_PERSON_CLASS_PROPHET",), **extra) -> dict:
    record = {"kind": "state", "ctx": "agent", "turn": turn, "founded_religions": [],
              "prophet_pending": pending, "great_person_exhausted": list(exhausted), **extra}
    if religion is not None:  # the mod leaves the key out until we found one
        record["founded_religion"] = religion
    return record


class ReligionNeverFoundedReading(unittest.TestCase):
    def read(self, records):
        state: dict = {}
        return [civ6_play.religion_never_founded_reading(state, record) for record in records]

    def test_fires_after_three_turns_with_no_prophet_left(self):
        results = self.read([founding_record(t) for t in (39, 40, 41)])
        self.assertEqual(results[:2], [None, None])
        self.assertEqual(results[2]["rule"], "religion_never_founded")
        self.assertEqual(results[2]["turn"], 41)
        self.assertEqual(results[2]["lost_turn"], 39)

    def test_prophets_still_available_never_fires(self):
        self.assertEqual(self.read([founding_record(t, exhausted=()) for t in range(30, 40)]),
                         [None] * 10)

    def test_a_pending_prophet_holds_the_rule_off(self):
        records = [founding_record(32), founding_record(33, pending=True),
                   founding_record(34, pending=True), founding_record(35, religion="RELIGION_ISLAM"),
                   founding_record(36, religion="RELIGION_ISLAM")]
        self.assertEqual(self.read(records), [None] * 5)

    def test_a_pending_prophet_restarts_the_count(self):
        results = self.read([founding_record(32), founding_record(33),
                             founding_record(34, pending=True),
                             founding_record(35), founding_record(36)])
        self.assertEqual(results, [None] * 5)

    def test_founded_religion_never_fires(self):
        self.assertEqual(self.read([founding_record(t, religion="RELIGION_ISLAM")
                                    for t in range(30, 40)]), [None] * 10)

    def test_a_record_without_the_pending_field_says_nothing(self):
        record = founding_record(40)
        del record["prophet_pending"]
        self.assertEqual(self.read([record] * 6), [None] * 6)

    def test_unreadable_records_are_skipped(self):
        state: dict = {}
        missing = founding_record(40)
        del missing["founded_religions"]
        for record in ({"kind": "turn", "ctx": "agent", "turn": 40},
                       founding_record(40, ctx="autoclose"), missing,
                       {**founding_record(40), "great_person_exhausted": None},
                       founding_record("40")):
            self.assertIsNone(civ6_play.religion_never_founded_reading(state, record))
        self.assertNotIn("no_religion_since", state)


if __name__ == "__main__":
    unittest.main()
