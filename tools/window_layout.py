"""The operator's saved window layout: one file, read by every window placer.

Operator 2026-10-14: "set the current window positions and sizings as the
default for the 4 quadrants". `python3 tools/computer_control.py layout --save`
captures the arrangement on screen into ``~/.civvis-window-layout.json``; then

* ``civ6_play`` puts the game window in its saved frame at every launch,
* ``computer_control.py layout`` places all saved quadrants (the lower-right
  one included, since the operator chose it), and
* the CIVVIS display keeper opens the mirror window at its saved frame,

instead of the computed half-screen quadrants. Frames are ``[x, y, w, h]`` in
points with y growing down from the top of the menu-bar screen, the units
System Events and Chrome's ``--window-position``/``--window-size`` use. The file
is honoured only on a screen of the size it was saved on, so a different
display falls back to the computed quadrants rather than placing windows
off-screen. A missing or malformed file is the same as no file.
"""
from __future__ import annotations

import json
import os
from datetime import datetime, timezone
from pathlib import Path

LAYOUT_FILE = Path(os.environ.get("CIVVIS_WINDOW_LAYOUT",
                                  Path.home() / ".civvis-window-layout.json"))
QUADRANTS = ("upper-left", "upper-right", "lower-left", "lower-right")


def load(screen: "tuple[int, int] | None" = None, path: "Path | None" = None) -> dict:
    """{quadrant: spec with an int (x, y, w, h) `frame`}; {} when unusable."""
    path = LAYOUT_FILE if path is None else path
    try:
        data = json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return {}
    if not isinstance(data, dict):
        return {}
    saved = data.get("screen")
    if screen is not None and saved is not None and list(saved) != list(screen):
        return {}
    out = {}
    for quadrant, spec in (data.get("windows") or {}).items():
        if quadrant not in QUADRANTS or not isinstance(spec, dict):
            continue
        frame = spec.get("frame")
        if (isinstance(frame, list) and len(frame) == 4
                and all(isinstance(v, (int, float)) for v in frame)
                and frame[2] > 0 and frame[3] > 0):
            out[quadrant] = dict(spec, frame=tuple(int(v) for v in frame))
    return out


def frame_for(quadrant: str, screen: "tuple[int, int] | None" = None,
              path: "Path | None" = None) -> "tuple[int, int, int, int] | None":
    spec = load(screen, path).get(quadrant)
    return spec["frame"] if spec else None


def save(windows: dict, screen: "tuple[int, int]", path: "Path | None" = None) -> dict:
    """Write `windows` ({quadrant: spec with `frame`}) atomically; returns the data."""
    path = LAYOUT_FILE if path is None else path
    data = {
        "saved_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "screen": [int(screen[0]), int(screen[1])],
        "windows": {q: dict(spec, frame=[int(v) for v in spec["frame"]])
                    for q, spec in windows.items() if q in QUADRANTS},
    }
    path = Path(path)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(data, indent=2) + "\n")
    os.replace(tmp, path)
    return data


def quadrant_of(frame: "tuple[int, int, int, int]", screen: "tuple[int, int]") -> str:
    """The quadrant holding a window's centre."""
    x, y, w, h = frame
    cx, cy = x + w / 2, y + h / 2
    upper = cy < screen[1] / 2
    left = cx < screen[0] / 2
    return ("upper-" if upper else "lower-") + ("left" if left else "right")
