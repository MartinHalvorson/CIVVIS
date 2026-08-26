"""Fast region screenshots for the macOS Civ VI popup backstop.

``screencapture -R`` is convenient but can take several seconds while a game
window is composited. The popup clearer needs a fresh frame inside its two
second budget, so use CoreGraphics directly and keep the shell command as a
compatibility fallback for hosts without Apple's compiler or screen permission.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


class CaptureUnavailable(RuntimeError):
    """Raised when the native macOS region capture cannot be initialized."""


_SWIFT_SOURCE = r'''
import CoreGraphics
import Darwin
import Foundation
import ImageIO

let args = Array(CommandLine.arguments.dropFirst())
guard args.count == 5 else { exit(64) }

func number(_ index: Int) -> Double {
    guard let value = Double(args[index]) else { exit(64) }
    return value
}

let rect = CGRect(
    x: number(0),
    y: number(1),
    width: number(2),
    height: number(3)
)
let output = URL(fileURLWithPath: args[4])
// The macOS 15 SDK marks this CoreGraphics entry point unavailable even though
// the symbol remains present and is substantially faster than launching
// `screencapture`. Resolve it dynamically so the source still compiles on the
// new SDK; hosts where Apple removes it fall back in the Python caller.
typealias CaptureImage = @convention(c) (
    CGRect, CGWindowListOption, CGWindowID, CGWindowImageOption
) -> Unmanaged<CGImage>?
guard let framework = dlopen(
    "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_LAZY
), let symbol = dlsym(framework, "CGWindowListCreateImage") else {
    FileHandle.standardError.write(Data("CoreGraphics capture symbol unavailable".utf8))
    exit(1)
}
let capture = unsafeBitCast(symbol, to: CaptureImage.self)
guard let unmanaged = capture(
    rect,
    .optionOnScreenOnly,
    kCGNullWindowID,
    [.bestResolution, .boundsIgnoreFraming]
) else {
    FileHandle.standardError.write(Data("CoreGraphics capture returned nil".utf8))
    exit(1)
}
let image = unmanaged.takeRetainedValue()
guard let destination = CGImageDestinationCreateWithURL(
    output as CFURL,
    "public.png" as CFString,
    1,
    nil
) else {
    FileHandle.standardError.write(Data("could not create PNG destination".utf8))
    exit(1)
}
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else {
    FileHandle.standardError.write(Data("could not finalize PNG".utf8))
    exit(1)
}
'''.strip()

_NATIVE_BINARY: Path | None = None


def _native_binary() -> Path:
    global _NATIVE_BINARY

    if _NATIVE_BINARY and _NATIVE_BINARY.is_file():
        return _NATIVE_BINARY
    if sys.platform != "darwin":
        raise CaptureUnavailable("native region capture requires macOS")

    compiler = shutil.which("swiftc")
    if not compiler:
        raise CaptureUnavailable("Apple Command Line Tools (swiftc) are required")

    digest = hashlib.sha256(_SWIFT_SOURCE.encode()).hexdigest()[:16]
    cache = Path(tempfile.gettempdir()) / "civvis-capture"
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    binary = cache / f"cgcapture-{digest}"
    if binary.is_file() and os.access(binary, os.X_OK):
        _NATIVE_BINARY = binary
        return binary

    source = cache / f"cgcapture-{digest}.swift"
    source.write_text(_SWIFT_SOURCE + "\n")
    temporary = cache / f"cgcapture-{digest}-{os.getpid()}"
    result = subprocess.run(
        [compiler, "-O", str(source), "-o", str(temporary)],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if result.returncode:
        temporary.unlink(missing_ok=True)
        detail = (result.stderr or result.stdout).strip()
        raise CaptureUnavailable(f"could not compile native region capture: {detail}")
    os.replace(temporary, binary)
    _NATIVE_BINARY = binary
    return binary


def prepare() -> bool:
    """Compile/cache the helper before the first game frame is needed."""
    try:
        _native_binary()
    except (CaptureUnavailable, OSError, subprocess.SubprocessError):
        return False
    return True


def capture_region(box_points, output: str | Path) -> None:
    """Write one screen-point region to ``output`` as a PNG."""
    x, y, width, height = box_points
    result = subprocess.run(
        [str(_native_binary()), str(x), str(y), str(width), str(height), str(output)],
        capture_output=True,
        text=True,
        check=False,
        timeout=5,
    )
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise CaptureUnavailable(detail or "native region capture failed")
    path = Path(output)
    if not path.is_file() or path.stat().st_size == 0:
        raise CaptureUnavailable("native region capture wrote no image")
