#!/usr/bin/env python3
"""Where a live turn's wall-clock went, from the `utc` receipt stamps in events.jsonl.

    python3 tools/live_turn_clock.py RUN_DIR [--slowest 8] [--json]

Per turn: `dur` is board-to-board (this turn's opening `state` to the next
turn's), `wait` is opening `state` to its frame-0 `orders` (the mod's polls plus
the brain's answer), `polls` is the mod's own count from the `turn` record,
`frames` counts replan/combat frames, `popups` counts autoclose traffic and
`stuck` the popups that needed desktop help. Events without a stamp (runs from
before `civ6_play.record` stamped them) are skipped, so an old run prints nothing.
"""
from __future__ import annotations

import argparse
import json
import statistics
import sys
from datetime import datetime, timezone
from pathlib import Path


def parse_stamp(stamp: object) -> datetime | None:
    if not isinstance(stamp, str):
        return None
    for layout in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ"):
        try:
            return datetime.strptime(stamp, layout).replace(tzinfo=timezone.utc)
        except ValueError:
            continue
    return None


def turn_clock(events_path: Path) -> list[dict]:
    turns: dict[int, dict] = {}
    with events_path.open("r", errors="replace") as handle:
        for raw in handle:
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            at = parse_stamp(event.get("utc"))
            turn = event.get("turn")
            if at is None or not isinstance(turn, int):
                continue
            row = turns.setdefault(turn, {"turn": turn, "polls": None, "frames": 0,
                                          "popups": 0, "stuck": 0})
            kind = event.get("kind")
            frame = int(event.get("frame") or 0)
            if kind == "state" and frame == 0:
                row.setdefault("board_at", at)
            elif kind == "orders" and frame == 0:
                row.setdefault("orders_at", at)
            elif kind == "turn":
                row["polls"] = event.get("orders_polls")
            elif kind == "replan_frame":
                row["frames"] += 1
            elif isinstance(kind, str) and kind.startswith("autoclose"):
                row["popups"] += 1
                if kind in ("autoclose_desktop", "autoclose_stuck"):
                    row["stuck"] += 1
    rows = [turns[t] for t in sorted(turns) if "board_at" in turns[t]]
    for this, following in zip(rows, rows[1:]):
        this["dur"] = round((following["board_at"] - this["board_at"]).total_seconds(), 1)
    for row in rows:
        if "orders_at" in row:
            row["wait"] = round((row["orders_at"] - row["board_at"]).total_seconds(), 1)
        row["board_at"] = row["board_at"].strftime("%H:%M:%S")
        row.pop("orders_at", None)
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", type=Path, help="run directory or its events.jsonl")
    parser.add_argument("--slowest", type=int, default=8)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    path = args.run / "events.jsonl" if args.run.is_dir() else args.run
    rows = turn_clock(path)
    if args.json:
        print(json.dumps(rows))
        return 0
    timed = [r for r in rows if "dur" in r]
    if not timed:
        print("no stamped turns (run predates receipt stamps?)")
        return 1
    durations = [r["dur"] for r in timed]
    print(f"turns {len(timed)}  median {statistics.median(durations):.1f}s  "
          f"total {sum(durations) / 60:.1f} min  "
          f"waits>=10s {sum(1 for r in timed if r.get('wait', 0) >= 10)}")
    print("turn  board     dur   wait polls frames popups stuck")
    for r in sorted(timed, key=lambda r: -r["dur"])[:args.slowest]:
        print(f"{r['turn']:>4}  {r['board_at']}  {r['dur']:>5}  {r.get('wait', '-'):>5} "
              f"{str(r['polls'] or '-'):>5} {r['frames']:>6} {r['popups']:>6} {r['stuck']:>5}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
