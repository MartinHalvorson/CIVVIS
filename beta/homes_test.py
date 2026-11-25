#!/usr/bin/env python3
"""Each world of Doctor Strange's Civ 6 on an engine of its own, in a real browser.

`beta/shim.js` and `beta/worker.js` are loaded as they ship, over the toy
engine of `forks_test.py`: a module that counts the requests it has answered
and says the count (`n`, its last digit) and the length of the request it was
sent (`l`, its last digit). A page framed by `?mode=strange` asks through its
host, as a world's page does, and the count says which engine answered.

What is checked:

  * outside Doctor Strange every world is answered by the page's one engine;
  * on it, a world's first request is the page's engine's, and from then on
    the world counts by itself on an engine of its own, as each other world
    and the page's engine do;
  * a copy of a world on another engine carries on from that world's count,
    on an engine made for it, and the world copied from is untouched;
  * a `twin` on another engine is dropped from the request, and kept where
    the two are on one engine;
  * what the engines hold is read as one, with how many there are;
  * a world whose page has gone has its engine let go;
  * the split closed onto a world makes that world's engine the page's.

Nothing here plays a game. That a world played on its own engine plays
the same turns as on the shared one is measured against a published build
in node; `docs/SIM_DEBUGGER.md` has the numbers.

Set `CIVVIS_TEST_CHROME` to run this with a particular browser binary.
"""

from __future__ import annotations

import functools
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

from forks_test import Quiet, toy_engine
from verify import Devtools, find_chrome, free_port, fresh_profile, tether

PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>homes</title></head>
<body><script src="shim.js"></script></body></html>"""

# What the checks ask with. `said` asks as the page; `world(k)` asks as the
# page framed k-th, which is a world named in its address.
HELPERS = """
window.ask = (where, path, body) => where.fetch(path, body === undefined
    ? undefined : {method: "POST", body: JSON.stringify(body)}).then((answer) => answer.json());
