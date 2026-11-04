#!/usr/bin/env python3
"""Where a live turn's wall-clock went, from the `utc` receipt stamps in events.jsonl.

    python3 tools/live_turn_clock.py RUN_DIR [--slowest 8] [--json]

Per turn: `dur` is board-to-board (this turn's opening `state` to the next
turn's), `wait` is opening `state` to its frame-0 `orders` (the mod's polls plus
the brain's answer), `polls` is the mod's own count from the `turn` record,
`frames` counts replan/combat frames, `popups` counts autoclose traffic and
`stuck` the popups that needed desktop help. Events without a stamp (runs from
before `civ6_play.record` stamped them) are skipped, so an old run prints nothing.

The decision summary also pairs every replanning frame's state/orders receipts.
These gaps include transport and native execution, not just brain computation;
overlapping gaps count once in the total. Unanswered frames are counted separately.
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


def rescue_stalls(events_path: Path) -> dict:
    """Sum gaps from recovery requests to the next journal event.

    This is an observation interval, not measured recovery execution time.
    Requests can be deferred while another app is foreground, and native
    event cadence also contributes to the next-event gap. Use these gaps to
    locate traces for investigation, not to attribute CPU or wall time to
    the desktop recovery implementation.
    """
    stamped = []
    with events_path.open("r", errors="replace") as handle:
        for raw in handle:
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            at = parse_stamp(event.get("utc"))
            if at is not None:
                stamped.append((at, event.get("kind"), event.get("screen")))
    stamped.sort(key=lambda row: row[0])
    if len(stamped) < 2:
        return {"metric": "request_to_next_event_gap", "asks": 0, "seconds": 0.0, "span": 0.0, "worst": 0.0, "screens": {}}
    span = (stamped[-1][0] - stamped[0][0]).total_seconds()
    asks = 0
    seconds = 0.0
    worst = 0.0
    screens: dict[str, float] = {}
    for (at, kind, screen), (nxt, _, _) in zip(stamped, stamped[1:]):
        if kind not in ("autoclose_desktop", "autoclose_stuck"):
            continue
        gap = (nxt - at).total_seconds()
        if gap < 0 or gap > 600:
            continue
        asks += 1
        seconds += gap
        worst = max(worst, gap)
        screens[str(screen)] = screens.get(str(screen), 0.0) + gap
    return {"metric": "request_to_next_event_gap", "asks": asks, "seconds": seconds, "span": span, "worst": worst,
            "screens": screens}


def decision_waits(events_path: Path) -> dict:
    """Pair state/orders receipts for every frame, including combat replans.

    This includes transport, planning and native order execution before its
    receipt; it is not a measurement of brain CPU time. Unanswered states
    remain visible, including the last frame of a running or interrupted game.
    """
    pending = {}
    completed = set()
    samples = []
    intervals = []
    with events_path.open("r", errors="replace") as handle:
        for raw in handle:
            try:
                event = json.loads(raw)
            except ValueError:
                continue
            if not isinstance(event, dict):
                continue
            kind = event.get("kind")
            turn = event.get("turn")
            frame = event.get("frame", 0)
            at = parse_stamp(event.get("utc"))
            if (kind not in ("state", "orders") or at is None
                    or type(turn) is not int or type(frame) is not int):
                continue
            key = (turn, frame)
            if key in completed:
                continue
            if kind == "state":
                pending.setdefault(key, at)
            elif key in pending and at >= pending[key]:
                start = pending.pop(key)
                samples.append({"turn": turn, "frame": frame,
                                "seconds": round((at - start).total_seconds(), 3)})
                intervals.append((start, at))
                completed.add(key)
    # A relay can interleave frames; merge intervals so the total never
    # double-counts their overlapping wall time.
    seconds = 0.0
    end = None
    for start, finish in sorted(intervals):
        uncovered = max(start, end) if end is not None else start
        seconds += max(0.0, (finish - uncovered).total_seconds())
        end = max(end, finish) if end is not None else finish
    return {"metric": "state_to_orders_receipt_gap", "seconds": round(seconds, 3),
            "matched": len(samples), "unanswered": len(pending), "samples": samples}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", type=Path, help="run directory or its events.jsonl")
    parser.add_argument("--slowest", type=int, default=8)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    path = args.run / "events.jsonl" if args.run.is_dir() else args.run
    rows = turn_clock(path)
    decisions = decision_waits(path)
    if args.json:
        print(json.dumps({"turns": rows, "rescue": rescue_stalls(path), "decisions": decisions}))
        return 0
    timed = [r for r in rows if "dur" in r]
    if not timed:
        print("no stamped turns (run predates receipt stamps?)")
        return 1
    durations = [r["dur"] for r in timed]
    print(f"turns {len(timed)}  median {statistics.median(durations):.1f}s  "
          f"total {sum(durations) / 60:.1f} min  "
          f"waits>=10s {sum(1 for r in timed if r.get('wait', 0) >= 10)}")
    rescue = rescue_stalls(path)
    if rescue["asks"]:
        share = 100 * rescue["seconds"] / rescue["span"] if rescue["span"] else 0.0
        busiest = max(rescue["screens"].items(), key=lambda item: item[1])
        print(f"recovery requests: {rescue['asks']} next-event gaps total "
              f"{rescue['seconds'] / 60:.2f} min = {share:.1f}% of the run "
              f"(longest {rescue['worst']:.1f}s, most on {busiest[0]}); "
              "not measured recovery time")
    else:
        print("recovery requests: no asks")
    print(f"all decision frames: {decisions['matched']} matched, "
          f"{decisions['unanswered']} unanswered; state-to-orders receipt gaps "
          f"{decisions['seconds'] / 60:.2f} min (overlaps counted once; not brain CPU time)")
    print("slowest decision receipts (turn/frame: seconds): " + ", ".join(
        f"{r['turn']}/{r['frame']}: {r['seconds']:.3f}"
        for r in sorted(decisions["samples"], key=lambda r: -r["seconds"])[:args.slowest]))
    print("turn  board     dur   wait polls frames popups stuck")
    for r in sorted(timed, key=lambda r: -r["dur"])[:args.slowest]:
        print(f"{r['turn']:>4}  {r['board_at']}  {r['dur']:>5}  {r.get('wait', '-'):>5} "
              f"{str(r['polls'] or '-'):>5} {r['frames']:>6} {r['popups']:>6} {r['stuck']:>5}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
