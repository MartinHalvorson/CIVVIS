#!/usr/bin/env python3
"""Prove a published build actually plays, in a real browser.

A static bundle can be complete, well-formed, correctly typed and still dead:
a missing sprite atlas, a module served as the wrong MIME type, a route the
shim forgot. None of that shows up in a compile. So the gate on publishing is
this — serve the exact directory that will be deployed, open it in Chrome, and
watch the game play turns.

Two things make a browser check of CIVVIS lie if you let them:

* **Headless Chrome presents no frames, so it fires no `requestAnimationFrame`
  at all** unless something consumes them. The viewer's spectator loop *is* a
  rAF loop, so without `Page.startScreencast` and an ack for every frame the
  page simply parks and a working build reads as a hang.
* A page can boot, paint its whole chrome, and be showing nothing — every panel
  empty behind a plausible-looking screenshot. Turns advancing is the only
  evidence that the engine underneath is alive.

Dependency-free on purpose, like the rest of `tools/`: a WebSocket client small
enough to read is cheaper than a wheel to install on every machine that cuts a
build.

    ./beta/verify.py                     check ./beta/dist
    ./beta/verify.py --seconds 30        watch it for longer
"""

from __future__ import annotations

import argparse
import base64
import functools
import http.server
import json
import os
import pathlib
import queue
import shutil
import socket
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
REQUIRED = [
    "index.html",
    "beta/index.html",
    "beta/civvis.wasm",
    "beta/shim.js",
    "beta/worker.js",
    "beta/cinematic3d.js",
    "beta/build.json",
    "beta/assets/terrain-atlas.png",
    "functions/beta/_middleware.js",
]


# --------------------------------------------------------------- the server


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
    }

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt, *args):
        pass


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def serve(root: pathlib.Path, port: int) -> socketserver.TCPServer:
    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(
        ("127.0.0.1", port), functools.partial(Handler, directory=str(root))
    )
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


# ------------------------------------------------------- a WebSocket client


