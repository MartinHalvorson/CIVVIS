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

Windows (Steam only) is resolved by its own functions further down, behind
``on_windows()``, so nothing a macOS host resolves changes. That half is
written from the Steam depot layout and the ``Documents\\My Games`` tree and is
UNMEASURED against a Windows Civilization VI: docs/CIV6_HOTSEAT_RELEASE.md
lists what has to be measured before it is called supported. Epic Games and
Linux/Proton installs are out of scope.
"""

from __future__ import annotations

import os
import re
import signal
import subprocess
import sys
from pathlib import Path

STEAM_APP_ID = "289070"

SUPPORT = Path.home() / "Library/Application Support"
LEGACY_USER_DIR = SUPPORT / "Sid Meier's Civilization VI"
NESTED_USER_DIR = LEGACY_USER_DIR / "Firaxis Games/Sid Meier's Civilization VI"

INSTALL_CANDIDATES = (
    SUPPORT / "Steam/steamapps/common/Sid Meier's Civilization VI",
    Path("/Applications/Sid Meier's Civilization VI"),
    # Windows, where a second Steam library is the common case. Carried here
    # rather than in each tool: `civ6_fidelity.py` shipped a Windows-only list
    # of its own and could not find an install on any machine in this fleet.
    Path(r"C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI"),
    Path(r"C:\Program Files\Steam\steamapps\common\Sid Meier's Civilization VI"),
    Path(r"D:\SteamLibrary\steamapps\common\Sid Meier's Civilization VI"),
    Path(r"E:\SteamLibrary\steamapps\common\Sid Meier's Civilization VI"),
)

#: Environment variables that name an install, most specific first. Two names
#: are honoured because two were already in use -- `civ6_fidelity.py` and
#: `civ6_type_names.py` documented `$CIV6_DIR` while everything else used
#: `$CIV6_INSTALL` -- and silently dropping either would break whatever is
#: already exporting it.
INSTALL_ENV_VARS = ("CIV6_INSTALL", "CIV6_DIR")

# Where the shipped rules database lives inside the macOS app bundle. The
# fidelity audit wants this path, not the bundle root.
ASSETS_SUBPATH = "Civ6.app/Contents/Assets"

#: The directory whose presence proves a path is a gameplay-database root.
GAMEPLAY_DATA = "Base/Assets/Gameplay/Data"


def install_dir(explicit: str | os.PathLike | None = None) -> Path:
    """The installation root (the directory holding ``Civ6.app``)."""
    named = [explicit, *(os.environ.get(name) for name in INSTALL_ENV_VARS)]
    for candidate in filter(None, named):
        path = Path(candidate)
        if path.is_dir():
            return path
    if on_windows():
        # Every Steam library the client knows, before the fixed guesses below.
        for path in windows_install_candidates():
            if path.is_dir():
                return path
    for path in INSTALL_CANDIDATES:
        if path.is_dir():
            return path
    raise SystemExit(
        "Civilization VI install not found; set $"
        + " or $".join(INSTALL_ENV_VARS)
    )


def assets_dir(explicit: str | os.PathLike | None = None) -> Path:
    """The gameplay-database root that ``civ6_fidelity.py`` audits against.

    A path that is *already* the assets root resolves to itself, so ``--civ6``
    keeps working whether it names the install or the bundle interior: an
    explicit directory is what ``install_dir`` returns, and an assets root has
    no ``Civ6.app`` nested inside it to descend into. An earlier draft of this
    added a branch for that case; a planted defect proved the branch changed
    no answer, so it is a comment instead of code.
    """
    root = install_dir(explicit)
    nested = root / ASSETS_SUBPATH
    return nested if (nested / GAMEPLAY_DATA).is_dir() else root


def user_dir() -> Path:
    """The user directory the game actually reads and writes.

    Chosen by recency of ``AppOptions.txt``: the running game rewrites its
    options on launch and on exit, so the live directory is always the freshly
    stamped one. When neither has been written -- a machine where the game has
    never run -- the nested layout is the modern one and wins.
    """
    if override := os.environ.get("CIV6_USER_DIR"):
        return Path(override)
    if on_windows():
        return windows_user_dir()
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


#: Seconds a `ps` may take before the answer is treated as unknown. A process
#: listing that has not returned in ten seconds is not slow, it is stuck.
GAME_PIDS_TIMEOUT_S = 10.0

#: The last listing that did return, so a stuck `ps` cannot be read as "no
#: game". See `game_pids`.
_LAST_GAME_PIDS: list[int] = []


def game_pids(timeout_s: float = GAME_PIDS_TIMEOUT_S) -> list[int]:
    """PIDs of the running game (never Steam, a launcher shim, or a query helper).

    `pgrep -f Civ6_Exe` is unsafe here: the popup watchdog periodically asks
    System Events about process ``Civ6_Exe_Child``, and `pgrep` therefore saw its
    own short-lived `osascript` child as a game.  That made a clean restart look
    like a competing live run.  Inspect the executable token instead.

    ⚠⚠⚠ THIS RAN WITH NO TIMEOUT, IN THE LOOP THAT DRIVES THE GAME.
    `watch.follow` calls it on EVERY poll, so an unbounded `ps` is an unbounded
    hang in that loop. The bound is the point of this function's shape.

    ⚠ WHAT IT DID *NOT* DO, CORRECTING #2700's OWN CLAIM. That commit said this
    call hung a game at turn 8 (run `civvis-20260828T160200Z`), on the strength
    of a traceback ending here after the wedge watchdog's SIGINT. That was wrong
    reasoning and the evidence refuted it the same afternoon: run
    `civvis-20260828T165926Z` wedged at turn 74 on a build carrying this
    timeout, its traceback ended HERE AGAIN, and `[env] ps did not answer`
    appears **zero** times in its log. So `ps` never reached ten seconds; the
    interrupt simply lands wherever the process happens to be, and a poll loop
    spends much of its wall time inside `subprocess.run`. A traceback at the
    moment of SIGINT names a location, not a cause.

    Both wedges were the end-turn deadlock instead: a unit CIVVIS did not
    mention, kept off explore automation by the guard, with no disposition at
    all, so Civilization VI would not end the turn (#2702, and #2703 for the
    civilian half). Turn 74's last orders event reads `guarded=1` and the log
    ends `blocked -> dismissed -> blocked` on `ENDTURN_BLOCKING_UNITS`.

    The timeout stays, on its own merits: an unbounded subprocess inside the
    driving loop is a hazard whether or not it has fired yet.

    ⚠⚠ A TIMEOUT MUST NOT RETURN THE EMPTY LIST. `watch.follow` reads
    ``if not env.game_pids()`` as **"game exited"** and ends the attempt, so an
    empty answer to a question we could not ask would throw away a live game —
    trading a hang for a silent loss, which is worse. A timed-out probe is
    UNKNOWN, and the honest answer to unknown is the last listing that did
    return; a game that really has gone is still caught by the next successful
    poll, by the stall timer, and by the wedge watchdog.

    `OSError` keeps returning ``[]``: no `ps` on this host is a real answer,
    not an unanswered question.

    On Windows there is no `ps`; the process table is read in-process
    (`windows_processes`) and matched on the executable names in
    `WINDOWS_GAME_EXES`, which are the Steam depot's and unmeasured.
    """
    global _LAST_GAME_PIDS
    if on_windows():
        try:
            return [pid for pid, name in windows_processes()
                    if name.lower() in {exe.lower() for exe in WINDOWS_GAME_EXES}]
        except OSError:
            return []
    try:
        out = subprocess.run(
            ["ps", "-axo", "pid=,command="], capture_output=True, text=True,
            timeout=timeout_s,
        ).stdout
    except subprocess.TimeoutExpired:
        print(f"[env] ps did not answer in {timeout_s:g}s; keeping the last "
              f"known game pids {_LAST_GAME_PIDS} rather than reporting none",
              flush=True)
        return list(_LAST_GAME_PIDS)
    except OSError:
        return []
    _LAST_GAME_PIDS = _game_pids_from_ps(out)
    return list(_LAST_GAME_PIDS)


def request_macos_quit() -> bool:
    """Ask Civ VI's own macOS menu to quit, without escalating a process.

    The direct child executable is not a normal scriptable application, so a
    bare Apple event or Command-Q can be swallowed by its render loop.  Its
    accessibility menu remains available and exposes ``Quit Civilization VI``.
    This is deliberately just a normal UI quit request: callers must still
    verify that the process actually left, and must never turn a refusal into
    SIGKILL.
    """
    if sys.platform != "darwin":
        return False
    script = '''tell application "System Events"
    tell process "Civ6_Exe_Child"
        if exists menu bar item 2 of menu bar 1 then
            click menu item "Quit Civilization VI" of menu 1 of menu bar item 2 of menu bar 1
            return true
        end if
    end tell
end tell
return false'''
    try:
        done = subprocess.run(
            ["osascript", "-e", script], capture_output=True, text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    return done.returncode == 0 and done.stdout.strip().lower() == "true"


def confirm_exit_dialog() -> bool:
    """Answer Civilization VI's EXIT TO DESKTOP confirmation. True when clicked.

    ★★★★★ THE POLITE QUIT ASKS A QUESTION AND NOTHING WAS ANSWERING IT.

    `request_macos_quit()` clicks *Quit Civilization VI* in the game's own menu.
    From the main menu that exits. From inside a game -- or the Create Game
    screen -- it raises an in-engine modal instead: **EXIT TO DESKTOP / "Are you
    sure you want to exit the game?" / OK · Cancel**. The SIGTERM that follows
    cannot get through a modal, and `quit_game` rightly refuses to escalate to
    SIGKILL, so the process sits there and the supervisor reports
    `LANE STALLED ... it needs an operator`. Measured 2026-09-10: it needed one
    twice, and both times the entire fix was a single click.

    The reading lives in `civ6_play`, with the other screen readers, and is
    imported HERE at call time: `civ6_play` imports this module, so a top-level
    import would be a cycle -- and this module is the dependency-free one that
    merely locates an installation, which a host with no capture grant must
    still be able to ask.
    """
    try:
        import civ6_play  # noqa: PLC0415 - call-time, to avoid an import cycle
    except ImportError:
        return False
    try:
        return civ6_play.confirm_exit_dialog()
    except Exception as error:  # a quit path must never raise
        print(f"[env] could not answer the exit confirmation: {error}",
              file=sys.stderr, flush=True)
        return False


#: ★ HOW LONG THE MENU QUIT GETS TO CLOSE THE GAME BY ITSELF before the exit
#: confirmation is looked for. From the end-of-game screen, where every live
#: game quits, there is no confirmation: 12 of 12 games on 2026-10-05 exited
#: with it never answered and no SIGTERM refused. But the look came 1 s in, and
#: it is a screenshot plus two OCR passes; while systemstatusd spins a capture
#: times out, and 8 of those 12 spent ~15 s on a failed exit-dialog shot. The
#: 11:16Z game (civvis-20261005T110504Z) exited mid-shot, which then ran on
#: 3.3 s after the process was gone. The pid is polled every 0.2 s meanwhile.
#:
#: ⚠ AND PAST CIV'S OWN EXIT TIME. Civ VI takes 5-8 s to close after the menu
#: quit (5.2, 6.9, 7.3, 7.8 s measured on 2026-10-05), longer after a long game,
#: and the look, once started, holds the loop for its whole failed capture
#: (~15 s while systemstatusd spins). civvis-20261005T130519Z (268 turns) quit
#: from its end screen, looked at 3 s, and stopped at +22.9 s; the game had left
#: underneath the shot. In-game quits no longer raise the modal at all (the
#: control mod answers UserRequestClose), so the look is only for a quit from
#: the front end, and 12 s costs that rare case nothing it can feel.
EXIT_CONFIRM_AFTER_S = 12.0
#: The whole menu-quit window, looks included, before the SIGTERM path, wide
#: enough for several looks after the settle above.
NATIVE_QUIT_WINDOW_S = 20.0


def quit_game(timeout_s: float = 20.0) -> bool:
    """Stop the game if it is running. True when nothing is left running.

    The game rewrites its options files on exit, so any configuration change
    has to happen with the process down or it is overwritten.
    """
    import time

    if not game_pids():
        return True
    if on_windows():
        # ⚠ `os.kill(pid, SIGTERM)` on Windows is TerminateProcess: no quit
        # request, no chance to flush a save, exactly the hard kill the end of
        # this function refuses to escalate to. There is no menu quit here
        # either, so the honest answer is to leave the game to the person.
        print("[env] not stopping Civilization VI on Windows (it would be a hard "
              "kill); quit it from its own menu", file=sys.stderr, flush=True)
        return False
    # Prefer the application's own Quit menu where macOS exposes it.  It gives
    # the game a chance to flush options/save state before the longstanding
    # TERM fallback.  Leave most of the caller's bounded stop window for that
    # fallback; this is not a second, unbounded shutdown attempt.
    deadline = time.time() + timeout_s
    if request_macos_quit():
        quit_at = time.time()
        native_deadline = min(deadline, quit_at + NATIVE_QUIT_WINDOW_S)
        next_look = quit_at + EXIT_CONFIRM_AFTER_S
        asked = False
        looks = 0
        while time.time() < native_deadline:
            if not game_pids():
                print(f"[env] the game exited {time.time() - quit_at:.1f}s after the "
                      f"menu quit ({looks} confirmation look(s))", flush=True)
                return True
            # The menu quit raises a confirmation when it is used from inside a
            # game or the Create Game screen. Answer it once the game has had
            # `EXIT_CONFIRM_AFTER_S` to leave by itself, and again each second
            # while it stays. Without this the loop waits out its whole window for
            # a click nobody sends, the SIGTERM below cannot cross the modal, and
            # the lane stalls for an operator.
            if not asked and time.time() >= next_look:
                looks += 1
                asked = confirm_exit_dialog()
                next_look = time.time() + 1.0
                continue
            time.sleep(0.2)
    for pid in game_pids():
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    while time.time() < deadline:
        if not game_pids():
            return True
        time.sleep(0.5)
    # A live Civilization VI process owns the player's save files.  Escalating
    # a normal termination request to SIGKILL can leave those files half-written
    # and turns a recoverable wedge into lost progress.  Report the refusal so
    # the caller can leave the game visible or use its normal UI, rather than
    # forcibly tearing down the process behind the operator's back.
    remaining = game_pids()
    if remaining:
        print(f"[env] Civilization VI did not exit after SIGTERM: {remaining}",
              file=sys.stderr, flush=True)
    return not remaining


def launch_game() -> None:
    """Ask Steam to start the game."""
    subprocess.run(["open", f"steam://rungameid/{STEAM_APP_ID}"], check=False)


# ------------------------------------------------------------------ Windows
#
# ⚠ UNMEASURED. Nothing in this section has been run against a Windows
# Civilization VI. The paths are the Steam depot's and the documented
# `Documents\My Games` tree; the process names are the depot's executables.
# `tools/civvis_hotseat_app.py doctor` prints what each resolves to on a real
# machine, and docs/CIV6_HOTSEAT_RELEASE.md lists the measurements still owed.
# Steam only: an Epic Games install and Linux/Proton are out of scope.


def on_windows() -> bool:
    """The switch for this section. A function so the tests can drive the
    Windows resolution over fake trees on any host."""
    return sys.platform == "win32"


#: The user directory below Documents: AppOptions.txt, Logs, Mods, Saves.
WINDOWS_USER_SUBPATH = Path("My Games") / "Sid Meier's Civilization VI"
#: Steam's `installdir` for app 289070, used when no app manifest names one.
STEAM_INSTALL_NAME = "Sid Meier's Civilization VI"
#: `Base\Binaries\Win64Steam\` in the depot: the DirectX 11 and 12 builds.
WINDOWS_GAME_EXES = ("CivilizationVI.exe", "CivilizationVI_DX12.exe")


def windows_documents_dir() -> Path:
    """The user's Documents folder, asked of the shell (CSIDL_PERSONAL).

    Documents is commonly redirected (OneDrive, a second drive), and a fixed
    `%USERPROFILE%\\Documents` would then name a folder the game never writes.
    """
    try:
        import ctypes  # noqa: PLC0415 - Windows only
        buffer = ctypes.create_unicode_buffer(260)
        # SHGetFolderPathW(hwnd, CSIDL_PERSONAL, token, SHGFP_TYPE_CURRENT, out)
        if ctypes.windll.shell32.SHGetFolderPathW(None, 5, None, 0, buffer) == 0 and buffer.value:
            return Path(buffer.value)
    except (AttributeError, OSError, ValueError):
        pass
    return Path(os.environ.get("USERPROFILE") or Path.home()) / "Documents"


def windows_user_dir_candidates(documents: Path | None = None,
                                environ: dict | None = None) -> list[Path]:
    """Every `My Games\\Sid Meier's Civilization VI` the game could be using:
    under the shell's Documents first, then the profile's and OneDrive's."""
    environ = os.environ if environ is None else environ
    roots = [documents if documents is not None else windows_documents_dir()]
    if environ.get("USERPROFILE"):
        roots.append(Path(environ["USERPROFILE"]) / "Documents")
    if environ.get("OneDrive"):
        roots.append(Path(environ["OneDrive"]) / "Documents")
    candidates: list[Path] = []
    for root in roots:
        candidate = root / WINDOWS_USER_SUBPATH
        if str(candidate).lower() not in {str(c).lower() for c in candidates}:
            candidates.append(candidate)
    return candidates