window.said = (path, body) => ask(window, path, body);
window.world = (name) => document.querySelector(`iframe[data-world="${name}"]`).contentWindow;
window.frame = (name) => new Promise((done) => {
  const made = document.createElement("iframe");
  made.dataset.world = name;
  made.src = `index.html?pane=${name}`;
  made.onload = () => {
    // A page's first request also sends its opening settings (`/pace`):
    // asked here, so that what the checks count is only their own.
    const ready = () => made.contentWindow.__civvisBeta && made.contentWindow.__civvisBeta.ready
      ? made.contentWindow.fetch("/runtime").then(() => done(true)) : setTimeout(ready, 20);
    ready();
  };
  document.body.appendChild(made);
});
true
"""


def opened(dev: Devtools, url: str) -> None:
    dev.call("Page.navigate", {"url": url})
    deadline = time.time() + 30
    while not dev.evaluate("!!(window.__civvisBeta && window.__civvisBeta.ready && document.readyState === 'complete')"):
        if time.time() > deadline:
            raise AssertionError("the toy engine never answered: " + json.dumps(dev.evaluate("window.__civvisBeta || null")))
        time.sleep(0.1)
    dev.evaluate(HELPERS)


def check_homes(dev: Devtools, origin: str) -> None:
    said = lambda expression: dev.evaluate(f"(async () => ({expression}))()")  # noqa: E731
    on = lambda count, by: (count + by) % 10  # noqa: E731

    # Outside Doctor Strange the worlds share the page's engine.
    opened(dev, f"{origin}/?mode=play")
    said('await frame("a-t1")')
    said('await frame("b-t1")')
    here = said('(await said("/status")).n')
    shared = said('[(await ask(world("a-t1"), "/status")).n, (await ask(world("b-t1"), "/status")).n, (await said("/status")).n]')
    assert shared == [on(here, 1), on(here, 2), on(here, 3)], (here, shared)
    assert said('window.__civvisBeta.homes || 0') == 0
    print("outside Doctor Strange every world is answered by the page's one engine.")

    # On it, each world counts by itself from its first request on.
    opened(dev, f"{origin}/?mode=strange")
    said('await frame("a-t1")')
    here = said('(await said("/status")).n')
    # The frame asked its first (`/runtime`) on the page's engine: that is
    # where its world is seated, and its engine is a fork made behind it.
    a = said('(await ask(world("a-t1"), "/status")).n')
    assert said('(await said("/status")).n') == on(here, 1), "the page's engine heard the world after its first request"
    a_next = said('(await ask(world("a-t1"), "/status")).n')
    assert a_next == on(a, 1), (a, a_next)
    said('await frame("b-t1")')
    b = said('(await ask(world("b-t1"), "/status")).n')
    both = said('(await Promise.all([ask(world("a-t1"), "/status"), ask(world("b-t1"), "/status"), said("/status")])).map((x) => x.n)')
    page = on(here, 2)  # the page's engine also seated B with B's first request
    assert both == [on(a_next, 1), on(b, 1), on(page, 1)], (a_next, b, page, both)
    assert said('window.__civvisBeta.homes') == 2
    print("on Doctor Strange each world counts by itself, on an engine of its own.")

    # A copy of A in B: B carries on from A's count; A is not asked anything.
    a_now = said('(await ask(world("a-t1"), "/status")).n')
    copied = said('(await ask(world("b-t1"), "/debug", {op: "copy", from: "a-t1"})).n')
    assert copied == on(a_now, 1), (a_now, copied)
    after = said('[(await ask(world("b-t1"), "/status")).n, (await ask(world("a-t1"), "/status")).n]')
    assert after == [on(copied, 1), on(a_now, 1)], (a_now, copied, after)
    assert said('window.__civvisBeta.homes') == 2
    print("a copy of a world on another engine carries on from that world, on an engine made for it.")

    # A twin on another engine is dropped: the request is the one without it.
    plain = said('(await ask(world("b-t1"), "/debug", {op: "step"})).l')
    twinned = said('(await ask(world("b-t1"), "/debug", {op: "step", twin: "a-t1"})).l')
    assert twinned == plain, (plain, twinned)
    print("a twin on another engine is dropped from the request.")

    # What the engines hold, as one: the page's and both worlds'.
    metrics = said('await said("/machine-metrics")')
    assert metrics.get("engine_threads") == 3, metrics
    alone = said('(await ask(world("a-t1"), "/machine-metrics")).engine_bytes')
    assert metrics["engine_bytes"] > alone, (metrics, alone)
    print("what the engines hold is read as one.")

    # A world whose page has gone has its engine let go.
    said('(document.querySelector(\'iframe[data-world="b-t1"]\').remove(), true)')
    deadline = time.time() + 15
    while said('window.__civvisBeta.homes') != 1:
        if time.time() > deadline:
            raise AssertionError(f"B's engine was kept: {said('window.__civvisBeta.homes')} engines")
        time.sleep(0.25)
    print("a world whose page has gone has its engine let go.")

    # The split closed onto A: A's engine answers, and is the page's from then on.
    a_now = said('(await ask(world("a-t1"), "/status")).n')
    kept = said('(await said("/debug", {op: "adopt", pane: "a-t1"})).n')
    assert kept == on(a_now, 1), (a_now, kept)
    assert said('(await said("/status")).n') == on(kept, 1)
    assert said('window.__civvisBeta.homes') == 0
    print("the split closed onto a world makes that world's engine the page's.")


def main() -> int:
    here = pathlib.Path(__file__).resolve().parent
    stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-homes-"))
    profile = fresh_profile("civvis-homes-profile-")
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
         "--disable-gpu", "about:blank"],
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
        dev.call("Page.enable")
        check_homes(dev, f"http://127.0.0.1:{port}")
        print("each world of Doctor Strange's Civ 6 plays on an engine of its own.")
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
