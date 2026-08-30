#!/usr/bin/env python3
"""Ask a parked Civilization VI to end its turn, from outside the game.

⚠⚠⚠ THIS IS A LAST RESORT, AND ITS SAFETY COMES FROM WHERE IT IS CALLED.

The wedge watchdog runs it immediately before killing a game it has already
judged dead.  A keystroke that lands on a healthy game would be a real hazard —
SHIFT+RETURN ends a turn — so nothing else may call this, and the watchdog's
own no-progress test is the gate.

Why a keystroke at all.  The dominant way a run dies is a parked Game Core: the
agent reports one blocker, `GameCoreEventPublishComplete` stops firing, and the
agent — driven only by that event — never ticks again.  Nine of twelve recent
games ended this way, against three that reached the operator's abandon rule.
The mod cannot reach the game from inside once that happens, and there is no
UI-side tick to borrow either: `ContextPtr:SetUpdate` runs only while its
context is visible (measured, #2784).

An external keystroke is the one remaining path.  It enters through the OS into
the application's event loop rather than through any mod tick, so it does not
depend on the Game Core publishing anything.

SHIFT+RETURN is Civilization VI's forced end turn — the request the shipped UI
sends, and the one end-turn form the engine does not refuse while a blocker
stands.  It is the same action the control mod issues as
`UI.RequestAction(ACTION_ENDTURN, { REASON = "UserForced" })` when it can still
run at all.

⚠ Escape is NOT an alternative.  With nothing to close it opens the pause menu,
which stops the game advancing and has already cost a run.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_input, popup_clear  # noqa: E402

#: One source of truth for the process name. `civ6_play` takes it from here too
#: (`GAME_PROCESS = popup_clear.GAME_PROCESS`); a second copy would drift.
GAME_PROCESS = popup_clear.GAME_PROCESS

#: How many forced end turns to send.  More than one because the first may land
#: while the game is between frames, few because each one is a real keystroke.
NUDGE_PRESSES = 3

#: Seconds between presses.  Long enough for the game to act on one before the
#: next arrives, short enough that the whole attempt fits inside the watchdog's
#: patience.
NUDGE_INTERVAL_S = 1.5

#: Seconds to let the raise take effect before the first key. A keystroke that
#: arrives while the window is still coming forward lands on the old frontmost
#: application.
FOCUS_SETTLE_S = 1.0


def focus_game() -> bool:
    """Raise Civilization VI so the keystroke reaches IT.

    ⚠⚠⚠ WITHOUT THIS THE NUDGE IS SENT TO WHATEVER IS FRONTMOST, and the
    harness reports it as sent regardless — `cliclick` exits zero whoever
    receives the key. The first live firing (2026-08-30T04:39) reported
    "sent SHIFT+RETURN to a parked game" and then "the forced end turn changed
    nothing", which is exactly what an unfocused keystroke also looks like. So
    that result proved nothing until this existed.

    ⚠ Raise only. Deliberately NOT placing or resizing the window: `civ6_play`
    records that re-placing on every focus pass resized the window between a
    menu read and its click and cost a whole run.
    """
    script = ('tell application "System Events" to set frontmost of '
              f'process "{GAME_PROCESS}" to true')
    try:
        done = subprocess.run(["osascript", "-e", script],
                              capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return False
    return done.returncode == 0


def nudge(presses: int = NUDGE_PRESSES, interval_s: float = NUDGE_INTERVAL_S) -> bool:
    """Send SHIFT+RETURN a few times.  Returns whether every press was sent.

    Sending is not landing: this reports what the harness managed to emit, and
    whether the turn actually advanced is for the caller to observe.  Saying
    otherwise would be the kind of claim that reads as a fix and is not one.
    """
    if not focus_game():
        return False
    # Give the raise a moment to take effect before the first key.
    time.sleep(FOCUS_SETTLE_S)
    sent = True
    for index in range(presses):
        if index:
            time.sleep(interval_s)
        try:
            result = macos_input.press_key("return", modifier="shift")
        except (OSError, ValueError, macos_input.InputUnavailable):
            return False
        sent = sent and getattr(result, "returncode", 1) == 0
    return sent


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--presses", type=int, default=NUDGE_PRESSES)
    parser.add_argument("--interval", type=float, default=NUDGE_INTERVAL_S)
    args = parser.parse_args(argv)
    ok = nudge(args.presses, args.interval)
    print(f"[nudge] SHIFT+RETURN x{args.presses} {'sent' if ok else 'FAILED'}",
          flush=True)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
