#!/usr/bin/env python3
"""Find the main menu's submenu rows by looking at the screen.

The Single Player submenu does not have a fixed number of entries: "Resume
Game" appears only once there is a game to resume and "Load Game" only once a
save exists, so the list is three, four or five rows deep depending on what has
happened before. Every entry therefore moves, and a click aimed at "Create
Game" by a measured fraction lands on "Scenarios" in one run and "Load Game" in
the next -- which starts the wrong game and reports it as the right one.

Counting the rows removes the guess. The entries are light text on a dark
panel, so a horizontal band containing an entry is measurably brighter than the
gaps between them; the bands are the rows, in order, and "Create Game" is
always the third from the bottom.

This is the only part of the harness that reads pixels, and it reads them to
locate a control, never to read a number out of the game -- game state comes
from the game's own log.
"""

from __future__ import annotations

from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - reported by the caller
    Image = None

# The submenu column, as fractions of the game window: left, top, right, bottom.
#
# Narrow on purpose. A wider crop takes in the artwork behind the menu, whose
# brightness swamps the text; and the top edge starts below the parent entry,
# because the highlighted "Single Player" row and its tooltip are brighter than
# any submenu entry and would be read as a row.
COLUMN = (0.50, 0.49, 0.60, 0.665)

# A menu row is 58 physical pixels apart in the 949-point live window, or about
# 0.0306 of its height. The range deliberately leaves room for UI scale and
# rounding, while rejecting the title-card glyph bands seen during transitions.
MENU_ROW_STEP = 0.0306
MENU_ROW_TOLERANCE = 0.006


def available() -> bool:
    return Image is not None


def rows_in(shot: Path, bounds: tuple[int, int, int, int], column,
            scale: float = 2.0, min_gap: int = 6) -> list[float]:
    """Row centres of the open submenu, as fractions of the window's height.

    ``bounds`` is the window in points; ``scale`` converts points to the
    screenshot's pixels (2.0 on a Retina display). Returns an empty list when
    nothing that looks like a row list is found, which the caller should treat
    as "could not tell" rather than "no rows".
    """
    if Image is None or not shot.is_file():
        return []
    x, y, w, h = bounds
    with Image.open(shot) as image:
        grey = image.convert("L")
        left = int((x + w * column[0]) * scale)
        top = int((y + h * column[1]) * scale)
        right = int((x + w * column[2]) * scale)
        bottom = int((y + h * column[3]) * scale)
        crop = grey.crop((left, top, right, bottom))
        width, height = crop.size
        if width <= 0 or height <= 0:
            return []
        pixels = crop.load()
        profile = [sum(pixels[cx, cy] for cx in range(width)) / width
                   for cy in range(height)]

    if not profile:
        return []
    # The threshold is measured against the *median* line, not the darkest one.
    # Entry text occupies a small share of the crop, so the median is the panel
    # background; using the minimum instead puts the threshold below the
    # background and reads the whole panel as one enormous row.
    ordered = sorted(profile)
    background = ordered[len(ordered) // 2]
    hi = ordered[-1]
    if hi - background < 8:  # a flat crop: no submenu is showing
        return []
    threshold = background + (hi - background) * 0.30
    # The submenu frame can add a two-pixel horizontal rule to this brightness
    # profile. It is not text and treating it as a row shifts Create Game down to
    # Scenarios. Menu glyphs occupy at least three physical pixels even at 1x,
    # while this keeps real low-resolution text eligible.
    min_band = 3
    rows: list[tuple[int, int]] = []
    start = None
    for index, value in enumerate(profile):
        if value >= threshold and start is None:
            start = index
        elif value < threshold and start is not None:
            if index - start >= min_band:
                rows.append((start, index))
            start = None
    if start is not None and height - start >= min_band:
        rows.append((start, height))
    if not rows:
        return []

    # Merge bands that are closer together than the gap between real entries:
    # a single entry can show as two bands when its underline is separated from
    # its text.
    merged: list[tuple[int, int]] = [rows[0]]
    for begin, end in rows[1:]:
        if begin - merged[-1][1] < min_gap:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((begin, end))

    centres = []
    for begin, end in merged:
        centre_px = top + (begin + end) / 2.0
        centres.append((centre_px / scale - y) / h)
    return centres


def submenu_rows(shot: Path, bounds: tuple[int, int, int, int],
                 scale: float = 2.0) -> list[float]:
    return _regular_menu_rows(rows_in(shot, bounds, COLUMN, scale))


def _regular_menu_rows(rows: list[float]) -> list[float]:
    """Keep the longest sequence that has Civ VI submenu row spacing.

    A dark loading card has bright letter strokes in this crop. They once
    produced seven arbitrary bands, which made the caller click a non-menu
    coordinate. A real submenu has at least Create Game, Scenarios and Play Now
    on a fixed vertical grid; without that evidence, return no rows.
    """
    best: list[float] = []
    for start, row in enumerate(rows):
        sequence = [row]
        previous = row
        for candidate in rows[start + 1:]:
            gap = candidate - previous
            if not MENU_ROW_STEP - MENU_ROW_TOLERANCE <= gap <= MENU_ROW_STEP + MENU_ROW_TOLERANCE:
                break
            sequence.append(candidate)
            previous = candidate
        if len(sequence) > len(best):
            best = sequence
    return best if len(best) >= 3 else []


def create_game_row(shot: Path, bounds: tuple[int, int, int, int],
                    scale: float = 2.0) -> float | None:
    """Where "Create Game" is, as a fraction of window height, or None.

    The Single Player submenu always ends with Create Game, Scenarios, Play
    Now, whatever precedes them, so the third row from the bottom is the one --
    counted from the end precisely because the *start* of the list is what
    varies.
    """
    rows = submenu_rows(shot, bounds, scale)
    if len(rows) < 3:
        return None
    return rows[-3]


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("shot", type=Path)
    ap.add_argument("--window", default="48,33,1632,1084",
                    help="game window as x,y,w,h in points")
    ap.add_argument("--scale", type=float, default=2.0)
    args = ap.parse_args()

    bounds = tuple(int(v) for v in args.window.split(","))  # type: ignore[assignment]
    rows = submenu_rows(args.shot, bounds, args.scale)  # type: ignore[arg-type]
    print(f"{len(rows)} rows: " + ", ".join(f"{r:.4f}" for r in rows))
    pick = create_game_row(args.shot, bounds, args.scale)  # type: ignore[arg-type]
    print(f"create game -> {pick}")
