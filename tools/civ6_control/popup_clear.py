#!/usr/bin/env python3
"""Clear Civilization VI screens that cover the map, from outside the game.

The mod's autoclose shim handles almost everything and is much cheaper than
this; see `mod/CivvisControlAutoClose.lua`. This is the backstop for the cases
it cannot reach:

  * a screen whose context has no closer that works. Measured live: a leader
    conversation asking a question ignores every in-Lua rung, and it ignores
    Escape too (`cliclick kp:esc` left Gorgo's embassy request exactly where it
    was) -- a question needs an ANSWER, and there is nothing for a dismiss to
    do. A held click on the dialogue button answers it.
  * a run whose shim is already dead. Before PR #689 every context disabled
    itself on its 20th popup, so a game in flight cannot be rescued by fixing
    the mod -- the installed copy is read at game start.

⚠ It clicks the real game, so it refuses to act unless it is sure:
  * Civilization VI must be frontmost. A click goes to whatever is in front,
    and this box runs a browser and a terminal beside the game.
  * the map must be positively covered. "I did not recognise this" is never
    treated as a popup -- the failure we can afford is leaving a screen up, not
    clicking a unit somewhere on a live map.

Both detectors were measured against the live game rather than guessed:
  * a leader scene is a full-screen cinema, 0.55 of the window darker than
    luminance 24, against 0.11 for any view with the map showing -- including
    one with a card popup over it.
  * a card popup (RESEARCH/CIVIC COMPLETED) has a dark red round close button.
    Found by clustering strong-red pixels, NOT by a centroid: a global centroid
    lands in empty space because city banners and combat warnings are red too.
    macOS's own red window button is 22x22 at the window's top-left corner and
    is excluded by position, or this would close Civilization VI.
  * tutorial-advisor cards use one or two long blue buttons in the upper-centre
    paper card. They are distinct from the HUD only when both their dimensions
    and the enclosing bright card agree; plain blue map UI is never enough.
"""

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from collections import deque
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from civ6_control import macos_capture, macos_input  # noqa: E402

PILLOW_MISSING = "popup_clear needs Pillow: python3 -m pip install pillow"


def pixel_values(image):
    """Return pixels without using Pillow's deprecated ``getdata`` API."""
    flattened = getattr(image, "get_flattened_data", None)
    if flattened is not None:
        return flattened()
    # Pillow before get_flattened_data still needs to run the popup backstop.
    return image.getdata()


def _image_library():
    """Pillow, demanded at the moment it is used rather than at import.

    ⚠ THIS USED TO BE A MODULE-LEVEL `sys.exit`, AND IT TOOK THE LAUNCHER WITH
    IT. `civ6_play` imports this module for one backstop it may never call, so
    on a machine without Pillow the whole launcher became unimportable — and
    `tools/test_civ6_play.py`, a suite about setup contracts that touch no
    pixels at all, died at collection with a message about an image library.
    Exactly one function here opens an image, so the requirement belongs to
    that function. The command-line path still fails with the same sentence
    the moment it captures anything.
    """
    try:
        from PIL import Image
    except ImportError:
        raise SystemExit(PILLOW_MISSING)
    return Image

GAME_PROCESS = "Civ6_Exe_Child"
DARK_LEVEL = 24          # luminance below this counts as "black"
LEADER_DARK_FRACTION = 0.35   # measured 0.55 leader vs 0.11 map
SHOT = "/tmp/civ6-popup-clear.png"
NATIVE_CAPTURE_DISABLED = False

# A covered game window is already a confirmed blocker. Keep the capture
# cadence below one second, and leave enough room for the click plus one fresh
# frame inside the two-second user-facing budget.
DIALOGUE_POLL_SECONDS = 0.25
MIN_POLL_SECONDS = 0.05
POINTER_SETTLE_SECONDS = 0.05
POST_CLICK_SETTLE_SECONDS = 0.20

# ScreenCaptureKit's status service can spin at a core while an output is being
# started or torn down.  The clearer must not add captures to that pressure.
# Cmd-Shift-5 itself is not a reason to yield: CoreGraphics' non-interactive
# preflight in ``capture`` proves whether an authorized recording can coexist.
SYSTEMSTATUSD_SPIN_PERCENT = 85.0
#: How long to wait before ASKING AGAIN whether capture is usable. This is a
#: re-check interval, not a cooldown: the question it re-asks is
#: `capture_pause_reason()`, which is a `pgrep` and a `ps` and never touches
#: ScreenCaptureKit. Only a capture touches SCK, and a capture happens only once
#: this says yes — so shortening it cannot hammer the daemon it is waiting on.
#:
#: ⚠⚠ IT WAS 30 s, AND THE SPIKES IT WAITS OUT LAST SECONDS. Sampled on this
#: host 2026-08-28 at 20 s intervals:
#:     0.0%  0.0%  99.2%  0.6%  93.2%  0.0%
#: `systemstatusd` bursts past the 85% threshold and is back under it within
#: one sample. A 30 s wait therefore keeps the popup backstop blind for roughly
#: a half-minute per burst, long after capture would work again — and blind
#: means no card on screen can be seen or cleared.
#:
#: That is not theoretical. Run civvis-20260828T210457Z wedged at turn 77 with
#: six cities after five straight minutes of `error, pause, resume, error` in
#: which not one frame was captured.
#:
#: ⚠ THE PAUSE ITSELF IS RIGHT AND STAYS. A probe armed against a live spin
#: attempted a capture during one three times, and every attempt failed at the
#: helper's own 3.5 s guard:
#:     SPIN PROBE: systemstatusd at 100.0% -- capture FAILED after 3.51s
#: So capture genuinely does not work through a spike; what was wrong was only
#: how long we waited before looking again.
SYSTEMSTATUSD_RECOVERY_PAUSE_SECONDS = 3.0
# Setup has no popup worth clearing, but its menu reader and the popup backstop
# both need the same accessibility service.  Yield that service briefly until
# the *newest* run records its first turn; once a game is live the ordinary
# per-popup cadence still applies.
SETUP_YIELD_SECONDS = 1.0


