"""The self-play benchmark hour: what the tournament finishes in an hour.

Operator 2026-11-24: "we should be benchmarking civvis self-play tournament
games as well. maybe every 6 hrs or so we can do 1 hr of civvis self-play
games on this machine". `tools/self_play_hour.py` watches the tournament the
continuous batch scheduler plays, reads the games it finished between two
lengths of its rows file, and adds the reading to the `ledger` branch. These
hold which games an hour counts, what the reading says of them, and that a
reading reaches the branch beside the verification games' results without
disturbing them.
"""

from __future__ import annotations

import io
import json
import os
import plistlib
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import self_play_hour  # noqa: E402

BATCH = "20261123T202653Z-b999"
NEXT = "20261124T181500Z-c0de"
COMMIT = "6126321f33c9c391c2ae2e9668f9cdbcd607c061"

#: An identity for the commits the tests make, so they do not depend on the
#: machine's own.
IDENTITY = {
    "GIT_AUTHOR_NAME": "test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "test", "GIT_COMMITTER_EMAIL": "test@example.invalid",
}


def seats(seed: int, secs: float, cpu: float | None = 40.0, turn: int = 200, victory: str = "science") -> str:
    """A finished game as the gene screen writes it: a row per seat, each
    with the game's whole wall time, written in one piece."""
    rows = []
    for seat in range(6):
        row = {"kind": "game", "seed": seed, "seat": seat, "secs": secs, "turn": turn, "victory": victory}
        if cpu is not None:
            row["cpu_secs"] = cpu
        rows.append(json.dumps(row))
    return "\n".join(rows) + "\n"


