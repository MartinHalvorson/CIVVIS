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
    let answer;
    if (message.path === "/runtime") {
      answer = { commit: new URL(location.href).searchParams.get("build") || "test" };
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
      const requested = JSON.parse(message.body || "{}").between_game_countdown_ms;
      if ([0, 3000, 5000, 10000].includes(requested))
        window.betweenGameCountdownMs = requested;
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

    # ⚠ The failure message runs in the same change that adds it. A diagnostic
    # nobody has exercised is a guess, and this one only ever prints on a
    # machine having a bad day — the worst time to discover it is wrong.
    check_stderr_tail(stage)

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
            f"http://127.0.0.1:{port}/?slow=1",
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
        assert dev.evaluate("localStorage.getItem('civvis-world-spin')") == "1"
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
        print("the WASM finale honors selected holds and opens an installed successor build.")
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
