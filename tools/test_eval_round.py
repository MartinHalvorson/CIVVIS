#!/usr/bin/env python3
"""A new evaluation round must not be appended to the archive's tail.

`docs/EVAL.md` is 10,386 lines with exactly one write point, and every author
queued at it: thirty-two commits touched the file in the seven days to
2026-08-18, all of them editing the last few lines. Rounds go in
`docs/eval/` now, one file each, which has no shared tail to collide over.

The convention only holds if something notices when it is broken, so the
archive's round count is ratcheted here: it may FALL — a round can be moved out
— and it may not rise.
"""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import eval_round  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
ARCHIVE = REPO / "docs" / "EVAL.md"
ROUNDS = REPO / "docs" / "eval"

# Rounds in the archive when it stopped being the write point (2026-08-18).
ARCHIVED_ROUNDS = 168


class TheArchiveStoppedGrowing(unittest.TestCase):
    def test_no_new_round_was_appended_to_the_archive(self):
        rounds = len(re.findall(r"^## ", ARCHIVE.read_text(encoding="utf-8"), re.M))
        self.assertLessEqual(
            rounds,
            ARCHIVED_ROUNDS,
            f"docs/EVAL.md has {rounds} rounds, up from {ARCHIVED_ROUNDS}. A new "
            f"round belongs in docs/eval/ as its own file — that is the whole "
            f"point, and appending here puts every author back on one write "
            f"point. `tools/eval_round.py \"<title>\"` starts one.",
        )

    def test_a_round_moved_out_lowers_the_recorded_number(self):
        rounds = len(re.findall(r"^## ", ARCHIVE.read_text(encoding="utf-8"), re.M))
        self.assertEqual(
            rounds,
            ARCHIVED_ROUNDS,
            f"docs/EVAL.md now has {rounds} rounds, not {ARCHIVED_ROUNDS}. If "
            f"rounds were moved out, set ARCHIVED_ROUNDS = {rounds} so the "
            f"ratchet holds the new floor.",
        )

    def test_the_archive_says_where_new_rounds_go(self):
        head = "\n".join(ARCHIVE.read_text(encoding="utf-8").split("\n")[:20])
        self.assertIn("docs/eval/", head,
                      "a reader who opens the archive to append must be told not to")


class TheToolMakesTheConventionExecutable(unittest.TestCase):
    def test_two_rounds_on_one_day_are_two_files(self):
        with TemporaryDirectory() as raw:
            with mock.patch.object(eval_round, "ROUNDS", Path(raw)):
                a = eval_round.new_round("the maritime splice", "2026-08-18", "abc1234")
                b = eval_round.new_round("the civilian snatch", "2026-08-18", "abc1234")
        self.assertNotEqual(a.name, b.name,
                            "two rounds finished the same afternoon must not "
                            "share a file, or the collision is back")

    def test_a_repeated_title_is_refused_rather_than_overwritten(self):
        with TemporaryDirectory() as raw:
            with mock.patch.object(eval_round, "ROUNDS", Path(raw)):
                eval_round.new_round("the maritime splice", "2026-08-18", "abc")
                with self.assertRaises(SystemExit):
                    eval_round.new_round("the maritime splice", "2026-08-18", "abc")

    def test_the_template_asks_for_intervals(self):
        with TemporaryDirectory() as raw:
            with mock.patch.object(eval_round, "ROUNDS", Path(raw)):
                path = eval_round.new_round("x", "2026-08-18", "abc")
            body = path.read_text()
        self.assertIn("interval", body,
                      "a point estimate with no interval cannot be compared "
                      "against the next one; that is how a 40-game figure came "
                      "to be read against a 480-game figure")
        for heading in ("What was asked", "How it was measured",
                        "What it measured", "What was decided"):
            self.assertIn(heading, body)

    def test_the_directory_explains_itself(self):
        readme = (ROUNDS / "README.md").read_text(encoding="utf-8")
        self.assertIn("one file", readme.lower())
        self.assertIn("EVAL.md", readme)


if __name__ == "__main__":
    unittest.main()
