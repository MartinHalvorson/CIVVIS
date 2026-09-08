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
- **Startup is verifiable from the logs.** ``Logs/Modding.log`` records both
  the early mod scan and the later completion of game-content configuration;
  the latter is when the front end can accept exact menu controls.
  ``Logs/Automation.log`` is written from app init onwards whenever the
  automation system is active. Both live in the *nested* user directory (see
  ``civ6_env.user_dir``), and both are removed before launch so a stale file
  cannot be read as this run's.
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

# ``Discovered`` proves only that the engine began scanning mods.  On this
# macOS front end it arrives while the menu is still non-interactive; Civ VI
# writes this final line only after it has finished applying content and made
# the front end usable.  `clear_run_logs` above makes the marker specific to
# the launch being observed.
MAIN_MENU_READY_MARKER = "No need to reconfigure game content, marking it finished"


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


def bundle_signature_error() -> str | None:
    """Why ``codesign`` rejects the installed bundle, or None if it is valid.

    ⚠ INSTALLING THE MOD IS WHAT BREAKS THIS. The mod goes into the install's
    ``DLC`` tree — inside a *signed application bundle* — so every file
    ``civ6_control/install.py`` writes invalidates the bundle's sealed resource
    manifest. ``codesign -v`` then reports "a sealed resource is missing or
    invalid" and names each added file; uninstalling restores "valid on disk".

    Reported rather than repaired. Re-sealing needs write access to
    ``Contents/_CodeSignature``, which macOS 26 refuses without App Management
    permission ("Operation not permitted", even for ``touch``), and a bundle
    whose signature is broken has still played whole games on hosts whose trust
    record predates the change. So this is evidence for a diagnostic, not a
    precondition to enforce.
    """
    app = game_binary().parent.parent.parent
    try:
        done = subprocess.run(["codesign", "-v", str(app)],
                              capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError) as error:
        return f"could not run codesign: {error}"
    if done.returncode == 0:
        return None
    first = next((line.strip() for line in done.stderr.splitlines() if line.strip()), "")
    return first or f"codesign exited {done.returncode}"


def gatekeeper_refusal() -> str | None:
    """The text of macOS's "damaged and can't be opened" modal, if it is up.

    ⚠⚠ THIS FAILURE LOOKS EXACTLY LIKE A SLOW MACHINE, AND THAT COST A SESSION.
    Measured 2026-08-07 on macOS 26.5.1: the game process starts and stays
    running, so ``env.game_pids()`` is non-empty and the loop below waits out
    its whole 420 seconds; but the core never initialises, so no ``Logs``
    directory is ever created, ``events.jsonl`` stays at zero bytes, and the
    attempt is recorded as a stall. Nothing anywhere names the cause. What is
    actually on screen is a modal owned by ``CoreServicesUIAgent``:

        "Civilization VI" is damaged and can't be opened. You should move it to
        the Trash.

    Deliberately NOT dismissed, unlike the crash dialogs ``civ6_civvis_climb``
    clears. Those sit on top of a working game and stealing their click is the
    fix; this one IS the refusal, so closing it just runs the next attempt into
    the same wall. It needs an operator (Open Anyway, App Management, or Steam's
    verify-integrity), and the useful thing the harness can do is say so once
    instead of stalling repeatedly.
    """
    script = ('tell application "System Events" to tell process '
              '"CoreServicesUIAgent" to get value of every static text of window 1')
    try:
        done = subprocess.run(["osascript", "-e", script],
                              capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    if done.returncode != 0:
        return None
    text = done.stdout.strip()
    lowered = text.lower()
    if "damaged" in lowered or "can't be opened" in lowered or "cannot be opened" in lowered:
        return text
    return None


def wait_for_main_menu(timeout_s: float = 420.0, poll_s: float = 3.0) -> bool:
    """Wait until the front end has finished content configuration.

    ``Modding.log`` gaining its ``Discovered`` line proves only that the core
    got past engine init; it can precede a non-interactive menu by more than a
    minute.  The final content-configured marker above is the stronger
    capture-free readiness signal. Polling the process as well means a crash
    fails fast instead of waiting out the whole timeout.

    A macOS refusal is polled for the same reason, and is the one case where a
    LIVE process is not evidence of progress — see ``gatekeeper_refusal``.
    """
    log = env.logs_dir() / "Modding.log"
    # mach_absolute_time (Python's monotonic clock on macOS) pauses with the
    # machine, so a closed lid cannot consume the whole launch allowance.
    deadline = time.monotonic() + timeout_s
    # None rather than 0.0, so the FIRST pass always asks. Seeding this with a
    # number and comparing against the clock made the first check wait out the
    # throttle instead of the interval: `time.monotonic()` is small at process
    # start here, so `now - 0.0 >= 15.0` stayed false for the first 15 seconds.
    refusal_checked: float | None = None
    while time.monotonic() < deadline:
        if log.is_file() and MAIN_MENU_READY_MARKER in log.read_text(errors="replace"):
            return True
        if not env.game_pids():
            return False
        # Every 15s rather than every poll: this shells out to osascript, and the
        # modal does not arrive and leave between two polls.
        now = time.monotonic()
        if refusal_checked is None or now - refusal_checked >= 15.0:
            refusal_checked = now
            refusal = gatekeeper_refusal()
            if refusal:
                signature = bundle_signature_error()
                print(f"macOS is REFUSING the game, not loading it: {refusal}",
                      file=sys.stderr)
                if signature:
                    print(f"the installed bundle is also unsigned: {signature}",
                          file=sys.stderr)
                print("this needs an operator: System Settings > Privacy & Security "
                      "> Open Anyway, or grant App Management, or Steam > Civ 6 > "
                      "Properties > Installed Files > Verify integrity of game files",
                      file=sys.stderr)
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
