#!/usr/bin/env python3
"""Exercise the browser-side WASM clock against a deterministic worker."""

from __future__ import annotations

import functools
import http.server
import json
import pathlib
import shutil
import socketserver
import subprocess
import tempfile
import threading
import time
import urllib.request

from verify import Devtools, find_chrome, free_port


HARNESS = """<!doctype html><html><head><meta charset="utf-8">
<script>
window.fakeNow = 0;
Object.defineProperty(performance, "now", { value: () => window.fakeNow });
window.workerCalls = [];
window.Worker = class {
  constructor() { this.onmessage = null; this.onerror = null; }
  postMessage(message) {
    workerCalls.push(message.path);
    let answer;
    if (message.path === "/runtime") {
      answer = { commit: "test" };
    } else if (message.path.startsWith("/state")) {
      answer = { seed: 7, turn: 227, winner: 0 };
    } else if (message.path === "/next-game") {
      answer = { seed: 8, turn: 1, winner: null };
    } else {
      answer = {};
    }
    const bytes = new TextEncoder().encode(JSON.stringify(answer));
    queueMicrotask(() => this.onmessage({
      data: { id: message.id, ok: true, answer: bytes },
    }));
  }
};
</script>
<script src="shim.js"></script></head><body></body></html>"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
    }

    def log_message(self, fmt, *args):
        pass


def main() -> int:
    here = pathlib.Path(__file__).resolve().parent
    stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-shim-"))
    profile = tempfile.mkdtemp(prefix="civvis-shim-profile-")
    shutil.copy(here / "shim.js", stage / "shim.js")
    (stage / "index.html").write_text(HARNESS, encoding="utf-8")

    port = free_port()
    server = socketserver.TCPServer(
        ("127.0.0.1", port), functools.partial(Quiet, directory=str(stage))
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    debug_port = free_port()
    chrome = subprocess.Popen(
        [
            find_chrome(),
            "--headless=new",
            f"--remote-debugging-port={debug_port}",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--disable-gpu",
            f"http://127.0.0.1:{port}/",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        target = None
        deadline = time.time() + 30
        while time.time() < deadline and not target:
            try:
                pages = json.load(
                    urllib.request.urlopen(
                        f"http://127.0.0.1:{debug_port}/json", timeout=2
                    )
                )
                target = next(
                    (
                        page
                        for page in pages
                        if page.get("type") == "page"
                        and page.get("webSocketDebuggerUrl")
                    ),
                    None,
                )
            except Exception:
                time.sleep(0.2)
        if not target:
            raise RuntimeError("Chrome never offered a debuggable page")

        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        deadline = time.time() + 10
        while time.time() < deadline and not dev.evaluate(
            "window.__civvisBeta && window.__civvisBeta.ready"
        ):
            time.sleep(0.1)

        first = dev.evaluate("fetch('/state?have=226').then(r => r.json())")
        assert first["seed"] == 7 and first["restart_in"] == 10, first

        # Metadata fetches happen throughout a result screen. Advancing fake
        # wall time across one proves they cannot restart the finale clock.
        dev.evaluate("window.fakeNow = 6000")
        runtime = dev.evaluate("fetch('/runtime').then(r => r.json())")
        assert runtime == {"commit": "test"}, runtime
        dev.evaluate("window.fakeNow = 11000")
        successor = dev.evaluate("fetch('/state?have=227').then(r => r.json())")
        calls = dev.evaluate("window.workerCalls")
        assert successor["seed"] == 8 and successor["turn"] == 1, successor
        assert calls.count("/next-game") == 1, calls
        print("the WASM finale survives metadata polling and opens one successor world.")
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=5)
        except subprocess.TimeoutExpired:
            chrome.kill()
        server.shutdown()
        server.server_close()
        shutil.rmtree(stage, ignore_errors=True)
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
