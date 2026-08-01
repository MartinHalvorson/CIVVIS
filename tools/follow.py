#!/usr/bin/env python3
"""Keep a CIVVIS window showing the Civilization VI game that is actually being played.

`civvis play --mirror <run-dir>` rebuilds the real board as a CIVVIS game, but it
reads `events.jsonl` exactly once at startup — so the window it opens is a
photograph, and ten minutes later it is a photograph of a game that has moved on.
Side by side with the live Civ 6 window that is worse than no mirror: the operator
is asked to compare two screens and shown one that cannot agree by construction.

This closes that gap without restarting anything the operator is looking at:

    throwaway `civvis play --mirror` on an ephemeral port   (fresh read of events)
      -> GET  /save   the rebuilt Game
      -> POST /load   into the server the browser is already attached to

`/load` swaps the session's game inside the running process, so `server_instance`
never changes and the page updates in place. Restarting the mirror server instead
would work, but this build reports `server_commit: "unknown"`, so the viewer's
adopt-a-successor path correctly refuses and every turn would cost a full ~3 MB
page reload — which is precisely the "loading screen" the exhibition viewer was
fixed to stop doing (see civvis-viewer-follows-the-server).

Two things it must survive on its own, because both have happened:
  * runs rotate. `civ6_civvis_climb.py` starts a fresh run dir per attempt, so
    following one fixed path means the window silently freezes on a finished game
    while a new one plays. The newest run with a recently-written events.jsonl
    wins, and a switch is logged.
  * the visible server dying leaves the tab pointing at nothing. It is restarted
    here rather than by a second daemon.

Port 8610 is deliberate: civvis-tabs.sh closes every 127.0.0.1:87xx tab that is
not the exhibition, so a mirror in that range would be pruned off the screen
within a minute.
"""

import json
import os
import re
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request

RIG = "/Users/martin/civvis-civ6-mirror"
BIN = os.path.join(RIG, "civvis")
RUNS = "/Users/martin/civvis-civ6-runs/control"
PORT = int(os.environ.get("CIVVIS_MIRROR_PORT", "8610"))
LOG = os.path.join(RIG, "follow.log")
STATUS = os.path.join(RIG, "status.json")
STAGE = os.path.join(RIG, "stage")

POLL_SECONDS = 8.0
# A rebuild costs a process launch, so don't do it for every appended line; a
# turn here takes minutes, and this still tracks units moving within a turn.
MIN_REPUBLISH_SECONDS = 30.0
# Older than this and the run is finished or dead, not "the ongoing game".
RUN_FRESH_SECONDS = 900.0


def log(message):
    line = f"[mirror] {time.strftime('%FT%TZ', time.gmtime())} {message}"
    print(line, flush=True)
    try:
        with open(LOG, "a") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def newest_run():
    """The run dir whose events.jsonl was written most recently, if it is live."""
    best, best_mtime = None, 0.0
    try:
        entries = os.listdir(RUNS)
    except OSError:
        return None, 0.0
    for name in entries:
        events = os.path.join(RUNS, name, "events.jsonl")
        try:
            mtime = os.path.getmtime(events)
        except OSError:
            continue
        if mtime > best_mtime:
            best, best_mtime = os.path.join(RUNS, name), mtime
    return best, best_mtime


def read_events(run_dir):
    """Complete JSON lines only — the game is appending to this file as we read.

    A half-written final line is normal, not corruption; dropping it costs at
    most one poll and keeps the reconstruction deterministic.
    """
    path = os.path.join(run_dir, "events.jsonl")
    good, turn, players, height = [], None, 4, 38
    try:
        with open(path, "rb") as handle:
            raw = handle.read()
    except OSError:
        return None, None, players, height
    for chunk in raw.split(b"\n"):
        if not chunk.strip():
            continue
        try:
            event = json.loads(chunk)
        except Exception:
            continue
        good.append(chunk)
        kind = event.get("kind")
        if kind == "seat":
            players = int(event.get("players") or players)
        # The map's own height decides the reflection axis; taking it from the
        # export beats assuming a size, because the ladder changes map size.
        if kind == "tiles" and isinstance(event.get("height"), int):
            height = event["height"]
        if kind in ("state", "turn") and isinstance(event.get("turn"), int):
            turn = event["turn"]
    return good, turn, players, height


def mirror_axis(height):
    """The row to reflect about, as a row index doubled — see `flip_north_up`.

    Must be an EVEN offset from 0 so that row and reflected row keep the same
    parity; the largest one that fits is `height - 1` rounded down to even.
    """
    top = height - 1
    return top if top % 2 == 0 else top - 1


