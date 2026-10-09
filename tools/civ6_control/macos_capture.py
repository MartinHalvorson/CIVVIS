"""Fast region screenshots for the macOS Civ VI popup backstop.

``screencapture -R`` is convenient but can take several seconds while a game
window is composited. The popup clearer needs a fresh frame inside its two
second budget, so use CoreGraphics directly.  A denied screen-capture grant is
reported without asking for one: an unattended game must never cover itself
with macOS's permission sheet.

Frames come from long-lived helpers (one per backend), not one process per
frame: see `_CaptureServer` for what the per-frame processes did to the
operator's own Cmd-Shift-5 recordings.
"""

from __future__ import annotations

import atexit
import hashlib
import os
import select
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path


class CaptureUnavailable(RuntimeError):
    """Raised when the native macOS region capture cannot be initialized."""


class CapturePermissionUnavailable(CaptureUnavailable):
    """Raised when macOS says capture is not currently authorized."""


SCREEN_CAPTURE_PERMISSION_DENIED = 77
#: The preferred ScreenCaptureKit backend returned no frame. This is distinct
#: from a permission denial: a freshly spawned window-list helper may still
#: capture safely while native recording is active.
SCREEN_CAPTURE_FALLBACK_NEEDED = 78


_SWIFT_SOURCE = r'''
import CoreGraphics
import Darwin
import Foundation
import ImageIO
import ScreenCaptureKit

let rawArguments = Array(CommandLine.arguments.dropFirst())
if rawArguments == ["--preflight"] {
    if CGPreflightScreenCaptureAccess() {
        exit(0)
    }
    FileHandle.standardError.write(Data("screen capture permission unavailable".utf8))
    exit(77)
}
let serveMode = rawArguments.first == "--serve"
let modeArguments = serveMode ? Array(rawArguments.dropFirst()) : rawArguments
let fallbackMode = modeArguments.first == "--fallback"
let args = fallbackMode ? Array(modeArguments.dropFirst()) : modeArguments

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

func windowListImage(_ rect: CGRect) -> CGImage? {
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

func mainDisplayImage(_ rect: CGRect) -> CGImage? {
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

func screenCaptureKitImage(_ rect: CGRect) -> CGImage? {
    guard #available(macOS 15.0, *) else { return nil }
    let semaphore = DispatchSemaphore(value: 0)
    // Local to this request: a callback that lands after the guard expired
    // writes into its own abandoned slot, never into a later request's.
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
// the preferred current-macOS path. But a native recording can also leave
// ScreenCaptureKit with a granted preflight and a nil frame. That callback can
// poison CoreGraphics work attempted in the same process, so Python asks a
// separate `--fallback` helper instead of chaining the window-list call here:
// a ScreenCaptureKit helper never runs CoreGraphics capture, and the reverse.
func capture(_ rect: CGRect, to output: URL, fallback: Bool) -> (Int32, String) {
    let image: CGImage?
    let signalFallback: Bool
    if fallback {
        // Window-list capture is the fast recording-safe alternate backend. Do
        // not reach direct-display capture from this fast retry: it can block
        // behind the native recorder for tens of seconds.
        image = windowListImage(rect)
        signalFallback = false
    } else if #available(macOS 15.0, *) {
        image = screenCaptureKitImage(rect)
        signalFallback = true
    } else {
        image = mainDisplayImage(rect) ?? windowListImage(rect)
        signalFallback = false
    }
    guard let image else {
        return (signalFallback ? 78 : 1, "CoreGraphics capture returned no image")
    }
    guard let destination = CGImageDestinationCreateWithURL(
        output as CFURL,
        "public.png" as CFString,
        1,
        nil
    ) else {
        return (1, "could not create PNG destination")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else {
        return (1, "could not finalize PNG")
    }
    return (0, "")
}

// The interactive request API would open the system permission dialog.  This
// helper is deliberately preflight-only: the caller can wait and retry later
// without ever putting a modal over the game.
if serveMode {
    // One line per frame in: `x y width height output-path`. One line out:
    // the status a one-shot helper would have exited with, then its detail.
    // EOF (the Python side closed the pipe or died) ends the helper.
    func reply(_ status: Int32, _ detail: String) {
        let line = detail.isEmpty ? "\(status)\n" : "\(status) \(detail)\n"
        FileHandle.standardOutput.write(Data(line.utf8))
    }
    while let request = readLine() {
        let fields = request.split(
            separator: " ", maxSplits: 4, omittingEmptySubsequences: false
        ).map(String.init)
        guard fields.count == 5,
              let x = Double(fields[0]), let y = Double(fields[1]),
              let width = Double(fields[2]), let height = Double(fields[3]),
              !fields[4].isEmpty else {
            reply(64, "malformed capture request")
            continue
        }
        guard CGPreflightScreenCaptureAccess() else {
            reply(77, "screen capture permission unavailable")
            continue
        }
        let (status, detail) = capture(
            CGRect(x: x, y: y, width: width, height: height),
            to: URL(fileURLWithPath: fields[4]),
            fallback: fallbackMode
        )
        reply(status, detail)
    }
    exit(0)
}

// One-shot form, kept for diagnosis by hand: `cgcapture-<digest> x y w h out.png`.
guard args.count == 5 else { exit(64) }
guard CGPreflightScreenCaptureAccess() else {
    FileHandle.standardError.write(Data("screen capture permission unavailable".utf8))
    exit(77)
}
func number(_ index: Int) -> Double {
    guard let value = Double(args[index]) else { exit(64) }
    return value
}
let (status, detail) = capture(
    CGRect(x: number(0), y: number(1), width: number(2), height: number(3)),
    to: URL(fileURLWithPath: args[4]),
    fallback: fallbackMode
)
if status != 0 {
    FileHandle.standardError.write(Data(detail.utf8))
}
exit(status)
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


def _capture_command(box_points, output: str | Path, *, fallback: bool) -> list[str]:
    x, y, width, height = box_points
    command = [str(_native_binary())]
    if fallback:
        command.append("--fallback")
    command.extend([str(x), str(y), str(width), str(height), str(output)])
    return command


#: ★★★★★ ONE PROCESS FOR MANY FRAMES, BECAUSE A PROCESS PER FRAME BROKE THE
#: OPERATOR'S OWN SCREEN RECORDINGS.
#:
#: Every capture used to start a fresh `cgcapture-<digest>` and let it exit.
#: The popup keeper polls at 0.25 s, so on 2026-10-08 that was ~34 helper
#: processes a minute, ~1,500 an hour, each one a NEW screen-capture client
#: that `systemstatusd` (the daemon behind the menu-bar recording indicator)
#: had to attribute, publish to Control Center, and -- once the helper had
#: already exited -- fail to name ("Failed to find any name for executable",
#: then a directory-services lookup). Measured the same evening:
#:
#:   * 48 attribution publishes in one minute against 34 helper launches,
#:     and the clip recorder's two long-lived streams contributing none;
#:   * 20 frames from ONE process published NOTHING for 5 s straight, at
#:     10-20 ms a frame; 20 spawned helpers each published, took 150-200 ms,
#:     and half of them hit the 3.5 s guard;
#:   * `popup_clear.log`'s own "systemstatusd is spinning" pauses climbed
#:     24 -> 52 -> 86 -> 137 -> 154 -> 177 an hour after a reboot, the same
#:     daemon that had sat at ~550 an hour (permanently spinning) for days.
#:
#: While `systemstatusd` spins, ScreenCaptureKit cannot start a stream:
#: Cmd-Shift-5 dies one second after "Recording started" with
#: `getDisplayForDisplayId timed out` (19:29:49 that night). So the lane's
#: churn, not anything the operator did, was stopping the operator's
#: recordings. A long-lived helper is attributed once and stays attributed.
#:
#: The helpers are still recycled: after a timeout (killed), a permission
#: denial, a crash, `SERVER_MAX_CONSECUTIVE_MISSES` failed frames in a row,
#: or `SERVER_MAX_AGE_SECONDS` -- two processes an hour instead of 1,500.
#: ScreenCaptureKit and the CoreGraphics fallback never share a process (see
#: the Swift comment above `capture`).
SERVER_MAX_AGE_SECONDS = 1800.0
SERVER_MAX_CONSECUTIVE_MISSES = 3


class _CaptureServer:
    """One long-lived `cgcapture --serve [--fallback]` helper.

    Requests are one line, `x y width height output`; the reply is one line,
    the status a one-shot helper would have exited with, then its detail. A
    reply comes back as the `subprocess.CompletedProcess` the one-shot helper
    produced, so every caller above `_capture_once` is unchanged.
    """

    def __init__(self, binary: Path, fallback: bool) -> None:
        self.binary = binary
        self.fallback = fallback
        self.process: subprocess.Popen | None = None
        self.buffer = b""
        self.started = 0.0
        self.misses = 0
        self.lock = threading.Lock()

    def command(self) -> list[str]:
        return [str(self.binary), "--serve"] + (["--fallback"] if self.fallback else [])

    def _start(self) -> subprocess.Popen:
        self.process = subprocess.Popen(
            self.command(),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=0,
        )
        self.buffer = b""
        self.started = time.monotonic()
        self.misses = 0
        return self.process

    def stop(self, *, kill: bool = False) -> None:
        process, self.process = self.process, None
        self.buffer = b""
        if process is None:
            return
        try:
            process.stdin.close()
        except OSError:
            pass
        if not kill:
            try:
                process.wait(timeout=1.0)  # EOF ends a healthy helper at once
                return
            except subprocess.TimeoutExpired:
                pass
        try:
            process.kill()
            process.wait(timeout=2.0)
        except (OSError, subprocess.TimeoutExpired):
            pass

    def _failed(self, arguments: list[str], detail: str) -> subprocess.CompletedProcess:
        self.stop(kill=True)
        return subprocess.CompletedProcess(arguments, 1, "", detail)

    def request(self, arguments: list[str], *, timeout: float) -> subprocess.CompletedProcess | None:
        """One frame; None when the helper had to be killed for taking too long."""
        with self.lock:
            process = self.process
            if process is not None and (
                    process.poll() is not None
                    or time.monotonic() - self.started >= SERVER_MAX_AGE_SECONDS):
                self.stop()
                process = None
            if process is None:
                process = self._start()
            try:
                os.write(process.stdin.fileno(), (" ".join(arguments) + "\n").encode())
            except OSError:
                return self._failed(arguments, "native capture helper exited")
            deadline = time.monotonic() + timeout
            descriptor = process.stdout.fileno()
            while b"\n" not in self.buffer:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    self.stop(kill=True)
                    return None
                ready, _, _ = select.select([descriptor], [], [], remaining)
                if not ready:
                    continue
                chunk = os.read(descriptor, 4096)
                if not chunk:
                    return self._failed(arguments, "native capture helper exited")
                self.buffer += chunk
            reply, _, self.buffer = self.buffer.partition(b"\n")
            status_text, _, detail = reply.decode("utf-8", "replace").partition(" ")
            try:
                status = int(status_text)
            except ValueError:
                return self._failed(arguments, f"unreadable capture reply {reply[:80]!r}")
            if status == 0:
                self.misses = 0
            else:
                self.misses += 1
                if (status == SCREEN_CAPTURE_PERMISSION_DENIED
                        or self.misses >= SERVER_MAX_CONSECUTIVE_MISSES):
                    self.stop()
            return subprocess.CompletedProcess(arguments, status, "", detail)


_SERVERS: dict[bool, _CaptureServer] = {}
_SERVERS_LOCK = threading.Lock()


def _server(binary: Path, fallback: bool) -> _CaptureServer:
    with _SERVERS_LOCK:
        server = _SERVERS.get(fallback)
        if server is not None and server.binary != binary:
            server.stop()  # the helper source changed under a running caller
            server = None
        if server is None:
            server = _SERVERS[fallback] = _CaptureServer(binary, fallback)
        return server


def stop_capture_servers() -> None:
    """End the long-lived helpers (also run at interpreter exit)."""
    with _SERVERS_LOCK:
        servers = list(_SERVERS.values())
        _SERVERS.clear()
    for server in servers:
        with server.lock:
            server.stop()


atexit.register(stop_capture_servers)


def _capture_once(command: list[str]) -> subprocess.CompletedProcess | None:
    """Run one native capture backend without letting a hung helper persist."""
    binary, arguments = Path(command[0]), list(command[1:])
    fallback = bool(arguments) and arguments[0] == "--fallback"
    if fallback:
        arguments = arguments[1:]
    if any("\n" in argument for argument in arguments):
        raise CaptureUnavailable("capture output path may not contain a newline")
    return _server(binary, fallback).request(arguments, timeout=NATIVE_TIMEOUT_SECONDS)


#: ★★★★★ HOW LONG THE FALLBACK IS LEFT ALONE ONCE IT HAS PROVED IT WILL HANG.
#:
#: The CoreGraphics fallback exists for one situation: ScreenCaptureKit
#: preflights successfully and then yields no frame because Cmd-Shift-5 owns the
#: native recording stream.  On this host, 2026-09-02, that situation is
#: permanent -- an interactive `screencapture -pdiU -z` picker has been open
#: since 10:04 and `systemstatusd` has been pinned at 99 % CPU ever since -- and
#: in it the fallback does not merely fail, it never returns:
#:
#:     native(SCK)  #0 3.52s rc=78 "CoreGraphics capture returned no image"
#:     native(SCK)  #1 3.52s rc=78    (its own 3.5 s guard, every time)
#:     fallback(CG) #0 TIMEOUT >20s
#:     fallback(CG) #1 TIMEOUT >20s   (killed by Python at 7.5 s in production)
#:
#: So every capture cost 3.5 + 7.5 = 11.0 s and returned nothing.  The
#: verification harness spends two per desktop rescue, which is the 23.5 s that
#: separated `autoclose_desktop` from the next line of a live run 23 times in
#: one 31-minute game -- 30 % of it -- and 25.8 min of 68.6 in run
#: civvis-20260902T095330Z.  The popup keeper pays the same 11 s on its own
#: polls, which is why `popup_clear.log` reached 60 MB in a day.
#:
#: ⚠ A BREAKER, NOT A REMOVAL.  The fallback is the only backend that can work
#: while a recording owns the stream, so it must come back on its own: a picker
#: closes, a grant changes, the spin ends.  After this many seconds the next
#: capture tries it again, and one success clears the breaker entirely.
FALLBACK_BREAKER_SECONDS = 120.0
#: Consecutive fallback timeouts before the breaker opens.  Two, not one: a
#: single kill under momentary load is the ordinary transient this module has
#: always tolerated, and `SHOT_BACKOFF_SECONDS` already retries it.
FALLBACK_BREAKER_TIMEOUTS = 2

_fallback_timeouts = 0
_fallback_opened_at: float | None = None


def _fallback_available(now: float | None = None) -> bool:
    """Whether the CoreGraphics fallback is worth its wall clock right now."""
    if _fallback_opened_at is None:
        return True
    if now is None:
        now = time.monotonic()
    return (now - _fallback_opened_at) >= FALLBACK_BREAKER_SECONDS


def _note_fallback(result: subprocess.CompletedProcess | None) -> None:
    """Record what the fallback did, and open or clear the breaker."""
    global _fallback_timeouts, _fallback_opened_at
    if result is None:
        _fallback_timeouts += 1
        if _fallback_timeouts >= FALLBACK_BREAKER_TIMEOUTS:
            _fallback_opened_at = time.monotonic()
        return
    _fallback_timeouts = 0
    _fallback_opened_at = None


def reset_fallback_breaker() -> None:
    """Forget the fallback's history. For tests and for a deliberate retry."""
    global _fallback_timeouts, _fallback_opened_at, _native_sticky_since
    _fallback_timeouts = 0
    _fallback_opened_at = None
    _native_sticky_since = None


#: ★ A STICKY FALLBACK: THE MIRROR OF THE BREAKER ABOVE. With systemstatusd
#: degrading ScreenCaptureKit, a native capture spends its whole 3.5 s guard
#: before answering "no frame" (rc 78) and only then does the CoreGraphics
#: fallback produce the image -- so every capture paid the guard again. At
#: the G102 boundary (2026-10-05, pin 4f7c715) two setup captures on the
#: critical path took 7.9 s and 6.6 s against 0.2 s for a healthy one. Once
#: the fallback has rescued a native "no frame", captures go to the fallback
#: FIRST for NATIVE_STICKY_SECONDS; then native is tried again, so a recovered
#: SCK is back on the next setup. A sticky fallback that stops producing a
#: frame drops straight back to native in the same call.
NATIVE_STICKY_SECONDS = 300.0
_native_sticky_since: float | None = None


def _wrote(result: subprocess.CompletedProcess | None, output: str | Path) -> bool:
    if result is None or result.returncode:
        return False
    try:
        return Path(output).stat().st_size > 0
    except OSError:
        return False


PROBE_REGION_POINTS = (0, 0, 8, 8)


def capture_probe() -> bool:
    """Return whether the capture path can produce a real frame right now.

    A privacy preflight is necessary but not sufficient on macOS: an active
    native recorder can leave the grant looking healthy while ScreenCaptureKit
    returns no image and the CoreGraphics fallback blocks.  Probe through
    :func:`capture_region` so this check exercises the exact primary/fallback
    route that setup and popup screenshots will use.  Permission denial still
    propagates as the specific safe error; every other transient failure is an
    ordinary false result for the caller's next poll.
    """
    try:
        with tempfile.TemporaryDirectory(prefix="civvis-capture-probe-") as root:
            capture_region(PROBE_REGION_POINTS, Path(root) / "frame.png")
    except CapturePermissionUnavailable:
        raise
    except (CaptureUnavailable, OSError, subprocess.SubprocessError):
        return False
    return True


def capture_region(box_points, output: str | Path) -> None:
    """Write one screen-point region to ``output`` as a PNG."""
    global _native_sticky_since
    now = time.monotonic()
    if _native_sticky_since is not None and now - _native_sticky_since >= NATIVE_STICKY_SECONDS:
        _native_sticky_since = None  # time to see whether SCK has recovered
    if _native_sticky_since is not None and _fallback_available(now):
        result = _capture_once(_capture_command(box_points, output, fallback=True))
        _note_fallback(result)
        if _wrote(result, output):
            return
        _native_sticky_since = None  # the fallback stopped working: native first again
    result = _capture_once(_capture_command(box_points, output, fallback=False))
    if result is not None and result.returncode == SCREEN_CAPTURE_FALLBACK_NEEDED:
        # ScreenCaptureKit can preflight successfully yet yield no image while
        # Cmd-Shift-5 owns the native recording stream. Start the CoreGraphics
        # fallback in a fresh helper because the first attempt can leave the
        # original process's capture state wedged.
        if not _fallback_available():
            raise CaptureUnavailable(
                "native region capture yielded no image and the CoreGraphics "
                "fallback is timing out; skipping it for "
                f"{FALLBACK_BREAKER_SECONDS:.0f}s")
        result = _capture_once(_capture_command(box_points, output, fallback=True))
        _note_fallback(result)
        if _wrote(result, output):
            _native_sticky_since = time.monotonic()
            print(f"[shot] capture: sticky fallback (native timed out at "
                  f"{time.strftime('%H:%M:%SZ', time.gmtime())}); CoreGraphics first "
                  f"for {NATIVE_STICKY_SECONDS:.0f}s", flush=True)
    if result is None:
        raise CaptureUnavailable("native region capture timed out")
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        if result.returncode == SCREEN_CAPTURE_PERMISSION_DENIED:
            raise CapturePermissionUnavailable(detail or "screen capture permission unavailable")
        raise CaptureUnavailable(detail or "native region capture failed")
    path = Path(output)
    if not path.is_file() or path.stat().st_size == 0:
        raise CaptureUnavailable("native region capture wrote no image")