def windows_user_dir(candidates: list[Path] | None = None) -> Path:
    """The Windows user directory: the same rule as macOS, the candidate whose
    AppOptions.txt the game stamped last, else the shell's Documents one."""
    candidates = windows_user_dir_candidates() if candidates is None else candidates
    stamps = []
    for index, candidate in enumerate(candidates):
        options = candidate / "AppOptions.txt"
        if options.is_file():
            stamps.append((options.stat().st_mtime, -index, candidate))
    if stamps:
        return max(stamps)[2]
    return candidates[0]


def app_options_path() -> Path:
    """The AppOptions.txt the game reads (both platforms)."""
    return user_dir() / "AppOptions.txt"


_VDF_TOKEN = re.compile(r'"((?:[^"\\]|\\.)*)"|([{}])|//[^\n]*')


def parse_vdf(text: str) -> dict:
    """Valve's KeyValues text, the format of `libraryfolders.vdf` and
    `appmanifest_*.acf`: quoted keys, quoted values or `{ }` blocks, `\\`
    escapes, `//` comments. Platform conditionals (`[$WIN32]`) are ignored."""
    stack: list[dict] = [{}]
    key: str | None = None
    for match in _VDF_TOKEN.finditer(text):
        quoted, brace = match.group(1), match.group(2)
        if brace == "{":
            child: dict = {}
            if key is not None:
                stack[-1][key] = child
            stack.append(child)
            key = None
        elif brace == "}":
            if len(stack) > 1:
                stack.pop()
            key = None
        elif quoted is not None:
            value = re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)),
                           quoted)
            if key is None:
                key = value
            else:
                stack[-1][key] = value
                key = None
    return stack[0]


