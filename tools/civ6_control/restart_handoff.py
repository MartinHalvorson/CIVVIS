#!/usr/bin/env python3
"""New game by restart: hand a parked Civilization VI to the next run.

Operator 2026-10-15: "new game button can restart the game if that is faster
and we want the same settings".  The page's New game used to retire the game
natively, quit Civilization VI, and let the next run launch it again and click
through Create Game -- about 65 s from the press to turn 1, most of it the
relaunch, the logos and the setup clicks (which also take the keyboard from
whoever is typing).  A restart inside the running game skips all of that.

A run is still one game: the retiring run keeps its tag, its record and its
``operator_retired`` ending, and the next run is a new tag, installed by its
own harness.  What crosses between them is Civilization VI itself, so the
handoff is one host file:

``~/.civvis-live-restart.json`` -- written by the retiring run's ``civ6_play``
    when its control mod reports the game PARKED for a restart (``retired``
    with ``why = "restart"``) instead of retired.  It names that run, its
    orders database, the Civ VI processes it leaves running and the settings
    the game was played at.  While it stands, the retiring run's harness, the
    climb's teardown and the climb's "something already holds the game" check
    all leave that game up.

The next run's ``civ6_play`` CLAIMS the file (it is single-use: claiming
removes it).  When the game it names is still the running one, the request is
fresh and the settings are the next run's own, the next run installs its mod
over the parked one, writes a ``restart`` row into the PARKED run's orders
channel, and the parked agent starts a fresh game in place (see
``CivvisBoard.restartPoll`` in ``mod/CivvisControlAgent.lua``).  The fresh
game loads the mod from disk, so it plays as the next run.  Anything else --
no file, a stale one, other settings, a restart that never brings the next
run's agent up -- falls back to quitting Civ VI and launching it as before.

Every reader fails toward the old path: a missing or malformed file is "no
handoff", never a reason to leave a game running.
"""

from __future__ import annotations

import json
import os
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

HANDOFF_ENV = "CIVVIS_LIVE_RESTART_FILE"
MODE_ENV = "CIVVIS_LIVE_RESTART_MODE"

#: A parked game older than this is a leftover, not a handoff: the boundary
#: between two runs (the climb's write-up, the supervisor's next cycle, the
#: next harness starting) is ~20-40 s.
MAX_AGE_S = 300.0

#: What the restarted game is asked to do (`x` on the `restart` row).
MODE_RESTART = 1     # new seeds, then the shipped Restart (`Network.RestartGame`)
MODE_HOST = 2        # new seeds, then host the current configuration again
MODE_SAME_MAP = 3    # the shipped Restart with the seeds it had
MODES = (MODE_RESTART, MODE_HOST, MODE_SAME_MAP)

#: The settings a restart keeps.  The next run must have asked for exactly
#: these, or the restarted game would be a different game than it records.
SETTING_KEYS = (
    "RuleSet", "MapScript", "MapSize", "Difficulty", "GameSpeed", "Leader",
    "StartEra", "MaxTurns", "HumanPlayers", "GameModes",
)


def handoff_path() -> Path:
    configured = os.environ.get(HANDOFF_ENV)
    return (Path(configured).expanduser() if configured
            else Path.home() / ".civvis-live-restart.json")


def restart_mode() -> int:
    """The restart call to ask for; `CIVVIS_LIVE_RESTART_MODE` overrides."""
    try:
        mode = int(os.environ.get(MODE_ENV) or MODE_RESTART)
    except ValueError:
        return MODE_RESTART
    return mode if mode in MODES else MODE_RESTART


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def settings_of(config: dict[str, Any]) -> dict[str, Any]:
    return {key: config.get(key) for key in SETTING_KEYS}


def _atomic_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=path.parent,
            prefix=f".{path.name}.", suffix=".tmp", delete=False,
        ) as handle:
            temporary = handle.name
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError:
        if temporary:
            Path(temporary).unlink(missing_ok=True)
        raise


def write(tag: str, orders_db: Path | str, config: dict[str, Any],
          civ_pids: Iterable[int], *, path: Path | None = None,
          now: float | None = None) -> dict[str, Any]:
    """Record that run `tag` parked the running game for the next run."""
    payload = {
        "from_tag": tag,
        "orders_db": str(orders_db),
        "civ_pids": sorted(int(pid) for pid in civ_pids),
        "settings": settings_of(config),
        "parked_utc": utc_stamp(),
        "parked_epoch": time.time() if now is None else now,
    }
    _atomic_json(path or handoff_path(), payload)
    return payload


def read(path: Path | None = None) -> dict[str, Any] | None:
    try:
        value = json.loads((path or handoff_path()).read_text())
    except (OSError, ValueError):
        return None
    if (not isinstance(value, dict) or not isinstance(value.get("from_tag"), str)
            or not isinstance(value.get("civ_pids"), list)):
        return None
    return value


def discard(path: Path | None = None) -> None:
    try:
        (path or handoff_path()).unlink()
    except OSError:
        pass


def parked_for(tag: str, path: Path | None = None) -> bool:
    """Whether run `tag` left the running game parked for the next run."""
    handoff = read(path)
    return handoff is not None and handoff.get("from_tag") == tag


def standing(game_pids: Iterable[int], path: Path | None = None) -> dict[str, Any] | None:
    """The handoff that accounts for exactly the Civ VI processes running now.

    Age is not checked here: a parked game is the lane's own leftover whatever
    its age, and it is the next run's to reuse or to stop -- never a reason to
    refuse that run as if somebody else held the game.
    """
    handoff = read(path)
    running = sorted(int(pid) for pid in game_pids)
    if handoff is None or not running:
        return None
    return handoff if sorted(handoff["civ_pids"]) == running else None


def claim(config: dict[str, Any], game_pids: Iterable[int], *,
          path: Path | None = None, now: float | None = None
          ) -> tuple[dict[str, Any] | None, str | None]:
    """Take the handoff for the run `config` describes, or say why not.

    Single-use: whatever the answer, the file is gone afterwards, so a refused
    or failed restart can never be retried against the same parked game.
    Returns ``(handoff, None)`` to restart in place, ``(None, reason)`` to
    launch afresh, and ``(None, None)`` when there was no handoff at all.
    """
    target = path or handoff_path()
    if not target.exists():
        return None, None
    handoff = read(target)
    discard(target)
    if handoff is None:
        return None, "the restart handoff could not be read"
    pids = sorted(int(pid) for pid in game_pids)
    if not pids:
        return None, "the parked game is no longer running"
    if sorted(handoff["civ_pids"]) != pids:
        return None, (f"the running game {pids} is not the one "
                      f"{handoff['from_tag']} parked {handoff['civ_pids']}")
    age = (time.time() if now is None else now) - float(handoff.get("parked_epoch") or 0)
    if age > MAX_AGE_S:
        return None, f"the parked game waited {age:.0f}s, past {MAX_AGE_S:.0f}s"
    wanted = settings_of(config)
    if handoff.get("settings") != wanted:
        changed = sorted(key for key in SETTING_KEYS
                         if (handoff.get("settings") or {}).get(key) != wanted.get(key))
        return None, f"this run asks for different settings ({', '.join(changed)})"
    if not handoff.get("orders_db"):
        return None, "the parked run left no orders channel to ask through"
    return handoff, None
