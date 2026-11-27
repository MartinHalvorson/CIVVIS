#!/usr/bin/env python3
"""Each world of Doctor Strange's CIVVIS on an engine of its own, in a real browser.

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
  * a world's scout, which the page asks for the world's future, is a fork
    of that world's engine: it carries on from the world's count on a thread
    of its own and the world hears nothing of it; a new one is forked from
    the world as it then stands, it is counted with the engines, and it is
    let go when the page says it is done or the world's page has gone;
  * Autosearch's copies of a world it names are forks of that world's
    engine, and with none named of the page's;
  * a world whose page has gone has its engine let go, and so has one whose
    page is out of sight, which is given none until it is seen again;
  * no more copies are made for Autosearch than the page has room for;
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
  made.src = `index.html?pane#${name}`;
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
    # And there are no scouts: a request that names one is the world's own.
    assert said('window.__civvisBeta.scouts === undefined') is True
    assert said('(await said("/debug?pane=a-t1&scout=new", {op: "status"})).n') == on(here, 4)
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

    # A world's scout (`?pane=<world>&scout=...`, asked by the page): a fork
    # of the world's own engine, so it carries on from the world's count,
    # and it counts on by itself while the world hears nothing of it.
    assert said('window.__civvisBeta.scouts') == 0
    a_now = said('(await ask(world("a-t1"), "/status")).n')
    scout = said('(await said("/debug?pane=a-t1&scout=new", {op: "foresee", turns: 1})).n')
    assert scout == on(a_now, 1), (a_now, scout)
    scout_next = said('(await said("/debug?pane=a-t1&scout=1", {op: "foresee", turns: 1})).n')
    assert scout_next == on(scout, 1), (scout, scout_next)
    assert said('window.__civvisBeta.scouts') == 1
    a_then = said('(await ask(world("a-t1"), "/status")).n')
    assert a_then == on(a_now, 1), (a_now, a_then)
    # Counted with the engines: the page's, both worlds' and the scout.
    assert said('(await said("/machine-metrics")).engine_threads') == 4
    # A new one is forked from the world as it stands now.
    anew = said('(await said("/debug?pane=a-t1&scout=new", {op: "foresee", turns: 1})).n')
    assert anew == on(a_then, 1), (a_then, anew)
    assert said('window.__civvisBeta.scouts') == 1
    # Asked for where there is none, one is made; done lets it go, and asks
    # no engine anything.
    b_now = said('(await ask(world("b-t1"), "/status")).n')
    assert said('(await said("/debug?pane=b-t1&scout=1", {op: "foresee", turns: 1})).n') == on(b_now, 1)
    assert said('window.__civvisBeta.scouts') == 2
    assert said('await said("/debug?pane=a-t1&scout=done", {op: "status"})') == {"error": None, "scout": "done"}
    assert said('window.__civvisBeta.scouts') == 1
    assert said('(await ask(world("a-t1"), "/status")).n') == on(a_then, 1)
    assert said('(await said("/machine-metrics")).engine_threads') == 4
    print("a world's scout is a fork of the world's own engine, counted with the engines and let go when it is done.")
    # Autosearch's copies, asked for with a world named (`POST /forks` with
    # `pane`), are forks of that world's own engine: each carries on from
    # the world's count, and the world hears nothing of them. Asked for with
    # none named, they are forks of the page's engine, as off this page.
    b_now = said('(await ask(world("b-t1"), "/status")).n')
    assert said('(await said("/forks", {count: 2, pane: "b-t1"})).forks') == 2
    copies = said('[(await said("/debug?fork=1&pane=b-t1", {op: "status"})).n, (await said("/debug?fork=2&pane=b-t1", {op: "status"})).n]')
    assert copies == [on(b_now, 1), on(b_now, 1)], (b_now, copies)
    assert said('(await ask(world("b-t1"), "/status")).n') == on(b_now, 1)
    page_now = said('(await said("/status")).n')
    assert said('(await said("/forks", {count: 1})).forks') == 1
    assert said('(await said("/debug?fork=1", {op: "status"})).n') == on(page_now, 1)
    assert said('(await said("/forks", {count: 0})).forks') == 0
    print("Autosearch's copies are of the world it names, and of the page's engine when it names none.")
    # No more copies are made than fit in the room the page gives all its
    # engines together (`tabEnginesMB`, assets/app_debugger.js), and never
    # none; the answer says how many there are.
    said('(window.tabEnginesMB = () => 0, true)')
    assert said('(await said("/forks", {count: 3})).forks') == 1
    said('(window.tabEnginesMB = () => 1e6, true)')
    assert said('(await said("/forks", {count: 3})).forks') == 3
    said('(delete window.tabEnginesMB, true)')
    assert said('(await said("/forks", {count: 0})).forks') == 0
    print("no more copies are made than the page has room for, and always one.")

    # A world's page out of sight (`strange-shut`: a box closed and kept for
    # the next branch, or a page made ahead for one) holds no world: its
    # engine and its scout are let go, and it is given none while it stays
    # out of sight. Seen again, its next request seats it once more.
    shut = 'document.querySelector(\'iframe[data-world="b-t1"]\').classList'
    engines = '[window.__civvisBeta.homes, window.__civvisBeta.scouts]'
    said('(await said("/debug?pane=b-t1&scout=1", {op: "foresee", turns: 1}), true)')
    assert said(engines) == [2, 1], said(engines)
    said(f'({shut}.add("strange-shut"), true)')
    deadline = time.time() + 15
    while said(engines) != [1, 0]:
        if time.time() > deadline:
            raise AssertionError(f"B's engines were kept out of sight: {said(engines)}")
        time.sleep(0.25)
    assert isinstance(said('(await ask(world("b-t1"), "/status")).n'), int)
    said('new Promise((done) => setTimeout(() => done(true), 300))')
    assert said(engines) == [1, 0], said(engines)
    said(f'({shut}.remove("strange-shut"), true)')
    said('(await ask(world("b-t1"), "/status")).n')
    deadline = time.time() + 15
    while said('window.__civvisBeta.homes') != 2:
        if time.time() > deadline:
            raise AssertionError(f"B, seen again, was given no engine: {said(engines)}")
        time.sleep(0.1)
    print("a world's page out of sight has its engine and its scout let go, and is given none until it is seen again.")

    # A world whose page has gone has its engine let go, and its scout.
    said('(document.querySelector(\'iframe[data-world="b-t1"]\').remove(), true)')
    deadline = time.time() + 15
    while said('[window.__civvisBeta.homes, window.__civvisBeta.scouts]') != [1, 0]:
        if time.time() > deadline:
            raise AssertionError(f"B's engines were kept: {said('[window.__civvisBeta.homes, window.__civvisBeta.scouts]')}")
        time.sleep(0.25)
    print("a world whose page has gone has its engine let go, and its scout with it.")

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
        print("each world of Doctor Strange's CIVVIS plays on an engine of its own.")
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
