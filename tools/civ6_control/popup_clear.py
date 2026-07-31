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
"""

import argparse
import json
import subprocess
import sys
import time
from collections import deque

try:
    from PIL import Image
except ImportError:
    sys.exit("popup_clear needs Pillow: python3 -m pip install pillow")

GAME_PROCESS = "Civ6_Exe_Child"
DARK_LEVEL = 24          # luminance below this counts as "black"
LEADER_DARK_FRACTION = 0.35   # measured 0.55 leader vs 0.11 map
SHOT = "/tmp/civ6-popup-clear.png"


def osa(script):
    done = subprocess.run(["osascript", "-e", script], capture_output=True, text=True, timeout=20)
    return (done.stdout or "").strip(), (done.stderr or "").strip()


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


def capture(box_points):
    """Grab just the game window, so the pixel:point scale calibrates itself.

    Capturing the whole screen would mean knowing the display's backing scale
    factor to turn an image pixel back into a click point. Capturing the region
    we already know in points makes the ratio measurable: `image width / w`.
    """
    x, y, w, h = box_points
    subprocess.run(["screencapture", "-x", "-t", "png", f"-R{x},{y},{w},{h}", SHOT],
                   check=True, timeout=30)
    image = Image.open(SHOT).convert("RGB")
    return image, image.size[0] / float(w)


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
        bright = sum(1 for x in range(x0, x1, 2) if px[x, y] > 140)
        if bright > 12:
            cur = [y, y] if cur is None else [cur[0], y]
        elif cur:
            bands.append(cur)
            cur = None
    if cur:
        bands.append(cur)
    out = []
    for top, bottom in bands:
        cy = (top + bottom) // 2
        xs = [x for x in range(x0, x1, 2) if px[x, cy] > 140]
        if xs:
            out.append(((min(xs) + max(xs)) // 2, cy))
    return out


def classify(window):
    """What is on screen: ('leader', targets) | ('card', targets) | ('map', []).

    `window` is the game window alone, so every coordinate returned is relative
    to it, in image pixels.
    """
    w, h = window.size
    box = (0, 0, w, h)
    grey = window.convert("L")
    histogram = grey.histogram()
    dark = sum(histogram[:DARK_LEVEL]) / max(1, w * h)

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
    subprocess.run(["cliclick", f"m:{px},{py}", "w:200",
                    f"dd:{px},{py}", "w:150", f"du:{px},{py}"], check=True, timeout=20)
    return px, py


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--interval", type=float, default=6.0)
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--dry-run", action="store_true", help="report only, never click")
    ap.add_argument("--log", default="/Users/martin/civvis-civ6-mirror/popup-clear.log")
    args = ap.parse_args()

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
    waiting_since = None
    while True:
        idle = False
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
                window, scale = capture(box)
                kind, targets, dark = classify(window)
                front = frontmost()
                if kind == "map":
                    pass
                elif not targets:
                    log(f"{kind} on screen (dark={dark:.2f}) but no target found; leaving it alone")
                elif not front.startswith("Civ6"):
                    log(f"{kind} on screen but {front!r} is frontmost; not clicking")
                elif args.dry_run:
                    log(f"DRY RUN: would click {kind} at {targets[-1]} (dark={dark:.2f})")
                elif (kind, tuple(int(v) for v in targets[-1])) == last_target and misses >= 2:
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
                    target = (kind, tuple(int(v) for v in targets[-1]))
                    where = held_click(targets[-1], box, scale)
                    time.sleep(1.5)
                    after, _ = capture(box)
                    kind_after, targets_after, _ = classify(after)
                    same = (kind_after == kind and targets_after
                            and tuple(int(v) for v in targets_after[-1]) == target[1])
                    misses = misses + 1 if (same and target == last_target) else 0
                    last_target = target
                    if not same:
                        cleared += 1
                        log(f"cleared {kind} with a held click at {where} "
                            f"(dark={dark:.2f}, now {kind_after}, total {cleared})")
                    else:
                        log(f"clicked {kind} at {where} but it is still there "
                            f"(miss {misses})")
        except Exception as error:
            log(f"error: {error}")
        if args.once:
            return
        time.sleep(args.interval * 5 if idle else args.interval)


if __name__ == "__main__":
    main()
