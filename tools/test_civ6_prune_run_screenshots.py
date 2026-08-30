"""The screenshot pruner may take pictures and nothing else."""
from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_prune_run_screenshots as prune  # noqa: E402


def _run(root: Path, tag: str, *, finished: bool, shots: int = 2) -> Path:
    run = root / tag
    run.mkdir()
    for i in range(shots):
        (run / f"leader-intro-attempt{i}.png").write_bytes(b"x" * 16)
    (run / "events.jsonl").write_text("{}\n")
    (run / "orders.sqlite").write_bytes(b"")
    (run / "why.log").write_text("why\n")
    if finished:
        (run / "summary.json").write_text(json.dumps({"tag": tag}))
    return run


def _tag(days_ago: float) -> str:
    stamp = time.strftime("%Y%m%dT%H%M%SZ",
                          time.gmtime(time.time() - days_ago * 86400))
    return f"civvis-{stamp}"


class ThePrunerTakesPicturesAndNothingElse(unittest.TestCase):
    """⚠⚠ THE EVIDENCE IS NOT THE PNG, BUT EVERYTHING BESIDE IT IS.

    Screenshots are 97% of the run store — 153.2 GB of 157.7 GB over 760 runs on
    2026-08-30 — and they cannot be shrunk, because the OCR that drives the menus
    reads these exact files. They can only be dropped once the run is over. What
    must survive is what every analysis in this repo actually reads.
    """

    def test_only_screenshots_are_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(30), finished=True)
            prune.main(["--run-root", str(root), "--apply"])
            self.assertFalse(list(run.glob("*.png")), "the pictures go")
            for keep in ("events.jsonl", "orders.sqlite", "why.log", "summary.json"):
                self.assertTrue((run / keep).exists(), f"{keep} must survive")

    def test_a_dry_run_removes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(30), finished=True)
            prune.main(["--run-root", str(root)])
            self.assertEqual(len(list(run.glob("*.png"))), 2)

    def test_a_recent_run_is_left_alone(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(1), finished=True)
            prune.main(["--run-root", str(root), "--apply"])
            self.assertEqual(len(list(run.glob("*.png"))), 2)

    def test_an_unfinished_run_waits_longer(self):
        """No `summary.json` means the failure may still be under diagnosis."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(4), finished=False)
            prune.main(["--run-root", str(root), "--apply"])
            self.assertEqual(len(list(run.glob("*.png"))), 2,
                             "four days is past the finished limit, not the unfinished one")
            prune.main(["--run-root", str(root), "--apply",
                        "--unfinished-after-days", "3"])
            self.assertFalse(list(run.glob("*.png")))

    def test_a_foreign_directory_is_never_touched(self):
        """⚠ July's runs are named `civvis-<map>-…` and any hand-made directory
        must be out of reach — the name pattern is the guard."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            odd = root / "civvis-duel-20260731T193918Z"
            odd.mkdir()
            (odd / "keep.png").write_bytes(b"x")
            prune.main(["--run-root", str(root), "--apply"])
            self.assertTrue((odd / "keep.png").exists())

    def test_a_live_run_is_never_touched(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tag = _tag(30)
            run = _run(root, tag, finished=True)
            original = prune.live_run_tags
            prune.live_run_tags = lambda: {tag}
            try:
                prune.main(["--run-root", str(root), "--apply"])
            finally:
                prune.live_run_tags = original
            self.assertEqual(len(list(run.glob("*.png"))), 2)

    def test_an_unreadable_process_table_protects_everything(self):
        """Not knowing what is live means assuming all of it is."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(30), finished=True)
            original = prune.live_run_tags
            prune.live_run_tags = lambda: {"*"}
            try:
                prune.main(["--run-root", str(root), "--apply"])
            finally:
                prune.live_run_tags = original
            self.assertEqual(len(list(run.glob("*.png"))), 2)

    def test_the_age_comes_from_the_tag_not_the_mtime(self):
        """⚠⚠ A directory's mtime moves when anything is created in it, and a
        read-only sqlite connect writes `-shm`/`-wal` — so analysing a run would
        make it look new. The tag cannot be disturbed by reading."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = _run(root, _tag(30), finished=True)
            (run / "orders.sqlite-wal").write_bytes(b"touched just now")
            prune.main(["--run-root", str(root), "--apply"])
            self.assertFalse(list(run.glob("*.png")), "still thirty days old")


if __name__ == "__main__":
    unittest.main()