def flip_north_up(event, top, dropped):
    """Turn the board the right way up, in place.

    ⚠ Civilization VI's `y` grows NORTH (plot 0 is the south edge). CIVVIS's `r`
    grows SOUTH — down the screen. `rebuild_from_state` copies `y` straight into
    the axial `r`, so the mirrored board is drawn UPSIDE DOWN: the operator sees
    a northern city below a southern one. Nothing complains, because both are
    small integers and every self-consistent check still passes — comparing the
    reconstruction against `hex::offset_to_axial` confirms the arithmetic, not
    the orientation. It took a person looking at the two screens (`tyre below
    when it should be above`) to catch it.

    The reflection has to be about a line through a ROW CENTRE. An odd-r grid
    shoves odd rows half a hex right, so a row may only be mapped onto a row of
    the SAME parity; the obvious `(height - 1) - y` flips parity on an even
    height and shears the whole map half a hex, which looks plausible and is
    wrong. Reflecting about row `top / 2` keeps parity, leaves the column
    untouched, and costs only the single polar row that falls off the end.
    """
    if not isinstance(event, dict):
        if isinstance(event, list):
            return [e for e in (flip_north_up(v, top, dropped) for v in event) if e is not None]
        return event
    y = event.get("y")
    if isinstance(y, int) and not isinstance(y, bool) and isinstance(event.get("x"), int):
        if y > top:                      # the row with no partner under this axis
            dropped.append(y)
            return None
        event["y"] = top - y
    for key, value in list(event.items()):
        if isinstance(value, (dict, list)):
            event[key] = flip_north_up(value, top, dropped)
    return event


def stage_events(lines, height):
    """`--mirror` wants a directory holding events.jsonl; give it a clean copy.

    The copy is turned north-up on the way through. The real run directory is
    never touched — this rewrites only what the throwaway reconstruction reads.
    """
    os.makedirs(STAGE, exist_ok=True)
    top, dropped = mirror_axis(height), []
    out = []
    for chunk in lines:
        try:
            event = json.loads(chunk)
        except Exception:
            continue
        flipped = flip_north_up(event, top, dropped)
        if flipped is not None:
            out.append(json.dumps(flipped).encode())
    if dropped:
        log(f"{len(dropped)} board object(s) on row {top + 1} dropped by the north-up "
            f"reflection (that row has no same-parity partner)")
    path = os.path.join(STAGE, "events.jsonl")
    with open(path, "wb") as handle:
        handle.write(b"\n".join(out) + b"\n")
    return STAGE


def http_get(port, path, timeout=30):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}{path}", timeout=timeout) as response:
        return response.read()


def http_post(port, path, payload, timeout=60):
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def server_alive(port):
    try:
        http_get(port, "/status", timeout=5)
        return True
    except Exception:
        return False


