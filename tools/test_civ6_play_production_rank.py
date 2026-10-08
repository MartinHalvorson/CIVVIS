"""The operator's turn-150 production-rank restart rule (2026-10-08).

"if not top 2 by prod by turn 150 can restart game": `production_rank_reading`
reads the mod's `major_production` map off the agent's `turn` record once, at
the first readable record at or after turn 150, and the host policy file's
CIVVIS_RESTART_BELOW_PRODUCTION_RANK turns it on.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402


def turn_record(turn: int, production: dict | None, us: int | None = 0) -> dict:
    event = {"kind": "turn", "ctx": "agent", "turn": turn}
    if production is not None:
        event["major_production"] = production
    if us is not None:
        event["local_player"] = us
    return event


class ProductionRankReadingTest(unittest.TestCase):
    def test_off_reads_nothing(self):
        event = turn_record(150, {"0": 10, "1": 50, "2": 40, "3": 30})
        self.assertIsNone(civ6_play.production_rank_reading(event, 0))
        self.assertIsNone(civ6_play.production_rank_reading(event, -1))
        self.assertIsNone(civ6_play.production_rank_reading(event, True))

    def test_third_by_production_at_turn_150_fires(self):
        reading = civ6_play.production_rank_reading(
            turn_record(150, {"0": 90, "1": 120, "2": 100.5, "3": 40}), 2)
        self.assertEqual(reading["rule"], "production_rank")
        self.assertEqual(reading["turn"], 150)
        self.assertEqual(reading["rank"], 3)
        self.assertEqual(reading["majors"], 4)
        self.assertEqual(reading["ours"], 90)
        self.assertEqual(reading["others"], {"1": 120, "2": 100.5, "3": 40})
        self.assertTrue(reading["fires"])

    def test_second_by_production_does_not_fire(self):
        reading = civ6_play.production_rank_reading(
            turn_record(152, {"0": 110, "1": 120, "2": 100, "3": 40}), 2)
        self.assertEqual(reading["rank"], 2)
        self.assertFalse(reading["fires"])

    def test_a_tie_does_not_push_us_down(self):
        reading = civ6_play.production_rank_reading(
            turn_record(150, {"0": 100, "1": 120, "2": 100, "3": 100}), 2)
        self.assertEqual(reading["rank"], 2)
        self.assertFalse(reading["fires"])

    def test_our_seat_need_not_be_player_zero(self):
        reading = civ6_play.production_rank_reading(
            turn_record(150, {"0": 300, "1": 20, "2": 200, "3": 10}, us=1), 2)
        self.assertEqual(reading["rank"], 3)
        self.assertEqual(sorted(reading["others"]), ["0", "2", "3"])

    def test_only_agent_turn_records_at_or_after_turn_150(self):
        production = {"0": 1, "1": 100, "2": 100, "3": 100}
        self.assertIsNone(civ6_play.production_rank_reading(
            turn_record(149, production), 2))
        state = dict(turn_record(150, production), kind="state")
        self.assertIsNone(civ6_play.production_rank_reading(state, 2))
        heartbeat = dict(turn_record(150, production), ctx="heartbeat")
        self.assertIsNone(civ6_play.production_rank_reading(heartbeat, 2))

    def test_a_record_without_our_standing_is_unreadable(self):
        for event in (turn_record(150, None),
                      turn_record(150, {"1": 100, "2": 50}),
                      turn_record(150, {"0": 10, "1": 100}, us=None),
                      turn_record(150, {"0": -1, "1": 100}),
                      turn_record(150, "garbage")):
            reading = civ6_play.production_rank_reading(event, 2)
            self.assertEqual(reading, {"rule": "production_rank", "turn": 150,
                                       "unreadable": True}, event)

    def test_unreadable_rivals_are_left_out_not_counted_as_zero(self):
        reading = civ6_play.production_rank_reading(
            turn_record(150, {"0": 50, "1": None, "2": 80, "3": "x"}), 2)
        self.assertEqual(reading["others"], {"2": 80})
        self.assertEqual(reading["rank"], 2)

    def test_the_live_loop_reads_once_and_retires_on_a_firing_reading(self):
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        branch = source.split('if production_rank_limit > 0 and "production_rank" not in state:', 1)
        self.assertEqual(len(branch), 2, "the reading is guarded to happen once")
        body = branch[1][:2000]
        self.assertIn('state["production_rank"] = reading', body)
        self.assertIn("return abandon_with_retire(reading)", body)
        self.assertIn('verdict["rule"]', source.split("def abandon_with_retire", 1)[1][:3000])


class ProductionRankPolicyTest(unittest.TestCase):
    def policy(self, text: str) -> Path:
        handle = tempfile.NamedTemporaryFile("w", suffix=".policy", delete=False)
        handle.write(text)
        handle.close()
        self.addCleanup(Path(handle.name).unlink)
        return Path(handle.name)

    def test_the_key_is_read_as_data(self):
        path = self.policy(
            "# comment\n"
            "CIVVIS_DIFFICULTY=DIFFICULTY_EMPEROR\n"
            "CIVVIS_RESTART_BELOW_PRODUCTION_RANK = 1  # first try\n"
            "CIVVIS_RESTART_BELOW_PRODUCTION_RANK=2\n")
        self.assertEqual(civ6_play.production_rank_policy(path), 2)

    def test_absent_zero_negative_or_invalid_is_off(self):
        for text in ("CIVVIS_DIFFICULTY=DIFFICULTY_EMPEROR\n",
                     "CIVVIS_RESTART_BELOW_PRODUCTION_RANK=0\n",
                     "CIVVIS_RESTART_BELOW_PRODUCTION_RANK=-3\n",
                     "CIVVIS_RESTART_BELOW_PRODUCTION_RANK=two\n",
                     "CIVVIS_RESTART_BELOW_PRODUCTION_RANK=\n",
                     "#CIVVIS_RESTART_BELOW_PRODUCTION_RANK=2\n"):
            with mock.patch("builtins.print"):
                self.assertEqual(civ6_play.production_rank_policy(self.policy(text)), 0, text)

    def test_a_missing_file_is_off(self):
        self.assertEqual(civ6_play.production_rank_policy(Path("/nonexistent/policy")), 0)

    def test_the_policy_path_follows_the_supervisor_override(self):
        with mock.patch.dict("os.environ", {"CIVVIS_VERIFICATION_POLICY": "/tmp/x.policy"}):
            self.assertEqual(civ6_play.verification_policy_path(), Path("/tmp/x.policy"))
        with mock.patch.dict("os.environ", {}, clear=True):
            with mock.patch.object(Path, "home", return_value=Path("/home/seat")):
                self.assertEqual(civ6_play.verification_policy_path(),
                                 Path("/home/seat/.civvis-verification-policy"))

    def test_the_flag_defaults_to_the_policy(self):
        parser_source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        self.assertIn('"--restart-below-production-rank", type=int, default=None',
                      parser_source)
        self.assertIn("production_rank_limit = production_rank_policy()", parser_source)


if __name__ == "__main__":
    unittest.main()


class PlayClosuresResolveTest(unittest.TestCase):
    """`_play`'s turn-record predicate is a closure: a name it reads must be
    bound in `_play` or the module. The first live build read the limit from
    `play`'s frame and died with a NameError at the first turn record (live
    G362, civvis-20261008T073740Z, turn 8)."""

    def test_play_closures_resolve_every_name(self):
        import builtins
        import symtable
        from pathlib import Path

        import civ6_play

        source = Path(civ6_play.__file__).read_text()
        known = set(dir(civ6_play)) | set(dir(builtins))
        unresolved = []

        def walk(table, path):
            for symbol in table.get_symbols():
                if (symbol.is_referenced() and symbol.is_global()
                        and not symbol.is_declared_global()
                        and symbol.get_name() not in known):
                    unresolved.append((path, symbol.get_name()))
            for child in table.get_children():
                walk(child, f"{path}.{child.get_name()}")

        walk(symtable.symtable(source, "civ6_play.py", "exec"), "civ6_play")
        play_scoped = [item for item in unresolved if item[0].startswith("civ6_play._play")]
        self.assertEqual(play_scoped, [])
