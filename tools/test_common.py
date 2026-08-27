#!/usr/bin/env python3
"""Regression checks for `tools/_common.py`'s shared helpers."""

from __future__ import annotations

import datetime
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import _common  # noqa: E402


class RepoRootTest(unittest.TestCase):
    def test_repo_root_is_the_parent_of_tools(self) -> None:
        root = _common.repo_root()
        self.assertTrue((root / "tools" / "_common.py").is_file())
        self.assertEqual(root, Path(__file__).resolve().parent.parent)


class RunTest(unittest.TestCase):
    def test_run_captures_stdout_in_text_mode(self) -> None:
        result = _common.run([sys.executable, "-c", "print('hi')"])
        self.assertEqual(result.stdout.strip(), "hi")
        self.assertIsInstance(result.stdout, str)

    def test_run_raises_on_nonzero_exit_by_default(self) -> None:
        with self.assertRaises(subprocess.CalledProcessError):
            _common.run([sys.executable, "-c", "import sys; sys.exit(3)"])

    def test_run_check_false_returns_completed_process(self) -> None:
        result = _common.run(
            [sys.executable, "-c", "import sys; sys.exit(3)"], check=False)
        self.assertEqual(result.returncode, 3)

    def test_run_respects_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = _common.run([sys.executable, "-c",
                                   "import os; print(os.getcwd())"], cwd=tmp)
            self.assertEqual(Path(result.stdout.strip()).resolve(),
                              Path(tmp).resolve())


class GitTest(unittest.TestCase):
    def test_git_returns_stdout_string(self) -> None:
        out = _common.git("rev-parse", "--show-toplevel",
                           cwd=str(_common.repo_root()))
        self.assertTrue(out.strip())

    def test_git_failure_returns_output_rather_than_raising(self) -> None:
        # A bogus subcommand must not raise -- callers rely on getting back
        # whatever stdout git produced (typically empty) rather than a
        # CalledProcessError, matching the two call sites this replaces.
        out = _common.git("not-a-real-git-subcommand")
        self.assertIsInstance(out, str)

    def test_git_accepts_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            # Not a git repo, so `git -C <tmp> rev-parse ...` fails; the
            # function still returns a string (its empty stdout) rather than
            # raising.
            out = _common.git("rev-parse", "--show-toplevel", cwd=tmp)
            self.assertEqual(out, "")


class NewestRunTest(unittest.TestCase):
    def test_returns_none_when_no_subdirectory_qualifies(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(_common.newest_run(tmp, "events.jsonl"))

    def test_ignores_subdirectories_without_the_pattern_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "no-events").mkdir()
            (root / "no-events" / "other.txt").write_text("x")
            self.assertIsNone(_common.newest_run(tmp, "events.jsonl"))

    def test_picks_the_most_recently_modified_events_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            older = root / "run-a"
            newer = root / "run-b"
            older.mkdir()
            newer.mkdir()
            (older / "events.jsonl").write_text("{}\n")
            (newer / "events.jsonl").write_text("{}\n")
            # Force an unambiguous mtime ordering; directory iteration order
            # is not otherwise guaranteed to match creation order.
            import os
            import time
            now = time.time()
            os.utime(older / "events.jsonl", (now - 100, now - 100))
            os.utime(newer / "events.jsonl", (now, now))
            self.assertEqual(_common.newest_run(tmp, "events.jsonl"), newer)


class ReadEventsTest(unittest.TestCase):
    def test_parses_valid_json_lines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            events_path = Path(tmp) / "events.jsonl"
            events_path.write_text('{"kind": "state", "turn": 1}\n'
                                    '{"kind": "state", "turn": 2}\n')
            events = _common.read_events(events_path)
            self.assertEqual(events, [{"kind": "state", "turn": 1},
                                       {"kind": "state", "turn": 2}])

    def test_skips_malformed_lines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            events_path = Path(tmp) / "events.jsonl"
            events_path.write_text('{"ok": true}\n'
                                    'not json\n'
                                    '{"ok": false}\n')
            events = _common.read_events(events_path)
            self.assertEqual(events, [{"ok": True}, {"ok": False}])

    def test_accepts_a_run_directory_and_resolves_events_jsonl(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            (run_dir / "events.jsonl").write_text('{"a": 1}\n')
            self.assertEqual(_common.read_events(run_dir), [{"a": 1}])


class UtcNowTest(unittest.TestCase):
    def test_returns_a_timezone_aware_utc_datetime(self) -> None:
        now = _common.utc_now()
        self.assertIsInstance(now, datetime.datetime)
        self.assertIsNotNone(now.tzinfo)
        self.assertEqual(now.utcoffset(), datetime.timedelta(0))

    def test_is_close_to_wall_clock_time(self) -> None:
        before = datetime.datetime.now(datetime.timezone.utc)
        now = _common.utc_now()
        after = datetime.datetime.now(datetime.timezone.utc)
        self.assertLessEqual(before, now)
        self.assertLessEqual(now, after)


if __name__ == "__main__":
    unittest.main()
