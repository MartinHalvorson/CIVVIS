#!/usr/bin/env python3
"""Fork an engine to parallel workers in a real browser, against a toy engine.

`beta/shim.js` and `beta/worker.js` are loaded as they ship. The engine is a
toy: a module assembled below that imports nothing and keeps all it knows in
its own memory, which is the one property a fork rests on. It counts the
requests it has answered, says the count (`n`) and the length of the request
it was sent (`l`), and grows its memory a page with every answer, so a fork
has to be given more memory than a new instance starts with.

What is checked is that a fork is the engine again and then its own:

  * a fork carries on from the count its engine had reached, and each fork
    and the engine count separately from there;
  * a request that names a fork reaches it without the name on it;
  * forking again starts every fork over from where the engine is now;
  * a count of 0 lets the forks go, and a fork that is not there says so.

Nothing here plays a game. That the real engine forks exactly (the same game
after the same turns, and the same turns to walk back along) is measured
against a published build in node; `docs/SIM_DEBUGGER.md` has the numbers.

Set `CIVVIS_TEST_CHROME` to run this with a particular browser binary.
"""

from __future__ import annotations

import functools
import http.server
import json
import os
import pathlib
import shutil
import socketserver
import subprocess
import tempfile
import threading
import time
import urllib.request

from verify import Devtools, find_chrome, free_port, fresh_profile, tether

# Where the toy keeps things in its memory.
COUNT = 0  # how many requests it has answered
ANSWER = 16  # the answer: a u32 length, then the text
NOTHING = 48  # an empty answer, for `civvis_last_panic`
REQUEST = 1024  # where a request is written
TEXT = b'{"n":0,"l":0}'
N_AT = ANSWER + 4 + TEXT.index(b"0")
L_AT = ANSWER + 4 + TEXT.rindex(b"0")


def leb(value: int) -> bytes:
    """An unsigned LEB128 number."""
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def sleb(value: int) -> bytes:
    """A signed LEB128 number, as `i32.const` takes it."""
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        done = (value == 0 and not byte & 0x40) or (value == -1 and byte & 0x40)
        out.append(byte | (0 if done else 0x80))
        if done:
            return bytes(out)


def toy_engine() -> bytes:
    """The toy module: the engine's five exports over one counter."""
    const = lambda value: b"\x41" + sleb(value)  # noqa: E731
    load, store, store8 = b"\x28\x02\x00", b"\x36\x02\x00", b"\x3a\x00\x00"
    add, rem_u, drop, end = b"\x6a", b"\x70", b"\x1a", b"\x0b"
    digit = const(10) + rem_u + const(48) + add
    bodies = [
        # civvis_alloc(len) -> ptr
        const(REQUEST) + end,
        # civvis_request(ptr, len) -> ptr
        const(COUNT) + const(COUNT) + load + const(1) + add + store
        + const(N_AT) + const(COUNT) + load + digit + store8
        + const(L_AT) + b"\x20\x01" + digit + store8
        + const(1) + b"\x40\x00" + drop
        + const(ANSWER) + end,
        # civvis_free(ptr, len)
        end,
        # civvis_last_panic() -> ptr
        const(NOTHING) + end,
    ]
    section = lambda number, body: bytes([number]) + leb(len(body)) + body  # noqa: E731
    vector = lambda items: leb(len(items)) + b"".join(items)  # noqa: E731
    name = lambda text: leb(len(text)) + text.encode()  # noqa: E731
    i32 = b"\x7f"
    types = vector([
        b"\x60" + vector([i32]) + vector([i32]),
        b"\x60" + vector([i32, i32]) + vector([i32]),
        b"\x60" + vector([i32, i32]) + vector([]),
        b"\x60" + vector([]) + vector([i32]),
    ])
    exports = vector([
        name("memory") + b"\x02\x00",
        name("civvis_alloc") + b"\x00\x00",
        name("civvis_request") + b"\x00\x01",
        name("civvis_free") + b"\x00\x02",
        name("civvis_last_panic") + b"\x00\x03",
    ])
    held = len(TEXT).to_bytes(4, "little") + TEXT
    return (
        b"\x00asm\x01\x00\x00\x00"
        + section(1, types)
        + section(3, vector([b"\x00", b"\x01", b"\x02", b"\x03"]))
        + section(5, vector([b"\x00\x01"]))
        + section(7, exports)
        + section(10, vector([leb(len(body) + 1) + b"\x00" + body for body in bodies]))
        + section(11, vector([b"\x00" + const(ANSWER) + b"\x0b" + leb(len(held)) + held]))
    )


PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>forks</title></head>
<body><script src="shim.js"></script></body></html>"""

# What the checks ask with: one request, answered with what the engine said.
HELPERS = """
window.said = (path, body) => fetch(path, body === undefined
    ? undefined : {method: "POST", body: JSON.stringify(body)}).then((answer) => answer.json());
