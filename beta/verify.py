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
import hashlib
import http.server
import json
import os
import pathlib
import queue
import re
import shutil
import socket
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request

# Where Chrome is, on whichever machine is cutting the build.
#
# Builds get published from a Mac and checked from the Windows box that runs
# the spectator, so a single hard-coded path means the check simply does not
# run on one of them — and a check that silently does not run is worse than
# no check.
CHROME_CANDIDATES = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    os.path.expandvars(r"%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe"),
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
]


def find_chrome() -> str:
    for candidate in CHROME_CANDIDATES:
        if candidate and pathlib.Path(candidate).exists():
            return candidate
    for name in ("google-chrome", "chromium", "chrome"):
        found = shutil.which(name)
        if found:
            return found
    return CHROME_CANDIDATES[0]


# Every cleanup this file promises happens in a ``finally``, and a ``finally``
# is a promise the operating system never co-signed: kill the checker — a
# harness timeout, a task kill, a closed session — and the headless Chrome it
# spawned keeps pumping animation frames forever. Twenty-six of them were once
# found on the Windows box sharing fifteen cores between them. A kill-on-close
# job object makes the OS itself the janitor: the browser's lifetime is tied
# to ours however we end, teardown code or none.
_JOBS: list = []  # job handles must outlive us; closing one is the kill


def tether(child: subprocess.Popen) -> None:
    """Make Windows kill *child* and its whole tree the moment we die."""
    if sys.platform != "win32":
        return
    import ctypes

    class Basic(ctypes.Structure):
        _fields_ = [
            ("PerProcessUserTimeLimit", ctypes.c_int64),
            ("PerJobUserTimeLimit", ctypes.c_int64),
            ("LimitFlags", ctypes.c_uint32),
            ("MinimumWorkingSetSize", ctypes.c_size_t),
            ("MaximumWorkingSetSize", ctypes.c_size_t),
            ("ActiveProcessLimit", ctypes.c_uint32),
            ("Affinity", ctypes.c_size_t),
            ("PriorityClass", ctypes.c_uint32),
            ("SchedulingClass", ctypes.c_uint32),
        ]

    class Io(ctypes.Structure):
        _fields_ = [(field, ctypes.c_uint64) for field in (
            "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
            "ReadTransferCount", "WriteTransferCount", "OtherTransferCount",
        )]

    class Extended(ctypes.Structure):
        _fields_ = [
            ("BasicLimitInformation", Basic),
            ("IoInfo", Io),
            ("ProcessMemoryLimit", ctypes.c_size_t),
            ("JobMemoryLimit", ctypes.c_size_t),
            ("PeakProcessMemoryUsed", ctypes.c_size_t),
            ("PeakJobMemoryUsed", ctypes.c_size_t),
        ]

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    job = kernel32.CreateJobObjectW(None, None)
    if not job:
        return
    limits = Extended()
    limits.BasicLimitInformation.LimitFlags = 0x2000  # KILL_ON_JOB_CLOSE
    assigned = kernel32.SetInformationJobObject(
        job, 9, ctypes.byref(limits), ctypes.sizeof(limits)
    ) and kernel32.AssignProcessToJobObject(job, int(child._handle))
    if assigned:
        _JOBS.append(job)
    else:
        kernel32.CloseHandle(job)


def fresh_profile(prefix: str) -> str:
    """A throwaway browser profile, minus the wreckage of dead runs.

    The ``shutil.rmtree`` in the ``finally`` has the same blind spot as the
    ``terminate`` beside it: a hard-killed run leaves its profile behind, a
    few hundred megabytes each. Anything under this prefix untouched for a
    day belongs to no live check — with ``tether`` its Chrome is already
    gone — so each run starts by burying the previous casualties.
    """
    temp = pathlib.Path(tempfile.gettempdir())
    cutoff = time.time() - 24 * 3600
    for stale in temp.glob(prefix + "*"):
        try:
            if stale.is_dir() and stale.stat().st_mtime < cutoff:
                shutil.rmtree(stale, ignore_errors=True)
        except OSError:
            pass
    return tempfile.mkdtemp(prefix=prefix)


# The site carries the same viewer twice: the promoted stable build at the
# root and the automatic head build under /test. Each lane is checked
# separately (`--mount root` / `--mount test`) because they are different
# engines from different commits, and a deploy that plays in one lane and not
# the other is exactly the mistake worth catching. The lane-independent pages
# are asserted both times; that costs nothing and keeps either invocation
# sufficient to reject a bundle with no landing page.
#
# The atlases are republished as WebP when Pillow is available and stay PNG
# when it is not, so the required-files list asks for a strategic map atlas in
# whichever form this build actually chose.
def required(mount: str) -> list:
    lane = "" if mount == "root" else "test/"
    return [
        "home/index.html" if mount == "root" else "index.html",
        "home/assets/civ-world.jpg",
        "home/assets/tactics-frontline.jpg",
        "download/index.html",
        f"{lane}index.html",
        f"{lane}civvis.wasm",
        f"{lane}shim.js",
        f"{lane}worker.js",
        f"{lane}build.json",
        (f"{lane}assets/feature-atlas.webp", f"{lane}assets/feature-atlas.png"),
        "_worker.js",
        # Its existence is behaviour: without it, Pages answers unknown paths
        # with the front page instead of a 404.
        "404.html",
    ]


