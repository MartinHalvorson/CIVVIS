#!/usr/bin/env python3
"""The tactical ledger reads what the mod recorded and says what it did not."""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_tactics_ledger as ledger  # noqa: E402


def _write_run(root: Path, events: list[dict], orders: list[tuple]) -> Path:
    run = root / "civvis-test"
    run.mkdir()
    with (run / "events.jsonl").open("w") as handle:
        for event in events:
            handle.write(json.dumps(event) + "\n")
    con = sqlite3.connect(run / "orders.sqlite")
    con.execute(
        "CREATE TABLE orders (run TEXT, turn INTEGER, seq INTEGER, kind TEXT, "
        "subject INTEGER, verb TEXT, x INTEGER, y INTEGER)"
    )
    con.executemany(
        "INSERT INTO orders VALUES ('civvis-test', ?, ?, 'unit', ?, ?, ?, ?)", orders
    )
    con.commit()
    con.close()
    return run


def _unit(uid: int, kind: str, x: int, y: int, **extra) -> dict:
    unit = {"id": uid, "kind": kind, "x": x, "y": y, "hp": 100, "moves": 2,
            "combat": 20 if kind != "UNIT_SETTLER" else 0, "ranged": 0}
    unit.update(extra)
    return unit


class GeometryTest(unittest.TestCase):
    def test_hex_distance_on_the_offset_grid(self) -> None:
        self.assertEqual(ledger.hex_distance((0, 0), (0, 0)), 0)
        self.assertEqual(ledger.hex_distance((0, 0), (1, 0)), 1)
        self.assertEqual(ledger.hex_distance((0, 0), (3, 0)), 3)
        # Odd-r offset (odd rows shifted right, the host's layout): the even
        # row's north-east neighbour is (0,1) and (1,1) is two hexes away.
        self.assertEqual(ledger.hex_distance((0, 0), (0, 1)), 1)
        self.assertEqual(ledger.hex_distance((0, 0), (1, 1)), 2)
        self.assertEqual(ledger.hex_distance((1, 1), (1, 2)), 1)
        self.assertEqual(ledger.hex_distance((1, 1), (2, 2)), 1)


class ArrivalTest(unittest.TestCase):
    def test_moves_are_judged_against_the_next_frame(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 5, "gold": 100, "units": [
                _unit(1, "UNIT_WARRIOR", 2, 2), _unit(2, "UNIT_SCOUT", 5, 5, moves=0),
                _unit(3, "UNIT_ARCHER", 8, 8), _unit(4, "UNIT_WARRIOR", 1, 1)]},
            {"kind": "state", "turn": 6, "gold": 100, "units": [
                _unit(1, "UNIT_WARRIOR", 3, 2),   # arrived
                _unit(2, "UNIT_SCOUT", 5, 5),     # did not move, no moves at export
                _unit(3, "UNIT_ARCHER", 8, 8)]},  # did not move WITH moves; unit 4 gone
        ]
        orders = [
            (5, 1, 1, "MOVE_TO", 3, 2),
            (5, 2, 2, "MOVE_TO", 6, 5),
            (5, 3, 3, "MOVE_TO", 9, 8),
            (5, 4, 3, "RANGE_ATTACK", 10, 8),   # a follow-up, not a move
            (5, 5, 4, "MOVE_TO", 2, 1),
            (5, 6, 1, "MOVE_TO", 4, 2),         # second move for unit 1: not judged
        ]
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, orders)
            report = ledger.ledger(run)
        arrival = report["arrival"]
        self.assertEqual(arrival["outcomes"]["arrived"], 1)
        self.assertEqual(arrival["outcomes"]["did_not_move"], 2)
        self.assertEqual(arrival["outcomes"]["gone"], 1)
        self.assertEqual(arrival["did_not_move_with_moves_at_export"], 1)
        self.assertEqual(arrival["did_not_move_without_moves_at_export"], 1)
        self.assertEqual(arrival["moves_judged"], 4)
        self.assertEqual(report["orders"]["unit_turns_with_more_than_one_order"], 2)
        # No queue field in the orders event, no combat events: say so.
        self.assertIsNone(report["orders"]["queue"])
        self.assertIsNone(report["combat"])
        text = ledger.render(report)
        self.assertIn("mod predates", text)


