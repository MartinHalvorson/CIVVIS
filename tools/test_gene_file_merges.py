#!/usr/bin/env python3
"""Two genes appended at one anchor merge without a conflict.

Every gene PR adds a row to `genes.rs` and `treatment_flags.rs` at a shared
per-letter marker, so concurrent gene PRs conflicted on lines that were never
really in dispute. `.gitattributes` keeps both sides for those two files. These
hold the attribute in place, prove what it does in a real merge, and stop it
spreading to a file whose conflicts are real.
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
UNION_FILES = {"src/ai/advanced/genes.rs", "src/ai/advanced/treatment_flags.rs"}
GIT = ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]


def run(cwd: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run([*GIT, *args], cwd=cwd, check=check, capture_output=True, text=True)


def attributed_files() -> set[str]:
    rows = (REPO / ".gitattributes").read_text(encoding="utf-8").splitlines()
    return {row.split()[0] for row in rows if "merge=union" in row and not row.startswith("#")}


class TheAttribute(unittest.TestCase):
    def test_only_the_two_row_files_merge_by_union(self):
        self.assertEqual(attributed_files(), UNION_FILES)

    def test_git_reads_it_for_both(self):
        for path in UNION_FILES:
            with self.subTest(path):
                out = run(REPO, "check-attr", "merge", "--", path).stdout
                self.assertTrue(out.strip().endswith("merge: union"), out)

    def test_the_named_files_exist(self):
        for path in UNION_FILES:
            with self.subTest(path):
                self.assertTrue((REPO / path).is_file())


class ARealMerge(unittest.TestCase):
    BASE = "const GENES = [\n    // ---- append: a-b ----\n    Gene { tag: \"old\" },\n];\n"

    def setUp(self):
        self.holder = tempfile.TemporaryDirectory()
        self.root = Path(self.holder.name)
        run(self.root, "init", "-q", "-b", "main")
        (self.root / ".gitattributes").write_text("rows.rs merge=union\n", encoding="utf-8")
        for name in ("rows.rs", "plain.rs"):
            (self.root / name).write_text(self.BASE, encoding="utf-8")
        run(self.root, "add", ".")
        run(self.root, "commit", "-qm", "base")

    def tearDown(self):
        self.holder.cleanup()

    def branch(self, name: str, file: str, row: str) -> None:
        run(self.root, "checkout", "-q", "-b", name, "main")
        text = (self.root / file).read_text(encoding="utf-8")
        marker = "    Gene { tag: \"old\" },\n"
        (self.root / file).write_text(text.replace(marker, marker + row + "\n"), encoding="utf-8")
        run(self.root, "commit", "-qam", name)

    def merge(self, file: str) -> subprocess.CompletedProcess:
        self.branch("first", file, '    Gene { tag: "first" },')
        self.branch("second", file, '    Gene { tag: "second" },')
        run(self.root, "checkout", "-q", "first")
        return run(self.root, "merge", "--no-edit", "second", check=False)

    def test_two_appends_at_one_anchor_keep_both_rows(self):
        self.assertEqual(self.merge("rows.rs").returncode, 0)
        text = (self.root / "rows.rs").read_text(encoding="utf-8")
        self.assertNotIn("<<<<<<<", text)
        self.assertEqual(text.count('tag: "first"'), 1)
        self.assertEqual(text.count('tag: "second"'), 1)
        self.assertEqual(text.count('tag: "old"'), 1)

    def test_the_same_append_in_an_ordinary_file_still_conflicts(self):
        self.assertNotEqual(self.merge("plain.rs").returncode, 0)


if __name__ == "__main__":
    unittest.main()
