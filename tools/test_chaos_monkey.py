#!/usr/bin/env python3
"""Chaos Monkey's record (`tools/chaos_monkey.py`): what it makes of the
games, how it tests every figure, and that the record in the tree says only
what its figures settle.

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


def batch(seeds: int = 16, hurt: dict | None = None) -> list[dict]:
    """Every seed at 1x, 10x and 100x. Two civilizations a world; under the
    storm the world has `hurt[frequency]` of its calm people (a share, or a
    function of the seed giving one)."""
    hurt = hurt or {1.0: 1.0, 10.0: 0.95, 100.0: 0.5}
    games = []
    for seed in range(1, seeds + 1):
        for frequency, share in hurt.items():
            kept = share(seed) if callable(share) else share
            people = lambda most: [max(1, round(most * kept * step)) for step in (0.1, 0.4, 0.7, 1.0, 1.0)]
            games.append(game(seed, frequency, [civ("Rome", people(400 + 10 * seed)), civ("Mali", people(300 + 10 * seed))]))
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

    def test_the_end_of_a_game_is_its_last_sample(self):
        # Not the sample the turn limit happened to share with it.
        full = game(1, 1.0, [civ("Rome", [4, 9, 20, 30, 31])])
        self.assertEqual(chaos_monkey.end(full)["people"], 31.0)
        self.assertEqual(chaos_monkey.end(full, 20)["people"], 20.0)

    def test_a_game_won_on_score_is_not_won_outright(self):
        self.assertTrue(chaos_monkey.won(game(1, 1.0, [civ("Rome", [1])], victory="science")))
        self.assertFalse(chaos_monkey.won(game(1, 1.0, [civ("Rome", [1])], victory="score")))
        self.assertFalse(chaos_monkey.won(game(1, 1.0, [civ("Rome", [1])], winner=None)))


class TheTest(unittest.TestCase):
    def test_the_signed_rank_test_is_exact(self):
        p = chaos_monkey.signed_rank_p
        # Nothing moved: nothing to say.
        self.assertEqual(p([]), 1.0)
        self.assertEqual(p([0.0, 0.0]), 1.0)
        # Five worlds all one way: 2 of the 32 equally likely sign patterns
        # are as far out.
        self.assertAlmostEqual(p([1, 2, 3, 4, 5]), 2 / 32)
        self.assertAlmostEqual(p([-1, -2, -3, -4, -5]), 2 / 32)
        # One against two: no way out of the middle.
        self.assertEqual(p([1, 2, -3]), 1.0)
        # A yes or no against a yes or no is the sign test: twelve worlds that
        # all changed one way.
        self.assertAlmostEqual(p([-1.0] * 12 + [0.0] * 12), 2 * 0.5**12)

    def test_a_shift_every_world_shows_is_sure_and_one_half_of_them_denies_is_not(self):
        data = chaos_monkey.record(batch())
        calm, mild, storm = data["doses"]
        # The calm worlds are what the others are read against: not tested.
        self.assertEqual(calm["kept"]["people"], {"value": 1.0})
        self.assertEqual(set(calm["wars"]), {"value"})
        people = storm["kept"]["people"]
        self.assertAlmostEqual(people["value"], 0.5, places=2)
        self.assertLessEqual(people["low"], people["value"])
        self.assertLessEqual(people["value"], people["high"])
        self.assertTrue(people["sure"])
        self.assertLess(people["p"], chaos_monkey.SURE)
        # What did not move (the cities, the wars) is not sure.
        self.assertFalse(storm["kept"]["cities"]["sure"])
        self.assertFalse(storm["wars"]["sure"])
        self.assertEqual(storm["wars"]["p"], 1.0)
        # Half the worlds up and half down by as much: the same average
        # change in some draws, the other way in others.
        noisy = chaos_monkey.record(batch(hurt={1.0: 1.0, 10.0: 1.0, 100.0: lambda seed: 0.5 if seed % 2 else 1.5}))
        people = noisy["doses"][2]["kept"]["people"]
        self.assertFalse(people["sure"])
        self.assertLess(people["low"], 1.0)
        self.assertGreater(people["high"], 1.0)

    def test_the_record_counts_what_it_tested(self):
        data = chaos_monkey.record(batch())
        tested = data["tested"]
        self.assertEqual(tested["figures"], 2 * chaos_monkey.TESTS_A_FREQUENCY)
        figures = [f for d in data["doses"][1:] for f in [*d["kept"].values(), *(d[k] for k in chaos_monkey.OUTCOMES)]]
        self.assertEqual(len(figures), tested["figures"])
        self.assertEqual(tested["sure"], sum(f["sure"] for f in figures))
        self.assertEqual((tested["p"], tested["level"], tested["redraws"]), (0.001, 0.95, 10000))

    def test_the_redraws_are_the_same_every_time(self):
        self.assertEqual(chaos_monkey.record(batch()), chaos_monkey.record(batch()))


class TheRecord(unittest.TestCase):
    def test_a_world_under_chaos_is_read_against_its_calm_self(self):
        games = batch(seeds=4)
        data = chaos_monkey.record(games)
        self.assertEqual([dose["frequency"] for dose in data["doses"]], [1, 10, 100])
        self.assertEqual([dose["games"] for dose in data["doses"]], [4, 4, 4])
        # The share is the storm's total over the same seeds' calm total.
        people = lambda f: sum(c["series"]["pop"][-1] for g in games if g["frequency"] == f for c in g["civs"])
        self.assertAlmostEqual(data["doses"][2]["kept"]["people"]["value"], people(100.0) / people(1.0), places=3)
        # What struck is a mean a game.
        self.assertEqual(data["doses"][2]["disasters"], 300.0)

    def test_there_is_no_record_without_the_calm_worlds(self):
        self.assertIsNone(chaos_monkey.record([]))
        with self.assertRaises(ValueError):
            chaos_monkey.record([g for g in batch() if g["frequency"] != 1.0])

    def test_a_redraw_needs_every_seed_at_every_frequency(self):
        with self.assertRaises(ValueError):
            chaos_monkey.record([g for g in batch() if not (g["seed"] == 3 and g["frequency"] == 10.0)])


class TheLessons(unittest.TestCase):
    def lessons(self, hurt: dict) -> dict:
        data = chaos_monkey.record(batch(hurt=hurt))
        return {lesson["key"]: lesson for lesson in data["lessons"]}

    def test_a_lesson_is_written_only_when_the_games_settle_it(self):
        said = self.lessons({1.0: 1.0, 10.0: 0.95, 100.0: 0.5})
        self.assertEqual(said["breaks"]["figure"], "100x")
        self.assertIn("50% fewer people", said["breaks"]["of"])
        self.assertIn("the loss was 5%", said["breaks"]["text"])
        self.assertNotIn("pays", said)
        # A gain every world shows pays.
        said = self.lessons({1.0: 1.0, 10.0: 1.2, 100.0: 0.5})
        self.assertEqual(said["pays"]["figure"], "+20%")
        # A world that shrugs everything off teaches no breaking point.
        self.assertNotIn("breaks", self.lessons({1.0: 1.0, 10.0: 1.0, 100.0: 0.97}))
        # And a loss half the worlds deny teaches nothing, however big the
        # average.
        said = self.lessons({1.0: 1.0, 10.0: 1.0, 100.0: lambda seed: 0.1 if seed % 2 else 1.3})
        self.assertNotIn("breaks", said)
        self.assertNotIn("pays", said)

    def test_a_lesson_quotes_the_records_own_figures_and_their_range(self):
        data = chaos_monkey.record(batch())
        said = {lesson["key"]: lesson for lesson in data["lessons"]}
        people = data["doses"][-1]["kept"]["people"]
        self.assertIn(chaos_monkey.pct(1 - people["value"]), said["breaks"]["of"])
        self.assertEqual(said["breaks"]["range"], f"{chaos_monkey.pct(1 - people['high'])} to {chaos_monkey.pct(1 - people['low'])} fewer")
        for lesson in data["lessons"]:
            self.assertEqual(sorted(lesson), ["figure", "key", "of", "range", "text", "title"])


class TheRecordInTheTree(unittest.TestCase):
    """`data/chaos_monkey.json` is what the page says. It is written by this
    script, so everything it claims is something its own figures settle."""

    @classmethod
    def setUpClass(cls):
        cls.data = json.loads(chaos_monkey.RECORD.read_text(encoding="utf-8"))

    def figures(self):
        for dose in self.data["doses"][1:]:
            for key, figure in dose["kept"].items():
                yield dose["frequency"], key, figure
            for key in chaos_monkey.OUTCOMES:
                yield dose["frequency"], key, dose[key]

    def test_its_lessons_are_the_ones_its_figures_give(self):
        self.assertEqual(self.data["lessons"], chaos_monkey.lessons(self.data["doses"]))
        self.assertGreaterEqual(len(self.data["lessons"]), 3)

    def test_every_figure_under_chaos_is_tested_and_sure_only_past_the_line(self):
        tested = self.data["tested"]
        self.assertEqual(tested["figures"], chaos_monkey.TESTS_A_FREQUENCY * (len(self.data["doses"]) - 1))
        sure = 0
        for frequency, key, figure in self.figures():
            self.assertLessEqual(figure["low"], figure["value"], (frequency, key))
            self.assertLessEqual(figure["value"], figure["high"], (frequency, key))
            self.assertEqual(figure["sure"], figure["p"] < tested["p"], (frequency, key))
            sure += figure["sure"]
        self.assertEqual(sure, tested["sure"])
        # 40 tests at one in a thousand: the chance that even one sure figure
        # is luck stays under one in twenty.
        self.assertLess(tested["figures"] * tested["p"], 0.05)

    def test_every_world_has_its_calm_self(self):
        doses = self.data["doses"]
        self.assertEqual(doses[0]["frequency"], 1)
        self.assertEqual({figure["value"] for figure in doses[0]["kept"].values()}, {1.0})
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
