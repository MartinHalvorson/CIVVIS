#!/usr/bin/env python3
"""Systematic host control for driving and verifying live Civilization VI runs.

    python3 tools/computer_control.py layout            # operator quadrants
    python3 tools/computer_control.py census            # what modals are up, JSON
    python3 tools/computer_control.py dismiss           # close the SAFE ones
    python3 tools/computer_control.py games             # Civ 6 processes, JSON
    python3 tools/computer_control.py games --ensure-single
    python3 tools/computer_control.py bundle            # signature + writability
    python3 tools/computer_control.py screenshot --out /tmp/desk.png

This is repository tooling for testing and verification. It is not part of the
web client and nothing under ``web/`` or the deploy path may import it, so none
of it reaches civvis.ai — the same standing as the rest of ``tools/``.

Every verb here exists because ad-hoc control of this host already failed in a
measured way (2026-08-07, macOS 26.5.1, the session the Steam reinstall forced):

- **Windows drift.** The operator's standing layout is quadrants — terminal
  lower-left, CIVVIS upper-left, Civilization VI upper-right, lower-right kept
  free for the operator — and only the game's own placement was scripted
  (`civ6_play --window-side right`). The rest was hand osascript, re-derived
  every session, and one wrong `key code` opened Mission Control over the game.
- **Modals are load-bearing.** A macOS Gatekeeper sheet ("damaged and can't be
  opened", owner `CoreServicesUIAgent`) reads exactly like a slow launch: the
  process stays up and no log is written. A `Problem Reporter` window steals
  the next run's first click (see `dismiss_crash_dialogs` in
  `civ6_civvis_climb.py`, which this generalizes). Census FIRST, then dismiss
  by button name under an allowlist — never by coordinate, and never a
  destructive button: the Gatekeeper sheet's default is literally
  "Move to Trash".
- **The single-copy guard lives in the stub we bypass.** `Civ6_Exe` refuses a
  second copy; `Civ6_Exe_Child` — the process the launcher actually starts —
  does not, and two children ran concurrently this session (one leftover from
  a failed teardown, one fresh). `System Events` targeting "first process
  whose name contains Civ6" then drives an arbitrary one of them.
- **The install tree is TCC-protected.** Writes into ``Civ6.app`` are refused
  for Terminal's children even with the game closed ("Operation not
  permitted"), while Finder is allowed through; `civ6_control/install.py`
  already falls back to Finder for exactly this. `bundle` reports both facts —
  signature validity and writability — because either alone misleads: a valid
  signature does not mean the next mod install can happen, and an invalid one
  does not mean the game will not launch (a fresh trust record tolerates it).

Symlinking the mod out of the bundle was tried and is a dead end, not merely
unimplemented: Finder's `move (POSIX file … as alias)` dereferences the link
and moves its TARGET; moving the link as a folder item is refused (-5000) from
a temp directory and times out (-1712) from the home directory. Directory
replace through Finder is the mechanism that works.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env as env  # noqa: E402

MENU_BAR_PT = 33

# The operator's standing layout (2026-08-02, reconfirmed 2026-08-07): the
# lower-right quadrant is deliberately ABSENT so it stays free for them.
#
# CIVVIS is matched by window TITLE, not process: the live mirror is a browser
# tab (follow.py serves it on :8610), so the window is Chrome's/Safari's and the
# title is the page's. The game and terminal are stable process names.
STANDARD_LAYOUT = (
    {"quadrant": "lower-left", "process": "Terminal", "title": None},
    {"quadrant": "upper-left", "process": None, "title": r"CIVVIS|127\.0\.0\.1"},
    {"quadrant": "upper-right", "process": "Civ6", "title": None},
)

# Modal owners this project has actually been stopped by, with the buttons that
# are safe to press. Anything not named here is REPORTED and left alone —
# in particular "Move to Trash" (Gatekeeper's default) and "Reopen" (Problem
# Reporter's, which relaunches the game under nobody's harness).
KNOWN_MODALS = {
    "CoreServicesUIAgent": {
        "matches": ("damaged", "can’t be opened", "can't be opened", "verifying"),
        "safe_buttons": ("Cancel", "Done", "OK"),
    },
    "Problem Reporter": {
        "matches": ("quit unexpectedly", "Problem Report"),
        "safe_buttons": ("OK", "Hide Details", "Don’t Send", "Don't Send"),
    },
    "steam_osx": {
        "matches": ("Game configuration unavailable", "error occurred while launching"),
        "safe_buttons": ("OK", "Close"),
        # Steam's main client window ALWAYS exists, so an unrecognized window
        # of this owner is ordinary, not a modal — census it only on a match.
        "recognized_only": True,
    },
    "UserNotificationCenter": {
        "matches": (),
        "safe_buttons": ("OK", "Cancel", "Close"),
    },
    # The admin-auth sheet ("Finder wants to make changes", Touch ID / password).
    # Measured 2026-08-07: asking Finder to move a SYMLINK into the protected
    # bundle raised this sheet; the osascript that asked timed out (-1712) and
    # the sheet then sat on screen for an hour — invisible to a census keyed on
    # Finder, because its owner is SecurityAgent. Cancel is the only safe
    # button: entering credentials is never this tool's to do.
    "SecurityAgent": {
        "matches": ("wants to make changes",),
        "safe_buttons": ("Cancel",),
    },
}


def _osascript(script: str, timeout: float = 30.0) -> "subprocess.CompletedProcess[str]":
    return subprocess.run(["osascript", "-e", script],
                          capture_output=True, text=True, timeout=timeout)


def desktop_points() -> "tuple[int, int] | None":
    """Logical size of the display holding the menu bar, in points.

    The same measurement `civ6_play.desktop_size` makes, for the same reason it
    stopped asking Finder: with a second display attached Finder answers with
    the UNION of every screen and a window placed from that lands off-screen.
    """
    swift = ("import AppKit\n"
             "if let s = NSScreen.screens.first(where: { $0.frame.origin == .zero })"
             " ?? NSScreen.main {"
             " print(\"\\(Int(s.frame.width)),\\(Int(s.frame.height))\") }")
    try:
        out = subprocess.run(["swift", "-"], input=swift,
                             capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    parts = [p.strip() for p in out.stdout.split(",") if p.strip()]
    if len(parts) != 2 or not all(p.isdigit() for p in parts):
        return None
    width, height = int(parts[0]), int(parts[1])
    if not (800 < width <= 4000 and 600 < height <= 2000):
        return None
    return (width, height)


def quadrant_frame(quadrant: str, screen_w: int, screen_h: int,
                   menu: int = MENU_BAR_PT) -> "tuple[int, int, int, int]":
    """(x, y, w, h) of a quadrant in window coordinates (y grows down from 0).

    The menu bar is carved out of the TOP half only, so the two rows meet at
    the true vertical midline and the lower row reaches the bottom edge.
    """
    if quadrant not in ("upper-left", "upper-right", "lower-left", "lower-right"):
        raise ValueError(f"not a quadrant: {quadrant!r}")
    width = screen_w // 2
    mid = screen_h // 2
    x = 0 if quadrant.endswith("left") else screen_w - width
    if quadrant.startswith("upper"):
        y, height = menu, mid - menu
    else:
        y, height = mid, screen_h - mid
    return (x, y, width, height)


def _as(text: "str") -> "str":
    """`text` as an AppleScript string literal.

    ★★★ NOT `json.dumps`. JSON escapes every non-ASCII character as `\\uXXXX`,
    and AppleScript has no `\\u` escape — it reads the backslash-u literally and
    dies with AppleScript error -2741, a syntax error naming an unknown token
    where it wanted a closing quote.
    Window titles here are full of characters that trip it: Chrome titles the
    live viewer `CIVVIS · Civ VI Simulator`, and Terminal separates its title
    fields with an em dash. So the ONE window the standard layout has to place
    by title could never be placed on this host, and `layout` reported
    `placed: false` with a syntax error where the operator expected a mirror
    beside the game.

    `osascript` reads UTF-8, so the characters go through untouched; only the
    two that end a literal need escaping.
    """
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def place_window(frame: "tuple[int, int, int, int]", process: "str | None" = None,
                 title: "str | None" = None,
                 skip_owners: "frozenset[str]" = frozenset()) -> "str | None":
    """Move one window into `frame`; returns an error string or None.

    Size before position, deliberately: Aspyr's window constrains a requested
    origin against its CURRENT size, so positioning first lands an upper
    quadrant at the bottom (measured in `civ6_play.place_game`, kept here).
    """
    x, y, w, h = frame
    if process and not title:
        finder = f'first process whose name contains {_as(process)}'
        window = "window 1"
    else:
        # Title match walks every visible process's windows; first match wins.
        finder = None
        window = None
    if finder:
        script = (f'tell application "System Events" to tell ({finder}) to tell {window}\n'
                  f'  set size to {{{w}, {h}}}\n'
                  f'  set position to {{{x}, {y}}}\n'
                  f'end tell')
        done = _osascript(script)
        return None if done.returncode == 0 else (done.stderr or done.stdout).strip()
    pattern = re.compile(title or "", re.I)
    listing = _osascript(
        'tell application "System Events"\n'
        '  set out to ""\n'
        '  repeat with proc in (every process whose background only is false)\n'
        '    repeat with win in (every window of proc)\n'
        '      set out to out & (name of proc) & "\\t" & (name of win) & "\\n"\n'
        '    end repeat\n'
        '  end repeat\n'
        '  return out\n'
        'end tell')
    if listing.returncode != 0:
        return (listing.stderr or listing.stdout).strip()
    for line in listing.stdout.splitlines():
        owner, _, window_name = line.partition("\t")
        if not window_name or not pattern.search(window_name):
            continue
        # ★★★ A WINDOW THIS LAYOUT ALREADY PLACED BY PROCESS IS NOT A CANDIDATE.
        # The title patterns are deliberately loose, and a loose pattern will
        # eventually match a window that is not the target: the standard
        # layout's `CIVVIS|127\.0\.0\.1` matched a *Terminal* window on this
        # host, because the operator had named a shell session "CIVVIS gaps and
        # priorities" — so the upper-left slot would have been given the
        # terminal that lower-left had just been given, and the live mirror
        # would never appear. Terminal is placed by an explicit process spec, so
        # its windows are spoken for; skip them rather than tighten a pattern
        # that will drift again.
        if owner in skip_owners:
            continue
        script = (f'tell application "System Events" to tell '
                  f'(first process whose name is {_as(owner)}) to tell '
                  f'(first window whose name is {_as(window_name)})\n'
                  f'  set size to {{{w}, {h}}}\n'
                  f'  set position to {{{x}, {y}}}\n'
                  f'end tell')
        done = _osascript(script)
        return None if done.returncode == 0 else (done.stderr or done.stdout).strip()
    return f"no visible window matches {title!r}"


def layout(assignments=STANDARD_LAYOUT) -> "list[dict]":
    size = desktop_points()
    if size is None:
        return [{"error": "desktop size unavailable"}]
    report = []
    # Processes this layout positions explicitly. A later title match must not
    # re-claim one of their windows; see `place_window`.
    claimed = frozenset(
        spec["process"] for spec in assignments if spec.get("process")
    )
    for spec in assignments:
        frame = quadrant_frame(spec["quadrant"], *size)
        error = place_window(frame, process=spec.get("process"),
                             title=spec.get("title"),
                             skip_owners=claimed)
        report.append({"quadrant": spec["quadrant"],
                       "target": spec.get("process") or spec.get("title"),
                       "placed": error is None,
                       **({"error": error} if error else {})})
    return report


def modal_census() -> "list[dict]":
    """Every known-owner modal currently on screen, with its text and buttons."""
    found = []
    for owner, spec in KNOWN_MODALS.items():
        count = _osascript(f'tell application "System Events" to count windows '
                           f'of process {_as(owner)}')
        if count.returncode != 0 or not count.stdout.strip().isdigit():
            continue
        if int(count.stdout.strip()) == 0:
            continue
        texts = _osascript(f'tell application "System Events" to tell process '
                           f'{_as(owner)} to get value of every static text '
                           f'of window 1')
        buttons = _osascript(f'tell application "System Events" to tell process '
                             f'{_as(owner)} to get name of every button '
                             f'of window 1')
        text = texts.stdout.strip() if texts.returncode == 0 else ""
        names = [b.strip() for b in buttons.stdout.split(",")] \
            if buttons.returncode == 0 and buttons.stdout.strip() else []
        recognized = any(m.lower() in text.lower() for m in spec["matches"])
        if spec.get("recognized_only") and not recognized:
            continue
        found.append({
            "owner": owner,
            "text": text,
            "buttons": [b for b in names if b and b != "missing value"],
            "recognized": recognized,
        })
    return found


def choose_dismissal(modal: dict) -> "str | None":
    """The button to press for this modal, or None to leave it alone.

    Policy, not heuristics: only a button named on its owner's allowlist is
    ever pressed, so an unfamiliar sheet — or a familiar owner showing an
    unfamiliar dialog with only destructive choices — is reported and kept.
    """
    spec = KNOWN_MODALS.get(modal.get("owner", ""))
    if spec is None:
        return None
    for name in spec["safe_buttons"]:
        if name in (modal.get("buttons") or []):
            return name
    return None


def dismiss_modals() -> "list[dict]":
    report = []
    for modal in modal_census():
        button = choose_dismissal(modal)
        entry = dict(modal, action=button or "left alone")
        if button:
            done = _osascript(
                f'tell application "System Events" to tell process '
                f'{_as(modal["owner"])} to click button {_as(button)} '
                f'of window 1')
            entry["dismissed"] = done.returncode == 0
        report.append(entry)
    return report


def game_processes() -> "list[dict]":
    """Every live Civ 6 process, oldest first, stub and child distinguished."""
    try:
        out = subprocess.run(
            ["ps", "-axo", "pid=,lstart=,command="],
            capture_output=True, text=True, timeout=15).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    rows = []
    for line in out.splitlines():
        if "Civ6_Exe" not in line or "grep" in line:
            continue
        fields = line.strip().split(None, 6)
        if len(fields) < 7 or not fields[0].isdigit():
            continue
        pid, started, command = int(fields[0]), " ".join(fields[1:6]), fields[6]
        rows.append({"pid": pid, "started": started,
                     "role": "child" if "Civ6_Exe_Child" in command else "stub"})
    return rows  # ps reports in pid order, which on this host is launch order.


def ensure_single_game(kill: bool = True) -> dict:
    """Enforce the single-copy rule the stub applies and the child bypasses.

    The OLDEST child is the one a harness may be attached to — this session's
    duplicate was the newer one, launched over a leftover — so extras are
    culled newest-first. `kill=False` reports what would happen.
    """
    children = [p for p in game_processes() if p["role"] == "child"]
    doomed = children[1:]
    if kill:
        for proc in doomed:
            subprocess.run(["kill", "-9", str(proc["pid"])], capture_output=True)
        if doomed:
            time.sleep(2.0)
    return {"kept": children[0] if children else None,
            "killed" if kill else "would_kill": doomed}


def bundle_report() -> dict:
    """Signature validity AND writability — the two independent launch facts."""
    app = env.install_dir() / "Civ6.app"
    try:
        done = subprocess.run(["codesign", "-v", str(app)],
                              capture_output=True, text=True, timeout=120)
        signature = "valid" if done.returncode == 0 else \
            next((l.strip() for l in done.stderr.splitlines() if l.strip()),
                 f"codesign exited {done.returncode}")
    except (OSError, subprocess.SubprocessError) as error:
        signature = f"could not run codesign: {error}"
    probe = app / "Contents" / "Assets" / "DLC" / ".civvis-write-probe"
    try:
        probe.touch()
        probe.unlink()
        writable = True
    except OSError:
        writable = False
    return {"bundle": str(app), "signature": signature, "writable": writable,
            "mod_installed": (app / "Contents" / "Assets" / "DLC" /
                              "CivvisControl").exists()}


def screenshot(out: Path, max_dimension: "int | None" = 1400) -> dict:
    """Capture the screen, downscaled; returns the path AND the scale.

    The scale is not a nicety. A 1400-wide capture of a 1728-point screen is
    0.81 of reality, and a click aimed at coordinates read off it lands 19%
    high and left of the target — measured 2026-08-07, when exactly that slip
    put a menu click on the ad carousel and opened the Steam store over the
    game. Divide screenshot coordinates by `scale` before clicking.
    """
    subprocess.run(["screencapture", "-x", "-t", "png", str(out)], check=True,
                   timeout=30)
    scale = 1.0
    if max_dimension:
        subprocess.run(["sips", "-Z", str(max_dimension), str(out)],
                       capture_output=True, timeout=60)
        size = desktop_points()
        if size:
            scale = min(1.0, max_dimension / max(size))
    return {"screenshot": str(out), "scale": round(scale, 4)}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="verb", required=True)
    sub.add_parser("layout")
    sub.add_parser("census")
    sub.add_parser("dismiss")
    games = sub.add_parser("games")
    games.add_argument("--ensure-single", action="store_true")
    games.add_argument("--dry-run", action="store_true")
    sub.add_parser("bundle")
    shot = sub.add_parser("screenshot")
    shot.add_argument("--out", type=Path, required=True)
    shot.add_argument("--max-dimension", type=int, default=1400)
    args = ap.parse_args()

    if args.verb == "layout":
        result = layout()
    elif args.verb == "census":
        result = modal_census()
    elif args.verb == "dismiss":
        result = dismiss_modals()
    elif args.verb == "games":
        result = ensure_single_game(kill=not args.dry_run) \
            if args.ensure_single else game_processes()
    elif args.verb == "bundle":
        result = bundle_report()
    else:
        result = screenshot(args.out, args.max_dimension)
    json.dump(result, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
