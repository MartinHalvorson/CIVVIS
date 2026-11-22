"""The games-per-minute benchmark counts finished games in the right minute and
puts back whatever it quietened."""

import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import games_per_minute as gpm  # noqa: E402

#: Stands in for `gene_screen`: a header, then one six-seat game every 0.2 s.
FAKE_WORKER = """#!/usr/bin/env python3
import json, sys, time
args = sys.argv[1:]
seed = int(args[args.index("--start-seed") + 1])
out = open(args[args.index("--out") + 1], "a")
out.write(json.dumps({"kind": "header", "players": 6, "map": "continents",
                      "build": {"commit": "abc123", "dirty": False}}) + "\\n")
out.flush()
while True:
    time.sleep(0.2)
    for seat in range(6):
        out.write(json.dumps({"seed": seed, "seat": seat, "turn": 100,
                              "victory": "science", "secs": 0.2}) + "\\n")
    out.flush()
    seed += 1
"""


def game(at, turn=100, secs=30.0):
    return {"seed": int(at * 1000), "turn": turn, "victory": "science", "secs": secs, "at": at}


class CountTests(unittest.TestCase):
    def test_workers_never_share_a_seed(self):
        seeds = gpm.worker_seeds(18)
        self.assertEqual(len(set(seeds)), 18)
        self.assertGreaterEqual(min(b - a for a, b in zip(seeds, seeds[1:])), gpm.GAMES_PER_WORKER)

    def test_a_game_on_the_bell_belongs_to_the_minute_it_ends(self):
        self.assertEqual(gpm.count_between([60.0, 60.001, 120.0], 0.0, 60.0), 1)
        self.assertEqual(gpm.count_between([60.0, 60.001, 120.0], 60.0, 120.0), 2)

    def test_the_headline_is_the_mean_of_the_minutes_after_the_first(self):
        finishes = [game(at) for at in (20, 45, 59, 61, 70, 90, 119, 130, 150, 185)]
        result = gpm.summarise(finishes, minutes=2, workers=2)
        self.assertEqual(result["first_minute"], 3)
        self.assertEqual(result["minutes"], [4, 2])
        self.assertEqual(result["games_per_minute"], 3.0)
        self.assertEqual(result["best_minute"], 4)
        self.assertEqual(result["games"], 6)  # the game at 185 s is past the last bell
        self.assertEqual(result["turns_per_second"], 5.0)
        self.assertEqual(result["games_per_minute_per_worker"], 1.5)

    def test_a_minute_with_no_finished_game_is_a_zero_not_an_error(self):
        result = gpm.summarise([], minutes=1, workers=18)
        self.assertEqual(result["minutes"], [0])
        self.assertEqual(result["games_per_minute"], 0.0)
        self.assertIsNone(result["median_game_seconds"])


