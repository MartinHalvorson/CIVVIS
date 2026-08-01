#!/usr/bin/env python3
"""Start, stop and observe Civilization VI without touching the mouse.

``tools/civ6_launch.py`` starts the game the way a person does: ask Steam to
run it, find the Aspyr LaunchPad window, click PLAY, then click through the
main menu. That works, but every step is a guess about where a window is, and
a guess that is wrong produces a click somewhere else on the desktop.

This starts the game the way its own test harness does instead. Three facts
make that possible, and each was established by measurement on this build:

- **The game binary takes arguments.** ``Civ6.app/Contents/MacOS/Civ6_Exe`` is
  a stub; the process that is actually the game is ``Civ6_Exe_Child``, and
  running it directly with arguments works. It reaches the main menu in about
  three minutes with no LaunchPad and no clicking, which also removes the
  single most fragile step in the old path.
- **The Steam client has to be running,** but nothing else about Steam
  matters: the child loads ``steamclient.dylib`` from the running client and
  reports the app ID from the environment. Steam's own launcher will
  separately complain "Game configuration unavailable" if it sees the process
  it did not start; that popup is cosmetic and does not affect the game.
- **Startup is verifiable from the logs.** ``Logs/Modding.log`` gets its mod
  scan once the game core is up, and ``Logs/Automation.log`` is written from
  app init onwards whenever the automation system is active. Both live in the
  *nested* user directory (see ``civ6_env.user_dir``), and both are removed
  before launch so a stale file cannot be read as this run's.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import civ6_env as env  # noqa: E402

STEAM_APP_ID = "289070"

# Logs that identify a run rather than the installation. Cleared before launch
# so "did this run write it" is answerable without parsing timestamps.
RUN_LOGS = ("Automation.log", "Modding.log", "Lua.log")


def game_binary() -> Path:
    """The process that is actually the game.

    ``Civ6_Exe`` spawns ``Civ6_Exe_Child`` and exits; arguments given to the
    stub do not reach the child, and the stub refuses to start a second copy
    while one is running -- which reads as "the launch did nothing".
    """
    return env.install_dir() / "Civ6.app/Contents/MacOS/Civ6_Exe_Child"


def steam_running() -> bool:
    out = subprocess.run(["pgrep", "-f", "steam_osx"], capture_output=True, text=True)
    return bool(out.stdout.strip())


def stop(timeout_s: float = 45.0) -> bool:
    """Stop the game and confirm every process is gone.

    The game rewrites its options and its mod database while exiting, so a
    relaunch that overlaps the exit silently runs the previous configuration.
    """
    return env.quit_game(timeout_s)


def clear_run_logs() -> None:
    logs = env.logs_dir()
    for name in RUN_LOGS:
        path = logs / name
        if path.exists():
            path.unlink()


def launch(args: list[str] | None = None, stdout: Path | None = None) -> subprocess.Popen:
    """Start the game directly, with arguments, and return the process."""
    binary = game_binary()
    if not binary.is_file():
        raise SystemExit(f"game binary not found: {binary}")
    if not steam_running():
        raise SystemExit("the Steam client is not running; the game cannot initialise")
    environ = dict(os.environ, SteamAppId=STEAM_APP_ID, SteamGameId=STEAM_APP_ID)
    sink = open(stdout, "w") if stdout else subprocess.DEVNULL
    return subprocess.Popen(
        [str(binary), *(args or [])],
        cwd=str(binary.parent),
        env=environ,
        stdout=sink,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )


def wait_for_main_menu(timeout_s: float = 420.0, poll_s: float = 3.0) -> bool:
    """Wait until the game core has scanned mods, which means the menu is up.

    ``Modding.log`` gaining its "Discovered" line is the first thing the core
    writes that proves it got past engine init. Polling the process as well
    means a crash fails fast instead of waiting out the whole timeout.
    """
    log = env.logs_dir() / "Modding.log"
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        if log.is_file() and "Discovered" in log.read_text(errors="replace"):
            return True
        if not env.game_pids():
            return False
        time.sleep(poll_s)
    return False


def restart(args: list[str] | None = None, stdout: Path | None = None,
            timeout_s: float = 420.0) -> bool:
    """Stop, clear this run's logs, start with arguments, wait for the menu."""
    if not stop():
        raise SystemExit("could not stop the running game")
    clear_run_logs()
    launch(args, stdout)
    return wait_for_main_menu(timeout_s)


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--stop", action="store_true")
    ap.add_argument("--start", action="store_true")
    ap.add_argument("--restart", action="store_true")
    ap.add_argument("--timeout", type=float, default=420.0)
    ap.add_argument("arg", nargs="*", help="arguments passed to the game binary")
    parsed = ap.parse_args()

    if parsed.stop or parsed.restart:
        print("stopped" if stop() else "could not stop", file=sys.stderr)
    if parsed.start or parsed.restart:
        clear_run_logs()
        launch(parsed.arg)
        ok = wait_for_main_menu(parsed.timeout)
        print("main menu reached" if ok else "game did not reach the main menu")
        sys.exit(0 if ok else 3)
