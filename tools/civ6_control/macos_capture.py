"""Fast region screenshots for the macOS Civ VI popup backstop.

``screencapture -R`` is convenient but can take several seconds while a game
window is composited. The popup clearer needs a fresh frame inside its two
second budget, so use CoreGraphics directly.  A denied screen-capture grant is
reported without asking for one: an unattended game must never cover itself
with macOS's permission sheet.
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


class CapturePermissionUnavailable(CaptureUnavailable):
    """Raised when macOS says capture is not currently authorized."""


SCREEN_CAPTURE_PERMISSION_DENIED = 77


_SWIFT_SOURCE = r'''
import CoreGraphics
import Darwin
import Foundation
import ImageIO
import ScreenCaptureKit

let args = Array(CommandLine.arguments.dropFirst())
if args == ["--preflight"] {
    if CGPreflightScreenCaptureAccess() {
        exit(0)
    }
    FileHandle.standardError.write(Data("screen capture permission unavailable".utf8))
    exit(77)
}
guard args.count == 5 else { exit(64) }

// The interactive request API would open the system permission dialog.  This
// helper is deliberately preflight-only: the caller can wait and retry later
// without ever putting a modal over the game.
guard CGPreflightScreenCaptureAccess() else {
    FileHandle.standardError.write(Data("screen capture permission unavailable".utf8))
    exit(77)
}

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
// The macOS 15 SDK marks both of these CoreGraphics entry points unavailable
// even though the symbols remain present. Resolve them dynamically so the
// source still compiles on the new SDK; neither path invokes the interactive
// permission API.
typealias WindowCaptureImage = @convention(c) (
    CGRect, CGWindowListOption, CGWindowID, CGWindowImageOption
) -> Unmanaged<CGImage>?
typealias DisplayCaptureImage = @convention(c) (CGDirectDisplayID) -> Unmanaged<CGImage>?
guard let framework = dlopen(
    "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_LAZY
) else {
    FileHandle.standardError.write(Data("CoreGraphics capture symbol unavailable".utf8))
    exit(1)
}

func windowListImage() -> CGImage? {
    guard let symbol = dlsym(framework, "CGWindowListCreateImage") else { return nil }
    let capture = unsafeBitCast(symbol, to: WindowCaptureImage.self)
    guard let unmanaged = capture(
        rect,
        .optionOnScreenOnly,
        kCGNullWindowID,
        [.bestResolution, .boundsIgnoreFraming]
    ) else { return nil }
    return unmanaged.takeRetainedValue()
}

func mainDisplayImage() -> CGImage? {
    guard let symbol = dlsym(framework, "CGDisplayCreateImage") else { return nil }
    let capture = unsafeBitCast(symbol, to: DisplayCaptureImage.self)
    let display = CGMainDisplayID()
    guard let unmanaged = capture(display) else { return nil }
    let image = unmanaged.takeRetainedValue()
    let bounds = CGDisplayBounds(display)
    guard rect.minX >= bounds.minX, rect.minY >= bounds.minY,
          rect.maxX <= bounds.maxX, rect.maxY <= bounds.maxY else { return nil }
    let scaleX = CGFloat(image.width) / bounds.width
    let scaleY = CGFloat(image.height) / bounds.height
    let crop = CGRect(
        x: (rect.minX - bounds.minX) * scaleX,
        y: (rect.minY - bounds.minY) * scaleY,
        width: rect.width * scaleX,
        height: rect.height * scaleY
    ).integral
    return image.cropping(to: crop)
}

func screenCaptureKitImage() -> CGImage? {
    guard #available(macOS 15.0, *) else { return nil }
    let semaphore = DispatchSemaphore(value: 0)
    var captured: CGImage?
    SCScreenshotManager.captureImage(in: rect) { image, _ in
        captured = image
        semaphore.signal()
    }
    guard semaphore.wait(timeout: .now() + .milliseconds(3500)) == .success else {
        return nil
    }
    return captured
}

// Cmd-Shift-5 can block or empty CGWindowListCreateImage while this process is
// still authorized to capture. ScreenCaptureKit takes a one-frame, exact
// display-space rectangle without interacting with the recording UI, so it is
// the current-macOS path. Older systems retain the direct-display and
// window-list paths without introducing a permission request.
let image: CGImage?
if #available(macOS 15.0, *) {
    image = screenCaptureKitImage()
} else {
    image = mainDisplayImage() ?? windowListImage()
}
guard let image else {
    FileHandle.standardError.write(Data("CoreGraphics capture returned no image".utf8))
    exit(1)
}
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
    """Compile/cache the helper before the first game frame is needed.

    This deliberately does not request or verify a macOS privacy grant.  Use
    :func:`screen_capture_access_available` when a caller needs the latter.
    """
    try:
        _native_binary()
    except (CaptureUnavailable, OSError, subprocess.SubprocessError):
        return False
    return True


def screen_capture_access_available() -> bool:
    """Return whether the native helper can capture without prompting macOS.

    ``CGPreflightScreenCaptureAccess`` is the non-interactive companion to
    macOS's permission request API.  A false result is an ordinary deferral,
    not a reason to run ``screencapture`` and create a dialog over Civ VI.
    """
    try:
        result = subprocess.run(
            [str(_native_binary()), "--preflight"],
            capture_output=True,
            text=True,
            check=False,
            timeout=NATIVE_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise CaptureUnavailable(f"could not preflight native screen capture: {error}") from error
    if result.returncode == 0:
        return True
    detail = (result.stderr or result.stdout).strip()
    if result.returncode == SCREEN_CAPTURE_PERMISSION_DENIED:
        return False
    raise CaptureUnavailable(detail or "could not preflight native screen capture")


#: The helper's own ScreenCaptureKit guard, in seconds — keep in step with the
#: `.milliseconds(3500)` semaphore in the Swift source above.
NATIVE_GUARD_SECONDS = 3.5

#: What Python allows the helper before killing it. It must be comfortably MORE
#: than the helper's own guard, or a helper that is giving up correctly is shot
#: while it unwinds.
#:
#: ⚠⚠ IT WAS 5, AND THE 1.5 s OF HEADROOM WAS NOT ENOUGH. Measured 2026-08-28 on
#: this host: a healthy capture takes 0.06 s, and a capture during a
#: `systemstatusd` spin returns cleanly at **3.51 s** — the guard doing its job.
#: That leaves 1.49 s for process start, framework load and exit, and under the
#: load a spin implies that is not always enough: `popup_clear.log` carries 379
#: `timed out after 5 seconds` kills in one day, steady at 10-100 per hour.
#:
#: The difference is not cosmetic. A helper that returns reports "no image this
#: pass" and the clearer retries on its next poll; a helper that is KILLED is an
#: error, and an error blinds the popup backstop for
#: `SYSTEMSTATUSD_RECOVERY_PAUSE_SECONDS` — thirty seconds during which no card
#: on screen can be seen or cleared. Run civvis-20260828T210457Z wedged at turn
#: 77 with six cities after five straight minutes of exactly that cycle:
#: error, pause 30 s, resume, error.
#:
#: ⚠ The spin itself is NOT fixed by this and pausing through one is right — a
#: probe caught a spin live and the capture genuinely failed. This only stops a
#: correct give-up from being recorded as a crash.
NATIVE_TIMEOUT_SECONDS = NATIVE_GUARD_SECONDS + 4.0


def capture_region(box_points, output: str | Path) -> None:
    """Write one screen-point region to ``output`` as a PNG."""
    x, y, width, height = box_points
    result = subprocess.run(
        [str(_native_binary()), str(x), str(y), str(width), str(height), str(output)],
        capture_output=True,
        text=True,
        check=False,
        timeout=NATIVE_TIMEOUT_SECONDS,
    )
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        if result.returncode == SCREEN_CAPTURE_PERMISSION_DENIED:
            raise CapturePermissionUnavailable(detail or "screen capture permission unavailable")
        raise CaptureUnavailable(detail or "native region capture failed")
    path = Path(output)
    if not path.is_file() or path.stat().st_size == 0:
        raise CaptureUnavailable("native region capture wrote no image")