def rebuild(run_dir, players):
    """Rebuild the live board and hand back its serialized Game.

    Runs on an ephemeral port so a lingering throwaway can never collide with
    the next rebuild, and at nice 10 so it never competes with the game whose
    frame budget the controller shares.
    """
    lines, _, _, height = read_events(run_dir)
    stage = stage_events(lines or [], height)
    process = subprocess.Popen(
        ["nice", "-n", "10", BIN, "play", "--mirror", stage,
         "--players", str(players), "--port", "0", "--paused", "--no-open"],
        cwd=RIG, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    port, summary = None, []
    deadline = time.time() + 90
    try:
        while time.time() < deadline:
            line = process.stdout.readline()
            if not line:
                break
            line = line.strip()
            if line:
                summary.append(line)
            found = re.search(r"127\.0\.0\.1:(\d+)", line)
            if found:
                port = int(found.group(1))
                break
        if port is None:
            return None, summary
        for _ in range(60):
            if server_alive(port):
                break
            time.sleep(0.5)
        else:
            return None, summary
        return json.loads(http_get(port, "/save")), summary
    finally:
        try:
            process.send_signal(signal.SIGTERM)
            process.wait(timeout=10)
        except Exception:
            try:
                process.kill()
            except Exception:
                pass


def start_visible_server(run_dir, players):
    """The window the operator is actually looking at.

    `--spectate`, not a human seat. A seat draws the lobby rail ("start new
    game", research pickers) and pops a unit-order prompt over the board for
    every idle unit — an invitation to act, on a window whose whole job is to
    show what another program is doing. The watcher is then pointed at seat 0
    with `/view`: a SEATLESS spectator gets the whole-world shot, and this world
    is 60x38 with ~160 revealed plots, so it would frame a three-city empire as
    a speck in blank ocean (see civvis-seat-framed-on-own-ground).

    `--speed online --turns 250` because the HUD prints them: a mirror that
    reports "Standard" for a game being played on Online is quietly wrong on
    screen, which is the one thing a mirror cannot be.
    """
    log(f"starting mirror server on :{PORT}")
    out = open(os.path.join(RIG, "server.log"), "a")
    # The staged copy, not the run directory: the server's own first read must be
    # turned north-up too, or the window is upside down until the first /load.
    lines, _, _, height = read_events(run_dir)
    staged = stage_events(lines or [], height)
    subprocess.Popen(
        ["nice", "-n", "5", BIN, "play", "--mirror", staged,
         "--players", str(players), "--port", str(PORT), "--paused", "--no-open",
         "--spectate", "--speed", "online", "--turns", "250",
         "--victories", "science,culture,religious,diplomatic,domination,score"],
        cwd=RIG, stdout=out, stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    for _ in range(60):
        if server_alive(PORT):
            hold_the_frame()
            return True
        time.sleep(1)
    return False


def hold_the_frame():
    """Watch our own seat, and never simulate.

    Both must be re-asserted after every `/load`: it rebuilds the Session from
    the incoming game, so the view player falls back to the seatless spectator
    and the spectator pause is not carried over. Left alone, the page would
    reframe onto the whole ocean and then play CIVVIS's own game forward from
    the real position — a window that starts as a mirror and silently becomes a
    different game is worse than no window.
    """
    for path, payload in (("/view", {"player": 0}),
                          ("/spectator-status", {"paused": True})):
        try:
            http_post(PORT, path, json.dumps(payload).encode(), timeout=15)
        except Exception as error:
            log(f"could not {path}: {error}")


MIRROR_URL = f"http://127.0.0.1:{PORT}/"
# Left half of the display, beside the Civilization VI window the controller
# parks on the right (`civ6_play --window-side right`).
MIRROR_BOUNDS = "{0, 33, 864, 1117}"


def chrome(script):
    done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=30)
    return (done.stdout or done.stderr).strip()


def mirror_on_screen():
    """Is a Chrome tab pointing at the mirror?

    Returns None when Chrome cannot be enumerated. An empty answer is NOT an
    empty browser — the enumeration fails intermittently under load, and
    treating that as "the window is gone" is what had civvis-keeper.sh opening a
    fresh window every couple of minutes and leaving the page no time to paint.
    """
    if subprocess.run(["pgrep", "-x", "Google Chrome"], capture_output=True).returncode != 0:
        return False
    answer = chrome('tell application "Google Chrome" to get URL of tabs of every window')
    if not answer:
        return None
    return f":{PORT}" in answer


def ensure_on_screen(misses):
    """Put the mirror back on the display if it has been closed.

    Deliberately does NOT `activate` Chrome. Civilization VI runs its turn loop
    off frame-tied events and macOS starves a background app of frames, so
    stealing the foreground every few seconds would stop the very game this
    window exists to show (civvis-civ6-computer-control).
    """
    shown = mirror_on_screen()
    if shown is None or shown:
        return 0
    misses += 1
    if misses < 3:                      # ~24s of agreement before acting
        return misses
    log("mirror window is not on screen; restoring it")
    chrome(f'''tell application "Google Chrome"
      make new window
      set URL of active tab of window 1 to "{MIRROR_URL}"
      set bounds of window 1 to {MIRROR_BOUNDS}
    end tell''')
    return 0


def write_status(**fields):
    fields["written"] = time.strftime("%FT%TZ", time.gmtime())
    try:
        with open(STATUS, "w") as handle:
            json.dump(fields, handle, indent=2)
    except OSError:
        pass


def main():
    log(f"following {RUNS} -> http://127.0.0.1:{PORT}/")
    current_run, published_turn, published_size, published_at = None, None, None, 0.0
    published_count, misses = 0, 0

    while True:
        misses = ensure_on_screen(misses)
        run_dir, mtime = newest_run()
        if run_dir is None:
            log("no run directory yet")
            time.sleep(POLL_SECONDS)
            continue

        stale = (time.time() - mtime) > RUN_FRESH_SECONDS
        if run_dir != current_run:
            log(f"following run {os.path.basename(run_dir)}"
                + (" (idle; showing its last position)" if stale else ""))
            current_run, published_turn, published_size = run_dir, None, None

        lines, turn, players, _ = read_events(run_dir)
        if lines is None:
            time.sleep(POLL_SECONDS)
            continue
        size = len(lines)

        if not server_alive(PORT):
            if start_visible_server(run_dir, players):
                published_turn, published_size = turn, size
                published_at = time.time()
            else:
                log(f"mirror server did not come up on :{PORT}")
            time.sleep(POLL_SECONDS)
            continue

        since = time.time() - published_at
        turn_moved = turn is not None and turn != published_turn
        grew = published_size is not None and size != published_size
        if not (turn_moved or (grew and since >= MIN_REPUBLISH_SECONDS)):
            time.sleep(POLL_SECONDS)
            continue

        game, summary = rebuild(run_dir, players)
        if game is None:
            # Usually "no tiles to mirror" on a run that has not exported yet.
            log("rebuild produced no board: " + " | ".join(summary[-2:]))
            published_size = size
            time.sleep(POLL_SECONDS)
            continue

        try:
            body = json.dumps({"game": game}).encode()
            answer = json.loads(http_post(PORT, "/load", body))
        except Exception as error:
            log(f"could not publish: {error}")
            time.sleep(POLL_SECONDS)
            continue

        if answer.get("error"):
            log(f"server refused the board: {answer['error']}")
            time.sleep(POLL_SECONDS)
            continue

        hold_the_frame()
        published_turn, published_size, published_at = turn, size, time.time()
        published_count += 1
        detail = next((s for s in summary if s.startswith("empire:")), "")
        log(f"turn {answer.get('turn')} on screen — {detail or 'no empire summary'}")
        write_status(
            run=os.path.basename(run_dir), turn=answer.get("turn"),
            cities=len(answer.get("cities") or []), units=len(answer.get("units") or []),
            port=PORT, publishes=published_count, run_idle=stale,
        )
        time.sleep(POLL_SECONDS)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(0)
