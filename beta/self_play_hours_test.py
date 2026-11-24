#!/usr/bin/env python3
"""The self-play benchmark hours as the CIVVIS Performance page takes them."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import self_play_hours

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent

sys.path.insert(0, str(ROOT / "tools"))
import self_play_hour  # noqa: E402


def line(taken: str, games: int = 40, **fields) -> dict:
    """One line of the ledger, as `tools/self_play_hour.py` writes it."""
    row = {
        "taken": taken, "minutes": 60.0, "games": games, "games_an_hour": float(games), "p": [120.0, 600.0, 2400.0],
        "cpu_a_turn": 0.2232, "cpu_a_game": 40.5, "turns": 194, "workers": 14, "running": 4.0, "cores": 3.9,
        "host_busy": 41.0, "load": 12.3, "civ6": 1.0, "battery": 0.0, "reserved": False,
        "build": "6126321f3", "batch": "20261123T202653Z-b999", "host": "mbp-m5-max-128", "machine": "Apple M5 Max", "machine_cores": 18,
    }
    row.update(fields)
    return row


class TheHours(unittest.TestCase):
    def test_it_reads_the_file_the_tool_writes(self):
        self.assertEqual(self_play_hours.HOURS_REF, "refs/remotes/origin/ledger:" + self_play_hour.LEDGER_PATH)

    def test_the_line_the_tool_writes_is_an_hour_the_page_takes_whole(self):
        # Built by the tool's own code, so neither side can rename a figure alone.
        start = {"at": 1795546800.0, "batch": "20261123T202653Z-b999", "offset": 0, "commit": "6126321f33c9c391c2ae2e9668f9cdbcd607c061"}
        games = [{"secs": 60.0 + 10 * n, "cpu_secs": 40.0 + n, "turn": 180 + 2 * n, "victory": "science"} for n in range(10)]
        samples = [{"civ6": True, "load": 12.0, "workers": 14, "running": 4, "cores": 3.8, "host_busy": 40.0, "battery": False}]
        written = json.loads(json.dumps(self_play_hour.reading(start, dict(start, at=start["at"] + 3600), games, samples, host="mbp-m5-max-128")))
        taken = self_play_hours.hour(written)
        self.assertEqual(taken, {key: written[key] for key in taken})
        # Nothing the tool says of an hour is dropped on the way to the page.
        self.assertEqual(set(written) - set(taken), set())
        for name in ("games_an_hour", "p", "cpu_a_turn", "cpu_a_game", "turns", "workers", "running", "cores", "host_busy", "load", "civ6", "battery"):
            self.assertIsNotNone(taken[name], name)

    def test_a_trial_run_is_not_an_hour(self):
        self.assertIsNone(self_play_hours.hour(line("2026-11-24T15:50:00Z", minutes=3.0)))
        self.assertIsNotNone(self_play_hours.hour(line("2026-11-24T15:50:00Z", minutes=30)))
        # An hour stretched by the machine sleeping is still one, at its own rate.
        slept = self_play_hours.hour(line("2026-11-24T20:00:00Z", games=30, minutes=120.0, games_an_hour=None))
        self.assertEqual((slept["minutes"], slept["games_an_hour"]), (120.0, 15.0))

    def test_a_line_that_is_not_an_hour_is_left_out_and_an_odd_figure_costs_only_itself(self):
        for odd in (
            {"taken": "yesterday", "minutes": 60, "games": 4}, {"taken": "2026-11-24T20:00:00Z", "games": 4},
            {"taken": "2026-11-24T20:00:00Z", "minutes": 60}, line("2026-11-24T20:00:00Z", games=-1),
            line("2026-11-24T20:00:00Z", games=True), line("2026-11-24T20:00:00Z", games=4.5), line("2026-11-24T20:00:00Z", minutes=float("nan")),
        ):
            self.assertIsNone(self_play_hours.hour(odd), odd)
        read = self_play_hours.hour(line(
            "2026-11-24T20:00:00Z", games=0, games_an_hour=0.0, p=None, cpu_a_turn=None, cpu_a_game=0, turns="194", running=-2,
            load=float("inf"), civ6=True, build=7, reserved="yes",
        ))
        # An hour in which nothing finished is an hour: that is a reading too.
        self.assertEqual((read["games"], read["games_an_hour"]), (0, 0.0))
        for name in ("p", "cpu_a_turn", "cpu_a_game", "turns", "running", "load", "civ6", "build"):
            self.assertIsNone(read[name], name)
        self.assertIs(read["reserved"], False)
        self.assertEqual((read["workers"], read["host"]), (14, "mbp-m5-max-128"))
        for p in ([600.0, 120.0, 2400.0], [120.0, 600.0], [0, 600.0, 2400.0], "120,600,2400"):
            self.assertIsNone(self_play_hours.hour(line("2026-11-24T20:00:00Z", p=p))["p"], p)
        self.assertIs(self_play_hours.hour(line("2026-11-24T20:00:00Z", reserved=True))["reserved"], True)


class TheRecord(unittest.TestCase):
    def test_it_is_the_hours_in_order_and_the_last_day_of_them(self):
        rows = [
            line("2026-11-24T20:00:00Z", 80, cpu_a_turn=0.21, running=5.0),
            line("2026-11-23T14:00:00Z", 10),  # more than a day before the newest
            line("2026-11-24T08:00:00Z", 20, cpu_a_turn=0.23, running=1.0),
            line("2026-11-24T14:00:00Z", 0, games_an_hour=0.0, cpu_a_turn=None, p=None, running=0.0),
            line("2026-11-23T20:00:00Z", 60, cpu_a_turn=0.22, running=3.0),  # exactly a day before: outside
            line("2026-11-24T15:50:00Z", 2, minutes=3.0),  # a trial
            "not a line",
        ]
        record = self_play_hours.record(rows)
        self.assertEqual([read["taken"] for read in record["hours"]], [
            "2026-11-23T14:00:00Z", "2026-11-23T20:00:00Z", "2026-11-24T08:00:00Z", "2026-11-24T14:00:00Z", "2026-11-24T20:00:00Z",
        ])
        self.assertEqual(record["day"], {
            "hours": 3, "from": "2026-11-24T08:00:00Z", "to": "2026-11-24T20:00:00Z", "games": 100,
            "games_an_hour": 33.3, "best": 80.0, "cpu_a_turn": 0.22, "running": 2.0,
        })
        self.assertEqual((record["first"], record["min_minutes"]), ("2026-11-23", 30))
        self.assertEqual(json.loads(json.dumps(record)), record)

    def test_an_hour_stands_once_per_machine_and_only_the_newest_are_kept(self):
        twice = [line("2026-11-24T20:00:00Z", 80), line("2026-11-24T20:00:00Z", 999), line("2026-11-24T20:00:00Z", 300, host="martbot-9985")]
        self.assertEqual([(read["host"], read["games"]) for read in self_play_hours.record(twice)["hours"]], [("martbot-9985", 300), ("mbp-m5-max-128", 80)])
        many = [line(f"2026-{month:02d}-{day:02d}T{hour:02d}:00:00Z") for month in (9, 10, 11) for day in range(1, 29) for hour in (2, 8, 14, 20, 23)]
        kept = self_play_hours.record(many)["hours"]
        self.assertEqual((len(kept), kept[-1]["taken"]), (self_play_hours.KEEP, "2026-11-28T23:00:00Z"))

    def test_without_an_hour_there_is_no_record(self):
        self.assertIsNone(self_play_hours.record([]))
        self.assertIsNone(self_play_hours.record([line("2026-11-24T15:50:00Z", minutes=3.0)]))

    def test_it_is_read_from_the_ledger_branch_of_the_clone(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            self.assertIsNone(self_play_hours.self_play(root))
            git = lambda *args, **more: subprocess.run(["git", "-C", str(root), *args], check=True, capture_output=True, **more).stdout.decode().strip()
            git("init", "-q")
            self.assertIsNone(self_play_hours.self_play(root))
            text = self_play_hour.written([line("2026-11-24T20:00:00Z", 80), line("2026-11-24T14:00:00Z", 20)]) + "half a li"
            blob = git("hash-object", "-w", "--stdin", input=text.encode())
            inner = git("mktree", input=f"100644 blob {blob}\tself_play_hours.jsonl\n".encode())
            tree = git("mktree", input=f"040000 tree {inner}\tbenchmarks\n".encode())
            commit = git("-c", "user.name=test", "-c", "user.email=test@example.invalid", "commit-tree", tree, "-m", "ledger")
            git("update-ref", "refs/remotes/origin/ledger", commit)
            record = self_play_hours.self_play(root)
            self.assertEqual([read["games"] for read in record["hours"]], [20, 80])
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                self.assertEqual(self_play_hours.main(["self_play_hours.py", str(root)]), 0)
            self.assertEqual(json.loads(printed.getvalue()), record)
        with mock.patch.object(self_play_hours.subprocess, "run", side_effect=OSError("no git")):
            self.assertIsNone(self_play_hours.self_play(ROOT))


if __name__ == "__main__":
    unittest.main()
