"""`tools/leader_ledger.py`: live results per exact LEADER_ id and lane."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import leader_ledger  # noqa: E402


def seat(leader: str, civ: str, players: int = 6, game_map: str = "Continents.lua") -> dict:
    return {"kind": "seat", "leader": leader, "civ": civ, "difficulty": "DIFFICULTY_IMMORTAL",
            "map": game_map, "players": players}


def turn(number: int, production: dict) -> dict:
    return {"kind": "turn", "ctx": "agent", "turn": number, "local_player": 0,
            "major_production": production}


class LedgerTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.root, ignore_errors=True))

    def run_dir(self, tag: str, summary: dict, events: list[dict] = ()) -> None:
        path = self.root / tag
        path.mkdir()
        (path / "summary.json").write_text(json.dumps(summary))
        (path / "events.jsonl").write_text("".join(json.dumps(e, sort_keys=True) + "\n" for e in events))

    def records(self, **filters) -> list[dict]:
        parts = leader_ledger.game_parts(self.root, filters.pop("since", ""))
        return [r for r in map(leader_ledger.game_record, parts.values()) if r]

    def test_a_continued_game_is_one_game_scored_by_its_last_part(self):
        ottoman = seat("LEADER_SULEIMAN", "CIVILIZATION_OTTOMAN")
        self.run_dir("civvis-20261031T011801Z", {"seat": ottoman, "victory_target": "science",
                                                 "last_turn": 169},
                     [turn(80, {"0": 40, "1": 100, "2": 160})])
        self.run_dir("civvis-20261031T011801Z-cont1", {
            "seat": ottoman, "victory_target": "science", "last_turn": 205,
            "outcome": {"victory_type": "VICTORY_TECHNOLOGY", "won": False}})
        (record,) = self.records()
        self.assertEqual(record["game"], "civvis-20261031T011801Z")
        self.assertEqual(record["result"], "loss:technology")
        self.assertEqual(record["last_turn"], 205)
        self.assertEqual(record["share_t80"], 0.25)

    def test_win_retired_and_unfinished(self):
        self.run_dir("civvis-20261031T025738Z", {
            "seat": seat("LEADER_SULEIMAN", "CIVILIZATION_OTTOMAN"), "victory_target": "diplomatic",
            "outcome": {"victory_type": "VICTORY_DIPLOMATIC", "won": True}})
        self.run_dir("civvis-20261031T024232Z", {
            "seat": seat("LEADER_SHAKA", "CIVILIZATION_ZULU"), "victory_target": "diplomatic",
            "abandoned": {"rule": "production_share", "turn": 178}})
        self.run_dir("civvis-20261031T011217Z", {
            "seat": seat("LEADER_JOHN_CURTIN", "CIVILIZATION_AUSTRALIA"), "last_turn": 71})
        results = {r["leader"]: r["result"] for r in self.records()}
        self.assertEqual(results, {"LEADER_SULEIMAN": "win:diplomatic",
                                   "LEADER_SHAKA": "retired",
                                   "LEADER_JOHN_CURTIN": "unfinished"})

    def test_the_leader_is_the_seat_events_exact_id(self):
        # Qin Shi Huang's alternate persona is its own id, not China's.
        self.run_dir("civvis-20261031T021615Z", {"seat": seat("LEADER_QIN_ALT", "CIVILIZATION_CHINA")})
        self.run_dir("civvis-20261031T021700Z", {"seat": {}})
        self.assertEqual([r["leader"] for r in self.records()], ["LEADER_QIN_ALT"])

    def test_since_and_the_seat_filters(self):
        self.run_dir("civvis-20261030T120000Z", {"seat": seat("LEADER_PERICLES", "CIVILIZATION_GREECE", 4, "Pangaea.lua")})
        self.run_dir("civvis-20261031T030000Z", {"seat": seat("LEADER_SHAKA", "CIVILIZATION_ZULU")})
        self.assertEqual(len(self.records(since="20261031T0000")), 1)
        out = self.capture(["--root", str(self.root)])
        self.assertIn("1 games", out)
        self.assertIn("LEADER_SHAKA", out)
        self.assertNotIn("LEADER_PERICLES", out)
        self.assertIn("2 games", self.capture(["--root", str(self.root), "--map", "any", "--players", "0"]))

    def test_rows_group_by_leader_and_lane(self):
        for stamp, lane, outcome in (("010000", "science", {"victory_type": "VICTORY_TECHNOLOGY", "won": True}),
                                     ("020000", "science", {"victory_type": "VICTORY_CULTURE", "won": False}),
                                     ("030000", "culture", {})):
            self.run_dir(f"civvis-20261031T{stamp}Z", {"seat": seat("LEADER_SHAKA", "CIVILIZATION_ZULU"),
                                                       "victory_target": lane, "outcome": outcome})
        rows = {(r["leader"], r["lane"]): r for r in leader_ledger.ledger(self.records())}
        science = rows[("LEADER_SHAKA", "science")]
        self.assertEqual((science["games"], science["wins"], science["losses"]), (2, 1, ["culture"]))
        self.assertEqual(rows[("LEADER_SHAKA", "culture")]["unfinished"], 1)

    def test_no_turn_80_record_reads_no_share(self):
        events = self.root / "events.jsonl"
        events.write_text(json.dumps(turn(79, {"0": 1, "1": 9})) + "\n")
        self.assertIsNone(leader_ledger.production_share_at(events))
        self.assertIsNone(leader_ledger.production_share_at(self.root / "missing.jsonl"))

    def capture(self, argv: list[str]) -> str:
        import contextlib
        import io
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            self.assertEqual(leader_ledger.main(argv), 0)
        return buffer.getvalue()


if __name__ == "__main__":
    unittest.main()
