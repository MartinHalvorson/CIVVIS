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
