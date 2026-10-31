"""Tests for the painted-turn handoff from the CIVVIS page to the live game.

`mirror_sync.py` is one host file the follower writes and `civ6_play.py`
reads; `orders.set_mirror_turn` is the row the mod waits on. What must hold:
only a fresh report for THIS run is waited on, the painted turn is preferred
while a page watches, and the row survives the brain answering turns.

Run: python3 -m unittest tools/civ6_control/test_mirror_sync.py
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import civ6_brain  # noqa: E402
from civ6_control import mirror_sync, orders  # noqa: E402


class ShownTurn(unittest.TestCase):
    def test_the_painted_turn_while_a_page_watches(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "shown.json"
            mirror_sync.write_shown("civvis-a", 41, 41, 40, 1, path=path, now=100.0)
            self.assertEqual(mirror_sync.shown_turn("civvis-a", path, now=101.0), 40)
            # No page: the server's board is what there is to wait for.
            mirror_sync.write_shown("civvis-a", 41, 41, 39, 0, path=path, now=100.0)
            self.assertEqual(mirror_sync.shown_turn("civvis-a", path, now=101.0), 41)
            # A page that has painted nothing yet (booting) is not waited on.
            mirror_sync.write_shown("civvis-a", 41, 41, None, 1, path=path, now=100.0)
            self.assertEqual(mirror_sync.shown_turn("civvis-a", path, now=101.0), 41)

    def test_only_a_fresh_report_for_this_run_counts(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "shown.json"
            self.assertIsNone(mirror_sync.shown_turn("civvis-a", path, now=100.0))
            mirror_sync.write_shown("civvis-a", 9, 9, 9, 1, path=path, now=100.0)
            self.assertIsNone(mirror_sync.shown_turn("civvis-b", path, now=100.5))
            self.assertEqual(mirror_sync.shown_turn("civvis-a", path, now=105.0), 9)
            self.assertIsNone(mirror_sync.shown_turn("civvis-a", path, now=105.1),
                              "a follower that stopped is not waited on")
            for body in ("{torn", "[]", '{"run": "civvis-a", "updated_at": "x"}',
                         '{"run": "civvis-a", "updated_at": 100, "published": true}'):
                path.write_text(body)
                self.assertIsNone(mirror_sync.shown_turn("civvis-a", path, now=100.0), body)

    def test_the_path_follows_the_environment(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "elsewhere.json"
            with mock.patch.dict("os.environ", {mirror_sync.SHOWN_ENV: str(path)}):
                self.assertEqual(mirror_sync.shown_path(), path)
                mirror_sync.write_shown("civvis-a", 3, 3, 3, 1)
                self.assertEqual(json.loads(path.read_text())["run"], "civvis-a")
                self.assertEqual(mirror_sync.shown_turn("civvis-a"), 3)


class OrdersRow(unittest.TestCase):
    def test_one_row_per_run_that_survives_the_brain(self):
        with TemporaryDirectory() as tmp:
            db_path = Path(tmp) / "orders.sqlite"
            conn = civ6_brain.connect(db_path)
            self.assertTrue(orders.set_mirror_turn(db_path, "civvis-a", 40))
            self.assertTrue(orders.set_mirror_turn(db_path, "civvis-a", 41))
            self.assertTrue(orders.set_mirror_turn(db_path, "civvis-b", 7))
            civ6_brain.write_turn(conn, "civvis-a", 41, [], frame=0)
            conn.commit()
            rows = conn.execute("SELECT run, turn, seq, kind, x, frame FROM orders"
                                " WHERE kind = 'mirror' ORDER BY run").fetchall()
            self.assertEqual(rows, [
                ("civvis-a", 0, orders.MIRROR_SEQ, "mirror", 41, orders.MIRROR_FRAME),
                ("civvis-b", 0, orders.MIRROR_SEQ, "mirror", 7, orders.MIRROR_FRAME),
            ])
            # None lifts the wait for this run only.
            self.assertTrue(orders.set_mirror_turn(db_path, "civvis-a", None))
            count = "SELECT count(*) FROM orders WHERE run = ? AND kind = 'mirror'"
            self.assertEqual(conn.execute(count, ("civvis-a",)).fetchone()[0], 0)
            self.assertEqual(conn.execute(count, ("civvis-b",)).fetchone()[0], 1)

    def test_no_database_is_not_created(self):
        with TemporaryDirectory() as tmp:
            db_path = Path(tmp) / "orders.sqlite"
            self.assertFalse(orders.set_mirror_turn(db_path, "civvis-a", 3))
            self.assertFalse(db_path.exists())

    def test_the_sentinels_are_unique(self):
        seqs = [orders.RETIRE_SEQ, orders.PAUSE_SEQ, orders.PACE_SEQ,
                orders.RESTART_SEQ, orders.MIRROR_SEQ]
        frames = [orders.RETIRE_FRAME, orders.PAUSE_FRAME, orders.PACE_FRAME,
                  orders.RESTART_FRAME, orders.MIRROR_FRAME]
        self.assertEqual(len(set(seqs)), len(seqs))
        self.assertEqual(len(set(frames)), len(frames))


if __name__ == "__main__":
    unittest.main()