class QueueAndCombatTest(unittest.TestCase):
    def test_queue_strikes_and_combat_events_are_summed(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 9, "gold": 50, "units": [_unit(1, "UNIT_ARCHER", 2, 2, ranged=25)],
             "hostiles": [{"x": 4, "y": 2, "type": "UNIT_WARRIOR", "combat": 20}]},
            {"kind": "orders", "turn": 9, "queued": 2, "applied": 3},
            {"kind": "orders_queue", "turn": 9, "applied": 2, "refused": 0, "refusals": {},
             "strikes_planned": 1, "strikes_landed": 1, "waited": 3},
            {"kind": "combat", "turn": 9,
             "attacker": {"player": 0, "id": 1, "kind": "UNIT_ARCHER", "type": "unit"},
             "defender": {"player": 63, "id": 900, "kind": "UNIT_WARRIOR", "type": "unit"},
             "damage_to_defender": 34, "damage_to_attacker": 0, "defender_killed": False,
             "attacker_killed": False, "preview": {"damage_to_defender": 30, "damage_to_attacker": 0}},
            {"kind": "combat", "turn": 9,
             "attacker": {"player": 63, "id": 901, "kind": "UNIT_WARRIOR", "type": "unit"},
             "defender": {"player": 0, "id": 1, "kind": "UNIT_ARCHER", "type": "unit"},
             "damage_to_defender": 100, "damage_to_attacker": 12, "defender_killed": True,
             "attacker_killed": False},
            {"kind": "state", "turn": 10, "gold": 50, "units": [],
             "hostiles": [{"x": 4, "y": 2, "type": "UNIT_WARRIOR", "combat": 20}]},
        ]
        orders = [(9, 1, 1, "MOVE_TO", 3, 2), (9, 2, 1, "RANGE_ATTACK", 4, 2)]
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, orders)
            report = ledger.ledger(run)
        queue = report["orders"]["queue"]
        self.assertEqual(queue["queued_followups"], 2)
        self.assertEqual(queue["strikes_planned"], 1)
        self.assertEqual(queue["strikes_landed"], 1)
        combat = report["combat"]
        self.assertEqual(combat["combats"], 2)
        self.assertEqual(combat["our_attacks"], 1)
        self.assertEqual(combat["attacks_received"], 1)
        self.assertEqual(combat["kills"], 0)
        self.assertEqual(combat["losses"], 1)
        self.assertEqual(combat["damage_dealt"], 34 + 12)
        self.assertEqual(combat["damage_taken"], 100)
        self.assertEqual(combat["losses_by_kind"], {"UNIT_ARCHER": 1})
        self.assertEqual(combat["host_preview"]["strikes_previewed"], 1)
        self.assertAlmostEqual(combat["host_preview"]["mean_actual_minus_predicted"], 4.0)
        # The archer left the board beside a hostile.
        self.assertEqual(report["roster"]["context_at_last_sight"], {"hostile_within_2": 1})
        text = ledger.render(report)
        self.assertIn("landed same turn 1", text)
        self.assertIn("kills 0, losses 1", text)


class SalvageableLossTest(unittest.TestCase):
    def test_a_unit_last_seen_wounded_is_a_loss_the_seat_saw_coming(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 3, "gold": 40, "units": [
                _unit(1, "UNIT_WARRIOR", 5, 5), _unit(2, "UNIT_ARCHER", 6, 5),
                _unit(3, "UNIT_SPEARMAN", 7, 5)], "hostiles": []},
            {"kind": "state", "turn": 4, "gold": 40, "units": [
                _unit(3, "UNIT_SPEARMAN", 7, 5)], "hostiles": []},
        ]
        # Unit 1 was at 24 hp when last seen, unit 2 at full health.
        events[1]["units"][0]["hp"] = 24
        events[1]["units"][1]["hp"] = 100
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, [])
            report = ledger.ledger(run)
        roster = report["roster"]
        self.assertEqual(roster["military_units_gone"], 2)
        self.assertEqual(roster["lost_when_salvageable"], 1)
        self.assertEqual(roster["salvageable_share"], 0.5)
        self.assertIn("had a turn's warning of", ledger.render(report))

    def test_a_run_that_lost_nothing_says_nothing_rather_than_zero(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 3, "gold": 0, "units": [_unit(1, "UNIT_WARRIOR", 5, 5)],
             "hostiles": []},
            {"kind": "state", "turn": 4, "gold": 0, "units": [_unit(1, "UNIT_WARRIOR", 5, 5)],
             "hostiles": []},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, [])
            report = ledger.ledger(run)
        self.assertEqual(report["roster"]["military_units_gone"], 0)
        self.assertIsNone(report["roster"]["salvageable_share"])


