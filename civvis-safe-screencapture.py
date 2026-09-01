#!/usr/bin/env python3
"""Private-window-picker-free screen capture for CIVVIS automation only.

macOS's `/usr/sbin/screencapture` asks Terminal for a broad Screen Recording
grant.  That consent sheet is a modal above Civ VI, so using it as a diagnostic
can freeze the game.  The project already ships a scoped CoreGraphics region
capture for its popup backstop; this adapter gives the existing screenshot
callers the same pixel source without requesting the command-line picker bypass.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path("/Users/martbot-mbp-m5-max-128")
TOOLS = ROOT / "CIVVIS" / "tools"
sys.path.insert(0, str(TOOLS))


def region_from_args(args: list[str]) -> tuple[int, int, int, int] | None:
    for item in args:
        if not item.startswith("-R"):
            continue
        match = re.fullmatch(r"-R(-?\d+),(-?\d+),(\d+),(\d+)", item)
        if not match:
            return None
        return tuple(int(part) for part in match.groups())
    try:
        import civ6_play

        size = civ6_play.desktop_size()
    except Exception:  # capture must fail closed, never call raw screencapture
        return None
    if size is None:
        return None
    return (0, 0, size[0], size[1])


def main() -> int:
    args = sys.argv[1:]
    if not args:
        return 64
    output = Path(args[-1])
    region = region_from_args(args[:-1])
    if region is None:
        return 1
    try:
        from civ6_control import macos_capture

        macos_capture.capture_region(region, output)
    except Exception:  # explicitly no compatibility fallback to raw capture
        return 1
    return 0 if output.is_file() and output.stat().st_size else 1


if __name__ == "__main__":
    raise SystemExit(main())