# --------------------------------------------------------------- the server


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".json": "application/json",
        ".webp": "image/webp",
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


# -------------------------------------------------------------- the password


def check_gate(dist: pathlib.Path) -> list[str]:
    """Prove the published directory routes the way it is meant to.

    Everything the site does before a file is served lives in `_worker.js`, and
    that file has already been lost once, silently: written as a Pages
    `functions/` directory it was resolved against the *working directory*
    rather than the directory being deployed, so a deploy from one level up
    uploaded a site with no routing at all. Nothing failed. So the routing is
    asked rather than assumed, against the real runtime under wrangler.

    What it asserts changed when the test lane was opened. It used to prove the
    door was shut; it now proves it is *open*, which is the property that
    matters when a channel is pointing people at it — a stray `BETA_PASSWORD`
    left in the Pages environment would be just as invisible, and just as
    wrong, as the missing door was. `beta/worker_test.py` covers the same
    ground plus the password path without needing Node.
    """
    port = free_port()
    log = tempfile.NamedTemporaryFile(prefix="wrangler-", suffix=".log", delete=False)
    dev = subprocess.Popen(
        [
            "npx", "--yes", "wrangler", "pages", "dev", str(dist),
            "--port", str(port), "--compatibility-date=2026-01-01",
        ],
        stdout=log, stderr=subprocess.STDOUT,
    )
    tether(dev)
    try:
        deadline = time.time() + 120
        ready = False
        while time.time() < deadline and not ready:
            if dev.poll() is not None:
                return [f"wrangler exited before serving anything (see {log.name})"]
            try:
                # `/home`, not `/`: the root is a redirect to YouTube, and
                # `urlopen` follows redirects, so probing it would make this
                # loop's readiness depend on reaching youtube.com.
                urllib.request.urlopen(f"http://127.0.0.1:{port}/home", timeout=2).read()
                ready = True
            except Exception:
                time.sleep(1)
        if not ready:
            return [f"wrangler never came up (see {log.name})"]

        problems: list[str] = []
        base = f"http://127.0.0.1:{port}"

        def get(path, cookie=None):
            request = urllib.request.Request(base + path)
            if cookie:
                request.add_header("Cookie", cookie)
            return urllib.request.urlopen(request, timeout=15)

        # The root is the stable simulator. A `ROOT_REDIRECT` left set in the
        # Pages environment would quietly turn the product back into a
        # forward, and nothing else would look wrong.
        root = get("/").read()
        if b"shim.js" not in root:
            problems.append("/ is not serving the stable viewer; is ROOT_REDIRECT set?")

        # The landing page moved to /home when the root became the product.
        # Its small, fixed utility nav is a more durable routing marker than
        # the old watch-button label, which is a deliberate part of the page's
        # editorial design rather than a route contract.
        landing = get("/home/").read()
        if b'aria-label="CIVVIS links"' not in landing:
            problems.append("the landing page is not being served at /home")

        # So is the downloads page.
        if b"releases/latest/download/" not in get("/download/").read():
            problems.append("/download/ is not serving the downloads page")

        # And so is the test lane. A `TEST_PASSWORD` left in the Pages environment
        # would put a door back in front of everyone the channel sends here,
        # and would look exactly like a working site from every other angle.
        head_lane = get("/test/").read()
        if b"Test build" in head_lane and b"shim.js" not in head_lane:
            problems.append("/test/ is asking for a password; is TEST_PASSWORD set?")
        elif b"shim.js" not in head_lane:
            problems.append("/test/ is not serving the viewer")

        # /beta — the lane's day-one name — is scrapped, and dead paths in
        # general answer 404. The second probe is what proves 404.html is
        # doing its job: without it, Pages serves the front page for any
        # unknown HTML path (its single-page-app fallback), which is how a
        # "scrapped" URL came back from the dead once already.
        for dead in ("/beta/", "/no-such-page"):
            try:
                get(dead)
                problems.append(f"{dead} still answers; it is meant to 404")
            except urllib.error.HTTPError as gone:
                if gone.code != 404:
                    problems.append(f"{dead} answers HTTP {gone.code} rather than 404")

        # Both engines reach their pages as modules, not as HTML apologies.
        for lane in ("/civvis.wasm", "/test/civvis.wasm"):
            module = get(lane)
            if module.read(4) != b"\x00asm":
                problems.append(f"{lane} is not being served as a WebAssembly module")
            if module.headers.get("Content-Type") != "application/wasm":
                problems.append(
                    f"{lane} is served as "
                    f"{module.headers.get('Content-Type')!r}; instantiateStreaming refuses it"
                )
        return problems
    finally:
        dev.terminate()
        try:
            dev.wait(timeout=15)
        except subprocess.TimeoutExpired:
            dev.kill()


