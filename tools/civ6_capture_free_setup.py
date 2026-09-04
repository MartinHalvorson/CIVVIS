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

# These are the host's already-proven generic Play Now route, measured on its
# 1728x1117 display with Civ VI in the full usable window.  Unlike Create Game,
# it never needs the fixed ScenarioSetup panel that has recently transitioned
# into Tutorial before our known controls landed.
SINGLE_PLAYER_POINT = (817, 492)
PLAY_NOW_POINT = (912, 691)
BEGIN_GAME_POINT = (677, 873)

MENU_SETTLE_S = 15.0
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
    # normal menu/logo allowance before touching its two known menu rows.
    time.sleep(MENU_SETTLE_S)
    click_point(*SINGLE_PLAYER_POINT)
    time.sleep(SUBMENU_SETTLE_S)
    click_point(*PLAY_NOW_POINT)

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
    click_point(*BEGIN_GAME_POINT)


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
