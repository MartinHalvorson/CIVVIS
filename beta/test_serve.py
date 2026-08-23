"""Tests for the trusted local side of the desktop WASM channel."""

import functools
import importlib.util
import pathlib
import tempfile
import threading
import unittest
import urllib.request


SCRIPT = pathlib.Path(__file__).with_name("serve.py")
SPEC = importlib.util.spec_from_file_location("civvis_beta_serve", SCRIPT)
serve = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(serve)


class ChannelTests(unittest.TestCase):
    def test_the_wasm_channel_maps_onto_the_viewer_at_the_lane_root(self):
        with tempfile.TemporaryDirectory() as held:
            root = pathlib.Path(held)
            # The published lane carries the viewer at its root now.
            (root / "index.html").write_text("wasm", encoding="utf-8")
            handler = functools.partial(serve.Handler, directory=str(root))
            server = serve.Server(("127.0.0.1", 0), handler)
            self.addCleanup(server.server_close)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.addCleanup(server.shutdown)
            origin = f"http://127.0.0.1:{server.server_address[1]}"

            with urllib.request.urlopen(origin + "/wasm/") as response:
                self.assertEqual(response.read(), b"wasm")
                self.assertIn("no-store", response.headers["Cache-Control"])


if __name__ == "__main__":
    unittest.main()