# ------------------------------------------------------------------- checks


def main(argv: list[str] | None = None) -> int:
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", default=str(here / "dist"))
    parser.add_argument(
        "--mount",
        choices=("test", "root"),
        default="test",
        help="which lane to open in the browser: 'test' (default, and the shape "
             "publish.sh emits on its own) or 'root' (the promoted stable lane "
             "of an assembled two-lane site)",
    )
    parser.add_argument("--seconds", type=float, default=25.0)
    parser.add_argument("--min-turns", type=int, default=3)
    # Deliberately beside the bundle, not inside it: anything written into
    # dist/ is deployed, and a check's own screenshot has no business on
    # civvis.ai. Named per lane so checking both does not overwrite the
    # evidence of the first.
    parser.add_argument("--screenshot", default=None)
    parser.add_argument("--chrome", default=find_chrome())
    parser.add_argument(
        "--no-gate",
        action="store_true",
        help="skip the routing check (it needs npx wrangler, and takes half a minute; "
             "beta/worker_test.py covers the same ground with only Chrome)",
    )
    args = parser.parse_args(argv)
    lane = "" if args.mount == "root" else "test/"
    if args.screenshot is None:
        args.screenshot = str(here / f"verify-{args.mount}.png")

    dist = pathlib.Path(args.dist).resolve()
    if not dist.is_dir():
        print(f"no build at {dist} — run ./beta/publish.sh first", file=sys.stderr)
        return 1

    needed = required(args.mount)
    print(f"==> checking the {args.mount} lane of the bundle at {dist}")
    missing = [
        entry
        for entry in needed
        if not any((dist / name).exists() for name in ((entry,) if isinstance(entry, str) else entry))
    ]
    if missing:
        for entry in missing:
            print(f"    MISSING {entry if isinstance(entry, str) else ' or '.join(entry)}", file=sys.stderr)
        return 1
    build = json.loads((dist / lane / "build.json").read_text())
    print(f"    {len(needed)} required files present, build {build['short']}")

    page = (dist / lane / "index.html").read_text(encoding="utf-8")
    for absolute in ('"/assets/',):
        if absolute in page:
            print(f"    the published viewer still asks for {absolute}", file=sys.stderr)
            return 1
    def digest(relative: str) -> str:
        return hashlib.sha256((dist / lane / relative).read_bytes()).hexdigest()

    def check_version(text: str, pattern: str, expected: str, label: str) -> bool:
        match = re.search(pattern, text)
        if not match:
            print(f"    the published viewer has no content-versioned {label}", file=sys.stderr)
            return False
        if match.group(1) != expected:
            print(
                f"    the published viewer gives {label} version {match.group(1)}, "
                f"but its bytes are {expected}",
                file=sys.stderr,
            )
            return False
        return True

    wasm_version = digest("civvis.wasm")
    shim_version = digest("shim.js")
    if not check_version(
        page,
        r'<link rel="preload" href="civvis\.wasm\?v=([0-9a-f]{64})" as="fetch" '
        r'type="application/wasm" crossorigin fetchpriority="high">',
        wasm_version,
        "WASM preload",
    ):
        return 1
    if not check_version(
        page,
        r'<script src="shim\.js\?v=([0-9a-f]{64})"></script>',
        shim_version,
        "shim script",
    ):
        return 1

    shim = (dist / lane / "shim.js").read_text(encoding="utf-8")
    if not check_version(
        shim,
        r'new URL\("civvis\.wasm\?v=([0-9a-f]{64})", here\)\.href',
        wasm_version,
        "WASM URL in shim.js",
    ):
        return 1
    if not check_version(
        shim,
        r'new URL\("worker\.js\?v=([0-9a-f]{64})", here\)\.href',
        digest("worker.js"),
        "worker URL in shim.js",
    ):
        return 1

    asset_urls = re.findall(r'''["'](assets/[^"']+)["']''', page)
    if not asset_urls:
        print("    the published viewer contains no atlas references", file=sys.stderr)
        return 1
    for asset_url in asset_urls:
        match = re.fullmatch(r"(assets/[^?]+)\?v=([0-9a-f]{64})", asset_url)
        if not match or match.group(2) != digest(match.group(1)):
            print(
                f"    the published viewer has an unversioned or mismatched atlas: {asset_url}",
                file=sys.stderr,
            )
            return 1

    print(
        f"    the viewer is relative-pathed for /{lane}; JS, WASM, and atlases are content-versioned"
    )

    # The download page names its binaries; the release workflow builds
    # binaries under names it chooses. Nothing connects the two but agreement,
    # and a disagreement is a 404 on the only page whose whole job is handing
    # somebody a program. Renaming an asset must therefore fail here rather
    # than on a visitor's click.
    downloads = (dist / "download" / "index.html").read_text(encoding="utf-8")
    linked = set(re.findall(r"releases/latest/download/([^\"']+)", downloads))
    workflow = here.parent / ".github" / "workflows" / "release.yml"
    if not linked:
        print("    the download page links no release assets at all", file=sys.stderr)
        return 1
    if workflow.exists():
        built = set(re.findall(r"^\s*asset:\s*(\S+)\s*$", workflow.read_text(encoding="utf-8"), re.M))
        orphans = sorted(linked - built)
        if orphans:
            for name in orphans:
                print(f"    the download page links {name}, which no release job builds", file=sys.stderr)
            return 1
        print(f"    {len(linked)} downloads, every one of them built by release.yml")
    else:
        print(f"    {len(linked)} downloads linked (no release.yml here to check them against)")

    if not args.no_gate:
        print("==> routing the exact directory to be deployed, under wrangler")
        gate_problems = check_gate(dist)
        for problem in gate_problems:
            print(f"    {problem}", file=sys.stderr)
        if gate_problems:
            print("\nFAILED: the site would not route correctly", file=sys.stderr)
            return 1
        print("    / is the stable lane; /home, /download and /test all answer")

    if not pathlib.Path(args.chrome).exists():
        print(f"    no Chrome at {args.chrome}; skipping the browser check", file=sys.stderr)
        return 1

    port = free_port()
    httpd = serve(dist, port)
    profile = fresh_profile("civvis-verify-")
    debug_port = free_port()
    url = f"http://127.0.0.1:{port}/{lane}"
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
    tether(chrome)

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
        build_marker = dev.evaluate(
            "document.getElementById('buildmark')?.textContent || ''"
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
        # The test lane is built from current main, but the root lane is
        # intentionally pinned to the last promoted revision.  publish.sh
        # supplies the current manifest to both lanes, so an old pinned
        # viewer can have ``wasm_bytes`` in build.json without knowing how to
        # render the new marker field.  Gate the new assertion on the viewer
        # contract, rather than on the publisher's manifest alone.  Since
        # #1289 that contract lives in ``assets/app.js``, not the page —
        # read both, or a current lane reads as a legacy viewer and the
        # size assertion silently skips.
        app_js = dist / lane / "assets" / "app.js"
        viewer_source = page + (
            app_js.read_text(encoding="utf-8") if app_js.is_file() else ""
        )
        wasm_bytes = build.get("wasm_bytes")
        marker_has_build_size = all(
            token in viewer_source
            for token in ("formatBuildSize", "server_artifact_bytes", "Build size")
        )
        if marker_has_build_size:
            if not isinstance(wasm_bytes, int) or isinstance(wasm_bytes, bool) or wasm_bytes <= 0:
                problems.append(
                    f"the build manifest has no positive integer wasm_bytes for its new marker: "
                    f"{wasm_bytes!r}"
                )
            else:
                # A pinned root lane can still carry the MiB formatter;
                # expect whichever units this viewer's own source renders.
                if "(bytes / 1e6).toFixed(1)} MB" in viewer_source:
                    expected_size = f"Build size {wasm_bytes / 1e6:.1f} MB"
                else:
                    expected_size = f"Build size {wasm_bytes / 1048576:.1f} MiB"
                if expected_size not in build_marker:
                    problems.append(
                        f"the build marker does not show {expected_size!r}: {build_marker!r}"
                    )
        else:
            print(
                "    pinned legacy viewer has no build-size marker; "
                "accepting its commit/age contract"
            )
        order = [
            build_marker.find(build["short"][:7]),
            build_marker.find("Commit is"),
            build_marker.find("Build is"),
        ]
        if marker_has_build_size:
            order.append(build_marker.find("Build size"))
        order_contract = "commit, commit age, build age"
        if marker_has_build_size:
            order_contract += ", build size"
        if any(position < 0 for position in order) or order != sorted(order):
            problems.append(
                f"the build marker order is not {order_contract}: {build_marker!r}"
            )
        fatal = [
            line
            for line in dev.console
            if "favicon" not in line.lower() and line.strip()
        ]
        for line in fatal[:8]:
            print(f"    console: {line[:180]}")

        print(f"    build       {build['short']}")
        if isinstance(wasm_bytes, int) and not isinstance(wasm_bytes, bool):
            print(f"    engine      {wasm_bytes:,} bytes")
        else:
            print("    engine      size not recorded")
        print(f"    marker      {build_marker}")
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
