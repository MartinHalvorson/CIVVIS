"""macOS window and capture primitives for the live Civilization VI harness.

``civ6_play`` owns the run lifecycle and game setup policy.  This module owns
the host-facing operations it needs: checking the session/window geometry,
placing or focusing the application, clicking, and taking verified screen
captures.  Keeping that boundary here lets the launcher stay orchestration
code and lets the macOS contracts be tested without importing the full runner.
"""

from __future__ import annotations

import subprocess
import textwrap
import time
from pathlib import Path

from civ6_control import macos_capture, macos_input, popup_clear


#: Seconds any single host probe may take before the loop gives up on it.
#:
#: These probes run on every poll of the loop that drives the game.  System
#: Events can block on Accessibility when a foreground app is busy, so unknown
#: geometry must be retried on the next poll rather than stall the game.
HOST_PROBE_TIMEOUT_S = 10.0

#: Waits between setup screenshot attempts.  The sequence is bounded and
#: spreads retries across a transient host-load spike rather than sampling it
#: repeatedly at the same instant.
SHOT_BACKOFF_SECONDS = (0.5, 1.5, 3.0, 4.0)
CAPTURE_ACCESS_POLL_SECONDS = 10.0
# Setup readers already have an outer bounded poll, so they take one native
# capture per pass instead of spending the whole retry schedule inside it.
SETUP_SCREENSHOT_ATTEMPTS = 1

# The autoclose handler photographs a screen only to decide whether its
# optional, visually-confirmed rescue is safe.  A failed frame is not a reason
# to retry five times in the same event: the Lua context will ask again while
# it remains visible, and a second event gives the capture service time to
# recover.  The normal screenshot callers retain the transient-frame retry
# schedule below.
AUTOCLOSE_SCREENSHOT_PREFIX = "autoclose-stuck-turn-"
AUTOCLOSE_SCREENSHOT_ATTEMPTS = 1


