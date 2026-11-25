#!/usr/bin/env python3
"""Chaos Monkey's record (`tools/chaos_monkey.py`): what it makes of the
games, and that the record in the tree says only what its figures give.

    python3 -m unittest discover -s tools -p test_chaos_monkey.py
"""

from __future__ import annotations

import copy
import json
import pathlib
import tempfile
import unittest

import chaos_monkey

ROOT = pathlib.Path(__file__).resolve().parent.parent
TURNS = [1, 10, 20, 30, 30]


def civ(name: str, pop: list[int], **more) -> dict:
    """One civilization of a game, as `civvis chaos-monkey` writes it."""
    flat = lambda value: [value] * len(pop)
    return {
        "civ": name,
        "alive": True,
        "won": False,
        "peak_pop": max(pop),
        "peak_turn": 30,
        "low_pop": max(pop),
        "cities_lost": 0,
        "cities_taken": 0,
        "war_turns": 0,
        "lost": {"citizens": 0, "tiles": 0, "units": 0},
        "shelter": {"cities": 2, "immune": 0, "flood": 0, "drought": 0, "coast": 0, "floodplains": 0, "by_volcano": 0, "on_coast": 0},
        "series": {
            "pop": pop,
            "cities": flat(2),
            "score": [value * 5 for value in pop],
            "techs": flat(10),
            "army": flat(4),
            "builders": flat(1),
            "improved": flat(10),
            "pillaged": flat(0),
            "gold": flat(50),
        },
        **more,
    }


def game(seed: int, frequency: float, civs: list[dict], **more) -> dict:
    return {
        "seed": seed,
        "frequency": frequency,
        "intensity": 2,
        "players": len(civs),
        "max_turns": 30,
        "ended": 30,
        "winner": civs[0]["civ"],
        "victory": "science",
        "draw": False,
        "disasters": {"river_flood": int(2 * frequency), "drought": int(frequency)},
        "storms_peak": 0,
        "droughts_peak": 0,
        "climate_phase": 0,
        "wars": 2,
        "nuclear_strikes": 0,
        "sample_turns": TURNS,
        "civs": civs,
        **more,
    }


def batch(seeds: int = 4, hurt: dict | None = None) -> list[dict]:
    """Every seed at 1x, 10x and 100x. Two civilizations a world; under the
    storm each keeps `hurt[frequency]` of its calm people, and Rome, which
    has the cities, keeps a little more than Mali."""
    hurt = hurt or {1.0: 1.0, 10.0: 0.95, 100.0: 0.5}
    games = []
    for seed in range(1, seeds + 1):
        for frequency, share in hurt.items():
            def people(most: int, edge: float) -> list[int]:
                kept = min(1.0, share * edge) if frequency > 1 else 1.0
                return [max(1, round(most * kept * step)) for step in (0.1, 0.4, 0.7, 1.0, 1.0)]
            rome = civ("Rome", people(40 + seed, 1.1))
            rome["series"]["cities"] = [4] * 5
            mali = civ("Mali", people(40 + seed, 0.9))
            games.append(game(seed, frequency, [rome, mali]))
    return games


class TheGames(unittest.TestCase):
    def test_a_rerun_replaces_the_game_it_reran(self):
        with tempfile.TemporaryDirectory() as scratch:
            first = pathlib.Path(scratch) / "a.jsonl"
            second = pathlib.Path(scratch) / "b.jsonl"
            games = batch(seeds=1)
            first.write_text("[0.1s] not a game\n" + "".join(json.dumps(g) + "\n" for g in games), encoding="utf-8")
            again = copy.deepcopy(games[0])
            again["wars"] = 9
            second.write_text(json.dumps(again) + "\n", encoding="utf-8")
            read = chaos_monkey.load([str(first), str(second)])
        self.assertEqual([(g["seed"], g["frequency"]) for g in read], [(1, 1.0), (1, 10.0), (1, 100.0)])
        self.assertEqual(read[0]["wars"], 9)

    def test_a_game_that_ended_early_stands_as_it_ended(self):
        early = game(1, 1.0, [civ("Rome", [4, 9, 9])], sample_turns=[1, 10, 14], ended=14)
        read = chaos_monkey.grid(early, [1, 10, 20, 30], lambda at: chaos_monkey.world(early, "pop", at))
        self.assertEqual(read, [4.0, 9.0, 9.0, 9.0])
        # The end of a full game is its last sample, not the sample the turn
        # limit happened to share with it.
        full = game(1, 1.0, [civ("Rome", [4, 9, 20, 30, 31])])
        self.assertEqual(chaos_monkey.end(full)["people"], 31.0)


