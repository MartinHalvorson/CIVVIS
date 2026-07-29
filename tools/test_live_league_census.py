import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("live_league_census.py")
SPEC = importlib.util.spec_from_file_location("live_league_census", MODULE_PATH)
assert SPEC and SPEC.loader
census = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = census
SPEC.loader.exec_module(census)


def write_json(path, value):
    path.write_text(json.dumps(value), encoding="utf-8")


class PlacementTests(unittest.TestCase):
    def test_historical_and_current_placement_rows_keep_strategy_and_civ(self):
        old = census.parse_placement("g20-21@Rome")
        current = census.parse_placement("g20-21@Trajan@Rome")
        self.assertEqual((old.strategy, old.civ), ("g20-21", "Rome"))
        self.assertEqual(current, old)


class ArchiveJoinTests(unittest.TestCase):
    def test_join_requires_recorded_seed_and_turn_not_later_victory(self):
        with tempfile.TemporaryDirectory() as directory:
            results = Path(directory)
            for turn in (272, 279):
                write_json(
                    results / f"game-{turn}.result.json",
                    {
                        "seed": 42,
                        "turn": turn,
                        "victory_type": "science",
                        "game_speed": "online",
                        "map_script": "continents",
                        "max_turns": 250,
                        "runtime": {"revision": "abc1234"},
                    },
                )
                write_json(results / f"game-{turn}.save.json", {})
            match = census.Match(
                round=294,
                seed=42,
                turn=272,
                victory="science",
                placements=(census.Placement("advanced", "Rome"),),
            )
            joined, missing = census.join_matches(
                [match], census.index_archives(results)
            )
        self.assertEqual(joined[0][1].turn, 272)
        self.assertEqual(missing, [])


class SaveAnalysisTests(unittest.TestCase):
    def test_final_state_uses_alive_city_states_and_strict_suzerainty(self):
        players = [
            {
                "id": 0,
                "civ": "Rome",
                "alive": True,
                "is_minor": False,
                "envoys": [[2, 4], [3, 9]],
                "envoys_free": 0,
                "met": [2, 3],
                "government": "democracy",
                "policies": ["gunboat_diplomacy"],
                "civics": ["political_philosophy", "ideology"],
            },
            {
                "id": 1,
                "civ": "Gaul",
                "alive": True,
                "is_minor": False,
                "envoys": [[2, 3]],
                "envoys_free": 2,
                "met": [2],
                "government": "chiefdom",
                "policies": [],
                "civics": [],
            },
            {
                "id": 2,
                "civ": "Kabul",
                "alive": True,
                "is_minor": True,
                "is_barbarian": False,
            },
            {
                "id": 3,
                "civ": "Geneva",
                "alive": False,
                "is_minor": True,
                "is_barbarian": False,
            },
            {
                "id": 4,
                "civ": "Barbarians",
                "alive": True,
                "is_minor": True,
                "is_barbarian": True,
                "is_free_city": False,
            },
        ]
        save = {
            "seed": 42,
            "turn": 200,
            "victory_type": "science",
            "players": players,
            "cities": [
                {
                    "owner": 0,
                    "districts": {"diplomatic_quarter": [1, 2]},
                    "buildings": ["consulate", "chancery"],
                }
            ],
        }
        match = census.Match(
            round=1,
            seed=42,
            turn=200,
            victory="science",
            placements=(
                census.Placement("g20-21", "Rome"),
                census.Placement("advanced", "Gaul"),
            ),
        )
        with tempfile.TemporaryDirectory() as directory:
            save_path = Path(directory) / "game.save.json"
            write_json(save_path, save)
            archive = census.Archive(
                result_path=Path(directory) / "game.result.json",
                save_path=save_path,
                seed=42,
                turn=200,
                revision="abc1234",
                game_speed="online",
                map_script="continents",
                max_turns=250,
                victory="science",
            )
            rows = census.analyze_save(
                match,
                archive,
                {
                    "democracy": {
                        "envoys_per_threshold": 3,
                        "influence_per_turn": 7,
                    },
                    "chiefdom": {
                        "envoys_per_threshold": 1,
                        "influence_per_turn": 1,
                    },
                },
            )

        rome, gaul = rows
        self.assertEqual(rome["city_states_met"], 1)
        self.assertEqual(rome["suzerain"], 1)
        self.assertEqual(rome["envoys_placed"], 13)
        self.assertTrue(rome["diplomatic_quarter"])
        self.assertTrue(rome["consulate"])
        self.assertTrue(rome["chancery"])
        self.assertTrue(rome["ideology"])
        self.assertTrue(rome["gunboat_diplomacy"])
        self.assertEqual(rome["envoys_per_threshold"], 3)
        self.assertEqual(gaul["envoy_deficits"], [2])

    def test_tied_three_envoys_has_no_suzerain(self):
        majors = [
            {"id": 0, "is_minor": False, "alive": True, "envoys": [[2, 3]]},
            {"id": 1, "is_minor": False, "alive": True, "envoys": [[2, 3]]},
        ]
        self.assertIsNone(census._suzerain(2, majors))


class SummaryTests(unittest.TestCase):
    def test_summary_separates_zero_pool_from_final_state_presence(self):
        base = {
            "round": 1,
            "envoys_free": 0,
            "envoys_placed": 8,
            "city_states_met": 4,
            "suzerain": 1,
            "envoy_deficits": [2, 4],
            "envoy_shortfall": 6,
            "envoys_per_threshold": 3,
            "influence_per_turn": 7.0,
            "political_philosophy": True,
            "ideology": True,
            "charismatic_leader": False,
            "gunboat_diplomacy": True,
            "diplomatic_quarter": True,
            "consulate": True,
            "chancery": False,
            "government": "democracy",
        }
        summary = census.summarize([base, {**base, "envoys_free": 2}])
        self.assertEqual(summary["envoys_free_mean"], 1.0)
        self.assertEqual(summary["envoys_free_zero_pct"], 50.0)
        self.assertEqual(summary["city_states_held_pct"], 25.0)
        self.assertEqual(summary["ideology_pct"], 100.0)
        self.assertEqual(summary["chancery_pct"], 0.0)
        self.assertEqual(summary["envoy_deficit_median"], 3.0)


if __name__ == "__main__":
    unittest.main()
