"""Tests for the trusted local side of the desktop WASM channel."""

import functools
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from unittest import mock


SCRIPT = pathlib.Path(__file__).with_name("serve.py")
SPEC = importlib.util.spec_from_file_location("civvis_beta_serve", SCRIPT)
serve = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(serve)


class RatingHostTests(unittest.TestCase):
    def test_native_helper_initializes_and_records_without_reimplementing_glicko(self):
        with tempfile.TemporaryDirectory() as held:
            league = pathlib.Path(held) / "league"
            host = serve.RatingHost(pathlib.Path(held) / "civvis", league)
            ready = subprocess.CompletedProcess(
                (), 0, json.dumps({"status": "ready", "round": 7}), ""
            )
            recorded = subprocess.CompletedProcess(
                (), 0, json.dumps({"status": "recorded", "round": 8}), ""
            )
            with mock.patch.object(
                serve.subprocess, "run", side_effect=(ready, recorded)
            ) as run:
                self.assertEqual(host.initialize()["round"], 7)
                self.assertEqual(host.record('{"result_id":"one"}')["round"], 8)

            self.assertEqual(run.call_args_list[0].args[0][1], "league-init")
            self.assertEqual(run.call_args_list[1].args[0][1], "rate-game")
            self.assertEqual(run.call_args_list[1].kwargs["input"], '{"result_id":"one"}')

    def test_local_routes_expose_the_roster_and_accept_only_same_origin_results(self):
        class FakeRatingHost:
            def __init__(self):
                self.reports = []

            def roster(self):
                return b'{"round":12,"strategies":[]}\n'

            def record(self, report):
                self.reports.append(json.loads(report))
                return {"status": "recorded", "round": 13}

        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            (root / "beta").mkdir()
            (root / "beta/index.html").write_text("wasm", encoding="utf-8")
            rating = FakeRatingHost()
            handler = functools.partial(
                serve.Handler, directory=str(root), rating_host=rating
            )
            server = serve.Server(("127.0.0.1", 0), handler)
            self.addCleanup(server.server_close)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            origin = f"http://127.0.0.1:{server.server_address[1]}"

            with urllib.request.urlopen(origin + "/wasm/league.json") as response:
                self.assertEqual(json.load(response)["round"], 12)
                self.assertIn("no-store", response.headers["Cache-Control"])

            report = json.dumps({"result_id": "wasm-v1:17:abc", "seats": [1, 2]}).encode()
            request = urllib.request.Request(
                origin + "/wasm/league-result",
                data=report,
                headers={"Content-Type": "application/json", "Origin": origin},
            )
            with urllib.request.urlopen(request) as response:
                self.assertEqual(json.load(response)["status"], "recorded")
            self.assertEqual(rating.reports[0]["result_id"], "wasm-v1:17:abc")

            foreign = urllib.request.Request(
                origin + "/wasm/league-result",
                data=report,
                headers={"Content-Type": "application/json", "Origin": "https://evil.invalid"},
            )
            with self.assertRaises(urllib.error.HTTPError) as refused:
                urllib.request.urlopen(foreign)
            self.assertEqual(refused.exception.code, 403)
            self.assertEqual(len(rating.reports), 1)


if __name__ == "__main__":
    unittest.main()