class TheRecord(unittest.TestCase):
    def test_a_world_under_chaos_is_read_against_its_calm_self(self):
        data = chaos_monkey.record(batch())
        self.assertEqual([dose["frequency"] for dose in data["doses"]], [1, 10, 100])
        self.assertEqual([dose["games"] for dose in data["doses"]], [4, 4, 4])
        calm, storm, worst = data["doses"]
        # The calm world is the whole of itself, in every measure.
        self.assertEqual(set(calm["kept"].values()), {1.0})
        # The share is the storm's total over the same seeds' calm total.
        games = batch()
        people = lambda f: sum(c["series"]["pop"][-1] for g in games if g["frequency"] == f for c in g["civs"])
        self.assertAlmostEqual(worst["kept"]["people"], people(100.0) / people(1.0), places=3)
        self.assertLess(worst["kept"]["people"], storm["kept"]["people"])
        # What struck is a mean a game, by kind and in all.
        self.assertEqual(worst["classes"], {"drought": 100.0, "river_flood": 200.0})
        self.assertEqual(worst["disasters"], 300.0)
        # Every series has a figure for every listed turn.
        self.assertEqual(data["turns"], [1, 10, 20, 30])
        for dose in data["doses"]:
            for series in dose["series"].values():
                self.assertEqual(len(series), len(data["turns"]))

    def test_there_is_no_record_without_the_calm_worlds(self):
        self.assertIsNone(chaos_monkey.record([]))
        with self.assertRaises(ValueError):
            chaos_monkey.record([g for g in batch() if g["frequency"] != 1.0])

    def test_losing_half_its_people_needs_people_to_lose(self):
        fallen = civ("Rome", [4, 20, 30, 12, 12], low_pop=12)
        steady = civ("Mali", [4, 20, 30, 28, 28], low_pop=28)
        hamlet = civ("Nubia", [1, 2, 4, 2, 2], low_pop=2)
        self.assertTrue(chaos_monkey.collapsed(fallen))
        self.assertFalse(chaos_monkey.collapsed(steady))
        self.assertFalse(chaos_monkey.collapsed(hamlet))

    def test_an_outcome_nobody_met_is_not_drawn(self):
        data = chaos_monkey.record(batch())
        # Nobody was halved, wiped out, captured or struck in these games:
        # what is left to draw is the wars and how the people are shared.
        self.assertEqual([outcome["key"] for outcome in data["outcomes"]], ["wars", "leader"])
        self.assertEqual(data["doses"][0]["outcomes"]["climate"], 0.0)


class WhoComesThrough(unittest.TestCase):
    def test_a_difference_is_two_halves_of_the_storm_worlds(self):
        found = chaos_monkey.factors(batch(seeds=16))
        spread = next(factor for factor in found if factor["key"] == "spread")
        # 16 seeds, the one frequency of the three that is a storm (100x),
        # two civilizations: 32 pairs, half of them on each side.
        self.assertEqual((spread["more"]["n"], spread["fewer"]["n"]), (16, 16))
        # Rome has the cities and keeps more of its people.
        self.assertGreater(spread["more"]["kept"], spread["fewer"]["kept"])
        # Widest gap first.
        gaps = [abs(factor["more"]["kept"] - factor["fewer"]["kept"]) for factor in found]
        self.assertEqual(gaps, sorted(gaps, reverse=True))

    def test_too_few_civilizations_is_no_difference(self):
        self.assertEqual(chaos_monkey.factors(batch(seeds=2)), [])

    def test_a_world_that_shrugs_the_disasters_off_is_not_read(self):
        # 10x is under the storm line: only the 100x worlds are paired.
        self.assertEqual({game["frequency"] for _, _, game, _ in chaos_monkey.pairs(batch())}, {100.0})


