"""Send the small set of local input events CIVVIS needs on macOS.

The live-game tools used to require ``cliclick`` even though it is not part of
macOS.  Keep using it when an operator has installed it, but make a stock
Command Line Tools installation sufficient by compiling a tiny CoreGraphics
helper into the temporary directory on first use.

Coordinates are Quartz screen points, which is also the coordinate system used
by System Events window bounds.  They are deliberately not screenshot pixels on
Retina displays.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


class InputUnavailable(RuntimeError):
    """Raised when this host has no supported way to send an input event."""


_SWIFT_SOURCE = r'''
import CoreGraphics
import Darwin
import Foundation

let args = Array(CommandLine.arguments.dropFirst())
let source = CGEventSource(stateID: .hidSystemState)

func invalidArguments() -> Never {
    exit(64)
}

func coordinate(_ index: Int) -> CGFloat {
    guard index < args.count, let value = Double(args[index]) else {
        invalidArguments()
    }
    return CGFloat(value)
}

func integer(_ index: Int) -> Int {
    guard index < args.count, let value = Int(args[index]) else {
        invalidArguments()
    }
    return value
}

func postMouse(_ type: CGEventType, _ point: CGPoint) {
    CGEvent(
        mouseEventSource: source,
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: .left
    )?.post(tap: .cghidEventTap)
}

guard let action = args.first else {
    invalidArguments()
}

switch action {
case "move":
    guard args.count == 3 else { invalidArguments() }
    postMouse(.mouseMoved, CGPoint(x: coordinate(1), y: coordinate(2)))
case "click":
    guard args.count == 4 else { invalidArguments() }
    let point = CGPoint(x: coordinate(1), y: coordinate(2))
    postMouse(.mouseMoved, point)
    Thread.sleep(forTimeInterval: 0.03)
    postMouse(.leftMouseDown, point)
    Thread.sleep(forTimeInterval: Double(max(0, integer(3))) / 1000.0)
    postMouse(.leftMouseUp, point)
case "key":
    // An optional third argument is a modifier held for the keystroke. The flag
    // must be set on BOTH the down and the up event: Civilization VI reads the
    // modifier off the event it receives, and a bare key-up leaves the shift
    // state ambiguous for whatever the game does next.
    guard args.count == 2 || args.count == 3 else { invalidArguments() }
    let code = CGKeyCode(integer(1))
    var flags: CGEventFlags = []
    if args.count == 3 {
        switch args[2] {
        case "shift": flags.insert(.maskShift)
        case "control": flags.insert(.maskControl)
        case "option": flags.insert(.maskAlternate)
        case "command": flags.insert(.maskCommand)
        default: invalidArguments()
        }
    }
    let down = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)
    down?.flags = flags
    down?.post(tap: .cghidEventTap)
    let up = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)
    up?.flags = flags
    up?.post(tap: .cghidEventTap)
case "scroll":
    // ⚠⚠⚠ PIXELS, NOT LINES — Civilization VI IGNORES LINE-UNIT SCROLL ENTIRELY.
    //
    // Measured 2026-08-03 against the Create Game leader picker, with the list
    // open, the pointer inside it and Civ6_Exe_Child frontmost: `cliclick w:-30`,
    // `w:-5`, `w:-1` and this helper's own line-unit event all moved the list by
    // ZERO rows, repeatedly. The same helper in pixel units scrolls it. Real
    // trackpad scrolling always worked, which is why nothing ever caught it.
    //
    // ⚠ The COUNT matters as much as the unit: one pixel event of -100 moves
    // nothing, while two of -40 move a page. So the caller says how many events
    // to post, and the magnitude of each stays small.
    guard args.count == 2 || args.count == 3 else { invalidArguments() }
    let pixels = Int32(integer(1))
    let times = args.count == 3 ? max(1, integer(2)) : 1
    for _ in 0..<times {
        CGEvent(
            scrollWheelEvent2Source: source,
            units: .pixel,
            wheelCount: 1,
            wheel1: pixels,
            wheel2: 0,
            wheel3: 0
        )?.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.02)
    }
default:
    invalidArguments()
}
'''.strip()

_NATIVE_BINARY: Path | None = None


def _cliclick() -> str | None:
    return shutil.which("cliclick")


def backend_name() -> str | None:
    """Return the usable input backend without sending an event."""
    if _cliclick():
        return "cliclick"
    if sys.platform == "darwin" and shutil.which("swiftc"):
        return "CoreGraphics via swiftc"
    return None


def _native_binary() -> Path:
    global _NATIVE_BINARY

    if _NATIVE_BINARY and _NATIVE_BINARY.is_file():
        return _NATIVE_BINARY
    if sys.platform != "darwin":
        raise InputUnavailable("CIVVIS input fallback requires macOS or cliclick")

    compiler = shutil.which("swiftc")
    if not compiler:
        raise InputUnavailable("install cliclick or Apple's Command Line Tools (swiftc)")

    digest = hashlib.sha256(_SWIFT_SOURCE.encode()).hexdigest()[:16]
    cache = Path(tempfile.gettempdir()) / "civvis-input"
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    binary = cache / f"cginput-{digest}"
    if binary.is_file() and os.access(binary, os.X_OK):
        _NATIVE_BINARY = binary
        return binary

    source = cache / f"cginput-{digest}.swift"
    source.write_text(_SWIFT_SOURCE + "\n")
    temporary = cache / f"cginput-{digest}-{os.getpid()}"
    result = subprocess.run(
        [compiler, "-O", str(source), "-o", str(temporary)],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if result.returncode:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
        detail = (result.stderr or result.stdout).strip()
        raise InputUnavailable(f"could not compile CoreGraphics input helper: {detail}")
    os.replace(temporary, binary)
    _NATIVE_BINARY = binary
    return binary


def probe() -> str:
    """Verify that a backend can be initialized without clicking anywhere."""
    backend = backend_name()
    if backend is None:
        raise InputUnavailable("install cliclick or Apple's Command Line Tools (swiftc)")
    if backend != "cliclick":
        _native_binary()
    return backend


def _run_cliclick(arguments: list[str], *, check: bool):
    executable = _cliclick()
    if not executable:
        raise InputUnavailable("cliclick is not available")
    return subprocess.run(
        [executable, *arguments],
        capture_output=True,
        text=True,
        check=check,
        timeout=20,
    )


def _run_native(arguments: list[str], *, check: bool):
    return subprocess.run(
        [str(_native_binary()), *arguments],
        capture_output=True,
        text=True,
        check=check,
        timeout=20,
    )


def move(x: int, y: int, *, check: bool = False):
    """Move the pointer to a Quartz point."""
    if _cliclick():
        return _run_cliclick([f"m:{x},{y}"], check=check)
    return _run_native(["move", str(x), str(y)], check=check)


def click(x: int, y: int, *, hold_s: float = 0.0, check: bool = False):
    """Click a point, optionally holding the button for a measured duration."""
    if _cliclick():
        if hold_s <= 0:
            return _run_cliclick([f"c:{x},{y}"], check=check)
        down = _run_cliclick([f"dd:{x},{y}"], check=check)
        if down.returncode:
            return down
        time.sleep(hold_s)
        return _run_cliclick([f"du:{x},{y}"], check=check)
    hold_ms = max(0, round(hold_s * 1000))
    return _run_native(["click", str(x), str(y), str(hold_ms)], check=check)


#: Virtual key codes for the keys this controller sends, and the name cliclick
#: knows each by.  Kept together so the two backends cannot drift.
KEY_CODES = {"escape": (53, "esc"), "return": (36, "return")}

#: Modifiers, canonical name -> the name cliclick knows it by.
#:
#: ⚠ THE TWO VOCABULARIES DIFFER AND ONLY `shift` OVERLAPS.  cliclick takes
#: `alt`, `cmd`, `ctrl`, `fn`, `shift` (its own `kd:` help lists them), while
#: the native helper switches on the Cocoa-ish spellings that name the
#: `CGEventFlags`.  Passing the canonical name straight through silently
#: refused three of the four.
MODIFIERS = {"shift": "shift", "control": "ctrl",
             "option": "alt", "command": "cmd"}


def press_key(name: str, *, modifier: str | None = None, check: bool = False):
    """Press a named key, optionally with one modifier held.

    ⚠ SHIFT+RETURN is Civilization VI's forced end turn — the same request the
    shipped UI sends, and the one form the engine does not refuse while a
    blocker stands.  It exists here so a turn that has parked can be nudged from
    OUTSIDE the game, where this harness still has its input grants even when
    the mod has stopped ticking.
    """
    normalized = name.lower()
    if normalized not in KEY_CODES:
        raise ValueError(f"unsupported key: {name}")
    if modifier is not None and modifier not in MODIFIERS:
        raise ValueError(f"unsupported modifier: {modifier}")
    code, cli_name = KEY_CODES[normalized]
    if _cliclick():
        if modifier is None:
            return _run_cliclick([f"kp:{cli_name}"], check=check)
        # One invocation, so the modifier cannot be left held by a crash between
        # two of them.
        held = MODIFIERS[modifier]
        return _run_cliclick(
            [f"kd:{held}", f"kp:{cli_name}", f"ku:{held}"], check=check
        )
    arguments = ["key", str(code)]
    if modifier is not None:
        arguments.append(modifier)
    return _run_native(arguments, check=check)


# One wheel notch, in pixels. Small on purpose: Civilization VI ignores a single
# large pixel event and honours a burst of small ones, so the caller's notch count
# is what does the scrolling and this only has to be big enough to register.
WHEEL_PIXELS = 40


def scroll(notches: int, *, check: bool = False):
    """Scroll under the pointer by a signed number of wheel notches.

    ⚠⚠⚠ NEVER cliclick, AND NEVER LINE UNITS. Civilization VI ignores line-unit
    scroll events completely — measured against the Create Game leader picker with
    the list open, the pointer inside it and the game frontmost, `cliclick w:-30`,
    `w:-5` and `w:-1` each moved it by zero rows. Real trackpad scrolling always
    worked, so the harness's own note ("thirteen -30 wheel ticks only reached
    Harald") recorded a hand-scrolled list and the automated path never scrolled at
    all. It cost the leader pin: `select_requested_leader` scrolled 100 times, never
    left the letter A, and correctly refused to start a game as the wrong leader.

    `cliclick` can only emit line units, so scrolling always takes the native
    helper. Every other verb still prefers cliclick.
    """
    notches = int(notches)
    if notches == 0:
        return None
    pixels = WHEEL_PIXELS if notches > 0 else -WHEEL_PIXELS
    return _run_native(["scroll", str(pixels), str(abs(notches))], check=check)
