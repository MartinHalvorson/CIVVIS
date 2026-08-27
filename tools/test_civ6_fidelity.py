#!/usr/bin/env python3
"""The fidelity audit must know which ruleset it is reading.

⚠⚠ THE REPORT USED TO ASSERT ITS REFERENCE RATHER THAN CHECK IT. The header line
read "(Gathering Storm load order)" unconditionally, and the compiled cache
under `Cache/DebugGameplay.sqlite` is whatever ruleset the game last ran — a
session launched without the expansion leaves a vanilla database exactly where
`--cache` looks for it.

Audited against one of those on 2026-08-18, the tool reported **210 divergent
fields across 27 tables** with full confidence. Acting on that report means
editing correct Gathering Storm values to match vanilla ones — `astronomy` from
730 down to 660, `banking` from 600 to 540, and on down forty-two technologies —
after which the audit reads clean and the rules data is wrong.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_fidelity  # noqa: E402


def database_with(rows: dict[str, list[dict]]) -> civ6_fidelity.Database:
    db = civ6_fidelity.Database()
    for table, entries in rows.items():
        db.tables[table] = {(index,): row for index, row in enumerate(entries)}
    return db


def gathering_storm_database() -> civ6_fidelity.Database:
    """A reference carrying every sentinel and nothing else."""
    rows: dict[str, list[dict]] = {}
    for table, column, value in civ6_fidelity.GATHERING_STORM_SENTINELS:
        rows.setdefault(table, []).append({column: value})
    return database_with(rows)


class TheReferenceIsChecked(unittest.TestCase):
    def test_a_gathering_storm_reference_is_accepted(self):
        self.assertEqual(
            civ6_fidelity.missing_gathering_storm_rows(gathering_storm_database()), [])

    def test_a_vanilla_reference_is_named_and_refused(self):
        """The real failure: 68 technologies and not one expansion row."""
        vanilla = database_with({
            "Technologies": [{"TechnologyType": "TECH_WRITING"}],
            "Civics": [{"CivicType": "CIVIC_CODE_OF_LAWS"}],
            "Units": [{"UnitType": "UNIT_WARRIOR"}],
        })
        missing = civ6_fidelity.missing_gathering_storm_rows(vanilla)
        self.assertEqual(len(missing), len(civ6_fidelity.GATHERING_STORM_SENTINELS))
        self.assertIn("Technologies.TECH_BUTTRESS", missing)
        self.assertEqual(
            civ6_fidelity.refuse_a_reference_from_another_ruleset(vanilla, Path("x")), 2)

    def test_the_loader_itself_refuses_a_foreign_ruleset(self):
        """⚠⚠ THE CHECK HAS TO BE IN THE DOOR, NOT BESIDE IT.

        `main` called the refusal after loading, so every other caller got
        nothing — and on 2026-08-18 an agent read the Founder-belief modifiers
        straight out of a cache that happened to hold the base game and shipped
        them as a Gathering Storm fidelity fix (#2049, reverted by #2050). The
        loader refuses now, so opening the cache at all is enough.
        """
        import sqlite3
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            empty = Path(directory) / "DebugGameplay.sqlite"
            sqlite3.connect(empty).close()
            with self.assertRaises(SystemExit) as refused:
                civ6_fidelity.load_cache_database(empty)
            self.assertEqual(refused.exception.code, 2)
            # And the deliberate escape hatch still works, so a caller that
            # genuinely wants a foreign ruleset has one clearly-named way in.
            allowed = civ6_fidelity.load_cache_database(
                empty, require_gathering_storm=False
            )
            self.assertEqual(allowed.tables, {})

    def test_one_sentinel_is_not_enough_to_pass(self):
        """Sentinels are spread across three tables so a partial or corrupt
        reference cannot pass by carrying one of them."""
        partial = database_with({
            "Technologies": [{"TechnologyType": "TECH_BUTTRESS"}],
            "Civics": [{"CivicType": "CIVIC_CODE_OF_LAWS"}],
            "Units": [{"UnitType": "UNIT_WARRIOR"}],
        })
        self.assertNotEqual(civ6_fidelity.missing_gathering_storm_rows(partial), [])

    def test_the_sentinels_span_more_than_one_table(self):
        tables = {table for table, _, _ in civ6_fidelity.GATHERING_STORM_SENTINELS}
        self.assertGreaterEqual(len(tables), 3, tables)

    def test_the_header_no_longer_asserts_what_it_did_not_check(self):
        source = Path(civ6_fidelity.__file__).read_text(encoding="utf-8")
        self.assertNotIn('(Gathering Storm load order).")', source)
        self.assertIn("expansion sentinels", source)


class ResourcePlacementWeightsAreAudited(unittest.TestCase):
    def test_projected_resources_keep_land_and_sea_weights_distinct(self):
        database = database_with({
            "Resources": [
                {
                    "ResourceType": "RESOURCE_FISH",
                    "ResourceClassType": "RESOURCECLASS_BONUS",
                    "Frequency": "0",
                    "SeaFrequency": "23",
                },
                {
                    "ResourceType": "RESOURCE_WHALES",
                    "ResourceClassType": "RESOURCECLASS_LUXURY",
                    "Frequency": "0",
                    "SeaFrequency": "1",
                },
                {
                    "ResourceType": "RESOURCE_STONE",
                    "ResourceClassType": "RESOURCECLASS_BONUS",
                    "Frequency": "10",
                    "SeaFrequency": "0",
                },
            ]
        })

        projected = civ6_fidelity.project_resources(database)
        self.assertEqual(projected["fish"]["frequency"], 0)
        self.assertEqual(projected["fish"]["sea_frequency"], 23)
        self.assertEqual(projected["whales"]["sea_frequency"], 1)
        self.assertEqual(projected["stone"]["frequency"], 10)
        self.assertEqual(projected["stone"]["sea_frequency"], 0)


class TheVolcanoFlagIsAudited(unittest.TestCase):
    """⚠ `Features_XP2.Volcano` rides FOUR features, not one.

    The generic cone plus Vesuvius, Kilimanjaro and Eyjafjallajokull. The audit
    never read the column at all, so an engine that recognised only the feature
    named `volcano` left the three Natural Wonders dormant for good and the
    report said the Features table agreed.
    """

    def test_the_three_volcanic_natural_wonders_project_as_volcanoes(self):
        database = database_with({
            "Features": [
                {"FeatureType": "FEATURE_VOLCANO"},
                {"FeatureType": "FEATURE_VESUVIUS", "NaturalWonder": "true"},
                {"FeatureType": "FEATURE_KILIMANJARO", "NaturalWonder": "true"},
                {"FeatureType": "FEATURE_EYJAFJALLAJOKULL", "NaturalWonder": "true"},
                {"FeatureType": "FEATURE_FOREST"},
            ],
            "Features_XP2": [
                {"FeatureType": "FEATURE_VOLCANO", "Volcano": "true"},
                {"FeatureType": "FEATURE_VESUVIUS", "Volcano": "true"},
                {"FeatureType": "FEATURE_KILIMANJARO", "Volcano": "true"},
                {"FeatureType": "FEATURE_EYJAFJALLAJOKULL", "Volcano": "true"},
                {"FeatureType": "FEATURE_FOREST", "ValidForReplacement": "true"},
            ],
        })

        projected = civ6_fidelity.project_features(database)
        self.assertEqual(
            sorted(name for name, entry in projected.items() if entry["volcano"]),
            ["eyjafjallajokull", "kilimanjaro", "vesuvius", "volcano"],
        )
        self.assertFalse(projected["forest"]["volcano"])

    def test_a_reference_without_the_side_table_flags_nothing(self):
        database = database_with({"Features": [{"FeatureType": "FEATURE_VOLCANO"}]})

        self.assertFalse(civ6_fidelity.project_features(database)["volcano"]["volcano"])


class ImprovementPlacementAlternativesAreAudited(unittest.TestCase):
    def test_a_feature_can_be_an_alternative_to_hills(self):
        database = database_with({
            "Improvements": [{"ImprovementType": "IMPROVEMENT_ROCK_HEWN_CHURCH"}],
            "Improvement_ValidTerrains": [
                {
                    "ImprovementType": "IMPROVEMENT_ROCK_HEWN_CHURCH",
                    "TerrainType": "TERRAIN_PLAINS_HILLS",
                },
            ],
            "Improvement_ValidFeatures": [
                {
                    "ImprovementType": "IMPROVEMENT_ROCK_HEWN_CHURCH",
                    "FeatureType": "FEATURE_VOLCANIC_SOIL",
                },
            ],
        })

        projected = civ6_fidelity.project_improvements(database)["rock_hewn_church"]
        self.assertFalse(projected["requires_hills"])
        self.assertTrue(projected["hills_or_feature"])

    def test_hills_only_improvement_stays_hills_only(self):
        database = database_with({
            "Improvements": [{"ImprovementType": "IMPROVEMENT_HILL_FORT"}],
            "Improvement_ValidTerrains": [
                {
                    "ImprovementType": "IMPROVEMENT_HILL_FORT",
                    "TerrainType": "TERRAIN_PLAINS_HILLS",
                },
            ],
        })

        projected = civ6_fidelity.project_improvements(database)["hill_fort"]
        self.assertTrue(projected["requires_hills"])
        self.assertFalse(projected["hills_or_feature"])

    def test_resource_and_feature_are_independent_hill_alternatives(self):
        database = database_with({
            "Improvements": [{"ImprovementType": "IMPROVEMENT_MINE"}],
            "Improvement_ValidTerrains": [
                {
                    "ImprovementType": "IMPROVEMENT_MINE",
                    "TerrainType": "TERRAIN_PLAINS_HILLS",
                },
            ],
            "Improvement_ValidFeatures": [
                {
                    "ImprovementType": "IMPROVEMENT_MINE",
                    "FeatureType": "FEATURE_VOLCANIC_SOIL",
                },
            ],
            "Improvement_ValidResources": [
                {
                    "ImprovementType": "IMPROVEMENT_MINE",
                    "ResourceType": "RESOURCE_IRON",
                },
            ],
        })

        projected = civ6_fidelity.project_improvements(database)["mine"]
        self.assertFalse(projected["requires_hills"])
        self.assertFalse(projected["hills_or_resource"])
        self.assertFalse(projected["hills_or_feature"])
        self.assertTrue(projected["hills_or_resource_or_feature"])


class PolicyRosterIsStrict(unittest.TestCase):
    def test_a_policy_only_civvis_offers_is_a_divergence(self):
        result = civ6_fidelity.compare(
            "Policies",
            {
                "discipline": {"slot": "military"},
                "retired_card": {"slot": "wildcard"},
            },
            {"discipline": {"slot": "military"}},
        )

        self.assertEqual(result["only_ours"], ["retired_card"])
        self.assertEqual(
            result["divergences"],
            [{
                "table": "Policies",
                "entry": "retired_card",
                "field": "row",
                "ours": "present",
                "theirs": "absent",
            }],
        )

    def test_an_unmodeled_source_policy_remains_a_scope_reading(self):
        result = civ6_fidelity.compare(
            "Policies",
            {"discipline": {"slot": "military"}},
            {
                "discipline": {"slot": "military"},
                "future_card": {"slot": "wildcard"},
            },
        )

        self.assertEqual(result["only_theirs"], ["future_card"])
        self.assertEqual(result["divergences"], [])


if __name__ == "__main__":
    unittest.main()


class TheDifficultyLadderIsProjected(unittest.TestCase):
    """The ladder is modifiers, start-unit rows and raid windows, not a table
    of numbers; the projection has to evaluate them the way the DLL does."""

    LADDER = [
        "DIFFICULTY_SETTLER", "DIFFICULTY_CHIEFTAIN", "DIFFICULTY_WARLORD",
        "DIFFICULTY_PRINCE", "DIFFICULTY_KING", "DIFFICULTY_EMPEROR",
        "DIFFICULTY_IMMORTAL", "DIFFICULTY_DEITY",
    ]

    def ladder_database(self) -> civ6_fidelity.Database:
        return database_with({
            "Difficulties": [{"DifficultyType": rung} for rung in self.LADDER],
            "Modifiers": [
                {"ModifierId": "HIGH_DIFFICULTY_SCIENCE_SCALING",
                 "OwnerRequirementSetId": "PLAYER_IS_HIGH_DIFFICULTY_AI"},
                {"ModifierId": "LOW_DIFFICULTY_COMBAT_SCALING",
                 "OwnerRequirementSetId": "PLAYER_IS_LOW_DIFFICULTY_HUMAN"},
                {"ModifierId": "BARBARIAN_CAMP_GOLD_SCALING",
                 "OwnerRequirementSetId": "PLAYER_IS_HUMAN"},
            ],
            "ModifierArguments": [
                {"ModifierId": "HIGH_DIFFICULTY_SCIENCE_SCALING", "Name": "Amount",
                 "Type": "LinearScaleFromDefaultHandicap", "Value": "0", "Extra": "8"},
                {"ModifierId": "LOW_DIFFICULTY_COMBAT_SCALING", "Name": "Amount",
                 "Type": "LinearScaleFromDefaultHandicap", "Value": "0", "Extra": "-1",
                 "SecondExtra": "DIFFICULTY_PRINCE"},
                {"ModifierId": "BARBARIAN_CAMP_GOLD_SCALING", "Name": "Amount",
                 "Type": "LinearScaleFromDefaultHandicap", "Value": "0", "Extra": "-5",
                 "SecondExtra": "DIFFICULTY_PRINCE"},
            ],
            "Requirements": [
                {"RequirementId": "REQUIRES_HIGH_DIFFICULTY",
                 "RequirementType": "REQUIREMENT_PLAYER_HANDICAP_AT_OR_ABOVE", "Inverse": "0"},
                {"RequirementId": "REQUIRES_LOW_DIFFICULTY",
                 "RequirementType": "REQUIREMENT_PLAYER_HANDICAP_AT_OR_ABOVE", "Inverse": "1"},
                {"RequirementId": "REQUIRES_PLAYER_IS_AI",
                 "RequirementType": "REQUIREMENT_PLAYER_IS_AI"},
            ],
            "RequirementArguments": [
                {"RequirementId": "REQUIRES_HIGH_DIFFICULTY", "Name": "Handicap",
                 "Value": "DIFFICULTY_PRINCE"},
                {"RequirementId": "REQUIRES_LOW_DIFFICULTY", "Name": "Handicap",
                 "Value": "DIFFICULTY_WARLORD"},
            ],
            "RequirementSetRequirements": [
                {"RequirementSetId": "PLAYER_IS_HIGH_DIFFICULTY_AI",
                 "RequirementId": "REQUIRES_PLAYER_IS_AI"},
                {"RequirementSetId": "PLAYER_IS_HIGH_DIFFICULTY_AI",
                 "RequirementId": "REQUIRES_HIGH_DIFFICULTY"},
                {"RequirementSetId": "PLAYER_IS_LOW_DIFFICULTY_HUMAN",
                 "RequirementId": "REQUIRES_LOW_DIFFICULTY"},
            ],
            "MajorStartingUnits": [
                {"Unit": "UNIT_WARRIOR", "Era": "ERA_ANCIENT", "District": "DISTRICT_CITY_CENTER",
                 "Quantity": "1", "AiOnly": "1", "MinDifficulty": "DIFFICULTY_KING",
                 "DifficultyDelta": "1.0"},
                {"Unit": "UNIT_BUILDER", "Era": "ERA_ANCIENT", "District": "DISTRICT_CITY_CENTER",
                 "Quantity": "1", "AiOnly": "1", "MinDifficulty": "DIFFICULTY_KING",
                 "DifficultyDelta": "0.5"},
                {"Unit": "UNIT_SETTLER", "Era": "ERA_ANCIENT", "District": "DISTRICT_CITY_CENTER",
                 "Quantity": "1", "AiOnly": "0"},
            ],
            "StartingBuildings": [
                {"Building": "BUILDING_WALLS", "Era": "ERA_ANCIENT",
                 "District": "DISTRICT_CITY_CENTER", "MinorOnly": "1",
                 "MinDifficulty": "DIFFICULTY_IMMORTAL"},
            ],
            "BarbarianAttackForces": [
                {"AttackForceType": "LowDifficultyStandardRaid",
                 "MaxTargetDifficulty": "DIFFICULTY_CHIEFTAIN", "SpawnRate": "2",
                 "MeleeTag": "CLASS_MELEE", "NumMeleeUnits": "1", "RaidingForce": "1"},
                {"AttackForceType": "StandardRaid",
                 "MinTargetDifficulty": "DIFFICULTY_WARLORD",
                 "MaxTargetDifficulty": "DIFFICULTY_EMPEROR", "SpawnRate": "2",
                 "MeleeTag": "CLASS_MELEE", "NumMeleeUnits": "2", "NumRangeUnits": "1",
                 "RaidingForce": "1"},
                {"AttackForceType": "HighDifficultyStandardRaid",
                 "MinTargetDifficulty": "DIFFICULTY_IMMORTAL", "SpawnRate": "1",
                 "MeleeTag": "CLASS_MELEE", "NumMeleeUnits": "3", "NumRangeUnits": "2",
                 "RaidingForce": "1"},
                {"AttackForceType": "CavalryRaid",
                 "MinTargetDifficulty": "DIFFICULTY_WARLORD",
                 "MaxTargetDifficulty": "DIFFICULTY_EMPEROR", "SpawnRate": "2",
                 "MeleeTag": "CLASS_LIGHT_CAVALRY", "NumMeleeUnits": "9", "RaidingForce": "1"},
            ],
        })

    def test_a_handicap_scales_linearly_off_prince(self):
        projected = civ6_fidelity.project_difficulties(self.ladder_database())
        self.assertEqual(projected["prince"]["ai_science_pct"], 0)
        self.assertEqual(projected["king"]["ai_science_pct"], 8)
        self.assertEqual(projected["deity"]["ai_science_pct"], 32)
        # Below the requirement's floor the modifier is not attached at all.
        self.assertEqual(projected["warlord"]["ai_science_pct"], 0)

    def test_an_inverse_requirement_stops_below_its_rung(self):
        """`REQUIRES_LOW_DIFFICULTY` is the inverse of at-or-above Warlord, so
        Settler and Chieftain pass it and Warlord does not."""
        projected = civ6_fidelity.project_difficulties(self.ladder_database())
        self.assertEqual(projected["settler"]["human_combat_strength"], 3)
        self.assertEqual(projected["chieftain"]["human_combat_strength"], 2)
        self.assertEqual(projected["warlord"]["human_combat_strength"], 0)

    def test_camp_gold_runs_negative_above_prince(self):
        projected = civ6_fidelity.project_difficulties(self.ladder_database())
        self.assertEqual(projected["settler"]["human_camp_gold"], 15)
        self.assertEqual(projected["deity"]["human_camp_gold"], -20)

    def test_start_units_floor_the_per_rung_delta(self):
        projected = civ6_fidelity.project_difficulties(self.ladder_database())
        self.assertEqual(projected["prince"]["ai_bonus_warrior"], 0)
        self.assertEqual(projected["king"]["ai_bonus_warrior"], 1)
        self.assertEqual(projected["deity"]["ai_bonus_warrior"], 4)
        self.assertEqual(projected["emperor"]["ai_bonus_builder"], 1, "1.5 Builders is one")
        self.assertEqual(projected["deity"]["ai_bonus_builder"], 2)
        self.assertNotIn("ai_bonus_settler", projected["deity"], "not an AI-only grant")
        self.assertEqual(projected["immortal"]["starting_buildings"], {"walls:minor"})
        self.assertEqual(projected["emperor"]["starting_buildings"], set())

    def test_the_raid_bands_break_at_warlord_and_immortal(self):
        projected = civ6_fidelity.project_difficulties(self.ladder_database())
        bands = {rung: entry["barb_band"] for rung, entry in projected.items()}
        self.assertEqual(bands["chieftain"], "low")
        self.assertEqual(bands["warlord"], "standard")
        self.assertEqual(bands["emperor"], "standard")
        self.assertEqual(bands["immortal"], "high")
        self.assertEqual(projected["immortal"]["barb_spawn_rate"], 1)
        self.assertEqual(projected["immortal"]["barb_raid_units"], 5)
        self.assertEqual(projected["chieftain"]["barb_raid_units"], 1,
                         "the cavalry window is not the land raid the engine bands by")

    def test_our_side_spells_the_same_bands(self):
        """The scales in data/difficulties.json map onto the shipped bands the
        way game.rs reads them, so the two sides meet on one field."""
        ours = civ6_fidelity.ours_difficulties()
        self.assertEqual(ours["chieftain"]["barb_band"], "low")
        self.assertEqual(ours["warlord"]["barb_band"], "standard")
        self.assertEqual(ours["immortal"]["barb_band"], "high")
        self.assertEqual(ours["immortal"]["barb_spawn_rate"], 1)
        self.assertEqual(ours["deity"]["human_camp_gold"], -20)


class EngineConstantsAreDiscovered(unittest.TestCase):
    def test_a_constant_under_a_shipped_name_is_found_in_the_engine(self):
        found = civ6_fidelity.ours_engine_constants()
        self.assertEqual(found["BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY"], {"value": 4})
        self.assertIn("BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING", found)

    def test_only_shared_names_are_audited(self):
        database = database_with({"GlobalParameters": [
            {"Name": "BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY", "Value": "4"},
            {"Name": "SOMETHING_THE_ENGINE_DOES_NOT_NAME", "Value": "1"},
        ]})
        ours, theirs = civ6_fidelity.audit_engine_constants(database)
        self.assertEqual(set(ours), {"BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY"})
        self.assertEqual(ours, theirs)


class TheCheckFormSkipsByName(unittest.TestCase):
    """`--check` on a machine with no database passes with a notice that says
    so; a skipped run must never read as a clean one."""

    def test_the_notice_names_the_tool_and_the_word_skipped(self):
        notice = civ6_fidelity.skip_notice()
        self.assertIn("civ6_fidelity", notice)
        self.assertIn("SKIPPED", notice)
        self.assertTrue(notice.startswith("::notice title="), "a GitHub annotation")

    def test_check_without_a_database_exits_zero(self):
        import contextlib
        import io
        import unittest.mock as mock
        stderr = io.StringIO()
        with mock.patch.object(sys, "argv", ["civ6_fidelity.py", "--check", "--max", "0",
                                             "--cache", "/nonexistent/DebugGameplay.sqlite"]):
            with contextlib.redirect_stderr(stderr):
                self.assertEqual(civ6_fidelity.main(), 0)
        self.assertIn(civ6_fidelity.SKIP_NOTICE, stderr.getvalue())


class TheWaiverFileOnlyShrinks(unittest.TestCase):
    """Thirteen on the day the ratchet was wired: nine from before, plus the
    Vampire Castle's mode-only terrain and three difficulty readings the DLL
    owns (Prince's AI at -1, Warlord's human bonuses under an inverse
    requirement). A fourteenth is a new accepted divergence, and this number
    moves only when one is retired or a new one is argued in its own PR."""

    CEILING = 13

    def test_the_waiver_count_does_not_grow(self):
        waivers = civ6_fidelity.load_waivers()
        self.assertLessEqual(len(waivers), self.CEILING)

    def test_every_waiver_gives_a_reason(self):
        import json
        path = Path(civ6_fidelity.__file__).resolve().parent / "fidelity_waivers.json"
        for entry in json.loads(path.read_text(encoding="utf-8"))["waivers"]:
            with self.subTest(entry=(entry["table"], entry["entry"], entry["field"])):
                self.assertGreater(len(entry.get("reason", "")), 40)
