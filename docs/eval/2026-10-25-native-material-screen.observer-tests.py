"""Isolated filesystem controls for this trial's start cutoff."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SOURCE = Path(__file__).with_name("2026-10-25-native-material-screen.observer.py")
SPEC = importlib.util.spec_from_file_location("observer", SOURCE)
observer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observer)


class ObserverTests(unittest.TestCase):
    def test_exactly_eight_starts_keep_continuations_and_overruns(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            snapshots = home / "civvis-civ6-runs/decider-snapshots"
            snapshots.mkdir(parents=True)
            old = "civvis-20261025T010000Z"
            (snapshots / old).mkdir()

            def arrive(_):
                (snapshots / (old + "-cont1")).mkdir()
                for index in range(1, 10):
                    (snapshots / f"civvis-20261025T02000{index}Z").mkdir()

            out = home / "evidence"
            argv = ["observer", "--repo", tmp, "--repair", "repair",
                    "--qualification-run", old, "--out", str(out)]
            with mock.patch.object(sys, "argv", argv), \
                 mock.patch.object(observer.Path, "home", return_value=home), \
                 mock.patch.object(observer, "qualify", return_value={"fixture": True}), \
                 mock.patch.object(observer, "drivers", return_value=[]), \
                 mock.patch.object(observer.time, "sleep", side_effect=arrive):
                observer.main()
            state = json.loads((out / "state.json").read_text())
            self.assertEqual(len(state["assigned"]), 8)
            self.assertEqual(len(state["overruns"]), 1)
            self.assertNotIn(old, state["assigned"])
            self.assertEqual(state["cutoff"]["action"], "removed_owned_screen")
            self.assertFalse((home / ".civvis-live-screen-gene").exists())

    def test_foreign_or_replaced_screen_is_never_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            screen = Path(tmp) / "screen"
            screen.write_text(observer.GENE + "\n")
            identity = observer.identity(screen)
            screen.unlink()
            screen.write_text(observer.GENE + "\n")
            self.assertEqual(observer.cut_off(screen, identity),
                             "foreign_or_changed_screen_preserved")
            screen.write_text("another-gene\n")
            self.assertEqual(observer.cut_off(screen, observer.identity(screen)),
                             "foreign_or_changed_screen_preserved")
            self.assertEqual(screen.read_text(), "another-gene\n")

    def test_only_run_tags_join_their_continuations(self):
        original = "civvis-20261025T010000Z"
        self.assertEqual(observer.stem(original + "-cont2"), original)
        self.assertIsNone(observer.stem("unrelated"))


if __name__ == "__main__":
    unittest.main()
