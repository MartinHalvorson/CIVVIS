#!/usr/bin/env python3
"""Check that every simulated turn reaches the viewer as one complete frame.

Every turn the spectator plays is owed at least one frame, and that frame has
to carry the whole turn — player stats, victory tracker, map and units, all
from the same snapshot. The server keeps the first half of that promise by
holding a finished turn until an active viewer has been handed it, whatever
pace the turn was played at. This checks that it is actually being kept.

Two modes:

``watch``
    Read a running exhibition's ``/status``. The page reports the turn it last
    painted, so the server can count turns nobody drew. ``frames_missed`` is
    that count and should be zero; ``frames_painted`` says whether anybody was
    watching at all, because zero misses with nobody there means nothing.

        python3 tools/civvis_frames.py watch --port 8766

``probe``
    Be the viewer. Poll ``/state`` the way the page does — one request in
    flight, a paint that costs real time — and report every turn that never
    arrived. ``--render-ms`` is the honest knob: a page that paints slower than
    the turn budget is the case that used to lose turns silently, so set it to
    something a loaded machine would actually spend.

        python3 tools/civvis_frames.py probe --port 8766 --render-ms 400

Neither mode changes the game. ``probe`` does count as a viewer, so it holds
the exhibition to its own cadence while it runs; ``watch`` does not.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.request
from urllib.error import URLError


def read_json(port: int, path: str, timeout: float = 10.0) -> dict:
    return read_sized(port, path, timeout)[1]


def read_sized(port: int, path: str, timeout: float = 10.0) -> tuple[int, dict]:
    """The response and what it cost on the wire.

    The size is the point of half this tool now: the map is most of a state and
    almost none of it changes between turns, so a viewer that says what it is
    holding is sent only the difference.
    """
    url = f"http://127.0.0.1:{port}{path}"
    with urllib.request.urlopen(url, timeout=timeout) as response:
        raw = response.read()
    return len(raw), json.loads(raw)


def gaps(turns: list[int]) -> list[int]:
    """Turns between the first and last seen that never appeared at all.

    The sequence is what a viewer actually received, so it repeats turns it
    polled twice inside and never goes backwards within one world. A hole in
    it is a turn the server simulated and nobody ever saw.
    """
    if not turns:
        return []
    seen = set(turns)
    return [turn for turn in range(min(turns), max(turns) + 1) if turn not in seen]


def watch(port: int, seconds: float, interval: float) -> int:
    first = read_json(port, "/status")
    deadline = time.monotonic() + seconds
    last = first
    while time.monotonic() < deadline:
        time.sleep(interval)
        last = read_json(port, "/status")

    missed = last.get("frames_missed")
    painted = last.get("frames_painted")
    report = {
        "mode": "watch",
        "seconds": round(seconds, 1),
        "turns_played": last["turn"] - first["turn"],
        "frames_missed": missed,
        "last_painted_turn": painted,
        # Every viewer is owed every turn and each is waited for separately, so
        # this is also how many paints a turn costs before the next one starts.
        "viewers": last.get("viewers"),
        "turn": last["turn"],
    }
    if painted is None:
        report["verdict"] = "nobody is watching: no page has reported a painted frame"
        ok = False
    elif missed:
        report["verdict"] = f"{missed} turns were simulated that no viewer drew"
        ok = False
    elif report["turns_played"] == 0:
        # Nothing was played, so nothing was skipped, so this says nothing.
        # Worth naming: a paused exhibition and a healthy one both read zero.
        report["verdict"] = "no turns were played in this window — nothing was tested"
        ok = False
    else:
        report["verdict"] = "every turn reached the viewer"
        ok = True
    print(json.dumps(report, indent=2))
    return 0 if ok else 1


def probe(port: int, seconds: float, render_ms: float, poll_ms: float) -> int:
    seen: list[int] = []
    errors = 0
    painted: tuple[int, int] | None = None
    # A seat of its own. Every viewer is owed every turn and the server waits
    # for each separately, so a probe sharing an id with the page in front of
    # somebody would take turns with it rather than testing anything.
    viewer = f"probe-{os.getpid()}"
    # What this stand-in holds, exactly as the page holds it: the tile array
    # and the frame it belongs to. Saying so is what earns a patch instead of
    # the whole map, and what asks the server to hold the answer back until
    # there is a next turn to give.
    held: tuple[int, int] | None = None
    tiles: list = []
    bytes_first = 0
    bytes_patched: list[int] = []
    started = time.monotonic()
    while time.monotonic() - started < seconds:
        report = ("painted=" if painted is None
                  else f"painted={painted[1]}&world={painted[0]}")
        baseline = f"&have={held[0]}:{held[1]}" if held else ""
        target = f"/state?{report}&viewer={viewer}{baseline}"
        try:
            size, state = read_sized(port, target, timeout=30.0)
        except (URLError, OSError, ValueError):
            errors += 1
            held = None  # a dropped response: take the next map whole
            time.sleep(poll_ms / 1000.0)
            continue
        patch = state.get("map", {}).get("tiles_changed")
        if patch is None:
            tiles = state.get("map", {}).get("tiles", [])
            bytes_first = bytes_first or size
        else:
            if held is None or state["seed"] != held[0]:
                errors += 1
                continue
            for at, tile in patch:
                tiles[at] = tile
            bytes_patched.append(size)
        held = (state["seed"], state["turn"])
        # A real page parses the whole observation and repaints the map before
        # it asks for the next one. Touch the payload and spend the time, or
        # this loop is a faster viewer than any browser and proves less.
        len(tiles)
        len(state.get("units", ()))
        if render_ms:
            time.sleep(render_ms / 1000.0)
        seen.append(state["turn"])
        painted = (state["seed"], state["turn"])
        if state.get("winner") is not None:
            break
        time.sleep(poll_ms / 1000.0)

    if not seen:
        print(json.dumps({"mode": "probe", "verdict": "no state was ever served",
                          "fetch_errors": errors}, indent=2))
        return 1
    missed = gaps(seen)
    elapsed = time.monotonic() - started
    print(json.dumps({
        "mode": "probe",
        "seconds": round(elapsed, 1),
        "render_ms": render_ms,
        "responses": len(seen),
        "turn_range": [min(seen), max(seen)],
        "turns_simulated": max(seen) - min(seen) + 1,
        "turns_seen": len(set(seen)),
        "turns_missed": len(missed),
        "missed": missed[:40],
        "fetch_errors": errors,
        "turns_per_sec": round((max(seen) - min(seen)) / elapsed, 2) if elapsed else 0,
        "first_poll_bytes": bytes_first,
        "patched_poll_bytes": round(sum(bytes_patched) / len(bytes_patched))
                              if bytes_patched else None,
        "tiles_held": len(tiles),
        "verdict": "every turn reached the viewer" if not missed
                   else f"{len(missed)} turns were simulated that never reached a frame",
    }, indent=2))
    return 0 if not missed else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("mode", choices=("watch", "probe"))
    parser.add_argument("--port", type=int, default=8766)
    parser.add_argument("--seconds", type=float, default=30.0)
    parser.add_argument("--interval", type=float, default=2.0,
                        help="watch: seconds between /status reads")
    parser.add_argument("--render-ms", type=float, default=250.0,
                        help="probe: what one repaint costs the viewer")
    parser.add_argument("--poll-ms", type=float, default=100.0,
                        help="probe: the page's gap between polls")
    args = parser.parse_args(argv)
    try:
        if args.mode == "watch":
            return watch(args.port, args.seconds, args.interval)
        return probe(args.port, args.seconds, args.render_ms, args.poll_ms)
    except (URLError, OSError) as error:
        print(json.dumps({"error": f"no spectator on port {args.port}: {error}"}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