def game_window(game_process: str) -> tuple[int, int, int, int] | None:
    """Position and size of the game window in points, or ``None``."""
    script = ('tell application "System Events" to tell '
              f'process "{game_process}" to '
              'get {position, size} of window 1')
    try:
        out = subprocess.run(["osascript", "-e", script], capture_output=True,
                             text=True, timeout=HOST_PROBE_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        print(f"[window] System Events did not answer in "
              f"{HOST_PROBE_TIMEOUT_S:g}s; treating the geometry as unknown",
              flush=True)
        return None
    parts = [part.strip() for part in out.stdout.split(",") if part.strip()]
    if len(parts) != 4 or not all(part.lstrip("-").isdigit() for part in parts):
        return None
    x, y, width, height = (int(part) for part in parts)
    return (x, y, width, height) if width > 400 and height > 300 else None


def screen_locked() -> bool:
    """Return whether the active macOS console session is locked.

    An unavailable probe is deliberately treated as unlocked: callers pause
    while this returns true, so a stuck ``ioreg`` must not park a healthy game.
    """
    try:
        result = subprocess.run(
            ["ioreg", "-n", "Root", "-d1"],
            capture_output=True,
            text=True,
            check=False,
            timeout=HOST_PROBE_TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        print(f"[session] ioreg did not answer in {HOST_PROBE_TIMEOUT_S:g}s; "
              "assuming the console is usable and continuing", flush=True)
        return False
    except OSError:
        return False
    return 'CGSSessionScreenIsLocked"=Yes' in result.stdout


def wait_for_unlocked_session(*, is_locked=None, poll_s: float = 2.0) -> None:
    """Wait at macOS's authentication boundary instead of aborting the run."""
    if is_locked is None:
        is_locked = screen_locked
    if not is_locked():
        return
    print("[session] macOS is locked; waiting to continue after unlock", flush=True)
    while is_locked():
        time.sleep(poll_s)
    print("[session] macOS unlocked; continuing", flush=True)


# ``swift -`` starts an AppKit interpreter.  One harness process plays one
# game, so a verified answer is stable for its lifetime.  Do not cache None: a
# transient AppKit failure must be able to recover on the next read.
_desktop_size_cache: tuple[int, int] | None = None


def reset_desktop_size_cache() -> None:
    """Forget the process-local display measurement (used by focused tests)."""
    global _desktop_size_cache
    _desktop_size_cache = None


def desktop_size() -> tuple[int, int] | None:
    """Logical size of the main display in points, or ``None`` if unreadable.

    This deliberately asks AppKit for one display, not Finder for the desktop
    union: sizing a window from a multi-display union can place it off-screen.
    """
    global _desktop_size_cache
    if _desktop_size_cache is not None:
        return _desktop_size_cache
    swift = textwrap.dedent("""
        import AppKit
        if let s = NSScreen.screens.first(where: { $0.frame.origin == .zero })
                    ?? NSScreen.main {
            print("\\(Int(s.frame.width)),\\(Int(s.frame.height))")
        }
    """)
    try:
        out = subprocess.run(["swift", "-"], input=swift,
                             capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    parts = [part.strip() for part in out.stdout.split(",") if part.strip()]
    if len(parts) != 2 or not all(part.isdigit() for part in parts):
        return None
    width, height = (int(part) for part in parts)
    # Window geometry is in points.  Taller than ~1700 points is a stacked
    # display union rather than a supported single Apple display.
    if not (800 < width <= 4000 and 600 < height <= 2000):
        return None
    _desktop_size_cache = (width, height)
    return _desktop_size_cache


def place_game(game_process: str, side: str = "left", fraction: float = 0.5,
               vfraction: float = 1.0, *, get_desktop_size=None,
               get_game_window=None) -> None:
    """Park the game on a screen portion without risking a blocking probe."""
    if side == "none":
        return
    if get_desktop_size is None:
        get_desktop_size = desktop_size
    if get_game_window is None:
        get_game_window = lambda: game_window(game_process)
    size = get_desktop_size()
    if size is None:
        return
    screen_w, screen_h = size
    menu = 33  # a y=0 window hides behind the menu bar
    width = max(640, int(screen_w * fraction))
    height = max(480, int((screen_h - menu) * max(0.1, min(1.0, vfraction))))
    if side == "bottomright":
        x, y = screen_w - width, screen_h - height
    else:
        x, y = (0 if side == "left" else screen_w - width), menu
    desired = (x, y, width, height)
    # Repeating an identical placement creates WindowServer traffic and can
    # make unrelated windows reflow, so leave an unchanged frame alone.
    if get_game_window() == desired:
        return
    script = (
        'tell application "System Events" to tell '
        f'process "{game_process}" to tell window 1\n'
        f'  set size to {{{width}, {height}}}\n'
        # Aspyr constrains the existing origin while applying a smaller size.
        f'  set position to {{{x}, {y}}}\n'
        'end tell')
    _best_effort_osascript(script, "place")


def _best_effort_osascript(script: str, what: str) -> None:
    """Run a retryable System Events action without risking the driving loop."""
    try:
        subprocess.run(["osascript", "-e", script], capture_output=True,
                       timeout=HOST_PROBE_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        print(f"[window] System Events did not answer in "
              f"{HOST_PROBE_TIMEOUT_S:g}s; skipping this {what} and "
              "retrying next poll", flush=True)


def focus_game(game_process: str) -> None:
    """Raise the game without moving it.

    Setup OCR reads before it clicks, so resizing while focusing would make a
    verified coordinate stale.  Placement belongs only in the in-game loop.
    """
    script = ('tell application "System Events" to set frontmost of '
              f'process "{game_process}" to true')
    _best_effort_osascript(script, "focus")


def click_at(px: int, py: int) -> None:
    """Move first, then make the held click Civilization VI accepts."""
    macos_input.move(px, py)
    time.sleep(0.5)
    # A zero-length cliclick press does not reliably trigger leader-dialogue
    # buttons; the held press is intentional.
    macos_input.click(px, py, hold_s=0.12)


def park_setup_pointer(bounds: tuple[int, int, int, int]) -> None:
    """Move the pointer to inert artwork before setup OCR reads the frame."""
    x, y, width, height = bounds
    macos_input.move(int(x + width * 0.15), int(y + height * 0.85))


def wait_for_safe_screen_capture(
    poll_s: float = CAPTURE_ACCESS_POLL_SECONDS,
) -> None:
    """Wait for a non-interactive screen-capture path before touching Civ VI."""
    last_reason = None
    while True:
        recording_ui = popup_clear.native_recording_ui_active()
        try:
            if macos_capture.screen_capture_access_available():
                if recording_ui:
                    print("[capture] native macOS recording/capture UI is active; using "
                          "pre-authorized CoreGraphics capture", flush=True)
                elif last_reason is not None:
                    print("[capture] safe screen capture is available; continuing", flush=True)
                return
            reason = "screen capture access is unavailable"
            if recording_ui:
                reason += " while a native macOS recording/capture UI is active"
        except macos_capture.CaptureUnavailable as error:
            reason = f"native screen capture is unavailable: {error}"
        if reason != last_reason:
            print(f"[capture] {reason}; waiting without opening a permission popup", flush=True)
            last_reason = reason
        time.sleep(poll_s)


def screenshot(path: Path, *, attempts: int | None = None,
               get_desktop_size=None) -> bool:
    """Capture a verified frame, retrying bounded transient host failures."""
    if get_desktop_size is None:
        get_desktop_size = desktop_size
    size = get_desktop_size()
    if size is None:
        print(f"[shot] display geometry is unreadable for {path.name}; treating this poll as "
              "unreadable", flush=True)
        return False
    if attempts is None:
        if path.name.startswith(AUTOCLOSE_SCREENSHOT_PREFIX):
            attempt_limit = AUTOCLOSE_SCREENSHOT_ATTEMPTS
        else:
            attempt_limit = len(SHOT_BACKOFF_SECONDS) + 1
    else:
        try:
            attempt_limit = max(1, int(attempts))
        except (TypeError, ValueError):
            attempt_limit = len(SHOT_BACKOFF_SECONDS) + 1
        attempt_limit = min(attempt_limit, len(SHOT_BACKOFF_SECONDS) + 1)
    for attempt in range(1, attempt_limit + 1):
        path.unlink(missing_ok=True)
        try:
            macos_capture.capture_region((0, 0, *size), path)
        except macos_capture.CapturePermissionUnavailable:
            print(f"[shot] screen capture access is unavailable for {path.name}; refusing to "
                  "open a permission popup", flush=True)
            return False
        except (macos_capture.CaptureUnavailable, OSError, subprocess.SubprocessError):
            pass
        try:
            if path.stat().st_size > 0:
                if attempt > 1:
                    print(f"[shot] native capture needed {attempt} attempts for "
                          f"{path.name}; the host is loaded", flush=True)
                return True
        except OSError:
            pass
        if attempt < attempt_limit:
            time.sleep(SHOT_BACKOFF_SECONDS[attempt - 1])
    print(f"[shot] native capture wrote nothing for {path.name} after "
          f"{attempt_limit} attempts; treating this poll as unreadable", flush=True)
    return False
