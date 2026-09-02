#!/usr/bin/env python3
"""The engagement section counts unit-vs-unit fights and says which rows it dropped.

One synthetic run, every KPI's numerator and denominator worked by hand in the
comments, so a change to a definition shows up as a changed number here rather
than as a quietly different report.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_tactics_ledger as ledger  # noqa: E402

US, BARBARIANS, RIVAL = 0, 63, 1


def _unit(uid: int, kind: str, x: int, y: int, **extra) -> dict:
    unit = {"id": uid, "kind": kind, "x": x, "y": y, "hp": 100, "moves": 2, "combat": 20,
            "ranged": 0, "fortified": False, "activity": "awake", "embarked": False,
            "attacks_remaining": 1}
    unit.update(extra)
    return unit


def _hostile(uid: int, kind: str, x: int, y: int, **extra) -> dict:
    hostile = {"id": uid, "type": kind, "x": x, "y": y, "hp": 100, "combat": 20, "ranged": 0,
               "player": BARBARIANS}
    hostile.update(extra)
    return hostile


def _side(player: int, uid: int, kind: str = "UNIT_WARRIOR", hp: int = 100, type_: str = "unit") -> dict:
    return {"player": player, "id": uid, "kind": kind, "hp": hp, "type": type_}


def _combat(turn: int, attacker: dict, defender: dict, dealt: int | None, *, taken: int = 0,
            defender_killed: bool = False, attacker_killed: bool = False,
            hp_end: int | None = None, **extra) -> dict:
    event = {"kind": "combat", "turn": turn, "attacker": attacker, "defender": defender,
             "damage_to_attacker": taken, "defender_killed": defender_killed,
             "attacker_killed": attacker_killed,
             "ours": attacker.get("player") == US,
             "against_us": defender.get("player") == US}
    if dealt is not None:
        event["damage_to_defender"] = dealt
    if hp_end is not None:
        event["defender_hp_end"] = hp_end
    event.update(extra)
    return event


def _strike(turn: int, unit: int, hp: int, damage_to_attacker: int | None) -> dict:
    event = {"kind": "strike", "turn": turn, "unit": unit, "hp": hp, "verb": "RANGE_ATTACK",
             "x": 7, "y": 5}
    if damage_to_attacker is not None:
        event["preview"] = {"damage_to_attacker": damage_to_attacker, "damage_to_defender": 30}
    return event


# The hostile warrior H1 stands at (7,5). Hex distances from it on the odd-r
# grid: (6,5) 1, (5,5) 2, (6,4) 2, (7,6) 1, (3,5) 4.
H1 = _hostile(900, "UNIT_WARRIOR", 7, 5)
# A builder beside our archer: no combat, no ranged, so never a hostile plot.
H_BUILDER = _hostile(901, "UNIT_BUILDER", 4, 5, combat=0)

STATE_T10 = {
    "kind": "state", "turn": 10, "frame": 0, "gold": 50,
    "units": [
        _unit(1, "UNIT_ARCHER", 5, 5, ranged=25, combat=15),        # in range (2), strikes
        _unit(2, "UNIT_WARRIOR", 6, 5, hp=40, fortified=True),      # wounded, exposed, healing; in range, idle
        _unit(3, "UNIT_CATAPULT", 6, 4),                            # siege kind reaches 2: in range, idle healthy
        _unit(4, "UNIT_SWORDSMAN", 3, 5, hp=30, embarked=True),     # wounded, out of reach, awake: not healing
        _unit(5, "UNIT_SETTLER", 5, 6, combat=0),                   # not military
        _unit(6, "UNIT_WARRIOR", 7, 6, embarked=True),              # adjacent but embarked: cannot strike
    ],
    "hostiles": [H1, H_BUILDER],
    "cities": [{"id": 77, "name": "Ostia", "x": 20, "y": 20}, {"id": 78, "name": "Antium", "x": 30, "y": 30}],
}
# A mid-turn frame of the same turn: a dying unit beside the hostile that must
# NOT be counted — the KPIs read the board the orders were decided from.
STATE_T10_FRAME1 = {
    "kind": "state", "turn": 10, "frame": 1, "gold": 50,
    "units": [_unit(9, "UNIT_WARRIOR", 6, 5, hp=10)],
    "hostiles": [H1], "cities": STATE_T10["cities"],
}
STATE_T11 = {
    "kind": "state", "turn": 11, "frame": 0, "gold": 50,
    "units": [
        _unit(1, "UNIT_ARCHER", 5, 5, ranged=25, combat=15),        # in range, no strike: idle healthy
        _unit(3, "UNIT_CATAPULT", 6, 4),                            # in range, strikes
        _unit(4, "UNIT_SWORDSMAN", 3, 5, hp=30, activity="sleep"),  # wounded, healing (sleep), not exposed
        _unit(7, "UNIT_WARRIOR", 20, 21),                           # one hex from Ostia (20,20)
    ],
    "hostiles": [H1],
    "cities": STATE_T10["cities"],
}
STATE_T12 = {
    "kind": "state", "turn": 12, "frame": 0, "gold": 50,
    "units": [_unit(1, "UNIT_ARCHER", 5, 5, ranged=25, combat=15)],
    "hostiles": [],
    "cities": [{"id": 78, "name": "Antium", "x": 30, "y": 30}],
}

COMBATS = [
    # C1+C2: two hits on 900 the same turn; the last leaves it at 50 (not low). C2 is a chip.
    _combat(10, _side(US, 1, "UNIT_ARCHER"), _side(BARBARIANS, 900), 40, hp_end=60,
            preview={"damage_to_defender": 38, "damage_to_attacker": 0}),
    _combat(10, _side(US, 1, "UNIT_ARCHER"), _side(BARBARIANS, 900, hp=60), 10, hp_end=50),
    # C3: one hit that leaves 902 alive at 25: left low.
    _combat(10, _side(US, 3, "UNIT_CATAPULT"), _side(BARBARIANS, 902), 75, hp_end=25),
    # C4: our wounded warrior (hp 40) killed defending.
    _combat(10, _side(BARBARIANS, 900, hp=50), _side(US, 2, "UNIT_WARRIOR", hp=40), 40, taken=10,
            defender_killed=True, hp_end=0),
    # C5: a kill attacking.
    _combat(11, _side(US, 3, "UNIT_CATAPULT"), _side(BARBARIANS, 903, hp=20), 20,
            defender_killed=True, hp_end=0),
    # C6: an enemy attacker dies attacking our archer: a kill defending.
    _combat(11, _side(BARBARIANS, 904, hp=15), _side(US, 1, "UNIT_ARCHER"), 12, taken=15,
            attacker_killed=True, hp_end=88),
    # C7: our healthy swordsman (hp 60) killed defending: a loss, not a wounded one.
    _combat(11, _side(BARBARIANS, 905), _side(US, 4, "UNIT_SWORDSMAN", hp=60), 60,
            defender_killed=True, hp_end=0),
    # C8: our attacker dies attacking: a loss attacking; target left at 70.
    _combat(11, _side(US, 1, "UNIT_ARCHER"), _side(BARBARIANS, 906), 30, taken=100,
            attacker_killed=True, hp_end=70),
    # C9: OUR CITY strikes a unit and kills it. The district carries the spurious
    # attacker_killed flag every `gone` district does: not a loss.
    _combat(11, {"type": "district", "id": 77, "player": US, "gone": True},
            _side(BARBARIANS, 907, hp=20), 20, defender_killed=True, attacker_killed=True),
    # C10+C11: the junk row, twice.
    _combat(11, {"type": "district", "id": -1, "player": -1, "gone": True},
            {"type": "district", "id": -1, "player": -1, "gone": True}, None,
            defender_killed=True, attacker_killed=True),
    _combat(11, {"type": "district", "id": -1, "player": -1, "gone": True},
            {"type": "district", "id": -1, "player": -1, "gone": True}, None,
            defender_killed=True, attacker_killed=True),
    # C12: our unit hits a rival district and "kills" it: unit-vs-city, unused.
    _combat(11, _side(US, 3, "UNIT_CATAPULT"), {"type": "district", "id": 500, "player": RIVAL, "gone": True},
            None, defender_killed=True),
    # C13: barbarians versus a rival: nothing to do with us.
    _combat(11, _side(BARBARIANS, 908), _side(RIVAL, 700), 50, defender_killed=True),
    # C14: a rival CITY kills our unit: a non-unit attacker, dropped.
    _combat(11, {"type": "district", "id": 600, "player": RIVAL, "gone": True},
            _side(US, 8, "UNIT_SCOUT", hp=30), 30, defender_killed=True, attacker_killed=True),
]

STRIKES = [
    _strike(10, 1, 100, 0),      # archer, safe
    _strike(10, 1, 100, 5),      # the same unit-turn again: still one firing unit-turn
    _strike(11, 3, 20, 25),      # the strike's own hp reading (20) against a 25 preview: suicidal
    _strike(11, 3, 20, None),    # no preview: not judged
]

CITY_LOSSES = [
    {"kind": "city_lost", "turn": 12, "city": 77, "name": "Ostia"},             # unit 7 adjacent at t11: defended
    {"kind": "city_lost", "turn": 13, "city": 78, "name": "LOC_CITY_NAME_ANTIUM"},  # by name; nobody near: undefended
    {"kind": "city_lost", "turn": 13, "city": 999, "name": "Nowhere"},           # never exported: unresolved
]

EVENTS = ([{"kind": "seat", "local_player": US}, STATE_T10, STATE_T10_FRAME1, STATE_T11, STATE_T12]
          + COMBATS + STRIKES + CITY_LOSSES)


class EngagementSectionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.section = ledger.engagement_section(EVENTS, US)

    def test_the_dropped_rows_are_named(self) -> None:
        self.assertEqual(self.section["rows"], {
            "combat_events": 14, "junk_district_pairs": 2, "non_unit_attackers": 2,
            "unit_vs_city": 1, "third_party": 1, "ours": 5, "against_us": 3})

    def test_initiative_is_our_unit_attacks_over_every_unit_fight_we_were_in(self) -> None:
        self.assertEqual(self.section["initiative_share"],
                         {"numerator": 5, "denominator": 8, "share": 0.625})

    def test_kills_per_loss_reads_neither_the_junk_nor_the_district_flags(self) -> None:
        self.assertEqual(self.section["army_kills_per_loss"], {
            "kills": 2, "kills_attacking": 1, "kills_defending": 1,
            "losses": 3, "losses_defending": 2, "losses_attacking": 1, "value": 0.67})
        self.assertEqual(self.section["city_strikes"], 1)
        self.assertEqual(self.section["city_strike_kills"], 1)

    def test_killed_when_wounded_reads_the_defenders_hp_at_the_start_of_the_combat(self) -> None:
        self.assertEqual(self.section["killed_when_wounded_share"],
                         {"numerator": 1, "denominator": 2, "share": 0.5, "hp_unknown": 0})

    def test_wounded_exposure_and_healing_read_frame_zero_only(self) -> None:
        # t10: warrior 2 (hp 40, one hex from H1, fortified) and swordsman 4 (hp 30,
        # four hexes away, awake); t11: swordsman 4 asleep. Frame 1's dying
        # warrior 9 is not on the board the turn was decided from.
        self.assertEqual(self.section["wounded_exposed_share"],
                         {"numerator": 1, "denominator": 3, "share": 0.333})
        self.assertEqual(self.section["wounded_healing_share"],
                         {"numerator": 2, "denominator": 3, "share": 0.667})

    def test_firepower_counts_units_with_a_target_in_their_own_range(self) -> None:
        # t10: archer (range 2, struck), warrior 2 (adjacent, idle, wounded),
        # catapult (siege kind, range 2, idle, healthy); embarked warrior 6 skipped.
        # t11: archer idle healthy, catapult struck. 5 in range, 2 fired.
        self.assertEqual(self.section["firepower_utilisation"],
                         {"numerator": 2, "denominator": 5, "share": 0.4})
        self.assertEqual(self.section["idle_healthy_share"],
                         {"numerator": 2, "denominator": 5, "share": 0.4})
        self.assertEqual(self.section["idle_wounded"], 1)

    def test_focus_groups_our_attacks_by_turn_and_target(self) -> None:
        focus = self.section["focus"]
        self.assertEqual(focus["targets"], 4)
        self.assertEqual(focus["multi_hit_share"], {"numerator": 1, "denominator": 4, "share": 0.25})
        self.assertEqual(focus["left_low_share"], {"numerator": 1, "denominator": 4, "share": 0.25})

    def test_chips_and_suicidal_strikes(self) -> None:
        self.assertEqual(self.section["chip_share"], {"numerator": 1, "denominator": 5, "share": 0.2})
        self.assertEqual(self.section["suicidal_attacks"], {"numerator": 1, "denominator": 3, "share": 0.333})

    def test_cities_lost_are_judged_on_the_last_frame_that_still_held_them(self) -> None:
        self.assertEqual(self.section["cities_lost_undefended"],
                         {"cities_lost": 3, "undefended": 1, "defended": 1, "unresolved": 1})

    def test_the_section_says_when_the_mod_predates_it(self) -> None:
        self.assertIsNone(ledger.engagement_section(
            [{"kind": "seat", "local_player": US}, STATE_T10], US))
        text = ledger.render({
            "run": "r", "turns_recorded": 1,
            "orders": ledger.orders_section([], []),
            "arrival": ledger.arrival_section([], []),
            "combat": None,
            "roster": ledger.roster_section([]),
            "hover": ledger.hover_section([], []),
            "engagement": None,
        })
        self.assertIn("engagement (mod predates the ledger", text)

    def test_rows_without_the_flags_fall_back_to_the_players(self) -> None:
        bare = [dict(event) for event in COMBATS]
        for event in bare:
            event.pop("ours", None)
            event.pop("against_us", None)
        flagged = ledger.engagement_section(COMBATS, US)
        unflagged = ledger.engagement_section(bare, US)
        self.assertEqual(flagged["rows"], unflagged["rows"])
        self.assertEqual(flagged["army_kills_per_loss"], unflagged["army_kills_per_loss"])


class EngagementInTheLedgerTest(unittest.TestCase):
    def test_the_report_carries_the_section_in_json_and_text(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-test"
            run.mkdir()
            with (run / "events.jsonl").open("w") as handle:
                for event in EVENTS:
                    handle.write(json.dumps(event) + "\n")
            report = ledger.ledger(run)
        self.assertEqual(report["engagement"]["initiative_share"]["numerator"], 5)
        json.dumps(report)  # every value serialises
        text = ledger.render(report)
        self.assertIn("engagement (unit-vs-unit; dropped 2 district-vs-district id -1 rows "
                      "and 2 non-unit attackers of 14 combat rows", text)
        self.assertIn("initiative 5/8 (62.5%)", text)
        self.assertIn("army kills/loss 0.67: kills 2 (1 attacking, 1 defending) / losses 3 "
                      "(2 defending, 1 attacking); city strikes 1, city-strike kills 1", text)
        self.assertIn("killed when wounded 1/2 (50.0%)", text)
        self.assertIn("wounded exposed 1/3 (33.3%)", text)
        self.assertIn("healing 2/3 (66.7%)", text)
        self.assertIn("firepower 2/5 (40.0%)", text)
        self.assertIn("idle healthy 2/5 (40.0%), idle wounded 1", text)
        self.assertIn("focus 4 targets: multi-hit 1/4 (25.0%), left low (alive at <= 30 hp) 1/4 (25.0%)", text)
        self.assertIn("chip 1/5 (20.0%)", text)
        self.assertIn("suicidal 1/3 (33.3%)", text)
        self.assertIn("cities lost 3: undefended 1, defended 1, unresolved 1", text)
        # The section that was already there is unchanged by the addition: it
        # still counts every row whose attacker is player 0, districts and all.
        self.assertEqual(report["combat"]["our_attacks"], 7)


if __name__ == "__main__":
    unittest.main()