def _vdf_get(table: object, name: str):
    """A KeyValues lookup; Valve's keys are case-insensitive."""
    if not isinstance(table, dict):
        return None
    return next((value for key, value in table.items() if key.lower() == name.lower()), None)


def _read_vdf(path: Path) -> dict:
    try:
        return parse_vdf(path.read_text(encoding="utf-8", errors="replace"))
    except OSError:
        return {}


def windows_steam_roots(environ: dict | None = None, read_registry: bool = True) -> list[Path]:
    """Steam installations: what the registry says (per user, then machine),
    then the two Program Files defaults."""
    environ = os.environ if environ is None else environ
    roots: list[Path] = []
    if read_registry:
        roots += _registry_steam_paths()
    for variable in ("ProgramFiles(x86)", "ProgramFiles"):
        if environ.get(variable):
            roots.append(Path(environ[variable]) / "Steam")
    unique: list[Path] = []
    for root in roots:
        if str(root).lower() not in {str(r).lower() for r in unique}:
            unique.append(root)
    return unique


def _registry_steam_paths() -> list[Path]:
    try:
        import winreg  # noqa: PLC0415 - Windows only
    except ImportError:
        return []
    found = []
    for hive, key, value in (
            (winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
            (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
            (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath")):
        try:
            with winreg.OpenKey(hive, key) as handle:
                path, _ = winreg.QueryValueEx(handle, value)
        except OSError:
            continue
        if isinstance(path, str) and path:
            found.append(Path(path))
    return found


def steam_libraries(steam_root: Path) -> list[Path]:
    """Every Steam library one installation knows: itself, then each library
    in `steamapps/libraryfolders.vdf`, in the current nested layout
    (`"1" { "path" "D:\\SteamLibrary" ... }`) or the older flat one
    (`"1" "D:\\SteamLibrary"`)."""
    libraries = [Path(steam_root)]
    folders = _vdf_get(_read_vdf(Path(steam_root) / "steamapps" / "libraryfolders.vdf"),
                       "libraryfolders")
    for key, value in (folders or {}).items():
        if not key.isdigit():
            continue
        path = _vdf_get(value, "path") if isinstance(value, dict) else value
        if isinstance(path, str) and path:
            library = Path(path)
            if str(library).lower() not in {str(known).lower() for known in libraries}:
                libraries.append(library)
    return libraries


def steam_app_dir(library: Path, app_id: str = STEAM_APP_ID) -> Path:
    """Where one library keeps the game: the `installdir` its app manifest
    names, or the depot's default folder name."""
    manifest = Path(library) / "steamapps" / f"appmanifest_{app_id}.acf"
    name = _vdf_get(_vdf_get(_read_vdf(manifest), "AppState"), "installdir")
    return Path(library) / "steamapps" / "common" / (name or STEAM_INSTALL_NAME)


def windows_install_candidates(steam_roots: list[Path] | None = None) -> list[Path]:
    """The game's folder in every Steam library, libraries holding its app
    manifest first."""
    roots = windows_steam_roots() if steam_roots is None else steam_roots
    manifested, others = [], []
    for root in roots:
        for library in steam_libraries(root):
            candidate = steam_app_dir(library)
            seen = {str(c).lower() for c in manifested + others}
            if str(candidate).lower() in seen:
                continue
            has_manifest = (library / "steamapps" / f"appmanifest_{STEAM_APP_ID}.acf").is_file()
            (manifested if has_manifest else others).append(candidate)
    return manifested + others


def mod_install_candidates() -> list[tuple[str, Path | None, str]]:
    """(name, folder, evidence) for each place a mod could be installed.

    `tools/civ6_control/install.py` writes the first, on both platforms.
    """
    try:
        dlc: Path | None = assets_dir() / "DLC"
    except SystemExit:
        dlc = None
    if on_windows():
        return [("install DLC", dlc, "UNMEASURED on Windows; this is where install.py writes"),
                ("user Mods", mods_dir(), "UNMEASURED on Windows; the folder hand-installed "
                 "Windows mods conventionally use; install.py does not write here")]
    return [("install DLC", dlc, "measured on macOS: the only folder this build scans"),
            ("user Mods", mods_dir(), "measured on macOS: never scanned (install.py)")]


# ------------------------------------------------- processes, on both systems
#
# ⚠ `os.kill(pid, 0)` IS NOT A LIVENESS PROBE ON WINDOWS. Signal 0 is
# `CTRL_C_EVENT` there, so Python sends a console control event instead of
# asking whether the process exists, and every other signal number is
# TerminateProcess. `pid_alive` asks the system properly on Windows; callers
# keep their own POSIX `os.kill(pid, 0)` and route only Windows here.


def pid_alive(pid: int) -> bool:
    """Whether `pid` names a running process (False for pids <= 0)."""
    try:
        pid = int(pid)
    except (TypeError, ValueError):
        return False
    if pid <= 0:
        return False
    if on_windows():
        return _windows_pid_alive(pid)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError:
        return False
    return True


def _windows_pid_alive(pid: int) -> bool:
    import ctypes  # noqa: PLC0415 - Windows only
    from ctypes import wintypes  # noqa: PLC0415

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = kernel32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not handle:
        return ctypes.get_last_error() == 5  # ERROR_ACCESS_DENIED: it exists
    try:
        code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(code)):
            return True  # unreadable: a lock must not be handed out on a guess
        return code.value == 259  # STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def windows_processes() -> list[tuple[int, str]]:
    """(pid, executable name) for every process, from a Toolhelp snapshot:
    in-process, so `game_pids` can be polled without spawning anything."""
    import ctypes  # noqa: PLC0415 - Windows only
    from ctypes import wintypes  # noqa: PLC0415

    class Entry(ctypes.Structure):  # PROCESSENTRY32W
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", ctypes.c_long),
                    ("dwFlags", wintypes.DWORD), ("szExeFile", ctypes.c_wchar * 260)]

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel32.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel32.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(Entry)]
    kernel32.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(Entry)]
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    snapshot = kernel32.CreateToolhelp32Snapshot(0x2, 0)  # TH32CS_SNAPPROCESS
    if not snapshot or snapshot == ctypes.c_void_p(-1).value:
        raise OSError(ctypes.get_last_error(), "CreateToolhelp32Snapshot failed")
    rows = []
    try:
        entry = Entry()
        entry.dwSize = ctypes.sizeof(Entry)
        more = kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
        while more:
            rows.append((int(entry.th32ProcessID), entry.szExeFile))
            more = kernel32.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel32.CloseHandle(snapshot)
    return rows


