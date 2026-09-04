#!/usr/bin/env python3
"""Start CIVVIS's fixed recorded-game profile without taking a screen capture.

This is intentionally a small, host-specific launcher rather than a second
general Civ VI menu driver.  The ordinary verification player reads menus with
screenshots; that is the right way to support arbitrary settings, but it is not
appropriate while the operator is recording the desktop.  This helper uses the
same known Create Game controls that were verified on the recorded Mac and lets
the installed control mod's FrontEnd defaults supply the remaining settings.

The caller must have already installed a Rome / Emperor / Online / Continents
configuration and must run from Terminal (the process with the host's
Accessibility grant).  No image capture, OCR, or image-derived action is used
here.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_input, macos_window  # noqa: E402


GAME_PROCESS = "Civ6_Exe_Child"

# These two menu rows are deliberately absolute display points.  Civ VI's
# FrontEnd uses a fixed 1024x768 shell and the recorded host's 1728x1117
# desktop maps Single Player and Create Game to these controls.  Once Create
# Game is open, the remaining controls are positioned relative to the
# right-half game window so moving that window does not move the targets.
SINGLE_PLAYER_POINT = (1272, 284)
CREATE_GAME_POINT = (1327, 351)

# A frontmost transition can consume the first pointer event while macOS makes
# Civ VI's window key.  The normal visual bootstrap primes the same way before
# aiming at a menu row.  Keep this well inside empty artwork so a transition
# that has already completed cannot turn the priming event into a menu choice.
MENU_ACTIVATION_FRACTION = (0.150, 0.850)

MENU_SETTLE_S = 15.0
SUBMENU_SETTLE_S = 6.0
CREATE_GAME_SETTLE_S = 8.0


def click_point(x: int, y: int) -> None:
    """Focus Civ VI and click a known control without reading the desktop."""
    macos_window.focus_game(GAME_PROCESS)
    macos_input.move(x, y, check=True)
    # Civ's custom controls can ignore a press in the compositor frame that
    # immediately follows a move.  This matches the normal window helper's
    # proven cadence without inspecting any pixels.
    time.sleep(0.5)
    macos_input.click(x, y, hold_s=0.12, check=True)


def click_window_fraction(fx: float, fy: float) -> None:
    """Click a fixed ScenarioSetup control within the prepared game window."""
    bounds = macos_window.game_window(GAME_PROCESS)
    if bounds is None:
        raise RuntimeError("Civ VI window is unavailable")
    x, y, width, height = bounds
    click_point(round(x + width * fx), round(y + height * fy))


def prepare_game_window() -> None:
    """Put the game in the geometry used by the known Create Game controls."""
    macos_window.place_game(GAME_PROCESS, "right", 0.5, 0.5)
    time.sleep(1.0)
    macos_window.focus_game(GAME_PROCESS)


def start_direct_game(*, restore_defaults: bool = True,
                      emperor_online: bool = True,
                      start_only: bool = False) -> None:
    """Press Create Game's deterministic Start Game control.

    ``restore_defaults`` ensures that a preceding session cannot leak a game
    mode or lobby setting into this game.  The installed FrontEnd database
    supplies Rome, Continents, Small, and the turn cap; Emperor and Online are
    also selected explicitly because their ScenarioSetup rows are stable on
    this host.
    """
    prepare_game_window()

    if not start_only:
        # The event log tells us the core is ready, but not that the FrontEnd
        # has completed its logo/tutorial transition.  Keep the normal
        # controller's transition allowance.  Focus alone is not enough: the
        # first pointer event after a frontmost transition can be consumed
        # while macOS makes the Civ VI window key.  Prime it on empty artwork,
        # as the normal visual bootstrap does, before clicking a menu row.
        time.sleep(MENU_SETTLE_S)
        click_window_fraction(*MENU_ACTIVATION_FRACTION)
        time.sleep(1.5)
        click_point(*SINGLE_PLAYER_POINT)
        time.sleep(SUBMENU_SETTLE_S)
        click_point(*CREATE_GAME_POINT)
        time.sleep(CREATE_GAME_SETTLE_S)

    if restore_defaults:
        click_window_fraction(0.150, 0.040)
        time.sleep(3.0)

    if emperor_online:
        # ScenarioSetup.xml fixes these rows for the verified right-half,
        # half-height window: difficulty's sixth choice is Emperor, and
        # Online is the first speed choice.  This does not infer controls from
        # screen pixels; it is the fixed recorded profile.
        click_window_fraction(0.500, 0.316)
        time.sleep(0.8)
        click_window_fraction(0.500, 0.511)
        time.sleep(0.8)
        click_window_fraction(0.500, 0.406)
        time.sleep(0.8)
        click_window_fraction(0.500, 0.447)
        time.sleep(1.0)

    click_window_fraction(0.500, 0.978)
    # Return commits Start Game when the pointer press landed during the final
    # animation, or clears the first Begin Game gate once hosting has begun.
    time.sleep(0.75)
    macos_input.press_key("return", check=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", required=True,
                        help="run identity retained in the caller's log")
    parser.add_argument("--start-only", action="store_true")
    parser.add_argument("--no-restore-defaults", dest="restore_defaults",
                        action="store_false")
    parser.add_argument("--no-emperor-online", dest="emperor_online",
                        action="store_false")
    parser.set_defaults(restore_defaults=True, emperor_online=True)
    args = parser.parse_args(argv)
    try:
        start_direct_game(restore_defaults=args.restore_defaults,
                          emperor_online=args.emperor_online,
                          start_only=args.start_only)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"capture-free setup failed for {args.tag}: {error}",
              file=sys.stderr)
        return 2
    print(f"[capture-free-setup] pressed Create Game for {args.tag}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
