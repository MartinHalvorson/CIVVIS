#!/usr/bin/env python3
"""Exercise the browser-side WASM clock against a deterministic worker."""

from __future__ import annotations

import functools
import http.server
import json
import pathlib
import re
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
window.workerBodies = [];
window.betweenGameCountdownMs = 10000;
// `?site=1` is the public site, whose viewer opens at Lightning. The real
// page sets this from its hostname in the script after the shim's.
window.CIVVIS_PUBLISHED_SITE = new URL(location.href).searchParams.has("site");
// A watched world, played the way the module plays one: a request naming the
// frame on the table plays the next, and any other is answered as it stands.
// It opens at Blitz, as the module does.
window.world = { seed: 11, turn: 1, sequence: 0, cursor: 0, paused: false, pace: 500 };
const watched = (path) => /[?&]viewer=watch/.test(path) || /[?&]have=11:/.test(path);
// `?slow=1` holds every `/runtime` reply until the test calls
// `releaseRuntime()`, so the loading notice stays up for as long as the test
// needs to attach and read it. A fixed delay here raced Chrome's startup: the
// notice came and went before DevTools attached on a busy CI runner.
window.holdRuntime = new URL(location.href).searchParams.has("slow");
window.heldRuntime = [];
window.releaseRuntime = () => {
  window.holdRuntime = false;
  window.heldRuntime.splice(0).forEach((deliver) => deliver());
};
window.Worker = class {
  constructor() { this.onmessage = null; this.onerror = null; }
  postMessage(message) {
    workerCalls.push(message.path);
    workerBodies.push([message.path, message.body]);
    let answer;
    if (message.path === "/runtime") {
      answer = { commit: new URL(location.href).searchParams.get("build") || "test" };
    } else if (message.path.startsWith("/state") && watched(message.path)) {
      const w = window.world;
      const have = (/[?&]have=([^&]*)/.exec(message.path) || [])[1];
      const stepped = have === `${w.seed}:${w.turn}:0:${w.sequence}` && !w.paused;
      if (stepped) { w.turn++; w.sequence++; w.cursor += 5; }
      answer = {
        seed: w.seed, turn: w.turn, frame_sequence: w.sequence, spectate: true,
        paused: w.paused, pace: w.pace, winner: null, finished: false,
        frame_budget_ms: 0, stepped, asked: message.path,
        ai_reasoning: { cursor: w.cursor, thoughts: [] },
      };
    } else if (message.path.startsWith("/state") && message.path.includes("seat=1")) {
      answer = { seed: 9, turn: 42, winner: null, spectate: false, difficulty: "king",
        players: [{ civ: "Rome" }] };
    } else if (message.path.startsWith("/state") && message.path.includes("have=priced")) {
      answer = { seed: 7, turn: 3, winner: null, frame_budget_ms: 700 };
    } else if (message.path.startsWith("/state")) {
      answer = {
        seed: 7, turn: 227, winner: 0, server_commit: "test",
        between_game_countdown_ms: window.betweenGameCountdownMs,
      };
    } else if (message.path === "/pace") {
      const asked = JSON.parse(message.body || "{}");
      const requested = asked.between_game_countdown_ms;
      if ([0, 3000, 5000, 10000].includes(requested))
        window.betweenGameCountdownMs = requested;
      if (typeof asked.paused === "boolean") window.world.paused = asked.paused;
      if (typeof asked.ms === "number") window.world.pace = asked.ms;
      answer = { between_game_countdown_ms: window.betweenGameCountdownMs };
    } else if (message.path === "/status") {
      answer = { seed: 7, finished: window.statusFinished !== false };
    } else if (message.path === "/next-game") {
      answer = { seed: 8, turn: 1, winner: null };
    } else if (message.path === "/save") {
      // Bigger than localStorage's whole quota, which is the point: a long
      // game must still fit. The envelope is the shape the engine answers.
      answer = { format: "civvis", game: { turn: 42, padding: "x".repeat(6 * 1024 * 1024) } };
    } else if (message.path === "/load") {
      answer = { turn: JSON.parse(message.body).game.game.turn, error: null };
    } else {
      answer = {};
    }
    const bytes = new TextEncoder().encode(JSON.stringify(answer));
    const deliver = () => setTimeout(() => this.onmessage({
      data: { id: message.id, ok: true, answer: bytes },
    }), 0);
    if (message.path === "/runtime" && window.holdRuntime) window.heldRuntime.push(deliver);
    else deliver();
  }
};
</script>
<script src="shim.js"></script></head><body></body></html>"""


#: How long Chrome is given to publish a debuggable page.
#:
#: It was 30, and `published-build` failed intermittently on a loaded hosted
#: runner with no way to tell a slow start from a dead browser. The wait costs
#: nothing when Chrome is ready in a second, which is the ordinary case, and a
#: check that fails at random is worse than a slow one: it teaches every reader
#: to ignore the colour, which is how a real failure gets through.
CHROME_READY_SECONDS = 90


def chrome_stderr_tail(path: pathlib.Path, lines: int = 12) -> str:
    """The tail of Chrome's own stderr, for a failure message.

    A browser that exited silently is a different problem from one that
    explained itself, so say which.
    """
    try:
        said = path.read_text(encoding="utf-8", errors="replace").strip()
    except OSError as why:
        return f"\nChrome's stderr could not be read: {why}"
    if not said:
        return "\nChrome wrote nothing to stderr."
    tail = "\n".join(said.splitlines()[-lines:])
    return f"\nChrome's stderr (last {lines} lines):\n{tail}"


def check_stderr_tail(stage: pathlib.Path) -> None:
    """Exercise every branch of `chrome_stderr_tail` before it is needed."""
    missing = stage / "no-such-log"
    assert "could not be read" in chrome_stderr_tail(missing), "absent log"
    empty = stage / "empty.log"
    empty.write_text("", encoding="utf-8")
    assert "wrote nothing" in chrome_stderr_tail(empty), "silent chrome"
    said = stage / "said.log"
    said.write_text("\n".join(f"line {n}" for n in range(20)), encoding="utf-8")
    tail = chrome_stderr_tail(said, lines=3)
    assert "line 19" in tail and "line 17" in tail, tail
    assert "line 16" not in tail, f"the tail is bounded: {tail}"


def check_opening_settings_match_the_viewer(here: pathlib.Path) -> None:
    """The shim sends the viewer's opening pace and hold ahead of its first read.

    They are the viewer's rules, restated in a second file, and a restatement
    drifts. If it did, nothing would break outright — the viewer would post
    its own choice on its first frame, as it always used to — so the cost
    would come back silently: two whole-state answers drawn on the first
    frame, and a first frame read ahead at the wrong pace. Hold the two files
    to the same keys and values here instead.
    """
    shim = (here / "shim.js").read_text(encoding="utf-8")
    viewer = (here.parent / "web" / "assets" / "app.js").read_text(encoding="utf-8")
    page = (here.parent / "web" / "index.html").read_text(encoding="utf-8")

    def constant(source: str, name: str) -> str:
        found = re.search(rf"const {name} = ([^;]+);", source)
        assert found, f"{name} is not declared"
        return found.group(1).strip()

    def numbers(text: str) -> set[int]:
        return {int(value) for value in re.findall(r"\d+", text)}

    def offered(select_id: str) -> set[int]:
        block = re.search(rf'<select id="{select_id}".*?</select>', page, re.S)
        assert block, f"#{select_id} is not on the page"
        return {int(value) for value in re.findall(r'<option value="(\d+)"', block.group(0))}

    assert constant(shim, "WATCH_PACE_KEY") == constant(viewer, "PACE_KEY"), "pace key"
    assert constant(shim, "FINALE_KEY") == constant(viewer, "BETWEEN_GAME_COUNTDOWN_KEY"), "hold key"
    assert constant(shim, "PUBLISHED_SITE_PACE_MS") == constant(viewer, "PUBLISHED_SITE_PACE_MS")
    assert numbers(constant(shim, "WATCH_PACES_MS")) == offered("specspeed"), "offered paces"
    assert numbers(constant(shim, "FINALE_OPTIONS_MS")) == offered("between-game-countdown")
    # The hold a full game opens on when nothing was picked by hand.
    assert "const ms = tactics ? 3_000 : 5_000;" in viewer, "the viewer's default hold moved"
    assert constant(shim, "DEFAULT_FINALE_MS") == "5000", "the shim's default hold"


def check_read_ahead(dev) -> None:
    """One frame is asked for early, the page's own next request, and never two."""
    calls = lambda: dev.evaluate("window.workerCalls")
    steps = lambda: [path for path in calls() if "have=11:" in path]
    report = lambda: dev.evaluate(
        "({hits: __civvisBeta.aheadHits, misses: __civvisBeta.aheadMisses})"
    )
    read = lambda target: dev.evaluate(f"fetch({target!r}).then(r => r.json())")
    post = lambda path, body: dev.evaluate(
        f"fetch({path!r}, {{method:'POST', body:JSON.stringify({body})}}).then(r => r.json())"
    )

    def settle(count: int) -> list[str]:
        """Wait for the engine requests that are coming, then prove no more do."""
        deadline = time.time() + 5
        while time.time() < deadline and len(steps()) < count:
            time.sleep(0.02)
        time.sleep(0.25)
        seen = steps()
        assert len(seen) == count, f"expected {count} frame requests, saw {seen}"
        return seen

    # At Blitz a frame is one seat's move, held for its share of the turn:
    # the engine is done long before the page may have it. Nothing is asked
    # for early, on the opening read or on any after it.
    dev.evaluate("window.world.pace = 500, true")
    opening = read("/state?painted=&viewer=watch&think=0")
    assert opening["turn"] == 1 and opening["pace"] == 500, opening
    settle(0)
    seat = read("/state?painted=1&viewer=watch&have=11:1:0:0&think=0")
    assert seat["turn"] == 2 and seat["stepped"] is True, seat
    settle(1)
    dev.evaluate("Object.assign(window.world, {turn: 1, sequence: 0, cursor: 0}), true")
    dev.evaluate("window.workerCalls.length = 0, true")

    # At Lightning the opening read is followed at once by the request for the
    # frame after it — the page's own next request, before the page makes it.
    dev.evaluate("window.world.pace = 0, true")
    opening = read("/state?painted=&viewer=watch&think=0")
    assert opening["turn"] == 1, opening
    assert settle(1) == ["/state?have=11:1:0:0&think=0"], steps()

    # The page asks, in the words the viewer uses, and is handed that frame.
    # The engine was not asked twice for it, and exactly one more is asked for.
    asks = "/state?painted=1&world=11&finished=0&frame=0&viewer=watch&have=11:1:0:0&think=0"
    second = read(asks)
    assert second["turn"] == 2 and second["stepped"] is True, second
    assert second["asked"] == "/state?have=11:1:0:0&think=0", second
    assert report() == {"hits": 1, "misses": 0}, report()
    assert settle(2)[1] == "/state?have=11:2:0:1&think=5", steps()

    # A read that names no frame — the globe's geometry — and the heartbeat
    # that restates the pause the page already has leave it standing.
    read("/state?planet=1&viewer=watch")
    post("/spectator-status", "{paused:false}")
    third = read("/state?painted=2&viewer=watch&have=11:2:0:1&think=5")
    assert third["turn"] == 3 and third["asked"] == "/state?have=11:2:0:1&think=5", third
    assert report() == {"hits": 2, "misses": 0}, report()
    settle(3)

    # A pause changes what a state says, so the frame asked for before it is
    # not shown after it. The page's request goes to the engine as written and
    # is answered with the world as it stands: held, and already one frame on.
    post("/pace", "{paused:true}")
    asks = "/state?painted=3&viewer=watch&have=11:3:0:2&think=10"
    held = read(asks)
    assert held["asked"] == asks and held["paused"] is True, held
    assert held["turn"] == 4 and held["stepped"] is False, held
    assert report() == {"hits": 2, "misses": 1}, report()
    # Nothing is read ahead while the world is held.
    settle(4)

    # Resumed, the page asks for the next frame and the read-ahead resumes.
    post("/pace", "{paused:false}")
    resumed = read("/state?painted=4&viewer=watch&have=11:4:0:3&think=15")
    assert resumed["turn"] == 5 and resumed["stepped"] is True, resumed
    assert settle(6)[-1] == "/state?have=11:5:0:4&think=20", steps()

    # A page that has not taken in the reasoning it was handed — the viewer
    # does that only when it repaints its side panel — names an older thought.
    # That is a miss once, and from then on the thought it names is the guess.
    # The world is already one frame past the page: the state the page holds
    # ended on the thought before this one, and the page names the one before
    # that.
    w = dev.evaluate("window.world")
    stale = w["cursor"] - 10
    asks = f"/state?painted=5&viewer=watch&have=11:5:0:4&think={stale}"
    lagging = read(asks)
    assert lagging["asked"] == asks and lagging["turn"] == 6, lagging
    assert report() == {"hits": 2, "misses": 2}, report()
    assert settle(8)[-1] == f"/state?have=11:6:0:5&think={stale}", steps()
    repeated = read(f"/state?painted=6&viewer=watch&have=11:6:0:5&think={stale}")
    assert repeated["turn"] == 7 and repeated["stepped"] is True, repeated
    assert report() == {"hits": 3, "misses": 2}, report()
    assert settle(9)[-1] == f"/state?have=11:7:0:6&think={stale}", steps()

    # A page that asks for something else is answered by the engine, not by
    # the guess: each time the request goes through as written. Three such in
    # a row and this visit stops guessing.
    for miss in range(3):
        before = len(steps())
        w = dev.evaluate("window.world")
        asks = (
            f"/state?painted=0&viewer=watch&have=11:{w['turn'] - 1}:0:{w['sequence'] - 1}"
            f"&think={w['cursor'] - 5 + 1}"
        )
        answer = read(asks)
        assert answer["asked"] == asks and answer["stepped"] is False, answer
        # The forwarded request, and — until the third miss — a fresh guess.
        settle(before + (2 if miss < 2 else 1))
    assert report() == {"hits": 3, "misses": 5}, report()
    w = dev.evaluate("window.world")
    asks = f"/state?painted=0&viewer=watch&have=11:{w['turn']}:0:{w['sequence']}&think={w['cursor']}"
    before = len(steps())
    played = read(asks)
    assert played["asked"] == asks and played["stepped"] is True, played
    settle(before + 1)

    # A page that changes its pace may ask differently from then on, so the
    # guessing starts again.
    post("/pace", "{ms:0}")
    w = dev.evaluate("window.world")
    asks = f"/state?painted=0&viewer=watch&have=11:{w['turn']}:0:{w['sequence']}&think={w['cursor']}"
    before = len(steps())
    again = read(asks)
    assert again["asked"] == asks and again["stepped"] is True, again
    settle(before + 2)

    # What the page is handed is the answer itself, read the way the viewer
    # reads it, and still text for anything that asks for text.
    shape = dev.evaluate(
        """fetch('/runtime').then(async (response) => ({
          ok: response.ok, status: response.status,
          json: (await response.json()).commit,
          text: JSON.parse(await response.clone().text()).commit,
        }))"""
    )
    assert shape == {"ok": True, "status": 200, "json": "test", "text": "test"}, shape


