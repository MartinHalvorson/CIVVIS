"""The turn the CIVVIS page has painted, handed from the mirror to the live game.

Operator 2026-10-30: "make sure the firaxis and civvis games aren't more than 1
turn apart. civvis should be driving firaxis so they should be roughly on the
same turn (+-1) at all times".  The page beside Civilization VI is rebuilt from
the game's own exports (``tools/follow.py``), so it can only trail the game,
and nothing used to stop the game running ahead of it: at 5.5 s a turn the page
painted each turn 2.5-5 s after the game began it, two turns behind at times.

The pieces are separate processes, so the protocol is one small host file:

``~/.civvis-mirror-shown.json`` -- the REPORT.  ``tools/follow.py`` writes it
    while it serves a run: the run, the game's own turn, the turn it last
    published to the mirror server, and the turn the page says it painted
    (``/status`` ``frames_painted``).  It is rewritten at least once a second,
    so a follower that stopped is told from one that has nothing new to say.

``civ6_play.py`` reads it (:func:`shown_turn`) and carries the turn to the mod
as an out-of-band ``mirror`` row in the run's orders database
(:func:`orders.set_mirror_turn`); the mod does not END a played turn until
that turn is on the page (``CivvisBoard.mirrorHold``), capped per turn so a
page that cannot paint never stalls the lane.

★ THE OPENING IS HELD TOO (operator 11-02: the videos "jump from turn 1 to
turn 5. 2, 3, 4 should all be present. no turns skipped").  The mirror server
cannot start until the game exports its first map, which it does at turn 1;
the server then boots and the browser reloads, ~4 s that the opening plays
through at ~1 s a turn.  The follower said nothing meanwhile, and no report is
"no page", which is not waited for -- so every game of 11-02 put turn 1 on the
page and then turn 3, 4 or 5.  Now the follower writes a PENDING report from
the moment it adopts a run until its page answers, and a pending report shows
turn 0: the game holds turn 1 until the page paints it, and every turn after.
A page that never comes costs one capped wait; ``civ6_play`` then lets it go.
"""

from __future__ import annotations

import json
import os
import tempfile
import time
from pathlib import Path
from typing import Any

SHOWN_ENV = "CIVVIS_MIRROR_SHOWN_FILE"

#: A report older than this is a follower that stopped: the game plays on
#: without waiting for a page nobody is drawing.
STALE_SECONDS = 5.0


def shown_path() -> Path:
    configured = os.environ.get(SHOWN_ENV)
    if configured:
        return Path(configured).expanduser()
    return Path.home() / ".civvis-mirror-shown.json"


def write_shown(run: str, game_turn: int | None, published: int | None,
                painted: int | None, viewers: int | None,
                path: Path | None = None, now: float | None = None,
                pending: bool = False) -> None:
    """Report what the mirror has on screen for ``run`` (atomically).

    ``pending``: the follower serves ``run`` but its page has nothing of it
    yet (no map export, or the server and browser still coming up).
    """
    target = path or shown_path()
    payload: dict[str, Any] = {
        "run": run, "game_turn": game_turn, "published": published,
        "painted": painted, "viewers": viewers,
        "updated_at": time.time() if now is None else now,
    }
    if pending:
        payload["pending"] = True
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=target.parent,
            prefix=f".{target.name}.", suffix=".tmp", delete=False,
        ) as handle:
            temporary_name = handle.name
            json.dump(payload, handle, sort_keys=True)
            handle.write("\n")
        os.replace(temporary_name, target)
        temporary_name = None
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except OSError:
                pass


def read_shown(path: Path | None = None) -> dict[str, Any] | None:
    try:
        value = json.loads((path or shown_path()).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


def _turn(value: Any) -> int | None:
    return value if isinstance(value, int) and not isinstance(value, bool) else None


def shown_turn(run: str, path: Path | None = None,
               now: float | None = None) -> int | None:
    """The turn the page shows for ``run``; ``None`` when nothing is to wait for.

    The PAINTED turn while a page is watching, since that is what the operator
    sees beside the game; the published one when no page is attached (the
    server still holds the board); 0 while the follower says this run's page is
    still coming (``pending``), so the opening waits for it.  ``None`` for
    another run's report, a stale one, or one with no turn at all.
    """
    report = read_shown(path)
    if report is None or report.get("run") != run:
        return None
    updated = report.get("updated_at")
    if isinstance(updated, bool) or not isinstance(updated, (int, float)):
        return None
    age = (time.time() if now is None else now) - float(updated)
    # A second of slack the other way: the writer's clock is this host's.
    if not -1.0 <= age <= STALE_SECONDS:
        return None
    painted, published = _turn(report.get("painted")), _turn(report.get("published"))
    viewers = _turn(report.get("viewers")) or 0
    if viewers > 0 and painted is not None:
        return painted
    if published is None and painted is None and report.get("pending") is True:
        return 0
    return published
