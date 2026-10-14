#!/usr/bin/env python3
"""The operator's Pause / Resume for the live Civilization VI verification game.

The CIVVIS page beside the game carries a Pause button.  Pressing it must stop
the Firaxis game itself — not the mirror, which is only a picture of it — and
pressing it again must let the game carry on exactly where it stood.  The
pieces involved are separate processes, so the protocol is two small files on
the host:

``~/.civvis-live-pause.json`` — the REQUEST.  Present means "hold the live
    game"; absent means "play".  The mirror server writes and removes it for
    the page (``POST /live-pause``); ``python3 -m civ6_control.live_pause
    pause|resume`` does the same from a terminal.  It is host-wide on purpose:
    a reload into ``-contN`` or a new attempt that starts while it stands is
    held too, because the operator asked for the lane to wait.

``~/.civvis-live-pause-state.json`` — the REPORT.  ``civ6_play.py`` writes it
    from the control mod's own ``operator_paused`` / ``operator_resumed``
    events, so the page can say "Paused at turn N" only once the game has
    actually stopped, rather than the moment somebody asked.

``civ6_play.py`` carries the request to the mod as an out-of-band ``pause``
row in the run's orders database (see :func:`orders.set_operator_pause`).  The
mod honours it on the next tick of our turn — wherever the turn stands, once
its board has gone out — and simply stops playing: Civ VI waits on our turn,
the AI cannot move, and nothing is lost.  Only a press after our turn's end
was sent waits for the next opening, a few seconds of rival play.  Every
watchdog that measures turn progress reads :func:`requested` and stands down
while it holds, so a paused game is never mistaken for a wedged one.

A held game is the operator's to look around: the mod's camera and screen
closer stand down while it holds, and every tool that would otherwise click or
key the game's window asks :func:`holds_game` first.

Both files are optional and every reader fails OPEN: a missing, unreadable or
malformed request means "play".  A pause nobody can see must never be the
reason a lane stops.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REQUEST_ENV = "CIVVIS_LIVE_PAUSE_FILE"
STATE_ENV = "CIVVIS_LIVE_PAUSE_STATE_FILE"
PACE_ENV = "CIVVIS_LIVE_PACE_FILE"
#: The page's slowest pace is 4 s; past a minute a "pace" is a pause.
MAX_PACE_MS = 60_000


def request_path() -> Path:
    configured = os.environ.get(REQUEST_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-pause.json")


def state_path() -> Path:
    configured = os.environ.get(STATE_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-pause-state.json")


def pace_path() -> Path:
    """``~/.civvis-live-pace.json``: the page's Watch pace for the live game.

    ``{"ms": N}``, a whole-turn budget (Blitz 500, Fast 1000, Standard 2000,
    Slow 4000; Lightning 0 is no wait).  The mirror server writes it when the
    page's pace changes; ``civ6_play.py`` relays it to the control mod as a
    ``pace`` row.  Host-wide like the pause, so the next game keeps the pace.
    """
    configured = os.environ.get(PACE_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-pace.json")


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _atomic_json(path: Path, payload: dict[str, Any]) -> None:
    """Replace a file atomically, so a reader never sees half of it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=path.parent,
            prefix=f".{path.name}.", suffix=".tmp", delete=False,
        ) as handle:
            temporary_name = handle.name
            json.dump(payload, handle, sort_keys=True)
            handle.write("\n")
        os.replace(temporary_name, path)
        temporary_name = None
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except OSError:
                pass


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def requested(path: Path | None = None) -> bool:
    """Whether the operator has asked for the live game to be held.

    The file's existence is the request.  Only an explicit ``"paused": false``
    inside it reads as "play", so a request written by hand as an empty file
    still pauses, and a torn read of a real request is not mistaken for none.
    """
    target = path or request_path()
    if not target.exists():
        return False
    payload = _read_json(target)
    return not (payload is not None and payload.get("paused") is False)


def set_requested(paused: bool, source: str = "cli", path: Path | None = None) -> None:
    """Ask for (or lift) the pause.  Resuming removes the file entirely."""
    target = path or request_path()
    if paused:
        _atomic_json(target, {"paused": True, "requested_utc": utc_stamp(),
                              "source": source})
        return
    try:
        target.unlink()
    except FileNotFoundError:
        pass


def holds_game(request: Path | None = None, report: Path | None = None) -> bool:
    """Whether the live game is held right now: asked for AND reported held.

    Asked before anything clicks or keys the game's window: while this is true
    every screen up is one the operator opened or is reading.  Lifting the
    request answers False at once, before the mod has even resumed.
    """
    if not requested(request):
        return False
    return (_read_json(report or state_path()) or {}).get("held") is True


def requested_pace_ms(path: Path | None = None) -> int:
    """The live game's per-turn budget in ms; 0 (no wait) when unset or unreadable."""
    payload = _read_json(path or pace_path()) or {}
    value = payload.get("ms")
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value != value:
        return 0
    return max(0, min(MAX_PACE_MS, int(value)))


def set_pace(ms: int, path: Path | None = None) -> None:
    """Set the live game's Watch pace (what the mirror server does for the page)."""
    _atomic_json(path or pace_path(),
                 {"ms": max(0, min(MAX_PACE_MS, int(ms))), "updated_utc": utc_stamp()})


def write_state(tag: str, held: bool, turn: int | None,
                path: Path | None = None) -> None:
    """Report what the game is actually doing, for the page to show."""
    target = path or state_path()
    previous = _read_json(target) or {}
    payload: dict[str, Any] = {"tag": tag, "held": bool(held), "turn": turn,
                               "updated_utc": utc_stamp()}
    if held:
        same_hold = previous.get("tag") == tag and previous.get("held") is True
        payload["since_utc"] = (previous.get("since_utc") if same_hold
                                else None) or utc_stamp()
    _atomic_json(target, payload)


def release_state(tag: str, path: Path | None = None) -> None:
    """Say this run no longer holds the game (it resumed, ended or exited).

    Leaves another run's report alone: a stale writer must not erase the hold
    of the game that replaced it.
    """
    target = path or state_path()
    previous = _read_json(target)
    if previous is None or previous.get("tag") not in (None, tag):
        return
    if previous.get("held") is not True:
        return
    try:
        write_state(tag, False, previous.get("turn"), path=target)
    except OSError:
        pass


def status() -> dict[str, Any]:
    request = _read_json(request_path()) if request_path().exists() else None
    report = _read_json(state_path()) or {}
    return {
        "requested": requested(),
        "requested_utc": (request or {}).get("requested_utc"),
        "source": (request or {}).get("source"),
        "held": report.get("held") is True,
        "tag": report.get("tag"),
        "turn": report.get("turn"),
        "since_utc": report.get("since_utc"),
        "pace_ms": requested_pace_ms(),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("action", choices=("pause", "resume", "status"))
    args = parser.parse_args(argv)
    if args.action == "pause":
        set_requested(True, source="cli")
    elif args.action == "resume":
        set_requested(False)
    print(json.dumps(status(), sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