def windows_command_lines(pid: int | None = None, timeout_s: float = 30.0) -> list[tuple[int, str]]:
    """(pid, command line) from WMI through PowerShell: one process, or all.

    Slow (a PowerShell start, about a second), so only for the rare checks
    that must tell one of our workers from a reused pid: never in a poll loop.
    Empty when PowerShell cannot be run or does not answer in `timeout_s`.
    """
    query = "Get-CimInstance Win32_Process"
    if pid is not None:
        query += f" -Filter 'ProcessId={int(pid)}'"
    script = ("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; "
              f"{query} | ForEach-Object {{ [Console]::Out.WriteLine("
              "([string]$_.ProcessId) + ' ' + [string]$_.CommandLine) }")
    try:
        out = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
                             capture_output=True, text=True, encoding="utf-8", errors="replace",
                             timeout=timeout_s).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    rows = []
    for line in out.splitlines():
        head, _, command = line.strip().partition(" ")
        if head.isdigit():
            rows.append((int(head), command))
    return rows


def process_command(pid: int) -> str:
    """The command line of `pid`, or "" when it cannot be read.

    POSIX asks `ps -o command= -p PID` exactly as the harness always has.
    """
    try:
        pid = int(pid)
    except (TypeError, ValueError):
        return ""
    if on_windows():
        return next((command for found, command in windows_command_lines(pid)
                     if found == pid), "")
    try:
        return subprocess.run(["ps", "-o", "command=", "-p", str(pid)],
                              capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


if __name__ == "__main__":
    print(f"install : {install_dir()}")
    print(f"assets  : {assets_dir()}")
    print(f"user    : {user_dir()}")
    print(f"logs    : {logs_dir()}")
    print(f"mods    : {mods_dir()}   (exists={mods_dir().is_dir()})")
    print(f"saves   : {saves_dir()}  (exists={saves_dir().is_dir()})")
    print(f"running : {game_pids() or 'no'}")