def osa(script):
    done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=20)
    return (done.stdout or "").strip(), (done.stderr or "").strip()


def game_in_progress(runs_dir, fresh_seconds=180.0):
    """Is a game actually being PLAYED, as opposed to being set up?

    ⚠ THE SETUP SCREENS LOOK LIKE A LEADER SCENE. Create Game is dark, and on
    2026-07-31 this tool clicked one at dark=0.36 while a run was still
    configuring itself -- a click that can change difficulty or map size and
    silently invalidate the run it was meant to protect. There is no popup worth
    clearing before the first turn, so the cheapest correct guard is to refuse
    to act until the harness has recorded one.
    """
    try:
        candidates = []
        for name in os.listdir(runs_dir):
            events = os.path.join(runs_dir, name, "events.jsonl")
            try:
                candidates.append((os.path.getmtime(events), events))
            except OSError:
                continue
        for modified, events in sorted(candidates, reverse=True):
            if time.time() - modified > fresh_seconds:
                break
            with open(events, "rb") as handle:
                blob = handle.read()
            # ⚠ Both spellings. The mod writes compact JSON from Lua and the harness
            # writes `json.dumps` with a space after the colon; checking only the
            # compact form matched nothing and the guard blocked a live game.
            if b'"kind": "turn"' in blob or b'"kind":"turn"' in blob:
                return True
        return False
    except OSError:
        return False


def newest_run_is_pre_turn(runs_dir):
    """Whether the current run has not yet recorded a turn.

    The popup keeper is deliberately a post-turn backstop: before then there
    is no in-game popup it can safely clear, and querying the Civ window races
    the setup harness for System Events.  Inspect only the newest event stream;
    a fresh setup must not inherit a previous game's turn as permission to
    capture or ask accessibility for window geometry.
    """
    try:
        candidates = []
        for name in os.listdir(runs_dir):
            events = os.path.join(runs_dir, name, "events.jsonl")
            try:
                candidates.append((os.path.getmtime(events), events))
            except OSError:
                continue
        if not candidates:
            return True
        _, events = max(candidates)
        with open(events, "rb") as handle:
            blob = handle.read()
        return (b'"kind": "turn"' not in blob
                and b'"kind":"turn"' not in blob)
    except OSError:
        # An event file can be created while its run directory is being made.
        # Yielding is safe: no recorded turn means no popup needs this helper.
        return True


def frontmost():
    out, _ = osa('tell application "System Events" to name of first process whose frontmost is true')
    return out


def window_box():
    """Civilization VI's window in SCREEN POINTS, as (x, y, w, h)."""
    out, err = osa(f'tell application "System Events" to tell process "{GAME_PROCESS}" '
                   'to get {position, size} of window 1')
    try:
        nums = [int(float(n)) for n in out.replace(",", " ").split()]
        return tuple(nums[:4])
    except Exception:
        return None


def native_recording_command(command):
    """Whether a process command line is Apple's recording UI or a video capture.

    Plain ``screencapture -x -t png`` calls belong to this helper's fallback
    path and must not make it pause itself.  Cmd-Shift-5 uses the persistent
    ``keyboard.interactive`` process, while command-line recordings carry a
    lower-case ``v`` video flag.
    """
    try:
        tokens = shlex.split(command)
    except ValueError:
        return False
    if not tokens or os.path.basename(tokens[0]) != "screencapture":
        return False
    return ("keyboard.interactive" in tokens
            or any(token.startswith("-") and "v" in token for token in tokens[1:]))


def interactive_recording_command(command):
    """Whether ``command`` is the Cmd-Shift-5 helper rather than a stale shell."""
    try:
        tokens = shlex.split(command)
    except ValueError:
        return False
    return (bool(tokens)
            and os.path.basename(tokens[0]) == "screencapture"
            and "keyboard.interactive" in tokens)


def screen_capture_ui_command(command):
    """Whether ``command`` belongs to the visible native capture UI."""
    try:
        tokens = shlex.split(command)
    except ValueError:
        return False
    return bool(tokens) and os.path.basename(tokens[0]) == "screencaptureui"