class RowsTests(unittest.TestCase):
    def test_a_game_is_counted_once_and_only_when_its_row_is_whole(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "rows.jsonl"
            rows = gpm.Rows(path)
            self.assertEqual(rows.finished(), [])  # no file yet
            seat = json.dumps({"seed": 7, "seat": 0, "turn": 180, "victory": "culture", "secs": 31.5})
            with path.open("a") as out:
                out.write(json.dumps({"kind": "header", "players": 6}) + "\n")
                out.write(seat + "\n" + seat[:20])  # the second seat's row is half written
            self.assertEqual(
                rows.finished(), [{"seed": 7, "turn": 180, "victory": "culture", "secs": 31.5}]
            )
            self.assertEqual(rows.header["players"], 6)
            with path.open("a") as out:
                out.write(seat[20:] + "\n" + json.dumps({"seed": 8, "seat": 0, "turn": 90}) + "\n")
            self.assertEqual([found["seed"] for found in rows.finished()], [8])
            self.assertEqual(rows.finished(), [])


class QuiesceTests(unittest.TestCase):
    def state(self, pid):
        return subprocess.run(
            ["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True
        ).stdout.strip()

    def test_a_quiesce_is_journalled_and_released_exactly(self):
        with tempfile.TemporaryDirectory() as folder:
            switch = Path(folder) / "headroom-off"
            journal = Path(folder) / "quiesce.json"
            compiler = subprocess.Popen(["sleep", "30"])
            self.addCleanup(compiler.kill)
            pause = mock.Mock()
            pause.requested.return_value = False
            pause.holds_game.return_value = True
            with mock.patch.object(gpm, "HEADROOM_OFF", switch), \
                    mock.patch.object(gpm, "live_pause", return_value=pause), \
                    mock.patch.object(gpm, "live_game_up", return_value=True), \
                    mock.patch.object(
                        gpm, "batch_work",
                        return_value=[{"pid": compiler.pid, "name": "rustc"}]):
                done = gpm.quiesce(journal, 5.0, lambda text: None)
                self.assertTrue(switch.exists())
                pause.set_requested.assert_called_once_with(True, source="games_per_minute")
                time.sleep(0.1)
                self.assertTrue(self.state(compiler.pid).startswith("T"))
                self.assertEqual(json.loads(journal.read_text()), done)
                # The journal alone is enough to undo it, as `--release` does.
                gpm.release(json.loads(journal.read_text()), lambda text: None)
                gpm.release(done, lambda text: None)  # twice is harmless
            self.assertFalse(switch.exists())
            pause.set_requested.assert_called_with(False)
            time.sleep(0.1)
            self.assertFalse(self.state(compiler.pid).startswith("T"))

    def test_somebody_elses_pause_and_switch_are_left_standing(self):
        with tempfile.TemporaryDirectory() as folder:
            switch = Path(folder) / "headroom-off"
            switch.write_text("the operator's own\n")
            pause = mock.Mock()
            pause.requested.return_value = True
            with mock.patch.object(gpm, "HEADROOM_OFF", switch), \
                    mock.patch.object(gpm, "live_pause", return_value=pause), \
                    mock.patch.object(gpm, "live_game_up", return_value=True), \
                    mock.patch.object(gpm, "batch_work", return_value=[]):
                done = gpm.quiesce(Path(folder) / "quiesce.json", 5.0, lambda text: None)
                gpm.release(done, lambda text: None)
            self.assertTrue(switch.exists())
            pause.set_requested.assert_not_called()

    def test_a_game_that_never_holds_stops_the_run_and_is_released(self):
        with tempfile.TemporaryDirectory() as folder:
            journal = Path(folder) / "quiesce.json"
            pause = mock.Mock()
            pause.requested.return_value = False
            pause.holds_game.return_value = False
            with mock.patch.object(gpm, "HEADROOM_OFF", Path(folder) / "off"), \
                    mock.patch.object(gpm, "live_pause", return_value=pause), \
                    mock.patch.object(gpm, "live_game_up", return_value=True):
                with self.assertRaises(RuntimeError):
                    gpm.quiesce(journal, 0.2, lambda text: None)
                gpm.release(json.loads(journal.read_text()), lambda text: None)
            pause.set_requested.assert_called_with(False)


class RunTests(unittest.TestCase):
    def test_a_whole_run_counts_the_workers_games_and_records_it(self):
        with tempfile.TemporaryDirectory() as folder:
            binary = Path(folder) / "gene_screen"
            binary.write_text(FAKE_WORKER)
            binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
            out = Path(folder) / "run"
            with mock.patch.object(gpm, "MINUTE", 1.0), mock.patch("sys.stdout"):
                code = gpm.main(["--binary", str(binary), "--workers", "3", "--minutes", "2",
                                 "--out", str(out), "--json"])
            self.assertEqual(code, 0)
            result = json.loads((out / "result.json").read_text())
            # Three workers, a game every 0.2 s each: about 15 a "minute".
            self.assertEqual(len(result["minutes"]), 2)
            for count in result["minutes"]:
                self.assertTrue(9 <= count <= 16, result["minutes"])
            self.assertEqual(result["workers"], 3)
            self.assertEqual(result["build"]["commit"], "abc123")
            self.assertEqual(result["setup"]["map"], "continents")
            self.assertFalse(result["conditions"]["quiesced"])
            leftover = subprocess.run(
                ["pgrep", "-f", str(binary)], capture_output=True, text=True).stdout.strip()
            self.assertEqual(leftover, "", "the workers are gone when the run ends")

    def test_recording_appends_newest_last(self):
        with tempfile.TemporaryDirectory() as folder:
            ledger = Path(folder) / "games_per_minute.json"
            gpm.record({"games_per_minute": 20.0}, ledger)
            gpm.record({"games_per_minute": 25.0}, ledger)
            book = json.loads(ledger.read_text())
            self.assertEqual([run["games_per_minute"] for run in book["runs"]], [20.0, 25.0])
            self.assertIn("docs/GAMES_PER_MINUTE.md", book["about"])

    def test_a_probe_is_never_recorded(self):
        with tempfile.NamedTemporaryFile() as binary, \
                mock.patch("sys.stderr") as stderr, self.assertRaises(SystemExit):
            gpm.main(["--binary", binary.name, "--record", "--", "--turns", "30"])
        self.assertIn("probe", "".join(call.args[0] for call in stderr.write.call_args_list))

if __name__ == "__main__":
    unittest.main()