class WebSocket:
    """Just enough RFC 6455 to talk to Chrome: text frames, client-masked."""

    def __init__(self, url: str):
        rest = url.split("://", 1)[1]
        hostport, _, path = rest.partition("/")
        host, _, port = hostport.partition(":")
        self.sock = socket.create_connection((host, int(port or 80)), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(
            (
                f"GET /{path} HTTP/1.1\r\n"
                f"Host: {hostport}\r\n"
                "Upgrade: websocket\r\n"
                "Connection: Upgrade\r\n"
                f"Sec-WebSocket-Key: {key}\r\n"
                "Sec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        self.buffer = b""
        while b"\r\n\r\n" not in self.buffer:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise RuntimeError("the browser closed the debugging socket")
            self.buffer += chunk
        head, _, self.buffer = self.buffer.partition(b"\r\n\r\n")
        if b"101" not in head.split(b"\r\n")[0]:
            raise RuntimeError(f"the debugging socket refused the upgrade: {head!r}")

    def _need(self, count: int) -> bytes:
        while len(self.buffer) < count:
            chunk = self.sock.recv(1 << 20)
            if not chunk:
                raise RuntimeError("the browser closed the debugging socket")
            self.buffer += chunk
        held, self.buffer = self.buffer[:count], self.buffer[count:]
        return held

    def send(self, text: str) -> None:
        payload = text.encode()
        header = bytearray([0x81])
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)
        elif length < (1 << 16):
            header.append(0x80 | 126)
            header += struct.pack(">H", length)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", length)
        mask = os.urandom(4)
        header += mask
        self.sock.sendall(
            bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        )

    def recv(self) -> str:
        while True:
            first, second = self._need(2)
            opcode = first & 0x0F
            length = second & 0x7F
            if length == 126:
                (length,) = struct.unpack(">H", self._need(2))
            elif length == 127:
                (length,) = struct.unpack(">Q", self._need(8))
            payload = self._need(length)
            if opcode == 0x8:  # close
                raise RuntimeError("the browser closed the debugging socket")
            if opcode == 0x9:  # ping -> pong
                self.sock.sendall(b"\x8a\x80" + os.urandom(4))
                continue
            if opcode in (0x1, 0x2, 0x0):
                return payload.decode("utf-8", "replace")


class Devtools:
    """Request/response and events over one CDP socket."""

    def __init__(self, url: str):
        self.ws = WebSocket(url)
        self.next_id = 1
        self.answers: dict[int, queue.Queue] = {}
        self.console: list[str] = []
        self.lock = threading.Lock()
        self.alive = True
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self) -> None:
        while self.alive:
            try:
                message = json.loads(self.ws.recv())
            except Exception:
                self.alive = False
                return
            if "id" in message:
                with self.lock:
                    waiting = self.answers.pop(message["id"], None)
                if waiting:
                    waiting.put(message)
            elif message.get("method") == "Page.screencastFrame":
                # Acking is what keeps frames — and so rAF — coming.
                self.call(
                    "Page.screencastFrameAck",
                    {"sessionId": message["params"]["sessionId"]},
                    wait=False,
                )
            elif message.get("method") == "Runtime.consoleAPICalled":
                if message["params"].get("type") in ("error", "warning"):
                    parts = [
                        str(a.get("value", a.get("description", "")))
                        for a in message["params"].get("args", [])
                    ]
                    self.console.append(" ".join(parts))
            elif message.get("method") == "Runtime.exceptionThrown":
                detail = message["params"]["exceptionDetails"]
                self.console.append(
                    detail.get("exception", {}).get("description") or detail.get("text", "")
                )

    def call(self, method: str, params: dict | None = None, wait: bool = True):
        with self.lock:
            ident = self.next_id
            self.next_id += 1
            box: queue.Queue = queue.Queue()
            if wait:
                self.answers[ident] = box
        self.ws.send(json.dumps({"id": ident, "method": method, "params": params or {}}))
        if not wait:
            return None
        try:
            answer = box.get(timeout=60)
        except queue.Empty:
            raise RuntimeError(f"{method} never answered")
        if "error" in answer:
            raise RuntimeError(f"{method} failed: {answer['error']}")
        return answer.get("result", {})

    def evaluate(self, expression: str):
        result = self.call(
            "Runtime.evaluate",
            {"expression": expression, "returnByValue": True, "awaitPromise": True},
        )
        if result.get("exceptionDetails"):
            raise RuntimeError(result["exceptionDetails"].get("text", "evaluation failed"))
        return result.get("result", {}).get("value")


# ------------------------------------------------------------------- checks


def main(argv: list[str] | None = None) -> int:
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", default=str(here / "dist"))
    parser.add_argument("--seconds", type=float, default=25.0)
    parser.add_argument("--min-turns", type=int, default=3)
    parser.add_argument("--screenshot", default=str(here / "dist" / "verify.png"))
    parser.add_argument("--chrome", default=CHROME)
    args = parser.parse_args(argv)

    dist = pathlib.Path(args.dist).resolve()
    if not dist.is_dir():
        print(f"no build at {dist} — run ./beta/publish.sh first", file=sys.stderr)
        return 1

    print(f"==> checking the bundle at {dist}")
    missing = [name for name in REQUIRED if not (dist / name).exists()]
    if missing:
        for name in missing:
            print(f"    MISSING {name}", file=sys.stderr)
        return 1
    build = json.loads((dist / "beta" / "build.json").read_text())
    print(f"    {len(REQUIRED)} required files present, build {build['short']}")

    page = (dist / "beta" / "index.html").read_text(encoding="utf-8")
    for absolute in ('src="/cinematic3d.js"', '"/assets/'):
        if absolute in page:
            print(f"    the published viewer still asks for {absolute}", file=sys.stderr)
            return 1
    if '<script src="shim.js"></script>' not in page:
        print("    the published viewer does not load the shim", file=sys.stderr)
        return 1
    print("    the viewer is rewritten for /beta/ and loads the shim")

    if not pathlib.Path(args.chrome).exists():
        print(f"    no Chrome at {args.chrome}; skipping the browser check", file=sys.stderr)
        return 1

    port = free_port()
    httpd = serve(dist, port)
    profile = tempfile.mkdtemp(prefix="civvis-verify-")
    debug_port = free_port()
    url = f"http://127.0.0.1:{port}/beta/"
    print(f"==> opening {url} in headless Chrome")

    chrome = subprocess.Popen(
        [
            args.chrome,
            "--headless=new",
            f"--remote-debugging-port={debug_port}",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-gpu",
            "--window-size=1600,1000",
            "--hide-scrollbars",
            url,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    try:
        target = None
        deadline = time.time() + 30
        while time.time() < deadline and target is None:
            try:
                listing = json.loads(
                    urllib.request.urlopen(
                        f"http://127.0.0.1:{debug_port}/json/list", timeout=2
                    ).read()
                )
                target = next(
                    (t for t in listing if t.get("type") == "page" and t.get("webSocketDebuggerUrl")),
                    None,
                )
            except Exception:
                time.sleep(0.25)
        if target is None:
            print("    Chrome never offered a page to drive", file=sys.stderr)
            return 1

        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        dev.call("Page.enable")
        # Without this the page never gets an animation frame and the whole
        # spectator loop stands still.
        dev.call(
            "Page.startScreencast",
            {"format": "jpeg", "quality": 40, "maxWidth": 800, "maxHeight": 500},
        )

        print(f"==> watching for {args.seconds:.0f}s")
        booted = False
        deadline = time.time() + args.seconds
        report = {}
        last_line = 0.0
        while time.time() < deadline:
            report = dev.evaluate(
                "(() => { const r = window.__civvisBeta; return r ? "
                "{ready: r.ready, error: r.error, requests: r.requests, turns: r.turns, "
                "paints: r.paints, lastTurn: r.lastTurn} : null; })()"
            ) or {}
            if report.get("error"):
                print(f"    the engine failed: {report['error']}", file=sys.stderr)
                return 1
            if report.get("ready") and not booted:
                booted = True
                print("    the engine answered; the page is live")
            if time.time() - last_line > 4:
                last_line = time.time()
                print(
                    f"    turn {report.get('lastTurn')} "
                    f"({report.get('turns', 0)} turns, {report.get('paints', 0)} repaints, "
                    f"{report.get('requests', 0)} requests)"
                )
            if report.get("turns", 0) >= args.min_turns and report.get("lastTurn", 0) >= args.min_turns:
                break
            time.sleep(0.4)

        shot = dev.call("Page.captureScreenshot", {"format": "png"})
        pathlib.Path(args.screenshot).write_bytes(base64.b64decode(shot["data"]))

        boot_failed = dev.evaluate(
            "document.body.innerText.includes('CIVVIS boot failed')"
        )
        painted = dev.evaluate(
            # A canvas that drew nothing is uniformly transparent. Sample it
            # rather than trusting that a screenshot with chrome in it means
            # the world underneath was rendered.
            "(() => { const c = document.querySelector('canvas'); if (!c) return -1;"
            " const g = c.getContext('2d'); if (!g) return -1;"
            " const d = g.getImageData(0, 0, c.width, c.height).data;"
            " const seen = new Set(); for (let i = 0; i < d.length; i += 4 * 997)"
            "   seen.add(`${d[i]},${d[i+1]},${d[i+2]},${d[i+3]}`);"
            " return seen.size; })()"
        )

        dev.call("Page.stopScreencast", wait=False)
        dev.alive = False

        print()
        problems = []
        if not report.get("ready"):
            problems.append("the engine never answered")
        if report.get("turns", 0) < args.min_turns:
            problems.append(
                f"only {report.get('turns', 0)} turns played, wanted {args.min_turns}"
            )
        if boot_failed:
            problems.append("the page reported 'CIVVIS boot failed'")
        if isinstance(painted, int) and painted < 8:
            problems.append(f"the map canvas holds only {painted} distinct colours")
        fatal = [
            line
            for line in dev.console
            if "favicon" not in line.lower() and line.strip()
        ]
        for line in fatal[:8]:
            print(f"    console: {line[:180]}")

        print(f"    build       {build['short']}")
        print(f"    engine      {build['wasm_bytes']:,} bytes")
        print(f"    turns       {report.get('turns', 0)} (reached turn {report.get('lastTurn')})")
        print(f"    repaints    {report.get('paints', 0)}")
        print(f"    canvas      {painted} distinct sampled colours")
        print(f"    screenshot  {args.screenshot}")

        if problems:
            print()
            for problem in problems:
                print(f"FAILED: {problem}", file=sys.stderr)
            return 1
        print()
        print("this build plays. it is publishable.")
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=10)
        except subprocess.TimeoutExpired:
            chrome.kill()
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
