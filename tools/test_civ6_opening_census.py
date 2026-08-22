"""The opening census reads the turns that decide a game out of one recorded run."""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_opening_census as census  # noqa: E402


def _state(turn: int, cities: int, pop: int, settlers: list[int], policies=(), pantheon=None, rival=0,
           producing=None):
    return {
        "kind": "state",
        "turn": turn,
        "cities": [{"capital": i == 0, "pop": pop if i == 0 else 1,
                    "producing": producing if i == 0 else None} for i in range(cities)],
        "units": [{"kind": "UNIT_SETTLER", "id": uid} for uid in settlers],
        "policies": list(policies),
        "pantheon": pantheon,
        "rivals": [{"public_stats": {"city_count": rival}}],
    }


def _write_run(root: Path) -> Path:
    run = root / "civvis-20260819T120000Z"
    run.mkdir()
    events = [
        _state(1, 0, 0, [65536]),
        {"kind": "found", "turn": 2, "unit": 65536, "x": 1, "y": 1},
        _state(2, 1, 1, []),
        _state(3, 1, 1, [], producing="UNIT_SCOUT"),
        _state(6, 1, 1, [], producing="UNIT_BUILDER"),
        # The host builds the Settler at 7 — the order at 7 and this agree; a
        # hint-built item would show here and nowhere in `orders`.
        _state(7, 1, 2, [], policies=["POLICY_DISCIPLINE", "POLICY_GOD_KING"], producing="UNIT_SETTLER"),
        _state(14, 1, 2, [7], policies=["POLICY_DISCIPLINE", "POLICY_GOD_KING"], producing="UNIT_WARRIOR"),
        _state(17, 1, 2, [7], policies=["POLICY_AGOGE", "POLICY_GOD_KING"]),
        {"kind": "found", "turn": 18, "unit": 7, "x": 5, "y": 5},
        _state(18, 2, 2, [], policies=["POLICY_AGOGE", "POLICY_GOD_KING"]),
        _state(20, 2, 3, [9], policies=["POLICY_AGOGE", "POLICY_GOD_KING"],
               pantheon="BELIEF_RELIGIOUS_SETTLEMENTS"),
        _state(21, 2, 3, [9, 11], policies=["POLICY_AGOGE", "POLICY_COLONIZATION"],
               pantheon="BELIEF_RELIGIOUS_SETTLEMENTS"),
        {"kind": "found", "turn": 24, "unit": 11, "x": 8, "y": 2},
        _state(30, 3, 3, [9], pantheon="BELIEF_RELIGIOUS_SETTLEMENTS", rival=2),
        _state(45, 3, 4, [], pantheon="BELIEF_RELIGIOUS_SETTLEMENTS", rival=3),
        {"kind": "found", "turn": 50, "unit": 13, "x": 2, "y": 8},
        _state(60, 4, 5, [], pantheon="BELIEF_RELIGIOUS_SETTLEMENTS", rival=4),
        _state(90, 4, 6, [], pantheon="BELIEF_RELIGIOUS_SETTLEMENTS", rival=5),
    ]
    (run / "events.jsonl").write_text("\n".join(json.dumps(e) for e in events) + "\n")
    connection = sqlite3.connect(run / "orders.sqlite")
    connection.execute(
        "create table orders (run text, turn integer, seq integer, kind text, subject integer, "
        "verb text, x integer, y integer)"
    )
    for turn, seq, kind, subject, verb in [
        (2, 0, "unit", 65536, "FOUND_CITY"),
        (3, 0, "produce", 65536, "UNIT_SCOUT"),
        (6, 0, "produce", 65536, "UNIT_BUILDER"),
        (7, 0, "produce", 65536, "UNIT_SETTLER"),
        (14, 0, "produce", 65536, "UNIT_WARRIOR"),
        (19, 0, "produce", 65536, "UNIT_SETTLER"),
        (30, 0, "produce", 131073, "UNIT_SETTLER"),
    ]:
        connection.execute("insert into orders values (?,?,?,?,?,?,?,?)",
                           (run.name, turn, seq, kind, subject, verb, None, None))
    connection.commit()
    connection.close()
    (run / "why.log").write_text(
        "[why] t6 Cities/Detail Rome holds the opening book's settler | the capital is population 1\n"
        "[why] t7 Cities/Decision Rome starts the opening book's settler | population 2\n"
    )
    return run


class OpeningCensusTest(unittest.TestCase):
    def test_one_run_reads_the_opening(self):
        with tempfile.TemporaryDirectory() as scratch:
            run = _write_run(Path(scratch))
            row = census.census(run)
        self.assertEqual(row["capital_turn"], 2)
        self.assertEqual(row["pop2_turn"], 7)
        self.assertEqual(row["first_settler_turn"], 7)
        self.assertEqual(row["book_settler_held"], 6)
        self.assertEqual(row["first_builds"], ["SCOUT", "BUILDER", "SETTLER", "WARRIOR"])
        self.assertEqual(row["city_turns"], [18, 24, 50])
        self.assertEqual((row["cities_at_30"], row["cities_at_45"], row["cities_at_60"]), (3, 3, 4))
        self.assertEqual(row["rival_cities_at_60"], 4)
        self.assertEqual(row["settler_orders"], 3)
        # Walkers: 7 (founded), 9 (vanished by turn 45 without founding), 11 (founded).
        self.assertEqual(row["walkers"], 3)
        self.assertEqual(row["walkers_lost"], 1)
        self.assertEqual((row["pantheon_turn"], row["pantheon"]), (20, "BELIEF_RELIGIOUS_SETTLEMENTS"))
        self.assertEqual(row["god_king"], [7, 20])

    def test_render_and_medians(self):
        with tempfile.TemporaryDirectory() as scratch:
            run = _write_run(Path(scratch))
            rows = census.rows_for([run, run])
        text = census.render(rows)
        self.assertIn("SCOUT,BUILDER,SETTLER,WARRIOR", text)
        self.assertIn("RELIGIOUS_SETTLE", text)
        self.assertIn("2 runs; medians", text)
        self.assertIn("city 2                    18 (2)", text)
        self.assertIn("walkers never founding    2/6", text)

    def test_missing_events_is_a_named_refusal(self):
        with tempfile.TemporaryDirectory() as scratch:
            with self.assertRaises(census.CensusError):
                census.census(Path(scratch))


if __name__ == "__main__":
    unittest.main()
