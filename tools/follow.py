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

RIG = os.environ.get(
    "CIVVIS_RIG", os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def default_binary() -> str:
    """Find a local CIVVIS executable without assuming one operator's home."""
    configured = os.environ.get("CIVVIS_BIN")
    if configured:
        return configured
    for candidate in (
        os.path.join(RIG, "target", "release", "civvis"),
        os.path.join(RIG, "target", "debug", "civvis"),
        os.path.join(RIG, "civvis"),
    ):
        if os.path.isfile(candidate):
            return candidate
    # Keep a useful path in the diagnostic when the build has not happened yet.
    return os.path.join(RIG, "target", "release", "civvis")


BIN = default_binary()
RUNS = os.environ.get(
    "CIVVIS_RUNS", os.path.join(os.path.expanduser("~"), "civvis-civ6-runs", "control"))
PORT = int(os.environ.get("CIVVIS_MIRROR_PORT", "8610"))
LOG = os.environ.get("CIVVIS_FOLLOW_LOG", os.path.join(RIG, "follow.log"))
STATUS = os.environ.get("CIVVIS_FOLLOW_STATUS", os.path.join(RIG, "status.json"))
STAGE = os.environ.get("CIVVIS_FOLLOW_STAGE", os.path.join(RIG, "stage"))

POLL_SECONDS = 1.0
# A rebuild costs a process launch, so don't do it for every appended line; a
# live Online-speed turn can take only a few seconds, and every completed state
# export must reach the board before the next decision makes it stale.
MIN_REPUBLISH_SECONDS = 1.0
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

    The last element is whether the run has exported a map YET, which is the
    precondition `start_visible_server` needs and this is the only pass that
    already reads every event. See `main` for what it is guarding.
    """
    path = os.path.join(run_dir, "events.jsonl")
    good, turn, players, height, tiles = [], None, 4, 38, False
    try:
        with open(path, "rb") as handle:
            raw = handle.read()
    except OSError:
        return None, None, players, height, tiles
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
        if kind == "tiles":
            tiles = True
            if isinstance(event.get("height"), int):
                height = event["height"]
        if kind in ("state", "turn") and isinstance(event.get("turn"), int):
            turn = event["turn"]
    return good, turn, players, height, tiles


def mirror_axis(height):
    """The row to reflect about, as a row index doubled — see `flip_north_up`.

    Must be an EVEN offset from 0 so that row and reflected row keep the same
    parity. An even-height odd-r map has no in-bounds even axis: using
    `height - 2` drops its last row. Give the display reconstruction one empty
    staging row instead, so all real rows map bijectively onto `1..=height`.
    This changes only the north-up spectator canvas; orders consume the raw run.
    """
    return height if height % 2 == 0 else height - 1


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
    wrong. Reflecting about row `top / 2` keeps parity and leaves the column
    untouched. Even-height maps use one empty staging row so neither pole is
    discarded.
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
    # Route endpoints and refusal origins are coordinate pairs too, but their
    # field names are qualified because the same record carries two positions.
    # Leaving these south-up while every city is flipped north-up makes an active
    # route impossible to resolve, so it vanishes from CIVVIS's economy even
    # though its Trader is still correctly marked busy.
    for prefix in ("origin", "destination", "from"):
        x_key, y_key = f"{prefix}_x", f"{prefix}_y"
        special_y = event.get(y_key)
        if (isinstance(special_y, int) and not isinstance(special_y, bool)
                and isinstance(event.get(x_key), int)):
            if special_y > top:
                dropped.append(special_y)
                # The id fields can still resolve a route whose endpoint lies on
                # the unpaired polar row. Negative coordinates select that path.
                event[x_key] = -1
                event[y_key] = -1
            elif special_y >= 0:
                event[y_key] = top - special_y
    for key, value in list(event.items()):
        if isinstance(value, (dict, list)):
            event[key] = flip_north_up(value, top, dropped)
    return event


def offset_to_axial(col, row):
    return col - (row - (row & 1)) // 2, row


def axial_to_offset(q, r):
    return q + (r - (r & 1)) // 2, r


def remap_river_masks(events, top):
    """Reflect every known river edge without requiring the opposite plot.

    Current exports carry all six edges on every revealed plot: bits 1..32 are
    E, SE, SW, W, NW and NE in the mirror's axial frame. Older exports carried
    only the first three. Either vocabulary is sufficient because the Rust
    reconstruction accepts all six bits and preserves a one-sided edge when
    the plot across a known boundary is still hidden.

    The previous north-up pass still gathered only bits 1, 2 and 4 onto their
    canonical Firaxis holders. That silently discarded the new 8, 16 and 32
    boundary facts, then reported visible rivers as dry whenever the hidden
    neighbour was the only possible old-style holder. A live turn-15 audit
    reproduced it as `river@57,23 Civ6=True CIVVIS=False` while every other
    field agreed.

    Treat a river as an undirected segment instead. Reflect both endpoints and
    encode the reflected direction on any revealed endpoint that survives the
    display's polar-row crop. This keeps an exact one-sided boundary edge, works
    for old three-bit archives, deduplicates the reciprocal six-bit export, and
    loses a segment only when neither revealed endpoint can appear on screen.

    Returns the number of genuinely unrepresentable segments on the newest
    export turn. Earlier turns are transformed identically but are not summed
    into the live diagnostic.
    """
    directions = ((1, 0), (1, -1), (0, -1),
                  (-1, 0), (-1, 1), (0, 1))
    encoded = (1, 2, 4, 8, 16, 32)
    lost_by_turn = {}
    by_turn = {}
    for event in events:
        if isinstance(event, dict) and event.get("kind") == "tiles":
            by_turn.setdefault(event.get("turn"), []).append(event)
    for turn, chunks in by_turn.items():
        lost = 0
        width = next((c["width"] for c in chunks if isinstance(c.get("width"), int)), 0)
        wrap = (lambda x: x % width) if width > 0 else (lambda x: x)
        plots = {}
        for c in chunks:
            for p in c.get("plots", []):
                if isinstance(p, dict) and isinstance(p.get("x"), int) \
                        and isinstance(p.get("y"), int):
                    plots[(p["x"], p["y"])] = p
        old = {xy: (p.get("rv") or 0) for xy, p in plots.items()}
        if not any(old.values()):
            continue
        segments = {}
        for start, mask in old.items():
            axial = offset_to_axial(*start)
            for direction, bit in enumerate(encoded):
                if not mask & bit:
                    continue
                delta = directions[direction]
                end = axial_to_offset(axial[0] + delta[0], axial[1] + delta[1])
                end = (wrap(end[0]), end[1])
                key = tuple(sorted((start, end)))
                carriers = segments.setdefault(key, [])
                if start not in carriers:
                    carriers.append(start)

        for plot in plots.values():
            if "rv" in plot:
                plot["rv"] = 0

        for endpoints, carriers in segments.items():
            available = [point for point in carriers if point in plots and point[1] <= top]
            if not available:
                # A flag held by the cropped polar row can move to its revealed
                # opposite endpoint as the reciprocal edge.
                available = [
                    point for point in endpoints
                    if point in plots and point[1] <= top
                ]
            if not available:
                lost += 1
                continue
            carrier = available[0]
            other = endpoints[1] if endpoints[0] == carrier else endpoints[0]
            reflected_carrier = (carrier[0], top - carrier[1])
            reflected_other = (wrap(other[0]), top - other[1])
            axial = offset_to_axial(*reflected_carrier)
            direction = next((
                index for index, delta in enumerate(directions)
                if (
                    wrap(axial_to_offset(axial[0] + delta[0], axial[1] + delta[1])[0]),
                    axial_to_offset(axial[0] + delta[0], axial[1] + delta[1])[1],
                ) == reflected_other
            ), None)
            if direction is None:
                lost += 1
                continue
            plots[carrier]["rv"] = (plots[carrier].get("rv") or 0) | encoded[direction]
        lost_by_turn[turn] = lost
    return lost_by_turn[max(lost_by_turn)] if lost_by_turn else 0


def stage_events(lines, height):
    """`--mirror` wants a directory holding events.jsonl; give it a clean copy.

    The copy is turned north-up on the way through. The real run directory is
    never touched — this rewrites only what the throwaway reconstruction reads.
    """
    os.makedirs(STAGE, exist_ok=True)
    top, dropped = mirror_axis(height), []
    events = []
    for chunk in lines:
        try:
            events.append(json.loads(chunk))
        except Exception:
            continue
    lost = remap_river_masks(events, top)
    if lost:
        log(f"{lost} river segment(s) lost by the north-up reflection "
            f"(no exported plot can carry them on the flipped board)")
    out = []
    for event in events:
        flipped = flip_north_up(event, top, dropped)
        if flipped is not None:
            if flipped.get("kind") == "tiles":
                flipped["height"] = top + 1
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



def stop_visible_server():
    """Take the finished game OFF THE SCREEN.

    ⚠⚠⚠ THE TWO WINDOWS MUST NEVER SHOW DIFFERENT GAMES. This follower used to
    switch `current_run` to the new attempt and leave the previous game's board
    served, because the new run has no map to publish until the mod's first tile
    export -- which is minutes away while Civilization VI generates a map and
    reaches turn 1. For that whole window the operator's screen showed a
    FINISHED empire beside a live game's setup menu, which is exactly the thing
    a mirror cannot do: it is not "stale", it is a different game, and it reads
    as the mirror being wrong about the game in front of it.

    Measured 2026-08-10: at 21:48:25 the follower logged `following run
    civvis-20260810T214824Z` while :8610 kept serving TURN 189 of the run before
    it -- five cities and twelve units against a brand-new game still choosing
    its leader.

    So the finished game comes down at the moment the run changes. The existing
    "waiting for the run's first map export" path then brings the server back up
    on the new run as soon as it has a map. A briefly empty port is honest; a
    confidently wrong board is not.
    """
    stopped = False
    for pattern in ("play --mirror",):
        try:
            found = subprocess.run(
                ["pgrep", "-f", f"{os.path.basename(BIN)} {pattern}"],
                capture_output=True, text=True,
            ).stdout.split()
        except Exception:
            found = []
        for pid in found:
            try:
                os.kill(int(pid), signal.SIGTERM)
                stopped = True
            except Exception:
                pass
    if stopped:
        # Give it a moment to release the port so the next start is not refused.
        for _ in range(20):
            if not server_alive(PORT):
                break
            time.sleep(0.5)
        log("took the finished game off the screen; the port is free for the "
            "next one")
    return stopped


def rebuild(run_dir, players):
    """Rebuild the live board and hand back its serialized Game.

    Runs on an ephemeral port so a lingering throwaway can never collide with
    the next rebuild, and at nice 10 so it never competes with the game whose
    frame budget the controller shares.
    """
    lines, _, _, height, _ = read_events(run_dir)
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
        value = json.loads(http_get(port, "/save"))
        # `/save` is a versioned envelope on current servers. Keep this
        # diagnostic's historical return value—the raw game—while accepting
        # old servers that returned the game directly.
        if (
            isinstance(value, dict)
            and value.get("format") == "civvis.save"
            and isinstance(value.get("game"), dict)
        ):
            value = value["game"]
        return value, summary
    finally:
        try:
            process.send_signal(signal.SIGTERM)
            process.wait(timeout=10)
        except Exception:
            try:
                process.kill()
            except Exception:
                pass


def server_log_reason():
    """The mirror server's own last words, for a diagnostic that names a cause.

    Bounded read from the end: `server.log` is appended to across every batch
    this machine has ever run, so it is the one file here that must never be
    read whole.
    """
    path = os.path.join(RIG, "server.log")
    try:
        with open(path, "rb") as handle:
            handle.seek(0, os.SEEK_END)
            handle.seek(max(0, handle.tell() - 4096))
            tail = handle.read().decode("utf-8", "replace")
    except OSError:
        return f"no reason recorded in {path}"
    for line in reversed(tail.splitlines()):
        if line.strip():
            return line.strip()
    return f"no reason recorded in {path}"


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
    lines, _, _, height, _ = read_events(run_dir)
    staged = stage_events(lines or [], height)
    process = subprocess.Popen(
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
            refresh_mirror_page(process.pid)
            # Chrome reconnects after the server answers and may restore its
            # default all-players spectator view after this first pin. Reassert
            # once the page has completed that handoff; otherwise a fresh server
            # can expose the generated world instead of the Firaxis seat's fog.
            time.sleep(2.0)
            hold_the_frame()
            return True
        if process.poll() is not None:
            # The status alone names no cause, and the cause is always one line
            # away in a DIFFERENT file that nothing pointed at: every refusal the
            # binary makes — a stage it will not mirror, a port already held, a
            # flag this build dropped — leaves exit 2 and an explanation on its
            # stderr, which is `server.log`. Carrying it here is the difference
            # between a transient wait and a misconfiguration, which this line
            # otherwise reports identically.
            log(f"mirror server exited during startup with status "
                f"{process.returncode}: {server_log_reason()}")
            return False
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
MIRROR_BOUNDS = os.environ.get("CIVVIS_MIRROR_BOUNDS", "{0, 33, 864, 1117}")


def chrome(script):
    # ⚠ A PENDING AUTOMATION CONSENT KILLED THE WHOLE FOLLOWER (2026-08-14).
    # On a host whose Terminal has never been granted control of Chrome, macOS
    # queues the osascript call behind its consent dialog. Nobody was at the
    # screen, the 30 s timeout fired, and the uncaught TimeoutExpired took the
    # follower down — so a first-boot machine lost its mirror to a dialog box.
    # `mirror_on_screen` already defines the contract for this state: an empty
    # answer means "Chrome cannot be enumerated", NOT "the tab is gone".
    try:
        done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=30)
    except subprocess.TimeoutExpired:
        log("chrome scripting timed out; likely an unanswered Automation "
            "consent dialog — the mirror keeps serving without tab management")
        return ""
    except OSError as exc:
        log(f"chrome scripting could not run: {exc}")
        return ""
    if done.returncode != 0:
        # A silent AppleScript failure reads exactly like a healthy no-op — the
        # tab simply never changes — and an Automation (TCC) denial persists
        # for this process's whole life. One line makes it observable.
        log(f"chrome scripting failed (status {done.returncode}): "
            f"{(done.stderr or done.stdout).strip()[:200]}")
    return (done.stdout or done.stderr).strip()


def mirror_target_url():
    """The address a mirror tab must actually open on.

    A bare mirror URL boots the LOBBY: Game Setup sidebar over a black map,
    and a lobby page never polls `/state` — restored that way, the pane reads
    "loading" forever while every server-side check stays green (2026-08-14).
    Only a URL naming the live server's instance boots the page as a spectator
    joining the running world. `/runtime` is lock-free, so asking is cheap; a
    server that cannot answer gets the bare URL, which is no worse than before.
    """
    try:
        runtime = json.loads(http_get(PORT, "/runtime", timeout=5))
        instance = runtime.get("server_instance")
        if instance:
            return f"{MIRROR_URL}?instance={instance}"
    except Exception:
        pass
    return MIRROR_URL


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


def refresh_mirror_page(server_pid):
    """Give an existing mirror tab the server that just replaced its old one.

    The URL is unique to the fresh `civvis play --mirror` process, and the
    index document uses that instance to give `app.js` its own fresh URL. This
    prevents a tab that survived a verification batch from rendering the old
    client's blank or stale map. Do not activate Chrome: Firaxis needs to keep
    receiving its frame-tied events while this happens.
    """
    if mirror_on_screen() is not True:
        return
    target = f"{MIRROR_URL}?instance={server_pid}"
    chrome(f'''tell application "Google Chrome"
      set refreshed to false
      repeat with thisWindow in every window
        repeat with thisTab in every tab of thisWindow
          if not refreshed and (URL of thisTab) contains ":{PORT}" then
            set URL of thisTab to "{target}"
            set refreshed to true
          end if
        end repeat
      end repeat
    end tell''')


def ensure_on_screen(misses):
    """Put the mirror back on the display if it has been closed.

    Leave placement and sizing to the operator. The follower owns the mirror's
    availability, not the desktop layout.

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
      set URL of active tab of window 1 to "{mirror_target_url()}"
    end tell''')
    return 0


# A visible healthy tab re-attaches within a couple of seconds of any of this
# module's navigations, so three quarters of a minute of silence is a wedge,
# not a slow boot. An occluded or backgrounded tab also legitimately reads
# zero viewers, which is why revival acts rarely and logs every action.
REVIVE_DEAD_SECONDS = 45.0
REVIVE_HOLDOFF_SECONDS = 300.0


def mirror_viewers():
    """Pages that fetched `/state` within the server's six-second window.

    `/status` `viewers` is the one honest signal that the pane is painting:
    tab presence, port health, and the turn counter all stay green around a
    wedged client. None when the server cannot answer.
    """
    try:
        return json.loads(http_get(PORT, "/status", timeout=5)).get("viewers", 0)
    except Exception:
        return None


def ensure_watching(watch):
    """Revive a mirror tab that is present but not painting.

    A page can wedge while every other check reads green — a boot crash (the
    saved-HUD-layout dead zone held this pane on its veil for four days,
    2026-08-14), a poisoned sessionStorage handoff, a veil no fetch path can
    lift. No `/load` publish can reach the screen through a wedged client, so
    the follower owns liveness the way it owns presence. First re-point the
    tab — a full navigation reboots the client; after two revivals without a
    cure, replace the tab entirely, which also abandons its sessionStorage.
    """
    if not server_alive(PORT):
        watch["dead_since"] = None
        return
    viewers = mirror_viewers()
    if viewers is None:
        return
    if viewers > 0:
        watch["dead_since"] = None
        watch["revivals"] = 0
        return
    if mirror_on_screen() is not True:
        # No tab, or Chrome could not be enumerated: presence is
        # ensure_on_screen's job, and an unreadable browser is not evidence.
        watch["dead_since"] = None
        return
    now = time.time()
    if watch["dead_since"] is None:
        watch["dead_since"] = now
        return
    if now - watch["dead_since"] < REVIVE_DEAD_SECONDS:
        return
    if now - watch["last_revival"] < REVIVE_HOLDOFF_SECONDS:
        return
    watch["last_revival"] = now
    watch["dead_since"] = now
    target = mirror_target_url()
    if watch["revivals"] >= 2:
        watch["revivals"] = 0
        log("mirror tab still not painting after re-pointing; replacing the tab")
        chrome(f'''tell application "Google Chrome"
          set doomed to {{}}
          repeat with thisWindow in every window
            repeat with thisTab in every tab of thisWindow
              if (URL of thisTab) contains ":{PORT}" then set end of doomed to thisTab
            end repeat
          end repeat
          repeat with thisTab in doomed
            close thisTab
          end repeat
          make new window
          set URL of active tab of window 1 to "{target}"
        end tell''')
        return
    watch["revivals"] += 1
    log(f"mirror tab is on screen but not painting (viewers 0 for at least "
        f"{int(REVIVE_DEAD_SECONDS)}s); re-pointing it at {target}")
    chrome(f'''tell application "Google Chrome"
      set refreshed to false
      repeat with thisWindow in every window
        repeat with thisTab in every tab of thisWindow
          if not refreshed and (URL of thisTab) contains ":{PORT}" then
            set URL of thisTab to "{target}"
            set refreshed to true
          end if
        end repeat
      end repeat
    end tell''')


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
    awaiting_export = False
    watch = {"dead_since": None, "last_revival": 0.0, "revivals": 0}

    while True:
        misses = ensure_on_screen(misses)
        ensure_watching(watch)
        run_dir, mtime = newest_run()
        if run_dir is None:
            log("no run directory yet")
            time.sleep(POLL_SECONDS)
            continue

        stale = (time.time() - mtime) > RUN_FRESH_SECONDS
        if run_dir != current_run:
            log(f"following run {os.path.basename(run_dir)}"
                + (" (already idle; leaving the port free until it says more)"
                   if stale else ""))
            # ⚠ Take the PREVIOUS game down before adopting this one. Leaving it
            # served makes the two windows show different games for as long as
            # the new attempt takes to export a map. See `stop_visible_server`.
            if current_run is not None and server_alive(PORT):
                stop_visible_server()
            current_run, published_turn, published_size = run_dir, None, None
            awaiting_export = False

        # A game we have finished playing and finished studying should not go on
        # occupying the screen. `stale` means this run has written nothing for
        # RUN_FRESH_SECONDS -- it is over, and the supervisor is between games
        # (pulling, building) rather than about to say more. Take it down and
        # leave the port free until the next attempt has a map.
        #
        # ⚠⚠ THE `continue` IS UNCONDITIONAL ON PURPOSE. Until 2026-08-17 this
        # whole branch was gated on `server_alive(PORT)`, so the tick that
        # followed a teardown found the port free, fell through to the start
        # path below -- which has no staleness guard of its own -- and re-served
        # the very game just taken off the screen. The follower then oscillated
        # forever at a fixed cost: take down, start, take down, start, one
        # 21 MB `civvis play` process spawned every ~6 seconds for as long as
        # the seat sat between games. Measured on the idle
        # science-domination-20260817T010000Z seat: the mirror pid advanced
        # 38418 -> 38584 -> 38620 -> 38657 inside 20 seconds, and the browser
        # tab pointing at `?instance=<server_pid>` was stale the moment it
        # loaded, because that pid had already been replaced.
        if stale:
            if server_alive(PORT):
                log(f"run {os.path.basename(run_dir)} has been idle for "
                    f"{int(RUN_FRESH_SECONDS)}s; taking its finished game down")
                stop_visible_server()
            time.sleep(POLL_SECONDS)
            continue

        lines, turn, players, _, tiles = read_events(run_dir)
        if lines is None:
            time.sleep(POLL_SECONDS)
            continue
        size = len(lines)

        if not server_alive(PORT):
            # ⚠⚠ SPAWNING HERE BEFORE THE FIRST EXPORT COSTS THE GAME ITS FRAMES.
            #
            # `civvis play --mirror` REFUSES a stage with no revealed plots and
            # exits 2 (`src/main.rs`, "has no tiles to mirror"). That is the
            # ORDINARY state of a run for its first minutes — the mod exports
            # every `TileExportEvery` turns and the game has to generate a map
            # and reach turn 1 first — so this branch used to relaunch a 21 MB
            # binary that loads the whole game database once per POLL_SECONDS,
            # forever, and log a line carrying none of that.
            #
            # Measured on run civvis-20260807T134625Z: 40 spawn-and-die cycles
            # in the four minutes while Civilization VI was still generating its
            # map — precisely the window `docs/CIV6_COMPUTER_CONTROL.md` records
            # as frame-budget-critical, where a background application starved of
            # frames is how this project's turn loop stops dead and reads as a
            # slow machine.
            #
            # `rebuild` has always tolerated this exact condition ("Usually 'no
            # tiles to mirror' on a run that has not exported yet"); only the
            # server-start path did not. The guard is one-directional on purpose:
            # no `tiles` event PROVES `revealed_count() == 0`, so it suppresses
            # only launches that are certain to fail. A stage that has tiles and
            # still refuses goes down the reporting path below, unchanged.
            if not tiles:
                if not awaiting_export:
                    log("waiting for the run's first map export before starting "
                        "the mirror server; the game has exported no tiles yet")
                    awaiting_export = True
                time.sleep(POLL_SECONDS)
                continue
            awaiting_export = False
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
