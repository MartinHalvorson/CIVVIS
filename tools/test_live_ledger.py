#!/usr/bin/env python3
"""The ledger branch round-trips: publish on one clone, pull and list on another."""

from __future__ import annotations

import gzip
import io
import json
import os
import subprocess
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import live_ledger  # noqa: E402

#: A CI runner has no git identity; supplied through the environment so the
#: suite never writes to any repository's config.
PROBE = {
    "GIT_AUTHOR_NAME": "ledger probe",
    "GIT_AUTHOR_EMAIL": "probe@civvis.invalid",
    "GIT_COMMITTER_NAME": "ledger probe",
    "GIT_COMMITTER_EMAIL": "probe@civvis.invalid",
}


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True,
                          text=True, check=True,
                          env={**os.environ, **PROBE}).stdout.strip()


def make_origin_and_clone(root: Path) -> tuple[Path, Path]:
    origin = root / "origin.git"
    subprocess.run(["git", "init", "-q", "--bare", str(origin)], check=True)
    work = root / "work"
    subprocess.run(["git", "init", "-q", str(work)], check=True)
    git(work, "remote", "add", "origin", str(origin))
    return origin, work


def write_run(runs: Path, tag: str, *, score: int, finished: str,
              events: list[dict] | None = None) -> None:
    run = runs / tag
    run.mkdir(parents=True)
    (run / "summary.json").write_text(json.dumps({
        "tag": tag, "finished_utc": finished, "difficulty": "DIFFICULTY_SETTLER",
        "configured": True, "last_turn": 250, "last_score": score,
        "rival_best": score + 100, "orders_seen": 100, "orders_applied": 90,
        "outcome": {"kind": "victory", "team": 4, "local_team": 0, "victory": 0},
        "seat": {"victory_types": [{"index": 0, "type": "VICTORY_SCORE"}]},
        "deals": {"sessions_opened": 3, "sessions_answered": 1,
                  "sessions_unanswered": 2, "closed": 1, "declined": 0,
                  "expired": 1, "peace_accepted": 0, "peace_refused": 2},
    }))
    (run / "events.jsonl").write_text("".join(
        json.dumps(event) + "\n" for event in (events or [{"kind": "seat"}])))


class RoundTrip(unittest.TestCase):
    def test_publish_pull_and_list(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            origin, work = make_origin_and_clone(root)
            reader = root / "reader"
            subprocess.run(["git", "init", "-q", str(reader)], check=True)
            git(reader, "remote", "add", "origin", str(origin))
            runs = root / "runs"
            write_run(runs, "civvis-a", score=500, finished="2026-08-20T10:00:00Z")
            write_run(runs, "civvis-b", score=700, finished="2026-08-21T10:00:00Z")
            for tag in ("civvis-a", "civvis-b"):
                self.assertEqual(civ6_ladder.publish_run(
                    tag, runs, repo=work, env=PROBE), "published")

            cache = root / "cache"
            fresh = live_ledger.pull(cache, repo=reader, env=PROBE)
            self.assertEqual(sorted(fresh), ["civvis-a", "civvis-b"])
            self.assertFalse((reader / "runs").exists(), "pull must not check out")
            with gzip.open(cache / "runs" / "civvis-b" / "events.jsonl.gz", "rt") as fh:
                self.assertEqual(json.loads(fh.readline())["kind"], "seat")
            # A second pull copies nothing and leaves the cache intact.
            self.assertEqual(live_ledger.pull(cache, repo=reader, env=PROBE), [])
            self.assertEqual((cache / "TIP").read_text().strip(),
                             git(origin, "rev-parse", "refs/heads/ledger"))

            out = live_ledger.runs_table(cache, last=1)
            self.assertIn("civvis-b", out)
            self.assertNotIn("civvis-a", out)
            self.assertIn("2026-08-21T10:00:00Z", out)
            self.assertIn("Settler", out)
            self.assertIn("700", out)
            self.assertIn("800", out)          # rival_best
            self.assertIn("VICTORY_SCORE", out)
            self.assertIn("90.0%", out)
            self.assertIn("s3/a1/u2 c1 d0 e1 p+0/-2", out)

            buffer = io.StringIO()
            with redirect_stdout(buffer):
                self.assertEqual(live_ledger.main(
                    ["--cache", str(cache), "runs", "--last", "5"]), 0)
            self.assertIn("civvis-a", buffer.getvalue())
            # A live runs directory reads the same way as the cache.
            self.assertIn("civvis-a", live_ledger.runs_table(runs, last=5))

    def test_pull_without_a_ledger_says_so(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            _, work = make_origin_and_clone(root)
            with self.assertRaises(RuntimeError):
                live_ledger.pull(root / "cache", repo=work, env=PROBE)


if __name__ == "__main__":
    unittest.main()
