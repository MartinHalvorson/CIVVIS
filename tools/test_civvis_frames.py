"""Tests for the frame-coverage checker's analysis."""
from __future__ import annotations

import contextlib
import io
import json
import pathlib
import sys
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from civvis_frames import autoplay, gaps  # noqa: E402


class GapsTest(unittest.TestCase):
    def test_a_viewer_that_saw_every_turn_has_no_gaps(self):
        self.assertEqual(gaps([4, 5, 6, 7]), [])

    def test_polling_twice_inside_one_turn_is_not_a_gap(self):
        # The page redraws whatever the server hands it, so a pace slower than
        # the poll gives the same turn back several times over.
        self.assertEqual(gaps([4, 4, 5, 5, 5, 6]), [])

    def test_turns_that_never_arrived_are_reported(self):
        self.assertEqual(gaps([4, 6, 9]), [5, 7, 8])

    def test_only_the_span_actually_watched_counts(self):
        # Turns before the viewer attached and after it stopped are nobody's
        # missed frames; the run is judged between its own first and last.
        self.assertEqual(gaps([100, 101, 102]), [])

    def test_no_observations_is_not_a_claim_either_way(self):
        self.assertEqual(gaps([]), [])


class FakeGame(BaseHTTPRequestHandler):
    """Just enough of a human game to answer the auto-play audit.

    ``turns_per_request`` is the whole point: at 1 the server behaves as the
    browser is supposed to make it behave, and above it every response still
    carries a single state while the turn counter jumps — which is the bug,
    reproduced without an engine.
    """

    turn = 1
    turns_per_request = 1
    missed = 0
    autoplayed = 0

    def log_message(self, *_args):  # keep the test output clean
        pass

    def _send(self, payload: dict):
        body = json.dumps(payload).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        cls = type(self)
        self._send({"turn": cls.turn, "seed": 7, "spectate": False,
                    "frames_missed": cls.missed, "frames_painted": None,
                    "viewers": 0, "autoplay_turns": cls.autoplayed})

    def do_POST(self):
        cls = type(self)
        asked = json.loads(self.rfile.read(int(self.headers["Content-Length"])))["turns"]
        played = min(asked, cls.turns_per_request)
        cls.turn += played
        cls.autoplayed += played
        # Exactly what the server charges itself: one response, one state.
        cls.missed += max(0, played - 1)
        self._send({"turn": cls.turn, "seed": 7, "autoplayed": played})


class AutoplayAuditTest(unittest.TestCase):
    def drive(self, turns_per_request: int, turns: int = 6):
        FakeGame.turn, FakeGame.missed, FakeGame.autoplayed = 1, 0, 0
        FakeGame.turns_per_request = turns_per_request
        server = HTTPServer(("127.0.0.1", 0), FakeGame)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        out = io.StringIO()
        try:
            with contextlib.redirect_stdout(out):
                code = autoplay(server.server_address[1], turns, "basic", turns_per_request)
        finally:
            server.shutdown()
            server.server_close()
        return code, json.loads(out.getvalue())

    def test_one_turn_per_request_shows_every_turn(self):
        code, report = self.drive(turns_per_request=1)
        self.assertEqual(code, 0, report["verdict"])
        self.assertEqual(report["turns_played"], 6)
        self.assertEqual(report["states_returned"], 6)
        self.assertEqual(report["turns_missed"], 0)
        self.assertEqual(report["server_charged_missed"], 0)

    def test_a_batch_hides_every_turn_but_its_last(self):
        # Six turns in two requests of three: turns 2, 3, 5 and 6 are simulated
        # into states that are never sent, and both ends of the count say so.
        code, report = self.drive(turns_per_request=3)
        self.assertEqual(code, 1)
        self.assertEqual(report["turns_played"], 6)
        self.assertEqual(report["states_returned"], 2)
        self.assertEqual(report["missed"], [2, 3, 5, 6])
        self.assertEqual(report["server_charged_missed"], 4)

    def test_a_game_that_plays_nothing_is_not_a_pass(self):
        code, report = self.drive(turns_per_request=1, turns=0)
        self.assertEqual(code, 1)
        self.assertIn("nothing was tested", report["verdict"])


if __name__ == "__main__":
    unittest.main()
