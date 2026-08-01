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
    guard args.count == 2 else { invalidArguments() }
    let code = CGKeyCode(integer(1))
    CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)?
        .post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)?
        .post(tap: .cghidEventTap)
case "scroll":
    guard args.count == 2 else { invalidArguments() }
    CGEvent(
        scrollWheelEvent2Source: source,
        units: .line,
        wheelCount: 1,
        wheel1: Int32(integer(1)),
        wheel2: 0,
        wheel3: 0
    )?.post(tap: .cghidEventTap)
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


def press_key(name: str, *, check: bool = False):
    """Press a named key supported by both the native and cliclick backends."""
    key_codes = {"escape": 53}
    normalized = name.lower()
    if normalized not in key_codes:
        raise ValueError(f"unsupported key: {name}")
    if _cliclick():
        return _run_cliclick([f"kp:{'esc' if normalized == 'escape' else normalized}"], check=check)
    return _run_native(["key", str(key_codes[normalized])], check=check)


def scroll(lines: int, *, check: bool = False):
    """Scroll under the pointer by a signed number of wheel lines."""
    if _cliclick():
        return _run_cliclick([f"w:{lines}"], check=check)
    return _run_native(["scroll", str(lines)], check=check)
