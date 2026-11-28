#!/usr/bin/env python3
"""The hotseat play copy's update: on real git repositories in a scratch
directory (a bare `origin`, a play copy cloned from it)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_hotseat_update as update  # noqa: E402

#: What the play copy's harness looked like before the #5998 fix, the fix the
#: operator applied by hand that night, and main's version once #5998 and
#: #5999 had both landed (the fix, and another change beside it).
BEFORE = "def slot_word(word):\n    return word if word in SLOT_WORDS else None\n\n\ndef rows():\n    return 29\n"
FIX = ("def slot_word(word):\n    return next((w for w in SLOT_WORDS if near(word, w)), None)\n\n\n"
       "def rows():\n    return 29\n")
MAIN_LATER = ("def slot_word(word):\n    return next((w for w in SLOT_WORDS if near(word, w)), None)\n\n\n"
              "def rows():\n    short = 0\n    return 29\n")
GIT_ENV = {"GIT_AUTHOR_NAME": "Test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
           "GIT_COMMITTER_NAME": "Test", "GIT_COMMITTER_EMAIL": "test@example.invalid",
           "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}


class PlayCopy:
    """A bare origin with main, a working copy that commits to it, and the play
    copy: a detached clone, as `~/civvis-hotseat-main` is."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.origin, self.work, self.play = root / "origin.git", root / "work", root / "play"
        self.run(root, "init", "-q", "--bare", "-b", "main", str(self.origin))
        self.run(root, "clone", "-q", str(self.origin), str(self.work))
        self.commit({"tools/civ6_hotseat.py": BEFORE, "src/lib.rs": "pub fn f() {}\n",
                     "src/genes.rs": 'const L: &str = include_str!("../docs/gene_ledger.json");\n',
                     "docs/gene_ledger.json": "{}\n", "docs/notes.md": "notes\n",
                     "Cargo.toml": "[package]\n"}, "first")
        self.run(root, "clone", "-q", str(self.origin), str(self.play))
        self.run(self.play, "checkout", "-q", "--detach", "origin/main")
        self.patches = root / "patches"

    @staticmethod
    def run(cwd: Path, *args: str) -> str:
        done = subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True,
                              env={**os.environ, **GIT_ENV}, check=True)
        return done.stdout.strip()

    def commit(self, files: dict[str, str], message: str) -> str:
        for name, text in files.items():
            path = self.work / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        self.run(self.work, "add", "-A")
        self.run(self.work, "commit", "-q", "-m", message)
        self.run(self.work, "push", "-q", "origin", "HEAD:main")
        return self.run(self.work, "rev-parse", "HEAD")

    def head(self) -> str:
        return self.run(self.play, "rev-parse", "HEAD")

    def build(self) -> None:
        binary = self.play / update.BINARY
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text("#!/bin/sh\n")
        assert update.built(self.play, update.BINARY, say=lambda line: None) == update.DONE

    def update(self, **more) -> tuple[int, list[str]]:
        said: list[str] = []
        code = update.update(self.play, self.patches, update.BINARY, say=said.append, **more)
        return code, said


class UpdateTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.copy = PlayCopy(Path(tmp.name))
        old = os.environ.copy()
        os.environ.update(GIT_ENV)
        self.addCleanup(lambda: (os.environ.clear(), os.environ.update(old)))

    def test_a_hand_applied_fix_main_now_has_is_dropped_only_as_the_copy_moves(self) -> None:
        # 2026-11-27: the fix in the play copy, main later carrying it and more.
        copy = self.copy
        (copy.play / "tools/civ6_hotseat.py").write_text(FIX)
        new = copy.commit({"tools/civ6_hotseat.py": MAIN_LATER}, "Hotseat lobby: seats (#5998, #5999)")
        code, said = copy.update()
        self.assertEqual(copy.head(), new)
        self.assertEqual((copy.play / "tools/civ6_hotseat.py").read_text(), MAIN_LATER)
        self.assertIn("origin/main already has them: they are dropped as the checkout moves", said)
        patch = next(copy.patches.glob("*.patch"))
        self.assertIn("near(word, w)", patch.read_text(), "the change is kept on disk all the same")
        self.assertIn(f"saved as {patch}", said[0])
        self.assertIn("for the hotseat (1):", said)

    def test_a_fix_main_does_not_have_is_kept_on_top(self) -> None:
        copy = self.copy
        (copy.play / "tools/civ6_hotseat.py").write_text(FIX)
        new = copy.commit({"docs/notes.md": "more notes\n"}, "Docs")
        code, said = copy.update()
        self.assertEqual(copy.head(), new)
        self.assertEqual((copy.play / "tools/civ6_hotseat.py").read_text(), FIX)
        self.assertEqual((copy.play / "docs/notes.md").read_text(), "more notes\n")
        self.assertIn(f"origin/main does not have them: they are kept, on top of {new[:8]}", said)

    def test_a_fix_main_conflicts_with_stops_the_update_and_keeps_everything(self) -> None:
        copy = self.copy
        old = copy.head()
        (copy.play / "tools/civ6_hotseat.py").write_text(FIX)
        copy.commit({"tools/civ6_hotseat.py": BEFORE.replace("else None", "else 'open'")}, "Other fix")
        code, said = copy.update()
        self.assertEqual(code, update.REFUSED)
        self.assertEqual(copy.head(), old, "nothing moved")
        self.assertEqual((copy.play / "tools/civ6_hotseat.py").read_text(), FIX, "the change is in place")
        self.assertTrue(list(copy.patches.glob("*.patch")))
        self.assertTrue(any("conflict with origin/main" in line for line in said), said)

    def test_the_decider_is_built_only_when_what_it_is_compiled_from_changed(self) -> None:
        copy = self.copy
        code, said = copy.update()
        self.assertEqual(code, update.BUILD, "no decider yet")
        copy.build()
        self.assertEqual(copy.update()[0], update.DONE, "built from this revision")
        copy.commit({"tools/civ6_hotseat.py": FIX}, "Hotseat only")
        code, said = copy.update()
        self.assertEqual(code, update.DONE, said)
        self.assertTrue(any("decider build: not needed" in line for line in said), said)
        # A file src/ compiles in (the gene ledger), and Rust itself.
        copy.commit({"docs/gene_ledger.json": '{"x": 1}\n'}, "Ledger")
        code, said = copy.update()
        self.assertEqual(code, update.BUILD)
        self.assertTrue(any("docs/gene_ledger.json" in line for line in said), said)
        copy.build()
        copy.commit({"docs/notes.md": "x\n"}, "Docs only")
        self.assertEqual(copy.update()[0], update.DONE)
        copy.commit({"src/lib.rs": "pub fn g() {}\n"}, "Rust")
        self.assertEqual(copy.update()[0], update.BUILD)
        # A stamp with no decider beside it (a `cargo clean`) is no build.
        copy.build()
        (copy.play / update.BINARY).unlink()
        code, said = copy.update()
        self.assertEqual(code, update.BUILD)
        self.assertTrue(any("there is no decider" in line for line in said), said)

    def test_a_local_change_to_rust_is_part_of_what_the_build_was(self) -> None:
        copy = self.copy
        copy.update()
        copy.build()
        (copy.play / "src/lib.rs").write_text("pub fn local() {}\n")
        self.assertEqual(copy.update()[0], update.BUILD, "the decider was built without it")
        copy.build()
        self.assertEqual(copy.update()[0], update.DONE, "the build had it")
        stamp = json.loads((copy.play / "target/release/civvis_orders.revision").read_text())
        self.assertEqual(stamp["revision"], copy.head())
        self.assertTrue(stamp["local"])

    def test_a_dry_run_moves_nothing(self) -> None:
        copy = self.copy
        old = copy.head()
        (copy.play / "tools/civ6_hotseat.py").write_text(FIX)
        new = copy.commit({"src/lib.rs": "pub fn g() {}\n"}, "Rust")
        code, said = copy.update(dry_run=True)
        self.assertEqual(code, update.BUILD)
        self.assertEqual(copy.head(), old)
        self.assertEqual((copy.play / "tools/civ6_hotseat.py").read_text(), FIX)
        self.assertTrue(any(f"would move {old[:8]} -> {new[:8]}" in line for line in said), said)

    def test_what_changed_puts_the_hotseat_first(self) -> None:
        copy = self.copy
        old = copy.head()
        copy.commit({"docs/notes.md": "a\n"}, "Docs one")
        copy.commit({"tools/civ6_hotseat.py": FIX}, "Hotseat: the fix")
        copy.commit({"docs/notes.md": "b\n"}, "Docs two")
        copy.run(copy.play, "fetch", "-q")
        lines = update.what_changed(copy.play, old, copy.run(copy.play, "rev-parse", "origin/main"))
        self.assertEqual(lines[0], "for the hotseat (1):")
        self.assertIn("Hotseat: the fix", lines[1])
        self.assertTrue(lines[-1].startswith("and 2 other commit(s)"))

    def test_the_command_line(self) -> None:
        self.assertEqual(update.main(["--checkout", str(self.copy.play), "--patches",
                                      str(self.copy.patches)]), update.BUILD)
        self.assertEqual(update.main(["--checkout", str(self.copy.play), "--built"]), update.REFUSED,
                         "no binary to stamp")
        self.assertEqual(update.main(["--checkout", str(self.copy.root / "nowhere")]), update.REFUSED)


if __name__ == "__main__":
    unittest.main()