class CityOccupationTest(unittest.TestCase):
    def test_a_captured_city_is_counted_and_a_lost_one_is_not_a_capture(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 5, "gold": 0, "units": [], "hostiles": []},
            {"kind": "combat", "turn": 5,
             "attacker": {"player": 0, "id": 1, "kind": "UNIT_WARRIOR", "type": "unit"},
             "defender": {"player": 2, "id": 900, "kind": "CITY", "type": "city"},
             "damage_to_defender": 20, "damage_to_attacker": 5,
             "defender_killed": False, "attacker_killed": False},
            # Taken from a rival.
            {"kind": "city_occupation", "turn": 5, "player": 0, "city": 77,
             "name": "Kumasi", "original_owner": 2, "ours_now": True},
            # Our own, retaken by us: not a capture.
            {"kind": "city_occupation", "turn": 6, "player": 0, "city": 78,
             "name": "Rome", "original_owner": 0, "ours_now": True},
            # And one of ours lost.
            {"kind": "city_occupation", "turn": 7, "player": 2, "city": 79,
             "name": "Ostia", "original_owner": 0, "ours_now": False},
            {"kind": "state", "turn": 8, "gold": 0, "units": [], "hostiles": []},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, [])
            report = ledger.ledger(run)
        combat = report["combat"]
        self.assertEqual(combat["cities_taken"], 1)
        self.assertEqual(combat["cities_lost"], 1)
        self.assertIn("cities taken 1, lost 1", ledger.render(report))

    def test_a_run_with_no_seat_event_claims_no_captures(self) -> None:
        events = [
            {"kind": "city_occupation", "turn": 5, "player": 0, "city": 77,
             "original_owner": 2, "ours_now": True},
        ]
        self.assertEqual(ledger.city_occupations(events, None), (0, 0))


class HoverTest(unittest.TestCase):
    def test_a_unit_that_neither_moves_nor_strikes_near_a_hostile_hovers(self) -> None:
        events = [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 3, "gold": 5, "units": [
                _unit(1, "UNIT_WARRIOR", 0, 0), _unit(2, "UNIT_WARRIOR", 0, 3), _unit(3, "UNIT_SETTLER", 0, 3)],
             "hostiles": [{"x": 3, "y": 0, "type": "UNIT_WARRIOR", "combat": 20}]},
            {"kind": "state", "turn": 4, "gold": 5, "units": [
                _unit(1, "UNIT_WARRIOR", 0, 0), _unit(2, "UNIT_WARRIOR", 1, 3), _unit(3, "UNIT_SETTLER", 0, 3)],
             "hostiles": []},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            run = _write_run(Path(tmp), events, [])
            report = ledger.ledger(run)
        hover = report["hover"]
        self.assertEqual(hover["military_unit_turns"], 2)
        self.assertEqual(hover["unit_turns_2_to_4_from_a_hostile"], 2)
        self.assertEqual(hover["hovering_unit_turns"], 1)


class HallOfFameTest(unittest.TestCase):
    def test_the_local_players_points_are_read(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            hof = Path(tmp) / "HallofFame.sqlite"
            con = sqlite3.connect(hof)
            con.execute("CREATE TABLE GamePlayers (GameId INTEGER, PlayerObjectId INTEGER, IsLocal INTEGER)")
            con.execute("CREATE TABLE ObjectDataPointValues (ObjectId INTEGER, DataPoint TEXT, ValueNumeric REAL)")
            con.executemany("INSERT INTO GamePlayers VALUES (?, ?, ?)", [(1, 10, 1), (1, 11, 0), (2, 20, 1)])
            con.executemany("INSERT INTO ObjectDataPointValues VALUES (?, ?, ?)", [
                (10, "UnitsLost", 8), (10, "UnitsKilled", 3), (11, "UnitsKilled", 40),
                (20, "UnitsLost", 2), (20, "UnitsKilled", 5), (20, "CitiesConquered", 1),
            ])
            con.commit()
            con.close()
            section = ledger.hall_of_fame_section(hof)
        self.assertEqual(section["games"], 2)
        self.assertEqual(section["totals"]["UnitsLost"], 10)
        self.assertEqual(section["totals"]["UnitsKilled"], 8)
        self.assertEqual(section["latest"]["CitiesConquered"], 1)
        self.assertAlmostEqual(section["kills_per_loss_all_games"], 0.8)
        self.assertIsNone(ledger.hall_of_fame_section(Path("/nonexistent/HallofFame.sqlite")))


if __name__ == "__main__":
    unittest.main()