class Tournament:
    """A scheduler's state directory, as much of it as an hour reads."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.play(BATCH)

    def rows(self, batch: str) -> Path:
        return self.root / "batches" / batch / self_play_hour.ROWS

    def play(self, batch: str, **governor) -> None:
        """The scheduler says it is playing `batch`."""
        self.rows(batch).parent.mkdir(parents=True, exist_ok=True)
        if not self.rows(batch).exists():
            self.rows(batch).write_text(json.dumps({"kind": "header", "players": 6}) + "\n", encoding="utf-8")
        seen = {"workers": 14, "workers_running": 4, "games_cores": 3.9, "host_busy_percent": 41.0, "on_battery": False, **governor}
        (self.root / "status.json").write_text(json.dumps({"batch": batch, "source_commit": COMMIT, "governor": seen}), encoding="utf-8")

    def finish(self, batch: str, text: str) -> None:
        with self.rows(batch).open("a", encoding="utf-8") as out:
            out.write(text)


class TheGamesOfAnHour(unittest.TestCase):
    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.state = Path(self.scratch.name)
        self.tournament = Tournament(self.state)

    def test_they_are_the_rows_written_between_two_marks_a_game_per_seed(self):
        self.tournament.finish(BATCH, seats(1, 90.0))  # before the hour
        start = self_play_hour.mark(self.state, now=1000.0)
        self.tournament.finish(BATCH, seats(2, 60.0, cpu=30.0, turn=150, victory="culture") + seats(3, 600.0, cpu=45.0))
        end = self_play_hour.mark(self.state, now=4600.0)
        self.tournament.finish(BATCH, seats(4, 75.0))  # after it
        games = self_play_hour.finished(self.state, start, end)
        self.assertEqual(games, [
            {"secs": 60.0, "cpu_secs": 30.0, "turn": 150, "victory": "culture"},
            {"secs": 600.0, "cpu_secs": 45.0, "turn": 200, "victory": "science"},
        ])
        self.assertEqual((start["batch"], start["commit"], end["at"] - start["at"]), (BATCH, COMMIT, 3600.0))

    def test_an_hour_in_which_nothing_finished_has_no_games(self):
        start = self_play_hour.mark(self.state, now=0.0)
        self.assertEqual(self_play_hour.finished(self.state, start, self_play_hour.mark(self.state, now=3600.0)), [])

    def test_a_batch_cut_during_the_hour_is_read_across_the_cut(self):
        start = self_play_hour.mark(self.state, now=0.0)
        self.tournament.finish(BATCH, seats(2, 60.0))
        # The batch is cut; one is played whole in between; a third begins.
        between = "20261124T170000Z-aaaa"
        self.tournament.play(between)
        self.tournament.finish(between, seats(2, 70.0))  # the same seed in another batch is another game
        self.tournament.play(NEXT)
        self.tournament.finish(NEXT, seats(9, 80.0))
        end = self_play_hour.mark(self.state, now=3600.0)
        self.tournament.finish(NEXT, seats(10, 85.0))
        self.assertEqual([game["secs"] for game in self_play_hour.finished(self.state, start, end)], [60.0, 70.0, 80.0])
        self.assertEqual(end["batch"], NEXT)

    def test_half_a_line_is_never_read_as_a_game(self):
        self.tournament.finish(BATCH, seats(1, 90.0))
        # The mark lands while a game's rows are being written...
        whole, half = seats(2, 60.0), seats(3, 70.0)
        self.tournament.finish(BATCH, whole[:40])
        start = self_play_hour.mark(self.state, now=0.0)
        self.tournament.finish(BATCH, whole[40:] + half[:50])
        end = self_play_hour.mark(self.state, now=3600.0)
        # ...so the game it cut is counted from its next whole row, and the
        # one still being written at the end is left for the next hour.
        self.assertEqual([game["secs"] for game in self_play_hour.finished(self.state, start, end)], [60.0])
        later = self_play_hour.mark(self.state, now=3600.0)
        self.tournament.finish(BATCH, half[50:])
        again = self_play_hour.finished(self.state, later, self_play_hour.mark(self.state, now=7200.0))
        self.assertEqual([game["secs"] for game in again], [70.0])

    def test_a_row_that_is_not_a_timed_game_is_left_out(self):
        start = self_play_hour.mark(self.state, now=0.0)
        odd = [
            {"kind": "header"}, {"kind": "game", "seed": 5, "secs": 0, "turn": 100}, {"kind": "game", "seed": 6, "secs": 50.0, "turn": 0},
            {"kind": "game", "seed": 7, "secs": True, "turn": 100}, {"kind": "game", "seed": 8, "secs": float("nan"), "turn": 100}, [1, 2],
        ]
        self.tournament.finish(BATCH, "not json\n" + "".join(json.dumps(row) + "\n" for row in odd) + seats(9, 80.0, cpu=None))
        games = self_play_hour.finished(self.state, start, self_play_hour.mark(self.state, now=3600.0))
        # A build from before rows carried the processor's clock still has its game.
        self.assertEqual(games, [{"secs": 80.0, "cpu_secs": None, "turn": 200, "victory": "science"}])

    def test_a_directory_that_is_no_tournament_s_is_refused(self):
        with tempfile.TemporaryDirectory() as empty:
            with self.assertRaises(RuntimeError):
                self_play_hour.mark(Path(empty))
        (self.state / "status.json").write_text("{half", encoding="utf-8")
        self.assertEqual(self_play_hour.status(self.state), {})


class TheReading(unittest.TestCase):
    def test_it_says_the_games_the_times_and_the_conditions(self):
        games = [{"secs": float(60 + 10 * n), "cpu_secs": 40.0 + n, "turn": 180 + 2 * n, "victory": "science"} for n in range(10)]
        samples = [
            {"civ6": True, "load": 12.0, "workers": 14, "running": 4, "cores": 3.8, "host_busy": 40.0, "battery": False},
            {"civ6": True, "load": 16.0, "workers": 14, "running": 2, "cores": 2.0, "host_busy": 60.0, "battery": False},
            {"civ6": False, "load": 8.0},  # the scheduler's status could not be read
        ]
        start = {"at": 1795546800.0, "batch": BATCH, "offset": 0, "commit": COMMIT}
        end = {"at": 1795546800.0 + 3600, "batch": NEXT, "offset": 0, "commit": "abcdef0123456789"}
        with mock.patch.object(self_play_hour, "machine", return_value={"machine": "Apple M5 Max", "machine_cores": 18}):
            row = self_play_hour.reading(start, end, games, samples, host="mbp-m5-max-128")
        self.assertEqual(row, {
            "taken": self_play_hour.utc(1795546800.0), "minutes": 60.0, "games": 10, "games_an_hour": 10.0,
            # Nearest rank on the ten games: the 1st, the 5th and the 9th.
            "p": [60.0, 100.0, 140.0],
            "cpu_a_turn": round(44.0 / 188, 4), "cpu_a_game": 44.0, "turns": 188,
            "workers": 14, "running": 3.0, "cores": 2.9, "host_busy": 50.0, "load": 12.0, "civ6": 0.67, "battery": 0.0,
            "reserved": False, "build": "abcdef012", "batch": NEXT, "host": "mbp-m5-max-128", "machine": "Apple M5 Max", "machine_cores": 18,
        })
        self.assertRegex(row["taken"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        # It is one line of JSON and comes back the same.
        self.assertEqual(json.loads(json.dumps(row)), row)

    def test_a_shorter_or_longer_hour_is_scaled_to_an_hour(self):
        start = {"at": 0.0, "batch": BATCH, "offset": 0, "commit": COMMIT}
        games = [{"secs": 50.0, "cpu_secs": None, "turn": 100, "victory": ""}] * 30
        half = self_play_hour.reading(start, dict(start, at=1800.0), games, [])
        slept = self_play_hour.reading(start, dict(start, at=7200.0), games, [])
        self.assertEqual((half["minutes"], half["games_an_hour"]), (30.0, 60.0))
        self.assertEqual((slept["minutes"], slept["games_an_hour"]), (120.0, 15.0))

    def test_an_hour_with_no_game_or_no_sample_says_nothing_it_does_not_know(self):
        start = {"at": 0.0, "batch": BATCH, "offset": 0, "commit": ""}
        row = self_play_hour.reading(start, dict(start, at=3600.0), [], [])
        self.assertEqual((row["games"], row["games_an_hour"]), (0, 0.0))
        for name in ("p", "cpu_a_turn", "cpu_a_game", "turns", "workers", "running", "cores", "host_busy", "load", "civ6", "battery"):
            self.assertIsNone(row[name], name)

    def test_the_hour_is_watched_by_the_wall_clock(self):
        with tempfile.TemporaryDirectory() as scratch:
            state = Path(scratch)
            tournament = Tournament(state)
            now = [1000.0]

            def sleep(seconds: float) -> None:
                # A game finishes while the watch sleeps, and once the machine
                # sleeps too: twenty minutes pass in one step.
                tournament.finish(BATCH, seats(int(now[0]), 55.0))
                now[0] += 1200.0 if now[0] == 1060.0 else seconds

            with mock.patch.object(self_play_hour, "civ6_running", return_value=True):
                start, end, samples = self_play_hour.watch(state, 10.0, 30.0, clock=lambda: now[0], sleep=sleep)
            self.assertEqual(start["at"], 1000.0)
            # Asked for ten minutes; it ended at the first look past them.
            self.assertEqual(end["at"], 2260.0)
            self.assertEqual(len(samples), 4)
            self.assertEqual(samples[0], {"civ6": True, "load": samples[0]["load"], "workers": 14, "running": 4, "cores": 3.9, "host_busy": 41.0, "battery": False})
            self.assertEqual(len(self_play_hour.finished(state, start, end)), 3)


def git(repo: Path, *args: str, stdin: bytes | None = None) -> str:
    done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, check=True, input=stdin, env={**os.environ, **IDENTITY})
    return done.stdout.decode().strip()


class TheLedger(unittest.TestCase):
    """The readings reach `benchmarks/self_play_hours.jsonl` on the `ledger`
    branch of `origin`, beside the verification games' results."""

    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        root = Path(self.scratch.name)
        self.origin, self.work = root / "origin.git", root / "work"
        subprocess.run(["git", "init", "-q", "--bare", str(self.origin)], check=True)
        subprocess.run(["git", "init", "-q", str(self.work)], check=True)
        git(self.work, "remote", "add", "origin", str(self.origin))
        # The branch as the verification games keep it: their results, a run.
        self.results = '{"tag":"civvis-1","outcome":"win"}\n'
        self.tip = self.commit(None, {"results.jsonl": self.results, "runs/civvis-1/summary.json": "{}"})
        git(self.work, "push", "-q", "origin", f"{self.tip}:refs/heads/ledger")

    def commit(self, parent: str | None, files: dict[str, str]) -> str:
        with tempfile.TemporaryDirectory() as scratch:
            return civ6_ladder._ledger_commit(self.work, parent, [(path, text.encode()) for path, text in files.items()], "ledger", Path(scratch), IDENTITY)

    def held(self, path: str = self_play_hour.LEDGER_PATH) -> str:
        return subprocess.run(["git", "-C", str(self.origin), "show", f"ledger:{path}"], capture_output=True, check=True).stdout.decode()

    def row(self, taken: str, games: int = 40, **fields) -> dict:
        return {"taken": taken, "host": "mbp-m5-max-128", "games": games, "minutes": 60.0, **fields}

    def test_a_reading_is_added_and_nothing_else_on_the_branch_moves(self):
        first = self.row("2026-11-24T20:00:00Z")
        self.assertEqual(self_play_hour.publish([first], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(self_play_hour.parse(self.held().splitlines()), [first])
        self.assertEqual(self.held("results.jsonl"), self.results)
        self.assertEqual(self.held("runs/civvis-1/summary.json"), "{}")
        # It is a fast forward of the tip it was built on.
        self.assertEqual(git(self.origin, "rev-parse", "ledger~1"), self.tip)
        self.assertEqual(git(self.work, "rev-parse", "refs/remotes/origin/ledger"), git(self.origin, "rev-parse", "ledger"))

    def test_only_what_the_branch_lacks_is_sent_oldest_first(self):
        early, late = self.row("2026-11-24T08:00:00Z", 12), self.row("2026-11-24T20:00:00Z", 55)
        self.assertEqual(self_play_hour.publish([late], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(self_play_hour.publish([late], repo=self.work, env=IDENTITY), "already")
        # The whole journal is offered every time; the missing hour goes in
        # at its place, and the branch's own line for an hour is not rewritten.
        changed = dict(late, games=999)
        self.assertEqual(self_play_hour.publish([changed, early], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(self.held(), self_play_hour.written([early, late]))
        self.assertEqual([row["games"] for row in self_play_hour.on_ledger(self.work, git(self.origin, "rev-parse", "ledger"))], [12, 55])
        # The same hour offered twice in one journal is one line.
        self.assertEqual(self_play_hour.publish([early, dict(early), late], repo=self.work, env=IDENTITY), "already")
        twice = self.row("2026-11-25T02:00:00Z", 7)
        self.assertEqual(self_play_hour.publish([twice, dict(twice)], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(self.held(), self_play_hour.written([early, late, twice]))
        self.assertEqual(self_play_hour.publish([twice], repo=self.work, env=IDENTITY), "already")
        # Another machine's hour at the same moment is another reading.
        other = dict(late, host="martbot-9985", games=300)
        self.assertEqual(self_play_hour.publish([other], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(len(self_play_hour.parse(self.held().splitlines())), 4)

    def test_a_game_that_reaches_the_branch_first_does_not_cost_the_reading(self):
        # A verification game finishes between our read of the tip and our
        # push: the push is refused, the tip read again, the commit rebuilt.
        real_tip = civ6_ladder.ledger_tip
        calls = []

        def tip(repo, remote="origin", branch="ledger", env=None):
            found = real_tip(repo, remote, branch, env=env)
            if not calls:
                moved = self.commit(found, {"results.jsonl": self.results + '{"tag":"civvis-2","outcome":"defeat"}\n'})
                git(self.work, "push", "-q", "origin", f"{moved}:refs/heads/ledger")
            calls.append(found)
            return found

        reading = self.row("2026-11-24T20:00:00Z")
        with mock.patch.object(civ6_ladder, "ledger_tip", tip):
            self.assertEqual(self_play_hour.publish([reading], repo=self.work, env=IDENTITY), "published")
        self.assertEqual(len(calls), 2)
        self.assertIn("civvis-2", self.held("results.jsonl"))
        self.assertEqual(self_play_hour.parse(self.held().splitlines()), [reading])

    def test_a_remote_without_the_branch_is_not_given_one(self):
        git(self.origin, "update-ref", "-d", "refs/heads/ledger")
        with self.assertRaises(RuntimeError):
            self_play_hour.publish([self.row("2026-11-24T20:00:00Z")], repo=self.work, env=IDENTITY)
        self.assertEqual(git(self.origin, "for-each-ref", "refs/heads"), "")

    def test_an_hour_is_journalled_first_and_a_failed_publish_leaves_it_for_the_next(self):
        state = Path(self.scratch.name) / "state"
        state.mkdir()
        Tournament(state)
        args = ["run", "--state-dir", str(state), "--minutes", "0.0001", "--sample-seconds", "0.001", "--record", "--repo", str(self.work), "--host", "mbp-m5-max-128"]
        hours = iter(["2026-11-24T20:00:00Z", "2026-11-25T02:00:00Z"])
        with mock.patch.dict(os.environ, IDENTITY), mock.patch.object(self_play_hour, "civ6_running", return_value=False), \
                mock.patch.object(self_play_hour, "utc", side_effect=lambda moment: next(hours)):
            with mock.patch.object(self_play_hour, "publish", side_effect=RuntimeError("no network")), redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()) as said:
                self.assertEqual(self_play_hour.main(args), 1)
            self.assertIn("not published", said.getvalue())
            kept = self_play_hour.journal(state)
            self.assertEqual(len(kept), 1)
            self.assertEqual((kept[0]["games"], kept[0]["host"], kept[0]["batch"], kept[0]["reserved"]), (0, "mbp-m5-max-128", BATCH, False))
            # The next hour's publish sends both.
            with redirect_stdout(io.StringIO()) as printed:
                self.assertEqual(self_play_hour.main(args), 0)
            self.assertIn("ledger: published", printed.getvalue())
            with redirect_stdout(io.StringIO()) as shown:
                self.assertEqual(self_play_hour.main(["show", "--repo", str(self.work)]), 0)
        self.assertEqual(len(self_play_hour.parse(self.held().splitlines())), 2)
        self.assertEqual(self_play_hour.parse(self.held().splitlines()), self_play_hour.journal(state))
        self.assertEqual(len(shown.getvalue().splitlines()), 3)


class TheSchedule(unittest.TestCase):
    def test_the_job_runs_an_hour_with_its_reading_published_six_hours_apart(self):
        job = self_play_hour.agent(Path("/state"), self_play_hour.HOURS, 60.0, Path("/repo"))
        self.assertEqual(job["Label"], "com.civvis.self-play-hour")
        self.assertEqual(job["ProgramArguments"], [
            "/usr/bin/python3", "/repo/tools/self_play_hour.py", "run", "--state-dir", "/state", "--minutes", "60", "--record", "--repo", "/repo",
        ])
        hours = [slot["Hour"] for slot in job["StartCalendarInterval"]]
        self.assertEqual(hours, [2, 8, 14, 20])
        self.assertEqual({(b - a) % 24 for a, b in zip(hours, hours[1:] + hours[:1])}, {6})
        # The command the job runs is one this tool takes.
        parser_args = job["ProgramArguments"][2:]
        with mock.patch.object(self_play_hour, "run", return_value=0) as ran:
            self.assertEqual(self_play_hour.main(parser_args), 0)
        self.assertTrue(ran.call_args[0][0].record)
        self.assertEqual(ran.call_args[0][0].minutes, 60.0)
        # It watches; it is not the work. And it is a plist launchd can read.
        self.assertEqual(job["ProcessType"], "Background")
        self.assertEqual(plistlib.loads(plistlib.dumps(job)), job)

    def test_hours_that_are_not_hours_are_refused(self):
        with tempfile.TemporaryDirectory() as scratch, redirect_stderr(io.StringIO()):
            Tournament(Path(scratch))
            for hours in ("24", "-1", ""):
                self.assertEqual(self_play_hour.main(["install", "--state-dir", scratch, "--hours", hours]), 2)


if __name__ == "__main__":
    unittest.main()