true
"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm"}

    def log_message(self, fmt, *args):
        pass


def check_forks(dev: Devtools) -> None:
    said = lambda expression: dev.evaluate(f"(async () => ({expression}))()")  # noqa: E731
    ahead = lambda count, by: (count + by) % 10  # noqa: E731

    here = said('(await said("/status")).n')
    made = said('await said("/forks", {count: 2})')
    assert made.get("forks") == 2, made
    # The toy grew a page with every answer, so this is more than it began with.
    assert made["memory_bytes"] > 65536 and made["memory_bytes"] % 65536 == 0, made

    # Each fork carries on from the engine's count, and from there by itself.
    first = said('[(await said("/status?fork=1")).n, (await said("/status?fork=1")).n]')
    assert first == [ahead(here, 1), ahead(here, 2)], (here, first)
    second = said('(await said("/status?fork=2")).n')
    assert second == ahead(here, 1), (here, second)
    # The engine never heard of any of it.
    after = said('(await said("/status")).n')
    assert after == ahead(here, 1), (here, after)
    print("a fork carries on from where its engine was, and each plays its own history.")

    # The fork is sent the request the engine would have been, without the name.
    plain = said('(await said("/status?x=1")).l')
    named = said('[(await said("/status?x=1&fork=1")).l, (await said("/status?fork=2&x=1")).l]')
    assert named == [plain, plain], (plain, named)
    print("a request that names a fork reaches it without the name.")

    # Both forks at once: two threads, two answers, neither waits on the other's count.
    here = said('(await said("/status")).n')
    assert said('(await said("/forks", {count: 3})).forks') == 3
    together = said('(await Promise.all([1, 2, 3].map((k) => said("/status?fork=" + k)))).map((a) => a.n)')
    assert together == [ahead(here, 1)] * 3, (here, together)
    print("forking again starts every fork over from where the engine is now.")

    assert said('await said("/forks", {count: 0})') == {"forks": 0}
    gone = said('await said("/status?fork=1")')
    assert "no fork 1" in str(gone.get("error")), gone
    many = said('await said("/forks", {count: 17})')
    assert "0 to 16" in str(many.get("error")), many
    never = said('await said("/status?fork=nine")')
    assert "no fork" in str(never.get("error")), never
    # And the engine is still the engine.
    assert said('(await said("/status")).n') == ahead(here, 1)
    print("a count of 0 lets the forks go, and a fork that is not there says so.")


def main() -> int:
    here = pathlib.Path(__file__).resolve().parent
    stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-forks-"))
    profile = fresh_profile("civvis-forks-profile-")
    for script in ("shim.js", "worker.js"):
        shutil.copy(here / script, stage / script)
    (stage / "civvis.wasm").write_bytes(toy_engine())
    (stage / "build.json").write_text('{"commit":"test"}', encoding="utf-8")
    (stage / "index.html").write_text(PAGE, encoding="utf-8")
    port = free_port()
    server = socketserver.TCPServer(("127.0.0.1", port), functools.partial(Quiet, directory=str(stage)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    debug_port = free_port()
    chrome_log_path = stage / "chrome-stderr.log"
    chrome_log = chrome_log_path.open("wb")
    chrome = subprocess.Popen(
        [os.environ.get("CIVVIS_TEST_CHROME") or find_chrome(), "--headless=new",
         f"--remote-debugging-port={debug_port}", f"--user-data-dir={profile}", "--no-first-run",
         "--disable-gpu", f"http://127.0.0.1:{port}/"],
        stdout=subprocess.DEVNULL, stderr=chrome_log)
    tether(chrome)
    try:
        target = None
        deadline = time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{debug_port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")), None)
            except OSError:
                pass
            if target is None:
                time.sleep(0.2)
        if target is None:
            tail = chrome_log_path.read_bytes()[-1500:].decode("utf-8", "replace")
            raise AssertionError(f"Chrome never offered a debuggable page within 60s.\n{tail}")
        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        deadline = time.time() + 30
        while not dev.evaluate("!!(window.__civvisBeta && window.__civvisBeta.ready)"):
            if time.time() > deadline:
                raise AssertionError(
                    "the toy engine never answered: " + json.dumps(dev.evaluate("window.__civvisBeta || null")))
            time.sleep(0.1)
        dev.evaluate(HELPERS)
        check_forks(dev)
        print("the engine forks to parallel workers.")
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=5)
        except subprocess.TimeoutExpired:
            chrome.kill()
        chrome_log.close()
        server.shutdown()
        server.server_close()
        shutil.rmtree(stage, ignore_errors=True)
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
