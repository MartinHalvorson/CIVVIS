"""`tools/path_steps.py`: the count of one game's path, and the ledger it
keeps."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import path_steps  # noqa: E402

MOVE = lambda unit, to: {"type": "move", "unit": unit, "to": to}  # noqa: E731
END = {"type": "end_turn"}


class PathOfTest(unittest.TestCase):
    def test_steps_bytes_and_kinds_are_counted_from_the_log(self):
        log = [[0, {"type": "research", "tech": "mining"}], [0, MOVE(1, [3, 4])], [0, END], [1, END]]
        path = path_steps.path_of(log)
        self.assertEqual(path["steps"], 4)
        self.assertEqual(path["kinds"], {"end_turn": 2, "research": 1, "move": 1})
        self.assertEqual(path["bytes"], sum(len(json.dumps(action, separators=(",", ":"))) for _, action in log))

    def test_a_units_consecutive_moves_are_one_walk(self):
        log = [[0, MOVE(1, [1, 0])], [0, MOVE(1, [2, 0])], [0, MOVE(1, [3, 0])], [0, MOVE(2, [9, 9])], [0, END]]
        path = path_steps.path_of(log)
        self.assertEqual((path["hops"], path["walks"]), (4, 2))
        # Five steps, of which three moves are one walk: three steps merged.
        self.assertEqual(path["merged"], 3)

    def test_a_walk_ends_at_any_other_step_and_at_another_seat(self):
        # The same unit moves, attacks, and moves on: two walks.
        log = [[0, MOVE(1, [1, 0])], [0, {"type": "attack", "unit": 1, "target": [2, 0]}], [0, MOVE(1, [2, 0])]]
        self.assertEqual(path_steps.path_of(log)["walks"], 2)
        # Unit 1 of seat 0 and unit 1 of seat 1 are different units.
        log = [[0, MOVE(1, [1, 0])], [1, MOVE(1, [5, 5])]]
        self.assertEqual(path_steps.path_of(log)["walks"], 2)
        # A unit's moves in two turns are two walks: the end of turn is a step between.
        log = [[0, MOVE(1, [1, 0])], [0, END], [0, MOVE(1, [2, 0])]]
        self.assertEqual(path_steps.path_of(log)["walks"], 2)

    def test_the_merged_path_is_never_longer_than_the_path(self):
        log = [[0, MOVE(unit % 3, [unit, 0])] for unit in range(30)] + [[0, END]]
        path = path_steps.path_of(log)
        self.assertLessEqual(path["merged"], path["steps"])
        self.assertEqual(path["merged"], path["steps"] - path["hops"] + path["walks"])


class FaithfulTest(unittest.TestCase):
    """What the probe prints for a replay, read as same world or not."""

    NOTES = frozenset({"commit", "raid_wars"})

    def test_nothing_refused_and_nothing_different_is_the_same_world(self):
        self.assertTrue(path_steps.faithful({"baseline": True, "refused": 0, "first_refusal": None, "differs_from_recorded": []}, self.NOTES))

    def test_the_agents_own_notes_may_differ(self):
        line = {"baseline": True, "refused": 0, "differs_from_recorded": ["players[*].counters"], "counter_keys": ["commit:made", "commit:settle:lost", "raid_wars"]}
        self.assertTrue(path_steps.faithful(line, self.NOTES))
        # With no list of notes to go by, none passes.
        self.assertFalse(path_steps.faithful(line, frozenset()))

    def test_a_rules_tally_or_any_other_field_may_not(self):
        counters = {"baseline": True, "refused": 0, "differs_from_recorded": ["players[*].counters"], "counter_keys": ["commit:made", "diplomatic_favor"]}
        self.assertFalse(path_steps.faithful(counters, self.NOTES))
        self.assertFalse(path_steps.faithful({"baseline": True, "refused": 0, "differs_from_recorded": ["units[*].hp"]}, self.NOTES))
        both = {"baseline": True, "refused": 0, "differs_from_recorded": ["players[*].counters", "cities[*].food"], "counter_keys": []}
        self.assertFalse(path_steps.faithful(both, self.NOTES))

    def test_a_refused_action_or_a_failed_replay_is_not_the_same_world(self):
        refused = {"baseline": True, "refused": 2, "first_refusal": [10, "no moves left"], "differs_from_recorded": []}
        self.assertFalse(path_steps.faithful(refused, self.NOTES))
        self.assertFalse(path_steps.faithful({"baseline": True, "failed": True}, self.NOTES))

    def test_which_keys_are_notes_is_read_from_the_engine_s_own_rule(self):
        notes = path_steps.agent_notes()
        # A namespaced head and a plain key the engine lists, and a rules
        # tally it does not.
        self.assertIn("commit", notes)
        self.assertIn("raid_wars", notes)
        self.assertNotIn("diplomatic_favor", notes)
        self.assertGreater(len(notes), 20)
        with tempfile.TemporaryDirectory() as folder:
            self.assertEqual(path_steps.agent_notes(Path(folder)), frozenset())


class SummariseTest(unittest.TestCase):
    def game(self, steps, **more):
        base = {"seed": 1, "turns": 200, "seats": 17, "won": True, "steps": steps, "bytes": steps * 40, "hops": steps // 2,
                "walks": steps // 4, "merged": steps - steps // 2 + steps // 4, "kinds": {"move": steps // 2, "end_turn": steps - steps // 2}}
        base.update(more)
        return base

    def test_a_reading_is_the_mean_over_its_games(self):
        reading = path_steps.summarise([self.game(100, seed=1, turns=200), self.game(300, seed=2, turns=250)])
        self.assertEqual((reading["games"], reading["seeds"]), (2, [1, 2]))
        self.assertEqual((reading["steps"], reading["turns"], reading["bytes"]), (200, 225.0, 8000))
        self.assertEqual(reading["kinds"], {"move": 100, "end_turn": 100})

    def test_instructions_are_kept_only_when_every_game_has_them(self):
        both = path_steps.summarise([self.game(100, instructions=10, replay_instructions=2), self.game(100, instructions=30, replay_instructions=4)])
        self.assertEqual((both["instructions"], both["replay_instructions"]), (20, 3))
        one = path_steps.summarise([self.game(100, instructions=10), self.game(100)])
        self.assertNotIn("instructions", one)

    def test_one_replay_that_differs_marks_the_reading(self):
        reading = path_steps.summarise([self.game(100, replay_identical=True), self.game(100, replay_identical=False)])
        self.assertIs(reading["replay_identical"], False)

    def test_no_games_is_an_error(self):
        with self.assertRaises(ValueError):
            path_steps.summarise([])


class LedgerTest(unittest.TestCase):
    def test_a_reading_is_appended_newest_last(self):
        with tempfile.TemporaryDirectory() as folder:
            ledger = Path(folder) / "path_steps.json"
            path_steps.record({"taken": "2026-11-24", "steps": 1}, ledger)
            path_steps.record({"taken": "2026-11-25", "steps": 2}, ledger)
            book = json.loads(ledger.read_text(encoding="utf-8"))
            self.assertEqual([reading["steps"] for reading in book["readings"]], [1, 2])
            self.assertIn("docs/PATH_STEPS.md", book["about"])

    def test_a_recorded_game_is_read_from_its_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            (directory / "meta.json").write_text(json.dumps({"seed": 7, "end_turn": 3, "seats": 2, "winner": 0}))
            (directory / "actions_all.json").write_text(json.dumps([[0, MOVE(1, [1, 0])], [0, END], [1, END]]))
            game = path_steps.game_of(directory)
            self.assertEqual((game["seed"], game["turns"], game["steps"], game["won"]), (7, 3, 3, True))

    def test_the_seeds_are_the_games_per_minute_benchmarks(self):
        self.assertEqual(path_steps.seeds(3), [20261122000000, 20261122100000, 20261122200000])

    def test_the_committed_ledger_holds_what_the_page_reads(self):
        if not path_steps.LEDGER.exists():
            self.skipTest("no reading has been recorded in this tree")
        book = json.loads(path_steps.LEDGER.read_text(encoding="utf-8"))
        self.assertTrue(book["readings"])
        for reading in book["readings"]:
            for key in ("taken", "build", "games", "turns", "steps", "bytes", "hops", "walks", "merged", "kinds"):
                self.assertIn(key, reading)
            self.assertEqual(reading["merged"], reading["steps"] - reading["hops"] + reading["walks"])
            self.assertLessEqual(abs(sum(reading["kinds"].values()) - reading["steps"]), len(reading["kinds"]))
            self.assertIsNot(reading.get("replay_identical"), False)
        self.assertEqual([r["taken"] for r in book["readings"]], sorted(r["taken"] for r in book["readings"]))


if __name__ == "__main__":
    unittest.main()
