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

from verify import Devtools, find_chrome, free_port, fresh_profile, tether


HARNESS = """<!doctype html><html><head><meta charset="utf-8">
<script>
window.fakeNow = 0;
Object.defineProperty(performance, "now", { value: () => window.fakeNow });
window.workerCalls = [];
window.betweenGameCountdownMs = 10000;
window.runtimeDelay = new URL(location.href).searchParams.has("slow") ? 600 : 0;
window.Worker = class {
  constructor() { this.onmessage = null; this.onerror = null; }
  postMessage(message) {
    workerCalls.push(message.path);
    let answer;
    if (message.path === "/runtime") {
      answer = { commit: new URL(location.href).searchParams.get("build") || "test" };
    } else if (message.path.startsWith("/state") && message.path.includes("have=priced")) {
      answer = { seed: 7, turn: 3, winner: null, frame_budget_ms: 700 };
    } else if (message.path.startsWith("/state")) {
      answer = {
        seed: 7, turn: 227, winner: 0, server_commit: "test",
        between_game_countdown_ms: window.betweenGameCountdownMs,
      };
    } else if (message.path === "/pace") {
      const requested = JSON.parse(message.body || "{}").between_game_countdown_ms;
      if ([0, 3000, 5000, 10000].includes(requested))
        window.betweenGameCountdownMs = requested;
      answer = { between_game_countdown_ms: window.betweenGameCountdownMs };
    } else if (message.path === "/next-game") {
      answer = { seed: 8, turn: 1, winner: null };
    } else {
      answer = {};
    }
    const bytes = new TextEncoder().encode(JSON.stringify(answer));
    setTimeout(() => this.onmessage({
      data: { id: message.id, ok: true, answer: bytes },
    }), message.path === "/runtime" ? window.runtimeDelay : 0);
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
    profile = fresh_profile("civvis-shim-profile-")
    shutil.copy(here / "shim.js", stage / "shim.js")
    (stage / "index.html").write_text(HARNESS, encoding="utf-8")
    build = stage / "build.json"
    build.write_text(
        '{"commit":"test","wasm_bytes":7340032}\n', encoding="utf-8"
    )

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
            f"http://127.0.0.1:{port}/?slow=1",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    tether(chrome)

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
            "!!document.getElementById('civvis-beta-loading')"
        ):
            time.sleep(0.1)
        loading_text = dev.evaluate(
            "document.getElementById('civvis-beta-loading')?.textContent || ''"
        )
        assert "Starting a new world" in loading_text, loading_text
        assert "Runs in this browser" in loading_text, loading_text
        deadline = time.time() + 10
        while time.time() < deadline and not dev.evaluate(
            "window.__civvisBeta && window.__civvisBeta.ready"
        ):
            time.sleep(0.1)
        time.sleep(0.5)
        assert not dev.evaluate(
            "!!document.getElementById('civvis-beta-loading')"
        ), "the startup notice remained after the local game was ready"

        # The engine prices each delivered frame in wall-clock milliseconds
        # and the shim spends that price on its own clock — even while the
        # shim's `pace` variable still holds its boot zero, which is the exact
        # state the unpaced-Blitz bug lived in: the module reported the Blitz
        # default, the page saw agreement and never pushed `/pace`, and this
        # clock waited nothing.
        begun = time.time()
        priced = dev.evaluate("fetch('/state?have=priced').then(r => r.json())")
        spent = time.time() - begun
        assert priced["turn"] == 3, priced
        assert spent >= 0.6, f"a 700ms frame budget was spent in {spent:.3f}s"

        selected = dev.evaluate(
            "fetch('/pace', {method:'POST', body:JSON.stringify({between_game_countdown_ms:3000})}).then(r => r.json())"
        )
        assert selected["between_game_countdown_ms"] == 3000, selected
        first = dev.evaluate("fetch('/state?have=226').then(r => r.json())")
        assert first["seed"] == 7 and first["restart_in"] == 3, first
        assert first["server_wasm_bytes"] == 7 * 1024 * 1024, first
        assert first["server_artifact_bytes"] == 7 * 1024 * 1024, first
        assert first["server_artifact_kind"] == "WASM", first

        # Metadata fetches happen throughout a result screen. Advancing fake
        # wall time across one proves they cannot restart the finale clock.
        dev.evaluate("window.fakeNow = 2000")
        runtime = dev.evaluate("fetch('/runtime').then(r => r.json())")
        assert runtime["commit"] == "test", runtime
        assert runtime["artifact_bytes"] == 7 * 1024 * 1024, runtime
        assert runtime["artifact_kind"] == "WASM", runtime
        dev.evaluate("window.fakeNow = 3100")
        successor = dev.evaluate("fetch('/state?have=227').then(r => r.json())")
        calls = dev.evaluate("window.workerCalls")
        assert successor["seed"] == 8 and successor["turn"] == 1, successor
        assert calls.count("/next-game") == 1, calls

        # None means no result-screen hold: the next state starts the next
        # world without waiting for the clock to advance at all.
        selected = dev.evaluate(
            "fetch('/pace', {method:'POST', body:JSON.stringify({between_game_countdown_ms:0})}).then(r => r.json())"
        )
        assert selected["between_game_countdown_ms"] == 0, selected
        immediate = dev.evaluate("fetch('/state?have=1').then(r => r.json())")
        calls = dev.evaluate("window.workerCalls")
        assert immediate["seed"] == 8 and immediate["turn"] == 1, immediate
        assert calls.count("/next-game") == 2, calls

        # A paired desktop refresh replaces the static bundle while the old
        # module finishes its current game. At the next finale it must load the
        # new module instead of asking the old worker to deal another world.
        build.write_text('{"commit":"fresh"}\n', encoding="utf-8")
        dev.evaluate("fetch('/state?have=1').then(r => r.json())")
        dev.evaluate("window.fakeNow = 3200")
        try:
            dev.evaluate("fetch('/state?have=2').then(r => r.json())")
        except Exception:
            # Navigation may retire the inspected execution context before the
            # fetch promise reports back through DevTools.
            pass
        deadline = time.time() + 10
        current_url = ""
        while time.time() < deadline:
            pages = json.load(
                urllib.request.urlopen(
                    f"http://127.0.0.1:{debug_port}/json", timeout=2
                )
            )
            current_url = next(
                (page.get("url", "") for page in pages if page.get("type") == "page"),
                "",
            )
            if "build=fresh" in current_url:
                break
            time.sleep(0.1)
        assert "build=fresh" in current_url, current_url
        print("the WASM finale honors selected holds and opens an installed successor build.")
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
