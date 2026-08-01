#!/usr/bin/env python3
"""Locate a Civilization VI installation and the user directory it actually reads.

Everything that grounds CIVVIS against the real game -- log tailing, option
flipping, mod installation, save inspection -- needs one thing first: the right
directory. On macOS that is not obvious. The game keeps two user directories
with the same name nested inside each other::

    ~/Library/Application Support/Sid Meier's Civilization VI/           <- legacy
    ~/Library/Application Support/Sid Meier's Civilization VI/
        Firaxis Games/Sid Meier's Civilization VI/                       <- live

Both hold an ``AppOptions.txt``, both hold a ``Logs/``. An install that was
played years ago leaves a fully populated legacy directory, so picking the
wrong one looks like it worked -- the file parses, the flag writes, and the
game reads none of it. That failure mode is silent and costs an entire
debugging session, because a tuner that never opens its port is
indistinguishable from a tuner the platform does not support.

This module resolves the live directory by asking which one the game most
recently wrote, and falls back to the nested layout when neither has been
touched.
"""

from __future__ import annotations

import os
import re
import signal
import subprocess
from pathlib import Path

STEAM_APP_ID = "289070"

SUPPORT = Path.home() / "Library/Application Support"
LEGACY_USER_DIR = SUPPORT / "Sid Meier's Civilization VI"
NESTED_USER_DIR = LEGACY_USER_DIR / "Firaxis Games/Sid Meier's Civilization VI"

INSTALL_CANDIDATES = (
    SUPPORT / "Steam/steamapps/common/Sid Meier's Civilization VI",
    Path("/Applications/Sid Meier's Civilization VI"),
)

# Where the shipped rules database lives inside the macOS app bundle. The
# fidelity audit wants this path, not the bundle root.
ASSETS_SUBPATH = "Civ6.app/Contents/Assets"


def install_dir(explicit: str | os.PathLike | None = None) -> Path:
    """The installation root (the directory holding ``Civ6.app``)."""
    for candidate in filter(None, [explicit, os.environ.get("CIV6_INSTALL")]):
        path = Path(candidate)
        if path.is_dir():
            return path
    for path in INSTALL_CANDIDATES:
        if path.is_dir():
            return path
    raise SystemExit("Civilization VI install not found; set $CIV6_INSTALL")


def assets_dir(explicit: str | os.PathLike | None = None) -> Path:
    """The gameplay-database root that ``civ6_fidelity.py`` audits against."""
    root = install_dir(explicit)
    nested = root / ASSETS_SUBPATH
    return nested if (nested / "Base/Assets/Gameplay/Data").is_dir() else root


def user_dir() -> Path:
    """The user directory the game actually reads and writes.

    Chosen by recency of ``AppOptions.txt``: the running game rewrites its
    options on launch and on exit, so the live directory is always the freshly
    stamped one. When neither has been written -- a machine where the game has
    never run -- the nested layout is the modern one and wins.
    """
    if override := os.environ.get("CIV6_USER_DIR"):
        return Path(override)
    stamps = []
    for candidate in (NESTED_USER_DIR, LEGACY_USER_DIR):
        options = candidate / "AppOptions.txt"
        if options.is_file():
            stamps.append((options.stat().st_mtime, candidate))
    if stamps:
        return max(stamps)[1]
    return NESTED_USER_DIR


def logs_dir() -> Path:
    return user_dir() / "Logs"


def mods_dir() -> Path:
    return user_dir() / "Mods"


def saves_dir() -> Path:
    return user_dir() / "Saves"


# ------------------------------------------------------------------- options


def read_option(path: Path, key: str) -> str | None:
    """Value of a ``Key Value`` line, or None when the key is absent."""
    if not path.is_file():
        return None
    for line in path.read_text(errors="replace").splitlines():
        parts = line.strip().split(None, 1)
        if parts and parts[0] == key:
            return parts[1].strip() if len(parts) == 2 else ""
    return None


def set_options(path: Path, changes: dict[str, object]) -> dict[str, tuple]:
    """Rewrite ``Key Value`` lines in place. Returns {key: (old, new)} for changes.

    Only keys already present are rewritten -- these files are the game's own,
    and a key it does not define is one this version does not honour, so adding
    it would quietly do nothing while looking like success. Missing keys are
    reported with an ``old`` of None so a caller can complain.
    """
    if not path.is_file():
        raise SystemExit(f"options file not found: {path}")
    text = path.read_text(errors="replace")
    applied: dict[str, tuple] = {}
    for key, value in changes.items():
        pattern = re.compile(rf"(?m)^({re.escape(key)})[ \t]+(\S*)[ \t]*$")
        match = pattern.search(text)
        if not match:
            applied[key] = (None, value)
            continue
        old = match.group(2)
        if old != str(value):
            text = pattern.sub(f"{key} {value}", text, count=1)
            applied[key] = (old, value)
    path.write_text(text)
    return applied


def _game_pids_from_ps(text: str) -> list[int]:
    """Extract real Civ VI executables from ``ps`` output, not argument text."""
    pids = []
    for line in text.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) != 2 or not fields[0].isdigit():
            continue
        # The application bundle lives below `Application Support`, so command
        # lines cannot be tokenized on whitespace.  Match the executable path
        # component instead; `osascript ... process "Civ6_Exe_Child"` has no
        # slash before the name and is deliberately excluded.
        if re.search(r"/Civ6_Exe(?:_Child)?(?:\s|$)", fields[1]):
            pids.append(int(fields[0]))
    return pids


def game_pids() -> list[int]:
    """PIDs of the running game (never Steam, a launcher shim, or a query helper).

    `pgrep -f Civ6_Exe` is unsafe here: the popup watchdog periodically asks
    System Events about process ``Civ6_Exe_Child``, and `pgrep` therefore saw its
    own short-lived `osascript` child as a game.  That made a clean restart look
    like a competing live run.  Inspect the executable token instead.
    """
    try:
        out = subprocess.run(
            ["ps", "-axo", "pid=,command="], capture_output=True, text=True
        ).stdout
    except OSError:
        return []
    return _game_pids_from_ps(out)


def quit_game(timeout_s: float = 20.0) -> bool:
    """Stop the game if it is running. True when nothing is left running.

    The game rewrites its options files on exit, so any configuration change
    has to happen with the process down or it is overwritten.
    """
    import time

    if not game_pids():
        return True
    for pid in game_pids():
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        if not game_pids():
            return True
        time.sleep(0.5)
    for pid in game_pids():
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    time.sleep(2.0)
    return not game_pids()


def launch_game() -> None:
    """Ask Steam to start the game."""
    subprocess.run(["open", f"steam://rungameid/{STEAM_APP_ID}"], check=False)


if __name__ == "__main__":
    print(f"install : {install_dir()}")
    print(f"assets  : {assets_dir()}")
    print(f"user    : {user_dir()}")
    print(f"logs    : {logs_dir()}")
    print(f"mods    : {mods_dir()}   (exists={mods_dir().is_dir()})")
    print(f"saves   : {saves_dir()}  (exists={saves_dir().is_dir()})")
    print(f"running : {game_pids() or 'no'}")
