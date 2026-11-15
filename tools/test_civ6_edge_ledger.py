#!/usr/bin/env python3
"""The edge ledger bins live blows the way the simulator's ledger does."""

from __future__ import annotations

import json
import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_edge_ledger as edge  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DRIVER = ROOT / "src" / "bin" / "gene_screen" / "edge_ledger_tests.rs"

BOOK = edge.UnitBook.load()


def _unit(kind: str, player: int) -> dict:
    return {"type": "unit", "kind": kind, "player": player, "id": 1, "hp": 100, "x": 0, "y": 0}


def _combat(turn: int, attacker: dict, defender: dict, *, ours: bool, against_us: bool,
            killed: bool = False, preview: tuple[int, int] | None = None,
            dealt: int = 30, taken: int = 10) -> dict:
    event = {
        "kind": "combat", "turn": turn, "ours": ours, "against_us": against_us,
        "attacker": attacker, "defender": defender,
        "damage_to_defender": dealt, "damage_to_attacker": taken,
        "defender_killed": killed, "attacker_killed": False,
    }
    if preview is not None:
        event["preview"] = {"attacker_total_strength": preview[0],
                            "defender_total_strength": preview[1]}
    return event


def _state(turn: int, units: list[dict], rivals: list[dict], **extra) -> dict:
    state = {
        "kind": "state", "turn": turn, "frame": 0, "techs": [], "units": units,
        "rivals": rivals, "gold": 150, "gold_per_turn": 12.5,
        "strategic_resources": {"RESOURCE_NITER": 25, "RESOURCE_IRON": 0},
        "unit_maintenance_discount": 0,
    }
    state.update(extra)
    return state


def _write_run(root: Path, name: str, events: list[dict], speed: str = "GAMESPEED_ONLINE") -> Path:
    run = root / name
    run.mkdir()
    seat = {"kind": "seat", "local_player": 0, "players": 4, "speed": speed,
            "civ": "CIVILIZATION_ROME", "map": "Continents.lua", "city_states": 9}
    with (run / "events.jsonl").open("w") as handle:
        for event in [seat] + events:
            handle.write(json.dumps(event, sort_keys=True) + "\n")
    return run


class UnitBookTest(unittest.TestCase):
    def test_unique_units_read_as_their_own_rows(self) -> None:
        self.assertEqual(BOOK.key("UNIT_ROMAN_LEGION"), "legion")
        self.assertEqual(BOOK.key("UNIT_COLOMBIAN_LLANERO"), "llanero")
        self.assertEqual(BOOK.key("UNIT_WARRIOR"), "warrior")
        self.assertIsNone(BOOK.key("UNIT_NOT_A_UNIT"))

    def test_attack_is_bombard_then_ranged_then_combat(self) -> None:
        self.assertEqual(BOOK.attack("catapult"), 35)
        self.assertEqual(BOOK.attack("archer"), 25)
        self.assertEqual(BOOK.attack("legion"), 40)
        self.assertEqual(BOOK.combat("archer"), 15)

    def test_an_aircraft_is_not_a_fighter_and_a_ship_is(self) -> None:
        self.assertFalse(BOOK.is_fighter("bomber"))
        self.assertTrue(BOOK.is_fighter("galley"))
        self.assertFalse(BOOK.is_land_army("galley"))
        self.assertTrue(BOOK.is_land_army("warrior"))


class BinTest(unittest.TestCase):
    def test_bins_round_to_whole_points_first(self) -> None:
        gaps = [-25, -10, -9.6, -4, -3, 0, 3.4, 4, 9, 9.5, 10, 60]
        self.assertEqual([edge.gap_bin(g) for g in gaps], [0, 0, 0, 1, 2, 2, 2, 3, 3, 4, 4, 4])

    def test_turns_normalise_to_the_online_clock(self) -> None:
        self.assertEqual(edge.online_turn(100, 50), 100)
        self.assertEqual(edge.online_turn(100, 100), 50)
        self.assertEqual([edge.turn_bucket(t) for t in (0, 49, 50, 149, 150, 400)], [0, 0, 1, 2, 3, 3])


class RunTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _events(self) -> list[dict]:
        return [
            # A rival major's Catapult (Bombard 35) on our Warrior (20): +15.
            _combat(120, _unit("UNIT_CATAPULT", 1), _unit("UNIT_WARRIOR", 0),
                    ours=False, against_us=True, killed=True, dealt=60),
            # A barbarian Warrior (20) on our Legion (40): -20.
            _combat(30, _unit("UNIT_WARRIOR", 63), _unit("UNIT_ROMAN_LEGION", 0),
                    ours=False, against_us=True),
            # A city-state's Archer (Ranged 25) on our Warrior (20): +5.
            _combat(40, _unit("UNIT_ARCHER", 5), _unit("UNIT_WARRIOR", 0),
                    ours=False, against_us=True),
            # Our Legion (40) on a rival Spearman (25), previewed at 45 v 30.
            _combat(60, _unit("UNIT_ROMAN_LEGION", 0), _unit("UNIT_SPEARMAN", 2),
                    ours=True, against_us=False, killed=True, preview=(45, 30)),
            # Not unit-on-unit, an aircraft, a stranger's type, others' fight.
            _combat(61, _unit("UNIT_ROMAN_LEGION", 0), {"type": "district", "player": 2},
                    ours=True, against_us=False),
            _combat(62, _unit("UNIT_BOMBER", 0), _unit("UNIT_SPEARMAN", 2),
                    ours=True, against_us=False, preview=(80, 30)),
            _combat(63, _unit("UNIT_FOO", 1), _unit("UNIT_WARRIOR", 0),
                    ours=False, against_us=True),
            _combat(64, _unit("UNIT_WARRIOR", 1), _unit("UNIT_WARRIOR", 2),
                    ours=False, against_us=False),
        ]

    def test_blows_are_binned_by_side_and_gap(self) -> None:
        run = _write_run(self.root, "civvis-20261112T000000Z", self._events())
        ledger = edge.read_run(run, BOOK)
        taken = ledger["taken_base"]
        self.assertEqual(taken["major"][4], [1, 60, 10, 1, 0])
        self.assertEqual(taken["barbarian"][0][0], 1)
        self.assertEqual(taken["city_state"][3][0], 1)
        self.assertEqual(sum(cell[0] for side in edge.SIDES for cell in taken[side]), 3)
        self.assertEqual(ledger["major_gap_by_turn"][2], [1, 15.0])
        self.assertEqual(ledger["major_gap_by_online_turn"][2], [1, 15.0])
        struck = ledger["struck_total"]["major"]
        self.assertEqual(struck[4], [1, 30, 10, 1, 0])
        self.assertEqual(ledger["struck_base"]["major"][4][0], 1)
        self.assertEqual(sum(c[0] for s in edge.SIDES for c in ledger["struck_total"][s]), 1)
        self.assertEqual(ledger["unmapped"], {"UNIT_FOO": 1})
        self.assertEqual(ledger["lane"], "ROME 4p Continents 9cs")

    def test_a_standard_game_reads_on_the_online_clock(self) -> None:
        run = _write_run(self.root, "civvis-20261112T000001Z", self._events()[:1],
                         speed="GAMESPEED_STANDARD")
        ledger = edge.read_run(run, BOOK)
        self.assertEqual(ledger["major_gap_by_turn"][2], [1, 15.0])
        self.assertEqual(ledger["major_gap_by_online_turn"][1], [1, 15.0])

    def test_marks_read_upgrades_treasury_and_rival_strength(self) -> None:
        ours = [
            {"kind": "UNIT_WARRIOR", "upgrade_to": "UNIT_MUSKETMAN", "upgrade_resource": "RESOURCE_NITER",
             "upgrade_resource_cost": 10, "upgrade_blocked_reason": "Not enough Gold"},
            {"kind": "UNIT_SPEARMAN", "upgrade_to": "UNIT_PIKEMAN"},
            {"kind": "UNIT_SWORDSMAN", "upgrade_to": "UNIT_MAN_AT_ARMS", "upgrade_resource": "RESOURCE_IRON",
             "upgrade_resource_cost": 20, "upgrade_blocked_reason": "Insufficient Resources"},
            {"kind": "UNIT_ARCHER"},
            {"kind": "UNIT_SETTLER"},
            {"kind": "UNIT_GALLEY", "upgrade_to": "UNIT_CARAVEL"},
        ]
        rivals = [{"player": 1, "units": [{"kind": "UNIT_MUSKETMAN"}, {"kind": "UNIT_GALLEY"}]},
                  {"player": 2, "units": [{"kind": "UNIT_FOO"}, {"type": "UNIT_ARCHER"}]}]
        run = _write_run(self.root, "civvis-20261112T000002Z", [
            _state(49, ours, rivals),
            _state(52, ours, rivals),
            _state(100, ours[:1], []),
        ])
        ledger = edge.read_run(run, BOOK)
        self.assertTrue(ledger["host_verdicts"])
        mark = ledger["marks"]["50"]
        self.assertEqual(len(mark), 1)
        row = mark[0]
        self.assertEqual(row["turn"], 52)
        self.assertEqual((row["army"], row["unlocked"], row["with_material"], row["offer"]), (4, 3, 2, 1))
        # Musketman 55 - Warrior 20, Pikeman 41 - Spearman 25, Man-at-Arms 45 - Swordsman 35.
        unit = BOOK.combat
        expected = (unit("musketman") - 20) + (unit("pikeman") - 25) + (unit("man_at_arms") - 35)
        self.assertEqual(row["gain_sum"], expected)
        # A Musketman is land army; a Galley is not; an unknown type is skipped;
        # a row that names its type under `type` (as `hostiles[]` does) still reads.
        self.assertEqual((row["rival_units"], row["rival_strength_sum"]),
                         (2, unit("musketman") + unit("archer")))
        summary = edge.mark_summary(mark)
        self.assertAlmostEqual(summary["unlocked_share"], 0.75)
        self.assertAlmostEqual(summary["strength_gap"],
                               (unit("musketman") + unit("archer")) / 2 - row["our_strength_sum"] / 4)
        self.assertEqual(summary["gold_median"], 150)
        self.assertEqual(len(ledger["marks"]["100"]), 1)

    def test_a_run_without_verdicts_walks_the_chain_and_reads_no_offer(self) -> None:
        run = _write_run(self.root, "civvis-20261112T000003Z", [
            _state(50, [{"kind": "UNIT_WARRIOR"}], [], techs=["TECH_BRONZE_WORKING", "TECH_IRON_WORKING"]),
        ])
        ledger = edge.read_run(run, BOOK)
        self.assertFalse(ledger["host_verdicts"])
        row = ledger["marks"]["50"][0]
        self.assertEqual(row["unlocked"], 1)
        self.assertIsNone(row["offer"])
        self.assertIsNone(edge.mark_summary([row])["offer_share"])

    def test_control_runs_respect_since_until_and_size(self) -> None:
        for name in ("civvis-20261111T230000Z", "civvis-20261112T010000Z", "civvis-20261113T060000Z"):
            _write_run(self.root, name, [])
        names = [run.name for run in edge.control_runs(self.root, "20261112", 0.0, "20261113T0530")]
        self.assertEqual(names, ["civvis-20261112T010000Z"])
        self.assertEqual(edge.control_runs(self.root, None, 1.0), [])

    def test_the_report_renders_and_pools(self) -> None:
        a = _write_run(self.root, "civvis-20261112T000004Z", self._events())
        b = _write_run(self.root, "civvis-20261112T000005Z", self._events()[:1])
        text, data = edge.ledger_report([a, b], BOOK, per_run=True, with_metrics=True)
        self.assertIn("== pooled: 2 run(s)", text)
        self.assertIn('("major_share_ge10", 1.000),', text)
        self.assertEqual(data["pooled"]["taken_base"]["major"][4][0], 2)


class MetricNamesTest(unittest.TestCase):
    def test_the_metric_list_is_the_simulators(self) -> None:
        """`gene_screen`'s driver pins its live columns to the simulator's
        metric names; this pins them to the live ledger's, so the three lists
        are one."""
        source = DRIVER.read_text()
        block = source.split("const LIVE_ROME")[1].split("];")[0]
        driver_names = re.findall(r'\("([a-z0-9_]+)", ', block)
        live_names = [name for name, _ in edge.metrics(edge.empty_ledger())]
        self.assertEqual(live_names, driver_names)


if __name__ == "__main__":
    unittest.main()
