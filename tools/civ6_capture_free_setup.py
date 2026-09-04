#!/usr/bin/env python3
"""Bootstrap CIVVIS's fixed recorded-game profile without screen capture.

This is intentionally a small, host-specific launcher rather than a second
general Civ VI menu driver.  The ordinary verification player reads menus with
screenshots; that is the right way to support arbitrary settings, but it is not
appropriate while the operator is recording the desktop.

The fixed Create Game screen was not reliable enough on this macOS front end:
an opening transition can leave its next recorded click on the Tutorial entry.
Instead, this helper uses the already-verified generic ``Play Now`` route.  The
in-game CIVVIS agent then rehosts exactly the requested Rome / Emperor / Online
/ Continents game through ``GameConfiguration``.  Its rehost receipt is the
authority for the final settings; the bootstrap game is deliberately never
played.

The caller must have already installed the control mod and must run from
Terminal (the process with the host's Accessibility grant).  No image capture,
OCR, or image-derived action is used here.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_env as env  # noqa: E402
from civ6_control import macos_input, macos_window  # noqa: E402


GAME_PROCESS = "Civ6_Exe_Child"

# These are the host's already-proven generic Play Now route, expressed as
# fractions of Civ VI's desktop render canvas.  The normal launcher has always
# used these desktop fractions: an accessibility window can be narrower when
# macOS reports Civ VI at a scaled resolution, while its front-end controls
# remain positioned against the full canvas.  Mapping the fractions into that
# narrower window selected the Alexander scenario instead of Play Now in the
# 2026-09-04 live recovery.  Unlike Create Game, this route never needs the
# ScenarioSetup panel that has recently transitioned into Tutorial before known
# controls landed.
SINGLE_PLAYER_FRACTION = (0.473, 0.441)
PLAY_NOW_FRACTION = (0.528, 0.619)
BEGIN_GAME_FRACTION = (0.392, 0.782)

# A frontmost transition can consume the first pointer event while macOS makes
# Civ VI's window key.  The normal visual bootstrap primes the same way before
# aiming at a menu row.  Keep this well inside empty artwork so a transition
# that has already completed cannot turn the priming event into a menu choice.
MENU_ACTIVATION_FRACTION = (0.150, 0.850)

MENU_SETTLE_S = 15.0
MENU_ACTIVATION_SETTLE_S = 1.5
SUBMENU_SETTLE_S = 6.0
BOOTSTRAP_READY_TIMEOUT_S = 300.0
BOOTSTRAP_READY_POLL_S = 2.0


def click_point(x: int, y: int) -> None:
    """Focus Civ VI and click a known control without reading the desktop."""
    macos_window.focus_game(GAME_PROCESS)
    macos_input.move(x, y, check=True)
    # Civ's custom controls can ignore a press in the compositor frame that
    # immediately follows a move.  This matches the normal window helper's
    # proven cadence without inspecting any pixels.
    time.sleep(0.5)
    macos_input.click(x, y, hold_s=0.12, check=True)


def click_desktop_fraction(fx: float, fy: float) -> None:
    """Click a known main-menu fraction within Civ VI's desktop canvas."""
    size = macos_window.desktop_size()
    if size is None:
        raise RuntimeError("desktop size is unavailable")
    width, height = size
    # Match the original generic launcher exactly: it truncates its calculated
    # coordinates rather than rounding across a menu-row boundary.
    click_point(int(width * fx), int(height * fy))


def prepare_bootstrap_window() -> None:
    """Put Civ VI in the geometry used by the recorded Play Now controls."""
    macos_window.place_game(GAME_PROCESS, "left", 1.0, 1.0)
    time.sleep(1.0)
    macos_window.focus_game(GAME_PROCESS)


def start_bootstrap_game() -> None:
    """Start only the generic game that will hand off to the in-game rehost.

    The controlled game must not use its generic settings.  Its sole purpose
    is to load the enabled-by-default CIVVIS in-game context, whose first
    startup pass applies the requested profile and issues ``Network.HostGame``.
    """
    prepare_bootstrap_window()
    # The mod-scan log proves only that the core is initialized.  Retain the
    # normal menu/logo allowance before touching its two known menu rows.  The
    # first pointer event can still be consumed while macOS keys the window,
    # so spend it on empty artwork just as the normal visual bootstrap does.
    time.sleep(MENU_SETTLE_S)
    click_desktop_fraction(*MENU_ACTIVATION_FRACTION)
    time.sleep(MENU_ACTIVATION_SETTLE_S)
    click_desktop_fraction(*SINGLE_PLAYER_FRACTION)
    time.sleep(SUBMENU_SETTLE_S)
    click_desktop_fraction(*PLAY_NOW_FRACTION)


def wait_for_agent_loaded(*, timeout_s: float = BOOTSTRAP_READY_TIMEOUT_S,
                          poll_s: float = BOOTSTRAP_READY_POLL_S,
                          automation_log: Path | None = None) -> bool:
    """Wait for the bootstrap game's in-game CIVVIS context by textual log.

    The ``loaded`` lifecycle record is written before the leader-introduction
    Begin Game gate.  It is therefore a deterministic, capture-free proof that
    the following recorded Begin Game press has a live game target.
    """
    log = automation_log or env.logs_dir() / "Automation.log"
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        try:
            if "CIVVISJSON " in log.read_text(errors="replace"):
                return True
        except OSError:
            pass
        if not env.game_pids():
            return False
        time.sleep(poll_s)
    return False


def begin_bootstrap_game() -> None:
    """Dismiss the generic game's recorded leader-introduction gate."""
    # Give the context that wrote ``loaded`` time to finish drawing its gate;
    # this is the same delay used by the previously verified Play Now harness.
    time.sleep(4.0)
    click_desktop_fraction(*BEGIN_GAME_FRACTION)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", required=True,
                        help="run identity retained in the caller's log")
    parser.add_argument("--ready-timeout", type=float,
                        default=BOOTSTRAP_READY_TIMEOUT_S)
    args = parser.parse_args(argv)
    try:
        start_bootstrap_game()
        if not wait_for_agent_loaded(timeout_s=args.ready_timeout):
            raise RuntimeError("bootstrap game did not load the CIVVIS agent")
        begin_bootstrap_game()
    except (OSError, RuntimeError, ValueError) as error:
        print(f"capture-free setup failed for {args.tag}: {error}",
              file=sys.stderr)
        return 2
    print(f"[capture-free-setup] started Play Now bootstrap for {args.tag}",
          flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
