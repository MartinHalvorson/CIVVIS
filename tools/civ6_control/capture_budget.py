"""When a desktop rescue may spend a native screen capture, and when it may not.

★★★★★ A CAPTURE THAT CANNOT SUCCEED STILL COSTS ELEVEN SECONDS, AND THE GAME
WAITS FOR EVERY ONE OF THEM.

The verification harness answers the control mod's `autoclose_desktop` ask on
the same thread that reads the mod's event log.  The answer photographs the
screen and then captures the game window a second time for the pixel
classifier, so one ask is two native captures.  A healthy capture takes 0.06 s.
A capture taken while `systemstatusd` is spinning takes 11.02 s and then fails:
the native helper's own guard gives up at 3.5 s, Python's kill timer at 7.5 s,
and the `--fallback` backend spends the rest before raising
`CaptureUnavailable`.  Two of those is 22 s, plus the window focus and its
settle sleep, which is the 23.5 s that separates `autoclose_desktop` from the
next line of the run.

Measured on this host, 2026-09-02:

* run `civvis-20260902T095330Z` (216 turns, 68.6 min): 25.8 min of wall clock
  sat in `autoclose_desktop -> next event` gaps.  38 % of the game.
* run `civvis-20260902T162829Z` (31.3 min so far): 23 such stalls, 9.5 min,
  30 % of the game, every one of them 23.5 s to within a tenth of a second.
  A uniform interval is a timeout, not a game timer.
* `popup_clear.capture_pause_reason()` answers "systemstatusd is spinning at
  99 % CPU" in 0.02 s -- 1/1000th of what the capture it predicts will cost.

So ask the cheap question first.  When the answer says a capture cannot work,
skip it and let the mod's own closer keep trying; it asks again every few
attempts anyway.

⚠⚠ SKIPPING MUST NOT BE PERMANENT.  A leader conversation cannot be dismissed
blind -- Escape does nothing on it, and Escape with nothing to close opens the
pause menu and kills the run -- so the pixel path is the only thing that can
rescue a genuinely stuck leader screen.  Run `civvis-20260829T093602Z` sat on
John Curtin's leader screen until the watchdog killed it at turn 40 because one
unreadable frame ended the only rescue that could have helped.  A predicted
failure is a prediction, not a fact: `systemstatusd` can be spinning and the
capture can still land.

So this is a BUDGET, not a veto.  While captures look unavailable each screen
still spends one real attempt every `RESCUE_INTERVAL_SECONDS`, and every ask in
between is declined for 0.02 s.  A stuck screen therefore costs 23.5 s a minute
instead of 23.5 s per ask, and a screen that is merely phantom -- up to the
context, invisible to the pixels, gone on its own in a moment -- costs nothing.
"""
from __future__ import annotations

import time

#: How long a screen waits between real capture attempts while the host says
#: capture is unavailable.  It bounds the worst case for a screen that truly
#: needs the desktop: one 23.5 s attempt a minute is 39 % of that minute, which
#: is bad, and the alternative -- an ask every four close attempts -- measured
#: as 30-38 % of a whole game.  The wedge watchdog allows five minutes, so five
#: rescue attempts still land inside its window.
RESCUE_INTERVAL_SECONDS = 60.0


class CaptureBudget:
    """Per-screen permission to spend a native capture on the desktop rescue.

    `clock` is injectable so the tests do not sleep; it must be monotonic.
    """

    def __init__(self, interval: float = RESCUE_INTERVAL_SECONDS,
                 clock=time.monotonic) -> None:
        self._interval = float(interval)
        self._clock = clock
        self._last_attempt: dict[str, float] = {}

    def spend(self, screen: object, unavailable_reason: object = None) -> tuple[bool, str]:
        """May `screen` spend a native capture now, and why or why not.

        `unavailable_reason` is `popup_clear.capture_pause_reason()`: a string
        saying the host cannot capture, or None when it believes it can.

        Returns `(allowed, note)`.  The note is for the run log either way --
        a declined ask that says nothing is indistinguishable from a rescue
        that never came, which is the failure this whole path exists to catch.
        """
        name = str(screen)
        now = self._clock()
        if not unavailable_reason:
            # The host believes capture works.  Do not throttle the rescue on a
            # healthy machine: the ask is rare and the capture is 0.06 s.
            self._last_attempt[name] = now
            return True, "capture available"
        last = self._last_attempt.get(name)
        if last is None or (now - last) >= self._interval:
            self._last_attempt[name] = now
            return True, f"{unavailable_reason}; spending the scheduled rescue attempt anyway"
        waited = now - last
        return False, (f"{unavailable_reason}; skipped, next attempt in "
                       f"{self._interval - waited:.0f}s")

    def record_success(self, screen: object) -> None:
        """A rescue that actually dismissed something clears the schedule.

        ⚠ A SCREEN CLOSING IS NOT SUCCESS.  The first draft reset the schedule
        on the mod's `autoclose gone:true`, which fires at the end of almost
        every one of these stalls -- the phantom context tears itself down on
        its own -- so the interval reset before it ever bound and the budget
        allowed every ask.  Only a click that landed is evidence the pixel path
        works on this screen, and that is the one thing worth being eager
        about.
        """
        self._last_attempt.pop(str(screen), None)