class TheLessons(unittest.TestCase):
    def lessons(self, hurt: dict) -> dict:
        data = chaos_monkey.record(batch(seeds=16, hurt=hurt))
        return {lesson["key"]: lesson for lesson in data["lessons"]}

    def test_a_lesson_is_written_only_when_the_games_bear_it_out(self):
        said = self.lessons({1.0: 1.0, 10.0: 0.95, 100.0: 0.5})
        self.assertEqual(said["absorbs"]["figure"], "10x")
        self.assertEqual(said["breaks"]["figure"], "100x")
        # A world that shrugs everything off teaches no breaking point.
        said = self.lessons({1.0: 1.0, 10.0: 1.0, 100.0: 0.97})
        self.assertNotIn("breaks", said)
        self.assertEqual(said["absorbs"]["figure"], "100x")
        # And one that breaks at the first step absorbed nothing.
        said = self.lessons({1.0: 1.0, 10.0: 0.6, 100.0: 0.3})
        self.assertNotIn("absorbs", said)
        self.assertEqual(said["breaks"]["figure"], "10x")

    def test_a_lesson_quotes_the_records_own_figures(self):
        data = chaos_monkey.record(batch(seeds=16))
        said = {lesson["key"]: lesson for lesson in data["lessons"]}
        worst = data["doses"][-1]
        self.assertIn(chaos_monkey.pct(1 - worst["kept"]["people"]), said["breaks"]["of"])
        for lesson in data["lessons"]:
            self.assertEqual(sorted(lesson), ["figure", "key", "of", "text", "title"])


class TheRecordInTheTree(unittest.TestCase):
    """`data/chaos_monkey.json` is what the page says. It is written by this
    script, so everything it claims is something its own figures give."""

    @classmethod
    def setUpClass(cls):
        cls.data = json.loads(chaos_monkey.RECORD.read_text(encoding="utf-8"))

    def test_its_lessons_are_the_ones_its_figures_give(self):
        self.assertEqual(self.data["lessons"], chaos_monkey.lessons(self.data["doses"], self.data["factors"]))
        self.assertGreaterEqual(len(self.data["lessons"]), 3)

    def test_every_world_has_its_calm_self(self):
        doses = self.data["doses"]
        self.assertEqual(doses[0]["frequency"], 1)
        self.assertEqual(set(doses[0]["kept"].values()), {1.0})
        self.assertEqual(len({dose["games"] for dose in doses}), 1, "every seed is played at every frequency")
        self.assertEqual([dose["frequency"] for dose in doses], sorted(dose["frequency"] for dose in doses))
        self.assertEqual(sum(dose["games"] for dose in doses), self.data["played"]["games"])

    def test_more_frequency_is_more_disasters(self):
        struck = [dose["disasters"] for dose in self.data["doses"]]
        self.assertEqual(struck, sorted(struck))
        self.assertGreater(struck[-1], struck[0] * 10)

    def test_it_says_which_games_it_read(self):
        played = self.data["played"]
        for key in ("games", "seeds", "setup", "commit", "date"):
            self.assertIn(key, played)
        # The commit the games were played on, so the same seeds give them
        # again; a stand-in record has none.
        self.assertRegex(played["commit"], r"^[0-9a-f]{7,40}$")
        self.assertRegex(played["date"], r"^\d{4}-\d{2}-\d{2}$")


if __name__ == "__main__":
    unittest.main()