def native_recording_ui_active():
    """Return true while a native screenshot/recording toolbar is in use.

    The interactive helper may survive after its toolbar vanishes.  Require its
    UI companion as well, while command-line video capture remains active on
    its own.  That distinction lets an unattended ladder resume after a stale
    helper without competing with a real user capture.
    """
    try:
        result = subprocess.run(
            ["ps", "-axo", "command="],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    commands = result.stdout.splitlines()
    if any(native_recording_command(line) and not interactive_recording_command(line)
           for line in commands):
        return True
    return (any(interactive_recording_command(line) for line in commands)
            and any(screen_capture_ui_command(line) for line in commands))


def systemstatusd_cpu():
    """Current CPU use of the status daemon ScreenCaptureKit waits on."""
    try:
        pids = subprocess.run(
            ["pgrep", "-x", "systemstatusd"],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        ).stdout.split()
        if not pids:
            return None
        value = subprocess.run(
            ["ps", "-p", pids[0], "-o", "%cpu="],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        ).stdout.strip()
        return float(value)
    except (OSError, ValueError, subprocess.SubprocessError):
        return None


def capture_pause_reason():
    """Why this optional screenshot loop should yield display capture now."""
    global NATIVE_CAPTURE_DISABLED
    # A visible recording toolbar is not an authorization result.  capture()
    # does a non-interactive CoreGraphics preflight for every frame; if it
    # fails, it marks this loop disabled and the recovery probe below waits.
    # Pausing solely because Cmd-Shift-5 is visible made an authorized user
    # recording block every popup-clear pass and every verification game.
    if NATIVE_CAPTURE_DISABLED:
        # A denied grant can change while this persistent watcher is alive.
        # Preflight is non-interactive, so it is safe to probe it once per
        # recovery pause and resume on its own after the operator changes it.
        try:
            if macos_capture.screen_capture_access_available():
                NATIVE_CAPTURE_DISABLED = False
            else:
                return "screen capture access is unavailable"
        except macos_capture.CaptureUnavailable as error:
            return f"native screen capture unavailable: {error}"
    cpu = systemstatusd_cpu()
    if cpu is not None and cpu >= SYSTEMSTATUSD_SPIN_PERCENT:
        return f"systemstatusd is spinning at {cpu:.0f}% CPU"
    return None


def capture(box_points):
    """Grab just the game window, so the pixel:point scale calibrates itself.

    Capturing the whole screen would mean knowing the display's backing scale
    factor to turn an image pixel back into a click point. Capturing the region
    we already know in points makes the ratio measurable: `image width / w`.
    """
    Image = _image_library()
    global NATIVE_CAPTURE_DISABLED
    try:
        macos_capture.capture_region(box_points, SHOT)
    except macos_capture.CapturePermissionUnavailable:
        # Do not fall back to the command-line utility.  Its permission sheet
        # is precisely the system popup that can freeze an unattended game.
        NATIVE_CAPTURE_DISABLED = True
        raise
    image = Image.open(SHOT).convert("RGB")
    return image, image.size[0] / float(box_points[2])


def red_clusters(rgb, step=2):
    """Compact strong-red blobs, largest first, in image pixels."""
    px, (W, H) = rgb.load(), rgb.size
    hits = set()
    for y in range(0, H, step):
        for x in range(0, W, step):
            r, g, b = px[x, y]
            if r > 80 and r > 2.0 * g and r > 2.0 * b:
                hits.add((x, y))
    found, seen = [], set()
    for start in hits:
        if start in seen:
            continue
        queue, comp = deque([start]), []
        seen.add(start)
        while queue:
            cx, cy = queue.popleft()
            comp.append((cx, cy))
            for dx in (-step, 0, step):
                for dy in (-step, 0, step):
                    nxt = (cx + dx, cy + dy)
                    if nxt in hits and nxt not in seen:
                        seen.add(nxt)
                        queue.append(nxt)
        xs = [c[0] for c in comp]
        ys = [c[1] for c in comp]
        found.append({"n": len(comp), "w": max(xs) - min(xs), "h": max(ys) - min(ys),
                      "cx": sum(xs) / len(xs), "cy": sum(ys) / len(ys)})
    return sorted(found, key=lambda c: -c["n"])


def blue_clusters(rgb, step=2):
    """Compact saturated-blue blobs, largest first, in image pixels."""
    px, (W, H) = rgb.load(), rgb.size
    hits = set()
    for y in range(0, H, step):
        for x in range(0, W, step):
            r, g, b = px[x, y]
            # Advisor buttons are the same blue in every first-run card seen
            # live. The green channel is deliberately required to exceed red:
            # purple borders and map pins otherwise look blue enough.
            if b > 60 and b > 1.25 * g and g > 1.10 * r:
                hits.add((x, y))
    found, seen = [], set()
    for start in hits:
        if start in seen:
            continue
        queue, comp = deque([start]), []
        seen.add(start)
        while queue:
            cx, cy = queue.popleft()
            comp.append((cx, cy))
            for dx in (-step, 0, step):
                for dy in (-step, 0, step):
                    nxt = (cx + dx, cy + dy)
                    if nxt in hits and nxt not in seen:
                        seen.add(nxt)
                        queue.append(nxt)
        xs = [c[0] for c in comp]
        ys = [c[1] for c in comp]
        found.append({"n": len(comp), "w": max(xs) - min(xs), "h": max(ys) - min(ys),
                      "cx": sum(xs) / len(xs), "cy": sum(ys) / len(ys)})
    return sorted(found, key=lambda c: -c["n"])


def green_clusters(rgb, step=2):
    """Compact teal/green controls, largest first, in image pixels."""
    px, (W, H) = rgb.load(), rgb.size
    hits = set()
    for y in range(0, H, step):
        for x in range(0, W, step):
            r, g, b = px[x, y]
            # World Congress's Return to Game button is teal, not pure green.
            if g > 50 and g > 1.25 * r and g > 1.05 * b:
                hits.add((x, y))
    found, seen = [], set()
    for start in hits:
        if start in seen:
            continue
        queue, comp = deque([start]), []
        seen.add(start)
        while queue:
            cx, cy = queue.popleft()
            comp.append((cx, cy))
            for dx in (-step, 0, step):
                for dy in (-step, 0, step):
                    nxt = (cx + dx, cy + dy)
                    if nxt in hits and nxt not in seen:
                        seen.add(nxt)
                        queue.append(nxt)
        xs = [c[0] for c in comp]
        ys = [c[1] for c in comp]
        found.append({"n": len(comp), "w": max(xs) - min(xs), "h": max(ys) - min(ys),
                      "cx": sum(xs) / len(xs), "cy": sum(ys) / len(ys)})
    return sorted(found, key=lambda c: -c["n"])


def congress_return_button(rgb, grey):
    """Return to Game on Civilization VI's full-screen World Congress review."""
    w, h = rgb.size
    # This screen is a broad charcoal panel, unlike an ordinary map with a
    # tooltip or a normal HUD button. It always ends in one teal button at the
    # bottom; require both before touching it.
    panel = grey.crop((int(w * 0.12), int(h * 0.12), int(w * 0.95), int(h * 0.90)))
    panel_dark = sum(value < DARK_LEVEL for value in pixel_values(panel))
    if panel_dark < 0.25 * (panel.size[0] * panel.size[1]):
        return None
    for cluster in green_clusters(rgb):
        if (
            cluster["n"] >= 100
            and w * 0.07 < cluster["w"] < w * 0.16
            and h * 0.01 < cluster["h"] < h * 0.05
            and w * 0.50 < cluster["cx"] < w * 0.68
            and h * 0.92 < cluster["cy"] < h * 0.99
        ):
            return (cluster["cx"], cluster["cy"])
    return None


def advisor_buttons(rgb, grey):
    """Blue tutorial-card buttons, only when a bright card encloses them."""
    w, h = rgb.size
    targets, candidates = [], []
    for cluster in blue_clusters(rgb):
        if not (
            cluster["n"] >= 80
            and w * 0.33 < cluster["cx"] < w * 0.67
            # Advisor cards have more than one vertical layout.  The new-city
            # loyalty card observed live puts its action row at 43% of the
            # window height; treating 40% as a universal boundary left that
            # real blocker classified as ordinary map.  The paper probes below
            # remain the safety boundary, so this widens only the legitimate
            # card band rather than accepting arbitrary lower HUD controls.
            and h * 0.10 < cluster["cy"] < h * 0.46
            and w * 0.05 < cluster["w"] < w * 0.22
            and h * 0.01 < cluster["h"] < h * 0.08
        ):
            continue
        candidates.append((cluster["cx"], cluster["cy"]))
        # The panel directly above the button is light paper. A long blue
        # control over desert, sea, or a city banner is not a tutorial card.
        left = max(0, int(cluster["cx"] - w * 0.10))
        right = min(w, int(cluster["cx"] + w * 0.10))
        top = max(0, int(cluster["cy"] - h * 0.18))
        bottom = max(top + 1, int(cluster["cy"] - h * 0.02))
        paper = grey.crop((left, top, right, bottom))
        bright = sum(1 for value in pixel_values(paper) if value > 180)
        # A nearby bright panel can leak into a wide crop, as the World Tracker
        # did beside the live World Congress advisor card. Sample immediately
        # around the control and require enough paper that it is inside the card.
        # The live Tribal Village card measured 34.85% in this deliberately
        # tight probe: parchment decoration and the advisor portrait occupy the
        # rest.  Keep a small margin below that observed card rather than
        # treating a valid acknowledgement as a map control.
        if bright < 0.34 * (paper.size[0] * paper.size[1]):
            continue
        # A wide probe can overlap a nearby card even when this control is a
        # map/HUD button just outside it. The card must also be bright directly
        # above the button itself; this keeps the measured Tribal Village card
        # while rejecting that adjacent-control false positive.
        core_left = max(0, int(cluster["cx"] - w * 0.025))
        core_right = min(w, int(cluster["cx"] + w * 0.025))
        core = grey.crop((core_left, top, core_right, bottom))
        core_bright = sum(1 for value in pixel_values(core) if value > 180)
        if core_bright < 0.30 * (core.size[0] * core.size[1]):
            continue
        targets.append((cluster["cx"], cluster["cy"]))
    # An advisor portrait or decoration can cover the paper immediately above
    # the left Continue/OK action while its right Tell me more neighbour still
    # passes the paper check.  A same-row, same-sized-looking blue control to
    # the LEFT of a confirmed card action at the standard paired-button spacing
    # is the one safe exception: recover it so the watchdog can acknowledge
    # the card without ever treating the right help route as a fallback.
    for point in candidates:
        if point in targets:
            continue
        if any(
            point[0] < confirmed[0]
            and w * 0.05 < confirmed[0] - point[0] < w * 0.15
            and abs(point[1] - confirmed[1]) < h * 0.03
            for confirmed in targets
        ):
            targets.append(point)
    # Advisor actions share a row. Centroid anti-aliasing can make the right
    # button a fraction of a pixel higher than the left one, so y-first sorting
    # selected "Tell me more" during a live run. The left action is the benign
    # acknowledge/continue choice on every measured advisor card.
    return sorted(targets, key=lambda point: (point[0], point[1]))


def near(a, b, tol=8):
    """The same point seen through animation wobble.

    A leader portrait breathes: the detected target drifts a pixel or two
    between passes ((1197,387) -> (1196,387) on run civvis-20260815T020727Z),
    which must not read as a NEW target — that reset is how one conversation
    ate clicks for 900 s until the outer watchdog killed the attempt.
    """
    return abs(a[0] - b[0]) <= tol and abs(a[1] - b[1]) <= tol


def same_scene(kind, choice, last_target, tol=8):
    """Whether this pass is still looking at the scene it clicked last pass."""
    return (last_target is not None and kind == last_target[0]
            and near(tuple(int(v) for v in choice), last_target[1], tol))


def click_target(kind, targets, width):
    """Return the only target the external watchdog may actuate.

    Advisor cards often pair Continue on the left with Tell me more on the
    right.  The latter opens Civilopedia and leaves the turn blocked, so a
    right-side action is never an acceptable fallback.  A card layout we do
    not recognize well enough to find a left acknowledgement is left for the
    in-game closer or an operator rather than guessed at.
    """
    if not targets:
        return None
    if kind == "advisor":
        return next((point for point in targets if point[0] < width * 0.50), None)
    if kind in ("governor", "congress"):
        return targets[0]
    return targets[-1]


def governor_close(rgb):
    """The close control on the full-width Governors title panel, if present."""
    w, h = rgb.size
    px = rgb.load()
    for cluster in red_clusters(rgb):
        if not (
            cluster["n"] >= 10
            and cluster["cx"] > w * 0.96
            and h * 0.18 < cluster["cy"] < h * 0.32
            and cluster["w"] <= 36
            and cluster["h"] <= 36
        ):
            continue
        # Governors has a distinctive blue title bar almost the full width of
        # the window. A red pin near the right map edge must not become a close
        # target merely because it shares the rough location.
        y = min(h - 1, max(0, int(cluster["cy"])))
        blue = sum(
            1 for x in range(0, w, 2)
            if (lambda r, g, b: b > g > r and b > 45)(*px[x, y])
        )
        if blue >= 0.70 * (w // 2):
            return (cluster["cx"], cluster["cy"])
    return None


def dialogue_buttons(gray, box):
    """Dialogue buttons in a leader scene, top to bottom, in image pixels.

    They are light-bordered bars low on the left of an otherwise black scene.
    """
    x0d, y0d, wd, hd = box
    px = gray.load()
    x0, x1 = x0d + int(wd * 0.03), x0d + int(wd * 0.40)
    y0, y1 = y0d + int(hd * 0.65), y0d + int(hd * 0.99)
    bands, cur = [], None
    for y in range(y0, y1, 2):
        # The Gathering Storm Goodbye button measured only 101-139 luminance
        # along most of its thin border; the old >140 probe saw no button at
        # all. Keep the scan inside the leader-only lower-left band and join
        # the top edge, label, and bottom edge into one control below.
        bright = sum(1 for x in range(x0, x1, 2) if px[x, y] > 100)
        if bright > 8:
            cur = [y, y] if cur is None else [cur[0], y]
        elif cur:
            bands.append(cur)
            cur = None
    if cur:
        bands.append(cur)
    merged = []
    for top, bottom in bands:
        if merged and top - merged[-1][1] <= 14:
            merged[-1][1] = bottom
        else:
            merged.append([top, bottom])
    out = []
    for top, bottom in merged:
        cy = (top + bottom) // 2
        xs = [
            x for y in range(top, bottom + 1, 2)
            for x in range(x0, x1, 2) if px[x, y] > 100
        ]
        if xs:
            out.append(((min(xs) + max(xs)) // 2, cy))
    return out


def notice_button(rgb, gray):
    """A centered one-button Firaxis notice, such as ``Unit Captured``.

    These notices sit over the map and therefore are neither dark leader
    scenes nor completion cards. Require both the narrow centered blue button
    and the much wider centered blue frame above it; either shape alone occurs
    in ordinary HUD controls, while the pair is the measured modal geometry.
    """
    w, h = rgb.size
    clusters = blue_clusters(rgb)
    buttons = [
        cluster for cluster in clusters
        if cluster["n"] >= 120
        and w * 0.42 < cluster["cx"] < w * 0.58
        and h * 0.50 < cluster["cy"] < h * 0.62
        and w * 0.05 < cluster["w"] < w * 0.16
        and cluster["h"] < h * 0.04
    ]
    for button in buttons:
        framed = any(
            frame["n"] >= 250
            and abs(frame["cx"] - button["cx"]) < w * 0.03
            and w * 0.18 < frame["w"] < w * 0.35
            and h * 0.05 < button["cy"] - frame["cy"] < h * 0.14
            for frame in clusters
        )
        if not framed:
            continue
        panel = gray.crop((
            int(w * 0.44), int(h * 0.43),
            int(w * 0.56), int(h * 0.56),
        ))
        bright = sum(1 for value in pixel_values(panel) if value > 160)
        if bright >= 0.35 * (panel.size[0] * panel.size[1]):
            return (button["cx"], button["cy"])
    return None


def classify(window):
    """What is on screen: leader, card, advisor, or map plus safe targets.

    `window` is the game window alone, so every coordinate returned is relative
    to it, in image pixels.
    """
    w, h = window.size
    box = (0, 0, w, h)
    grey = window.convert("L")
    histogram = grey.histogram()
    dark = sum(histogram[:DARK_LEVEL]) / max(1, w * h)

    congress = congress_return_button(window, grey)
    if congress:
        return "congress", [congress], dark

    governor = governor_close(window)
    if governor:
        return "governor", [governor], dark

    # A confirmed advisor panel is more specific than a red cluster on the map.
    # Do this before the broad dark leader-scene fallback as well. The World
    # Congress introduction keeps the panel behind its advisor card dark enough
    # to resemble a leader scene, but its centered blue Continue control and
    # bright paper panel are the stronger, safely actionable signal.
    #
    # Do this before looking for a completion-card close button: barbarians and
    # danger pins can otherwise make an advisor appear to be an unsafe card.
    advisor = advisor_buttons(window, grey)
    if advisor:
        return "advisor", advisor, dark

    notice = notice_button(window, grey)
    if notice:
        return "notice", [notice], dark

    if dark > LEADER_DARK_FRACTION:
        return "leader", dialogue_buttons(grey, box), dark

    # A card popup leaves the map showing, so it is found by its close button.
    #
    # ⚠ TAKE THE TOPMOST CANDIDATE, NOT THE BIGGEST. The card's own content is
    # full of red: a policy icon in the "unlocked by this" strip is a larger red
    # blob than the close button, and picking by size clicked the middle of the
    # card three times in a row without closing anything. The close button is at
    # the card's top-right, always above its content.
    candidates = [c for c in red_clusters(window)
                  # macOS's own window button lives in the window's top-left corner.
                  if not (c["cx"] < w * 0.08 and c["cy"] < h * 0.08)
                  and 8 <= c["w"] <= 40 and 8 <= c["h"] <= 40 and c["n"] >= 18
                  # ⚠ Keep this band TIGHT. A completion card is drawn in the
                  # middle of the window and its close button sits in the upper
                  # third. A wider band caught a red marker in the top-left HUD
                  # and clicked (1126,183) when the card's button was at
                  # (1357,185) -- a click on the live map, which is the one
                  # mistake this tool must not make.
                  and w * 0.40 < c["cx"] < w * 0.68
                  and h * 0.08 < c["cy"] < h * 0.45]
    if candidates:
        best = min(candidates, key=lambda c: c["cy"])
        return "card", [(best["cx"], best["cy"])], dark
    return "map", [], dark


def held_click(point_px, box_points, scale):
    """A click must be HELD; a zero-length `cliclick c:` does nothing here."""
    px = int(box_points[0] + point_px[0] / scale)
    py = int(box_points[1] + point_px[1] / scale)
    macos_input.move(px, py, check=True)
    time.sleep(POINTER_SETTLE_SECONDS)
    macos_input.click(px, py, hold_s=0.15, check=True)
    return px, py


def covered_poll_delay(interval):
    """Return the maximum retry delay while a dialogue covers the map."""
    return min(DIALOGUE_POLL_SECONDS, max(MIN_POLL_SECONDS, float(interval)))


def main():
    global NATIVE_CAPTURE_DISABLED
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--interval", type=float, default=0.5)
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--dry-run", action="store_true", help="report only, never click")
    ap.add_argument("--runs", default="/Users/martin/civvis-civ6-runs/control",
                    help="run directories; nothing is clicked until one records a turn")
    ap.add_argument("--cards", action="store_true",
                    help="also click completion cards (off: the mod closes those, "
                         "and a false positive here clicks the live map)")
    ap.add_argument("--log",
                    default=str(Path.home() / "civvis-civ6-runs"
                                / "popup_clear.log"),
                    help="activity ledger; the keeper passes this "
                         "explicitly. ⚠ The default named one "
                         "operator's home directory, so on every "
                         "other host a hand-run clearer logged into "
                         "a path that did not exist")
    args = ap.parse_args()

    # Compile the cached helper before the first frame. On a fresh host this is
    # startup work while the game is loading, not latency charged to a dialogue.
    # Hosts that cannot build it pause safely; they never fall back to a tool
    # that can ask a permission question over the game.
    NATIVE_CAPTURE_DISABLED = not macos_capture.prepare()

    def log(message):
        line = f"[popup] {time.strftime('%FT%TZ', time.gmtime())} {message}"
        print(line, flush=True)
        try:
            with open(args.log, "a") as handle:
                handle.write(line + "\n")
        except OSError:
            pass

    cleared = 0
    last_target, misses = None, 0
    esc_budget_used = 0
    advisor_stuck_passes = 0
    leader_passes = 0
    no_target_passes = 0
    waiting_since = None
    warned_setup = False
    warned_cards = False
    last_pause_reason = None
    while True:
        # Do this before even probing capture or window geometry.  A pre-turn
        # Create Game screen is setup, not a popup, so it must have exclusive
        # access to System Events while the recorder's UI is present.
        if newest_run_is_pre_turn(args.runs):
            if not warned_setup:
                log("newest run has no turn yet; yielding popup checks during setup")
                warned_setup = True
            if args.once:
                return
            time.sleep(max(SETUP_YIELD_SECONDS, args.interval))
            continue
        if warned_setup:
            log("newest run recorded a turn; resuming popup checks")
            warned_setup = False

        # CoreGraphics capture remains non-interactive under an authorized
        # native recording.  A denied grant or a spinning display service is
        # still paused before even grabbing the Civ window.
        pause_reason = capture_pause_reason()
        if pause_reason:
            if pause_reason != last_pause_reason:
                log(f"pausing pixel capture: {pause_reason}")
            last_pause_reason = pause_reason
            if args.once:
                return
            time.sleep(SYSTEMSTATUSD_RECOVERY_PAUSE_SECONDS)
            continue
        if last_pause_reason is not None:
            log(f"resuming pixel capture after: {last_pause_reason}")
            last_pause_reason = None

        idle = False
        covered = False
        try:
            box = window_box()
            if not box:
                # Say it once. This runs for the length of a ladder, and a line
                # every six seconds between games buries the lines that matter.
                if waiting_since is None:
                    waiting_since = time.time()
                    log("no Civilization VI window; waiting for a game")
                idle = True
            else:
                if waiting_since is not None:
                    log(f"game window back after {time.time() - waiting_since:.0f}s")
                    waiting_since = None
                    warned_setup = False
                window, scale = capture(box)
                kind, targets, dark = classify(window)
                front = frontmost()
                # An advisor card can be the reason the log stopped advancing.
                # Give that one strongly classified surface a longer grace period;
                # all broader dark/card recognition remains tied to a fresh turn so
                # setup controls can never be clicked because of an old run.
                freshness = 3600.0 if kind in ("advisor", "congress") else 180.0
                playing = game_in_progress(args.runs, fresh_seconds=freshness)
                covered = kind != "map"
                # ★ ESCALATE ON PERSISTENCE, NOT ON ANY ONE BRANCH'S LATCH.
                # Three measured leader stalls took three different branches:
                # the animated scene reset the exact-match no-op guard (#1595),
                # the target-less scene never clicked at all (#1631), and on
                # 2026-08-15T12:3x a QUEUE of scenes alternated targets 60 px
                # apart, so the per-branch once-per-map-return latch spent its
                # single ESC on scene one and left the rest to the watchdog.
                # A leader covering a live map is the one condition all
                # variants share, so escalate early: ESC at about 0.75s, 1.5s
                # and 3s of continuous cover, never more than three per
                # episode. Question scenes ignore ESC by design; the click
                # paths keep running between escalations regardless.
                if kind == "leader" and playing and front.startswith("Civ6"):
                    leader_passes += 1
                    if (leader_passes in (3, 6, 12)
                            and esc_budget_used < 3
                            and not args.dry_run):
                        macos_input.press_key("escape", check=True)
                        esc_budget_used += 1
                        log(f"leader has covered the map for {leader_passes} "
                            f"passes; sent ESC ({esc_budget_used}/3)")
                elif kind != "leader":
                    leader_passes = 0
                if kind == "map":
                    esc_budget_used = 0
                    leader_passes = 0
                    no_target_passes = 0
                    # The map is back, so whatever the card was, it is gone.
                    advisor_stuck_passes = 0
                elif kind == "card" and not args.cards:
                    # ⚠ OFF BY DEFAULT, ON EVIDENCE. With the counter fixed, the
                    # mod closes completion cards from Lua and says so:
                    # TechCivicCompletedPopup reported `gone: true` on 9 of 9
                    # closes on run civvis-20260731T161131Z. The only screen that
                    # still resists is the leader conversation (`gone: false` on
                    # 5 of 6). So a "card" seen here is usually a red map marker
                    # that slipped the band -- and clicking it is a click on the
                    # live map, which is the one mistake this must not make.
                    if not warned_cards:
                        warned_cards = True
                        log("a card matched, but the mod closes those now; "
                            "not clicking it (pass --cards to override)")
                elif not playing:
                    if not warned_setup:
                        warned_setup = True
                        log(f"{kind} on screen but no turn recorded yet; "
                            "this is setup, not a popup -- not clicking")
                elif not targets:
                    log(f"{kind} on screen (dark={dark:.2f}) but no target found; leaving it alone")
                    if kind == "leader":
                        no_target_passes += 1
                    else:
                        no_target_passes = 0
                elif (choice := click_target(kind, targets, window.size[0])) is None:
                    # ⚠⚠⚠ "LEAVING IT ALONE" MEANT LEAVING THE GAME DEAD.
                    #
                    # Refusing the click is right: on an advisor card the
                    # right-hand action is "Tell me more", which opens
                    # Civilopedia and leaves the turn blocked. But the comment on
                    # `click_target` says such a card is left "for the in-game
                    # closer or an operator", and on an unattended ladder there
                    # is no operator — so nothing came.
                    #
                    # Measured 2026-08-28, run civvis-20260828T173743Z, the best
                    # King game of the day at turn 100 with six cities:
                    #   18:13:05 advisor has no safe left-side acknowledgement
                    #   18:13:06 clicked advisor at (1234, 96)
                    #   18:13:07 advisor has no safe left-side acknowledgement
                    # and then nothing until the wedge watchdog killed it at
                    # 18:18. Its sample shows the Game Core thread parked in
                    # `__psynch_cvwait` for 1433 of 1433 frames: the engine was
                    # waiting for a response to that card.
                    #
                    # ESC is the safe answer, and this file already trusts it —
                    # the leader branch below spends the same three-per-episode
                    # budget. ESC dismisses rather than activates, so unlike a
                    # right-side click it cannot open Civilopedia; a scene that
                    # ignores ESC by design is simply unchanged, which is where
                    # we already are. Escalate on the same 3/6/12 ladder so a
                    # card that clears itself never sees one.
                    advisor_stuck_passes += 1
                    if (advisor_stuck_passes in (3, 6, 12)
                            and esc_budget_used < 3
                            and playing
                            and front.startswith("Civ6")
                            and not args.dry_run):
                        macos_input.press_key("escape", check=True)
                        esc_budget_used += 1
                        log(f"advisor has had no safe acknowledgement for "
                            f"{advisor_stuck_passes} passes; sent ESC "
                            f"({esc_budget_used}/3)")
                    else:
                        log("advisor has no safe left-side acknowledgement; "
                            f"leaving it alone (pass {advisor_stuck_passes})")
                elif not front.startswith("Civ6"):
                    log(f"{kind} on screen but {front!r} is frontmost; not clicking")
                elif args.dry_run:
                    log(f"DRY RUN: would click {kind} at {choice} (dark={dark:.2f})")
                elif (kind != "leader"
                      and same_scene(kind, choice, last_target) and misses >= 2):
                    # ⚠ Never keep clicking something that demonstrably does
                    # nothing. A repeated no-op is either a target we have
                    # misread or a screen we cannot drive, and both are safer
                    # left alone than hammered on a live map.
                    if misses == 2:
                        log(f"{kind} at {targets[-1]} did not respond twice; leaving it alone")
                    misses += 1
                else:
                    # Bottom-most button: 'Goodbye' on a farewell, and on a
                    # question the answer list ends with the exit -- clicking it
                    # repeatedly walks the conversation to its end.
                    target = (kind, tuple(int(v) for v in choice))
                    where = held_click(choice, box, scale)
                    time.sleep(POST_CLICK_SETTLE_SECONDS)
                    after, _ = capture(box)
                    kind_after, targets_after, _ = classify(after)
                    # ⚠ SAY WHAT HAPPENED, NOT WHAT WAS INTENDED. This used to
                    # call anything "cleared" whose target had merely MOVED, so
                    # a card popup replaced by another card popup was logged as
                    # a success. That is the same lie as the mod's `ended`, in
                    # the tool written to expose it.
                    next_choice = click_target(kind_after, targets_after, after.size[0])
                    identical = (kind_after == kind and next_choice is not None
                                 and near(tuple(int(v) for v in next_choice), target[1]))
                    misses = misses + 1 if (identical and same_scene(*target, last_target)) else 0
                    last_target = target
                    if kind_after == "map":
                        cleared += 1
                        log(f"cleared {kind} with a held click at {where} "
                            f"(dark={dark:.2f}, map is back, total {cleared})")
                    elif not identical:
                        # A queue: one went, another took its place. Progress,
                        # but the map is still covered, so do not claim a clear.
                        log(f"clicked {kind} at {where}; a {kind_after} is up now")
                    else:
                        log(f"clicked {kind} at {where} but it is still there "
                            f"(miss {misses})")
        except Exception as error:
            log(f"error: {error}")
        if args.once:
            return
        # ★★★★ POLL SLOWLY WHEN THERE IS NOTHING TO DO, FAST WHILE A SCREEN IS UP.
        #
        # A leader conversation is not one click. Measured 2026-08-02 on run
        # civvis-20260802T014139Z: a single Barbarossa scene logged `clicked leader;
        # a leader is up now` then `a card is up now` before the map came back, and
        # twice it read `leader on screen but no target found` while the dialogue
        # buttons were still fading in. The covered path now retries every 250ms,
        # so a target that appears during the animation is picked up on the next
        # frame rather than waiting for the between-game interval.
        #
        # Once `classify` says the map is covered there is no cheapness argument
        # left: we already know the screen needs clearing, and the only question is
        # how soon we look again. A pass costs ~0.5s in-loop, so chasing at 250ms
        # while covered is only charged while it matters.
        #
        # ⚠ The chase does NOT weaken any guard. Frontmost, map-positively-covered,
        # turn-recorded and the two-strike no-op rule are all still checked on every
        # pass; this changes when we look, never what we are willing to click.
        if idle:
            delay = max(MIN_POLL_SECONDS, args.interval * 5)  # between games
        elif covered:
            delay = covered_poll_delay(args.interval)
        else:
            delay = max(MIN_POLL_SECONDS, args.interval)
        time.sleep(delay)


if __name__ == "__main__":
    main()