def opening_settings(dev) -> list[dict]:
    """Every `/pace` body the engine has been sent, in order."""
    return [
        json.loads(body)
        for path, body in dev.evaluate("window.workerBodies")
        if path == "/pace"
    ]


def wait_for_harness(dev, url_part: str) -> None:
    """Wait until the page at `url_part` has a shim that reports ready.

    A reload keeps the first page's `?slow=1`, so its `/runtime` reply is held
    again; let it go, or the reloaded shim never reports ready at all.
    """
    deadline = time.time() + 15
    while time.time() < deadline:
        try:
            if dev.evaluate(
                f"location.href.includes({url_part!r}) && !!window.__civvisBeta && "
                "(window.releaseRuntime(), window.__civvisBeta.ready)"
            ):
                return
        except Exception:
            # The execution context is replaced while the page navigates.
            pass
        time.sleep(0.1)
    raise AssertionError(f"the page at {url_part} never reported ready")


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
        '{"commit":"test","wasm_bytes":7340032,"built_at":"2026-11-10T04:00:00Z"}\n',
        encoding="utf-8"
    )

    # ⚠ The failure message runs in the same change that adds it. A diagnostic
    # nobody has exercised is a guess, and this one only ever prints on a
    # machine having a bad day — the worst time to discover it is wrong.
    check_stderr_tail(stage)
    check_opening_settings_match_the_viewer(here)

    port = free_port()
    server = socketserver.TCPServer(
        ("127.0.0.1", port), functools.partial(Quiet, directory=str(stage))
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    debug_port = free_port()
    chrome_log_path = stage / "chrome-stderr.log"
    chrome_log = chrome_log_path.open("wb")
    chrome = subprocess.Popen(
        [
            find_chrome(),
            "--headless=new",
            f"--remote-debugging-port={debug_port}",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--disable-gpu",
            f"http://127.0.0.1:{port}/?slow=1&site=1",
        ],
        stdout=subprocess.DEVNULL,
        # ⚠ NOT DEVNULL. When Chrome fails to start — a missing shared
        # library, a sandbox refusal, a port already taken — the reason it
        # printed is the only thing that says which. Throwing it away left
        # `Chrome never offered a debuggable page` as the entire diagnosis of
        # an intermittent CI failure, which is a sentence nobody can act on.
        stderr=chrome_log,
    )
    tether(chrome)

    try:
        target = None
        # What the last attempt actually saw, so a failure can say which of
        # the three shapes it was: Chrome died, the endpoint never answered,
        # or it answered with pages that were not yet debuggable.
        last_error: str | None = "the debugging endpoint never answered"
        seen: list[str] = []
        deadline = time.time() + CHROME_READY_SECONDS
        while time.time() < deadline and not target:
            # A Chrome that has already exited is never going to offer a page,
            # and waiting the whole deadline to say so hides its exit code.
            if chrome.poll() is not None:
                last_error = f"chrome exited with status {chrome.returncode}"
                break
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
                if not target:
                    seen = sorted({str(page.get("type")) for page in pages})
                    last_error = (
                        f"the endpoint answered with {len(pages)} target(s) "
                        f"({', '.join(seen) or 'none'}) and no debuggable page"
                    )
            except Exception as why:
                last_error = f"{type(why).__name__}: {why}"
            if not target:
                # Sleep on BOTH paths. The original slept only after an
                # exception, so an endpoint that answered without a page yet
                # was polled in a tight loop for the whole deadline.
                time.sleep(0.2)
        if not target:
            raise RuntimeError(
                "Chrome never offered a debuggable page within "
                f"{CHROME_READY_SECONDS}s: {last_error}."
                + chrome_stderr_tail(chrome_log_path)
            )

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
        assert dev.evaluate("localStorage.getItem('civvis-world-spin-2')") == "1"
        assert dev.evaluate("localStorage.getItem('civvis-unit-tail-turns')") == "2"
        dev.evaluate("window.releaseRuntime(), true")
        deadline = time.time() + 10
        while time.time() < deadline and not dev.evaluate(
            "window.__civvisBeta && window.__civvisBeta.ready"
        ):
            time.sleep(0.1)
        # The notice fades for 450ms after `ready`, so wait for it to go rather
        # than sleeping 500ms and trusting a loaded runner to keep that margin.
        deadline = time.time() + 10
        while time.time() < deadline and dev.evaluate(
            "!!document.getElementById('civvis-beta-loading')"
        ):
            time.sleep(0.1)
        assert not dev.evaluate(
            "!!document.getElementById('civvis-beta-loading')"
        ), "the startup notice remained after the local game was ready"

        # Nothing is sent to the engine on the page's behalf until the page
        # asks for something. Its first request is preceded by the pace and
        # hold its viewer opens on: Lightning on the public site, five seconds
        # between games, and nothing stored to say otherwise.
        assert opening_settings(dev) == [], opening_settings(dev)
        dev.evaluate("fetch('/machine-metrics').then(r => r.json())")
        assert opening_settings(dev) == [
            {"ms": 0, "between_game_countdown_ms": 5000}
        ], opening_settings(dev)
        calls = dev.evaluate("window.workerCalls")
        assert calls.index("/pace") < calls.index("/machine-metrics"), calls

        check_read_ahead(dev)

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
        # The module no longer says when it was built; the manifest does.
        assert first["server_built_at"] == "2026-11-10T04:00:00Z", first

        # Metadata fetches happen throughout a result screen. Advancing fake
        # wall time across one proves they cannot restart the finale clock.
        dev.evaluate("window.fakeNow = 2000")
        runtime = dev.evaluate("fetch('/runtime').then(r => r.json())")
        assert runtime["commit"] == "test", runtime
        assert runtime["artifact_bytes"] == 7 * 1024 * 1024, runtime
        assert runtime["artifact_kind"] == "WASM", runtime
        assert runtime["built_at"] == "2026-11-10T04:00:00Z", runtime
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

        # A result left on Pause is held however long it is left, and the
        # count starts over at its full length when the world resumes.
        post_pace = "fetch('/pace', {method:'POST', body:JSON.stringify(%s)}).then(r => r.json())"
        dev.evaluate(post_pace % "{between_game_countdown_ms:3000, paused:true}")
        dev.evaluate("fetch('/state?have=2').then(r => r.json())")
        dev.evaluate("window.fakeNow = 60000")
        held = dev.evaluate("fetch('/state?have=2').then(r => r.json())")
        calls = dev.evaluate("window.workerCalls")
        assert held["seed"] == 7 and "restart_in" not in held, held
        assert calls.count("/next-game") == 2, calls
        dev.evaluate(post_pace % "{paused:false}")
        armed = dev.evaluate("fetch('/state?have=2').then(r => r.json())")
        assert armed["restart_in"] == 3, armed

        # One more turn can land while the count's last check is out on the
        # network. The world it answers for is live again, so it stays.
        dev.evaluate("window.statusFinished = false; window.fakeNow = 64000")
        kept = dev.evaluate("fetch('/state?have=2').then(r => r.json())")
        calls = dev.evaluate("window.workerCalls")
        assert kept["seed"] == 7, kept
        assert "/status" in calls, calls
        assert calls.count("/next-game") == 2, calls
        dev.evaluate("window.statusFinished = true; window.fakeNow = 0")

        # A save from before IndexedDB moves across on first use, and the
        # listing carries what the Continue button shows without loading a
        # game. The new save is over the size localStorage could ever hold.
        saves = dev.evaluate(
            """(async () => {
              localStorage.setItem('civvis.save.legacy', JSON.stringify(
                {game: {turn: 12, difficulty: 'prince', players: [{civ: 'Mali'}]}}));
              const post = (path, body) => fetch(path, {method: 'POST', body: JSON.stringify(body)})
                .then(r => r.json());
              const moved = (await (await fetch('/saves')).json()).saves;
              await fetch('/state?seat=1');
              const written = await post('/save', {name: 'long-game'});
              const listed = (await (await fetch('/saves')).json()).saves;
              const loaded = await post('/load', {name: 'long-game'});
              const missing = await post('/load', {name: 'nothing'});
              return {moved, written, listed, loaded, missing,
                legacyLeft: localStorage.getItem('civvis.save.legacy')};
            })()"""
        )
        assert saves["legacyLeft"] is None, saves["legacyLeft"]
        moved = {save["name"]: save for save in saves["moved"]}
        assert moved["legacy"]["turn"] == 12 and moved["legacy"]["civ"] == "Mali", saves["moved"]
        assert saves["written"]["error"] is None, saves["written"]
        listed = {save["name"]: save for save in saves["listed"]}
        assert listed["long-game"]["civ"] == "Rome", saves["listed"]
        assert listed["long-game"]["turn"] == 42, saves["listed"]
        assert listed["long-game"]["bytes"] > 6 * 1024 * 1024, saves["listed"]
        assert saves["loaded"]["turn"] == 42 and saves["loaded"]["error"] is None, saves["loaded"]
        assert "no save called" in saves["missing"]["error"], saves["missing"]

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

        # A pace picked on an earlier visit is the one sent, and sent once.
        wait_for_harness(dev, "build=fresh")
        dev.evaluate("localStorage.setItem('civvis-watch-pace', '1000'), true")
        dev.call("Page.navigate", {"url": f"http://127.0.0.1:{port}/?site=1&visit=2"})
        wait_for_harness(dev, "visit=2")
        dev.evaluate("fetch('/machine-metrics').then(r => r.json())")
        dev.evaluate("fetch('/machine-metrics').then(r => r.json())")
        assert opening_settings(dev) == [
            {"ms": 1000, "between_game_countdown_ms": 5000}
        ], opening_settings(dev)

        # A link that configured its own world keeps the module's pace — the
        # viewer decides that one when it sees which kind of world arrived —
        # and `?ahead=0` asks for no frame early even at Lightning.
        dev.evaluate("localStorage.removeItem('civvis-watch-pace'), true")
        dev.call(
            "Page.navigate",
            {"url": f"http://127.0.0.1:{port}/?site=1&players=2&ahead=0&visit=3"},
        )
        wait_for_harness(dev, "visit=3")
        dev.evaluate("window.world.pace = 0, true")
        opening = dev.evaluate(
            "fetch('/state?painted=&viewer=watch&think=0').then(r => r.json())"
        )
        assert opening["turn"] == 1, opening
        assert opening_settings(dev) == [{"between_game_countdown_ms": 5000}], opening_settings(dev)
        time.sleep(0.3)
        calls = dev.evaluate("window.workerCalls")
        assert calls[0] == "/new", calls
        assert not [path for path in calls if "have=11:" in path], calls
        print(
            "the WASM clock reads one frame ahead and never two, sends the viewer's "
            "opening settings first, honors selected holds, and opens an installed "
            "successor build."
        )
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=5)
        except subprocess.TimeoutExpired:
            chrome.kill()
        # After the wait, so the tail read above sees everything Chrome wrote.
        chrome_log.close()
        server.shutdown()
        server.server_close()
        shutil.rmtree(stage, ignore_errors=True)
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
