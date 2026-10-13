"""Tests for the operator's Pause / Resume of the live verification game.

The protocol (`live_pause.py`) is two host files and one orders row; what must
hold is that every reader agrees on what a request says, that the hold report
belongs to the run that wrote it, and that the row the mod polls survives the
brain answering the very turn it is holding.

Run: python3 -m unittest tools/civ6_control/test_live_pause.py
"""

from __future__ import annotations

import json
import sqlite3
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import civ6_brain  # noqa: E402
from civ6_control import live_pause, orders  # noqa: E402


class Request(unittest.TestCase):
    def test_pause_and_resume_write_and_remove_the_request(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "pause.json"
            self.assertFalse(live_pause.requested(path))
            live_pause.set_requested(True, source="test", path=path)
            self.assertTrue(live_pause.requested(path))
            payload = json.loads(path.read_text())
            self.assertEqual(payload["paused"], True)
            self.assertEqual(payload["source"], "test")
            self.assertRegex(payload["requested_utc"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
            live_pause.set_requested(False, path=path)
            self.assertFalse(path.exists())
            # Lifting a pause nobody asked for is not an error.
            live_pause.set_requested(False, path=path)

    def test_the_file_is_the_request(self):
        """The same reading as the server and the wedge watchdog: only an
        explicit `"paused": false` plays; an empty or torn file still pauses."""
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "pause.json"
            for body, wanted in (("", True), ("{not json", True), ("{}", True),
                                 ('{"paused": true}', True),
                                 ('{"paused": false}', False)):
                path.write_text(body)
                self.assertIs(live_pause.requested(path), wanted, body)

    def test_the_environment_moves_both_files(self):
        with TemporaryDirectory() as tmp:
            env = {live_pause.REQUEST_ENV: f"{tmp}/r.json",
                   live_pause.STATE_ENV: f"{tmp}/s.json"}
            with mock.patch.dict("os.environ", env):
                self.assertEqual(live_pause.request_path(), Path(tmp) / "r.json")
                self.assertEqual(live_pause.state_path(), Path(tmp) / "s.json")


class Report(unittest.TestCase):
    def test_a_hold_keeps_its_start_and_is_released_by_its_own_run(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "state.json"
            live_pause.write_state("civvis-a", True, 151, path=path)
            first = json.loads(path.read_text())
            self.assertTrue(first["held"])
            self.assertEqual(first["turn"], 151)
            # The 30 s heartbeat refreshes the report, not the hold's start.
            with mock.patch.object(live_pause, "utc_stamp", lambda: "2099-01-01T00:00:00Z"):
                live_pause.write_state("civvis-a", True, 151, path=path)
            self.assertEqual(json.loads(path.read_text())["since_utc"], first["since_utc"])
            # Another run cannot release this one's hold...
            live_pause.release_state("civvis-b", path=path)
            self.assertTrue(json.loads(path.read_text())["held"])
            # ...its own can.
            live_pause.release_state("civvis-a", path=path)
            released = json.loads(path.read_text())
            self.assertFalse(released["held"])
            self.assertEqual(released["turn"], 151)
            self.assertNotIn("since_utc", released)

    def test_a_missing_report_is_released_quietly(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "state.json"
            live_pause.release_state("civvis-a", path=path)
            self.assertFalse(path.exists())


class OrdersRow(unittest.TestCase):
    def test_the_pause_row_survives_the_brain_answering_the_held_turn(self):
        with TemporaryDirectory() as tmp:
            db_path = Path(tmp) / "orders.sqlite"
            conn = civ6_brain.connect(db_path)
            self.assertTrue(orders.set_operator_pause(db_path, "civvis-a", 151, True))
            # Idempotent, as the harness re-asserts it.
            self.assertTrue(orders.set_operator_pause(db_path, "civvis-a", 151, True))
            civ6_brain.write_turn(conn, "civvis-a", 151, [], frame=0)
            conn.commit()
            count = "SELECT count(*) FROM orders WHERE run = ? AND kind = 'pause'"
            self.assertEqual(conn.execute(count, ("civvis-a",)).fetchone()[0], 1)
            # The mod's fetch filters on an exact frame; the row is out of band.
            row = conn.execute("SELECT turn, seq, frame, verb FROM orders"
                               " WHERE kind = 'pause'").fetchone()
            self.assertEqual(row, (151, orders.PAUSE_SEQ, orders.PAUSE_FRAME, "operator"))
            # A pause asked before any turn is known still lands.
            self.assertTrue(orders.set_operator_pause(db_path, "civvis-a", None, True))
            self.assertEqual(conn.execute(count, ("civvis-a",)).fetchone()[0], 2)
            # Resume clears every pause row of this run, and only this run.
            self.assertTrue(orders.set_operator_pause(db_path, "civvis-b", 7, True))
            self.assertTrue(orders.set_operator_pause(db_path, "civvis-a", 152, False))
            self.assertEqual(conn.execute(count, ("civvis-a",)).fetchone()[0], 0)
            self.assertEqual(conn.execute(count, ("civvis-b",)).fetchone()[0], 1)
            conn.close()

    def test_a_database_not_yet_made_is_left_for_the_brain(self):
        """The game attaches whatever file is at the path, so the pause must
        never create an empty one ahead of the brain's schema."""
        with TemporaryDirectory() as tmp:
            db_path = Path(tmp) / "orders.sqlite"
            self.assertFalse(orders.set_operator_pause(db_path, "civvis-a", 1, True))
            self.assertFalse(db_path.exists())
            sqlite3.connect(str(db_path)).close()  # an empty file, no table yet
            self.assertFalse(orders.set_operator_pause(db_path, "civvis-a", 1, True))


if __name__ == "__main__":
    unittest.main()
