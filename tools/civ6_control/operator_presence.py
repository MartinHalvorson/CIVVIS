"""Is a person using this Mac right now?

The live harness drives Civilization VI through the GUI: it raises the game
window, moves the pointer, clicks. Every one of those takes the desktop away from
whoever is sitting at the machine. `~/.civvis-shared-desktop` was the answer, and
it is a blunt one -- with the marker present the game is never refocused, even
at 3 a.m. with nobody there, and the operator has to remember to remove it.

This module makes the decision automatic. macOS keeps a single clock of the time
since the last keyboard or pointer event (`HIDIdleTime`, in nanoseconds, on the
`IOHIDSystem` node). If that clock is young, a person is here and the harness
should keep its hands off; if it is old, the desktop is unattended and the
harness may do its upkeep.

⚠⚠ THE HARNESS'S OWN CLICKS RESET THAT CLOCK. A synthetic click through
`cliclick` or the native helper is a HID event like any other, so right after
the harness acts, `HIDIdleTime` says "someone just moved the mouse" -- and a
gate reading only that would defer forever, blocked by its own footsteps. So
`macos_input` records the wall-clock time of every synthetic event in a small
file, and an idle clock that resets at or after that moment is attributed to the
harness, not the person. Only an event the harness did not send counts as a
human.

The file, not a module global, because the popup clearer is a separate process
from the game controller and both of them click.
"""

from __future__ import annotations

import os
import subprocess
import time
from pathlib import Path

#: Seconds of human inactivity after which the desktop counts as unattended.
#: Long enough that a pause to read or think does not hand the desktop to the
#: harness mid-sentence; short enough that a game does not sit unfocused for
#: long after the operator walks away.
DEFAULT_THRESHOLD_SECONDS = 15.0
#: HID events this soon after a synthetic event are the synthetic event.
SYNTHETIC_ATTRIBUTION_SECONDS = 0.75
#: Where `macos_input` records its last event. Overridable for tests.
LAST_SYNTHETIC_FILE = Path(os.environ.get(
    "CIVVIS_LAST_SYNTHETIC_INPUT_FILE",
    str(Path.home() / ".civvis-last-synthetic-input")))
#: `ioreg` is fast, but a wedged IOKit answer must not stall a game turn.
IOREG_TIMEOUT_SECONDS = 2.0


def hid_idle_seconds() -> float | None:
    """Seconds since the last keyboard or pointer event, or None if unreadable."""
    try:
        result = subprocess.run(
            ["ioreg", "-c", "IOHIDSystem", "-d", "4"],
            capture_output=True, text=True, timeout=IOREG_TIMEOUT_SECONDS,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    for line in result.stdout.splitlines():
        if "HIDIdleTime" not in line:
            continue
        try:
            return int(line.rsplit("=", 1)[1].strip()) / 1e9
        except (IndexError, ValueError):
            return None
    return None


def note_synthetic_input(path: Path | None = None, now: float | None = None) -> None:
    """Record that the harness just sent a keyboard or pointer event."""
    path = LAST_SYNTHETIC_FILE if path is None else path
    stamp = time.time() if now is None else now
    try:
        path.write_text(f"{stamp:.3f}\n")
    except OSError:
        # Presence detection is a courtesy; input must never fail because of it.
        pass


def last_synthetic_input(path: Path | None = None) -> float | None:
    """Wall-clock time of the harness's last synthetic event, or None."""
    path = LAST_SYNTHETIC_FILE if path is None else path
    try:
        return float(path.read_text().strip())
    except (OSError, ValueError):
        return None


#: "Not injected" for `operator_active`'s test hooks, so that an explicit None
#: can mean "no synthetic event on record" -- the other test suites in a full
#: run exercise `macos_input` and write the real record file as they go.
_READ_IT = object()


def operator_active(threshold: float = DEFAULT_THRESHOLD_SECONDS, *,
                    idle: float | None = None, synthetic_at=_READ_IT,
                    now: float | None = None) -> bool:
    """Whether a person has used this Mac within ``threshold`` seconds.

    ``idle``, ``synthetic_at`` and ``now`` are injectable for tests; a real
    call reads them from the machine. An unreadable idle clock answers False:
    the harness has run unattended for months without this gate, so "cannot
    tell" must fall back to that behaviour rather than freezing the game.
    """
    if idle is None:
        idle = hid_idle_seconds()
    if idle is None:
        return False
    if idle >= threshold:
        return False
    now = time.time() if now is None else now
    last_event_at = now - idle
    if synthetic_at is _READ_IT:
        synthetic_at = last_synthetic_input()
    if synthetic_at is not None and last_event_at <= synthetic_at + SYNTHETIC_ATTRIBUTION_SECONDS:
        # The clock reset when WE clicked. That is not a person.
        return False
    return True
