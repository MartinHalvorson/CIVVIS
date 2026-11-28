#!/usr/bin/env python3
"""The engine's download, broken on purpose, in a real browser.

A first visit draws a baked opening before the engine has arrived
(`beta/shim.js`, "baked"). The engine was asked for once: a download that
failed kept its failure for every request after it, and one that stopped was
waited on for ever, so the opening stood on screen with its clock running and
nothing moving (operator 2026-11-27: "civvis,.ai doesnt always appear to start
a game", "sometimes has map and clock starts but nothing moving").

`beta/worker.js` is loaded as it ships, with its stall limit cut to 1.5 s so
this runs in seconds, over the toy engine of `forks_test.py`. A server breaks
the first download of the engine in each way it was seen to break:

  * cut off part way: asked for again, and the engine answers;
  * stopped part way, the connection held open: given up after the stall
    limit, asked for again, and the engine answers;
  * a server error: asked for again, and the engine answers;
  * not there at all (404): not asked for again, and the request fails;
  * whole: one download, no second.

A try after the first goes to the network itself (`cache: "no-store"`, sent
as `Cache-Control: no-cache`), and the page hears how the download goes:
bytes as they come, each try given up and why, and the end.

Set `CIVVIS_TEST_CHROME` to run this with a particular browser binary.
"""

from __future__ import annotations

import http.server
import json
import os
import pathlib
import shutil
import socketserver
import subprocess
import threading
import time
import urllib.request

from forks_test import toy_engine
from verify import Devtools, find_chrome, free_port, fresh_profile, tether

HERE = pathlib.Path(__file__).resolve().parent
WORKER = (HERE / "worker.js").read_text(encoding="utf-8")
STALL_LINE = "const STALL_MS = 15000;"
assert STALL_LINE in WORKER, "worker.js no longer names its stall limit as this test expects"
FAST_WORKER = WORKER.replace(STALL_LINE, "const STALL_MS = 1500;")
ENGINE = toy_engine()

PAGE = """<!doctype html><meta charset="utf-8"><script>
window.runs = {};
window.run = (name) => {
  const told = [];
  const result = window.runs[name] = { told, answer: null, began: performance.now() };
  const worker = new Worker("worker.js");
  worker.onmessage = (event) => {
    if (event.data.loading) { told.push(event.data.loading); return; }
    result.answer = { ok: event.data.ok, error: event.data.error || null, ms: Math.round(performance.now() - result.began) };
  };
  worker.postMessage({ id: 1, method: "GET", path: "/runtime", body: "", wasmUrl: "civvis.wasm?case=" + name, seed: 1 });
  return true;
};
</script>"""

asked: dict[str, list[dict]] = {}
lock = threading.Lock()


class Breaker(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args) -> None:
        pass

    def send(self, code: int, kind: str, body: bytes) -> None:
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        path, _, query = self.path.partition("?")
        if path == "/":
            self.send(200, "text/html; charset=utf-8", PAGE.encode())
            return
        if path == "/worker.js":
            self.send(200, "text/javascript", FAST_WORKER.encode())
            return
        if path != "/civvis.wasm":
            self.send(404, "text/plain", b"no")
            return
        case = query.removeprefix("case=")
        with lock:
            tries = asked.setdefault(case, [])
            tries.append({"cache": self.headers.get("Cache-Control"), "at": time.time()})
            first = len(tries) == 1
        if case == "gone":
            self.send(404, "text/plain", b"gone")
            return
        if first and case == "busy":
            self.send(503, "text/plain", b"busy")
            return
        if first and case in ("cut", "stopped"):
            self.send_response(200)
            self.send_header("Content-Type", "application/wasm")
            self.send_header("Content-Length", str(len(ENGINE)))
            self.end_headers()
            self.wfile.write(ENGINE[:16])
            self.wfile.flush()
            if case == "stopped":
                time.sleep(6)
            self.close_connection = True
            self.connection.shutdown(2)
            return
        self.send(200, "application/wasm", ENGINE)


class Server(socketserver.ThreadingMixIn, socketserver.TCPServer):
    daemon_threads = True
    allow_reuse_address = True


