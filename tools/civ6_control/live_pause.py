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
mod honours it at the opening of our turn — after the turn's board has gone
out, before any of its orders are applied — and simply stops playing: Civ VI
waits on our turn, the AI cannot move, and nothing is lost.  Every watchdog
that measures turn progress reads :func:`requested` and stands down while it
holds, so a paused game is never mistaken for a wedged one.

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


def request_path() -> Path:
    configured = os.environ.get(REQUEST_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-pause.json")


def state_path() -> Path:
    configured = os.environ.get(STATE_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-pause-state.json")


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