def main() -> int:
    port = free_port()
    server = Server(("127.0.0.1", port), Breaker)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    profile = fresh_profile("civvis-engine-download-")
    debug_port = free_port()
    chrome = subprocess.Popen(
        [os.environ.get("CIVVIS_TEST_CHROME") or find_chrome(), "--headless=new",
         f"--remote-debugging-port={debug_port}", f"--user-data-dir={profile}", "--no-first-run",
         "--disable-gpu", f"http://127.0.0.1:{port}/"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    tether(chrome)
    try:
        target = None
        deadline = time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{debug_port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")
                               and page.get("url", "").startswith(f"http://127.0.0.1:{port}/")), None)
            except OSError:
                pass
            if target is None:
                time.sleep(0.2)
        assert target, "Chrome never offered the test page"
        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        deadline = time.time() + 20
        while not dev.evaluate("typeof window.run === 'function'"):
            assert time.time() < deadline, "the test page never loaded"
            time.sleep(0.1)
        cases = ["whole", "cut", "stopped", "busy", "gone"]
        for name in cases:
            dev.evaluate(f"window.run({json.dumps(name)})")
        deadline = time.time() + 40
        while time.time() < deadline:
            runs = dev.evaluate("JSON.stringify(window.runs)")
            runs = json.loads(runs)
            if all(runs.get(name, {}).get("answer") for name in cases if name != "gone") \
                    and time.time() > min(t["at"] for t in asked.get("gone", [{"at": time.time()}])) + 4:
                break
            time.sleep(0.2)
        runs = json.loads(dev.evaluate("JSON.stringify(window.runs)"))
        tries = {name: asked.get(name, []) for name in cases}
        report = {name: {"answer": runs[name]["answer"], "tries": len(tries[name]),
                         "told": runs[name]["told"]} for name in cases}

        # Whole: one download, which the page hears of byte by byte and then
        # as done, and the engine answers.
        whole = report["whole"]
        assert whole["answer"] and whole["answer"]["ok"], report
        assert whole["tries"] == 1, report
        assert any(t.get("bytes") for t in whole["told"]), report
        assert whole["told"][-1] == {"attempt": 0, "done": True}, report

        # Broken on the way, in three ways: asked for again, and answered.
        for name, why in (("cut", None), ("stopped", "the engine's download stopped"), ("busy", "could not load CIVVIS (503)")):
            got = report[name]
            assert got["answer"] and got["answer"]["ok"], (name, report)
            assert got["tries"] == 2, (name, report)
            retried = [t for t in got["told"] if "retrying" in t]
            assert len(retried) == 1 and retried[0]["attempt"] == 0 and retried[0]["retrying"] == 1000, (name, report)
            if why:
                assert retried[0]["why"] == why, (name, report)
            assert got["told"][-1] == {"attempt": 1, "done": True}, (name, report)
            # The second try did not take the first's copy from the cache.
            assert tries[name][1]["cache"] == "no-cache", (name, tries[name])
            assert tries[name][0]["cache"] is None, (name, tries[name])
        # A stopped download is given up at the stall limit, not before and
        # not much after: its second try comes at the limit and the pause.
        gap = tries["stopped"][1]["at"] - tries["stopped"][0]["at"]
        assert 2.3 <= gap <= 5.5, ("stopped", gap)

        # Not there: no second try, and the request fails with its reason.
        gone = report["gone"]
        assert gone["tries"] == 1, report
        assert gone["answer"] and not gone["answer"]["ok"], report
        assert gone["answer"]["error"] == "could not load CIVVIS (404)", report
        assert gone["told"] == [{"attempt": 0, "failed": "could not load CIVVIS (404)"}], report
        assert not dev.console, dev.console

        for name in cases:
            print(f"{name:<8} tries {report[name]['tries']}  answered {report[name]['answer']}")
        print("engine download: a cut, stopped or refused download is asked for again; a missing one is not")
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=10)
        except subprocess.TimeoutExpired:
            chrome.kill()
        server.shutdown()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
