"""Report whether CIVVIS is actually driving a Civilization VI run, and how it is doing.

    python3 tools/civ6_civvis_status.py                 # newest run
    python3 tools/civ6_civvis_status.py --run <tag>

⚠ THE POINT IS THE DENOMINATOR. "CIVVIS is deciding" reads identical in a log
whether every order landed or every one was refused, so this prints the fractions
that can distinguish them:

  source        civvis / civvis_stale / fallback per turn. A run that is mostly
                `fallback` is a measurement of the built-in heuristics, not CIVVIS.
  applied       orders the engine accepted, over orders CIVVIS issued. `pcall`
                succeeding is not acceptance; the mod asks `CanStartOperation` first.
  residual      built-in passes that still ran on a turn credited to CIVVIS. Any
                non-zero entry is a decision CIVVIS did not make.
  refusals      why orders were rejected, by reason. One dominant reason is a bug,
                not noise — `no_params` on every produce order was a wrong lookup.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"


def newest_run() -> Path | None:
    runs = [p for p in RUN_ROOT.iterdir() if (p / "events.jsonl").exists()]
    if not runs:
        return None
    return max(runs, key=lambda p: (p / "events.jsonl").stat().st_mtime)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=None)
    ap.add_argument("--tail", type=int, default=6, help="turns to print in detail")
    args = ap.parse_args()

    run = (RUN_ROOT / args.run) if args.run else newest_run()
    if run is None or not (run / "events.jsonl").exists():
        print("no run with events found")
        return 2

    turns, orders, wars, stale, errors = [], [], [], [], []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        kind = event.get("kind")
        if kind == "turn":
            turns.append(event)
        elif kind == "orders":
            orders.append(event)
        elif kind == "war":
            wars.append(event)
        elif kind == "orders_stale":
            stale.append(event)
        elif kind == "error":
            errors.append(event)

    print(f"run {run.name}")
    if not turns:
        print("  no turn records yet")
        return 0

    last = turns[-1]
    print(f"  turns {turns[0].get('turn')}..{last.get('turn')}  "
          f"score={last.get('score')} rival_best={last.get('rival_best')} "
          f"lead={last.get('lead')}")
    print(f"  cities={last.get('cities')} units={last.get('units')} "
          f"army={last.get('army')} met={last.get('met')} gold={last.get('gold')}")

    sources = Counter(t.get("orders_source") for t in turns)
    total = sum(sources.values()) or 1
    parts = [f"{k}={v} ({100 * v // total}%)" for k, v in sources.most_common()]
    print(f"  source: {'  '.join(parts)}")

    seen = sum(o.get("seen", 0) for o in orders)
    applied = sum(o.get("applied", 0) for o in orders)
    refused = sum(o.get("refused", 0) for o in orders)
    pct = (100 * applied // seen) if seen else 0
    print(f"  orders: applied {applied}/{seen} ({pct}%)  refused {refused}")

    by = Counter()
    for order in orders:
        for key, value in (order.get("by") or {}).items():
            by[key] += value
    if by:
        print(f"  by kind: {dict(by.most_common())}")

    refusals = Counter()
    for order in orders:
        reasons = order.get("refusals")
        if isinstance(reasons, dict):
            for key, value in reasons.items():
                refusals[key] += value
    if refusals:
        print(f"  refusals: {dict(refusals.most_common(6))}")

    residual = Counter()
    for turn in turns:
        entries = turn.get("residual")
        if isinstance(entries, dict):
            for key, value in entries.items():
                residual[key] += value
    print(f"  residual (built-ins on a CIVVIS turn): "
          f"{dict(residual.most_common(6)) if residual else 'none'}")

    if wars:
        print(f"  WARS DECLARED: {[(w.get('turn'), w.get('target')) for w in wars]}")
    else:
        print("  wars declared: none")
    if stale:
        print(f"  stale answers used: {len(stale)} "
              f"(worst {max(s.get('behind', 0) for s in stale)} turns behind)")
    if errors:
        kinds = Counter(str(e.get("error", ""))[:60] for e in errors)
        print(f"  ERRORS: {len(errors)}")
        for text, count in kinds.most_common(3):
            print(f"    {count}x {text}")

    # ★ CAPTURE IS THE THING THIS PROJECT HAS NEVER PROVEN. Everything up to the wall
    # has worked before — rams built, walls hit, 118 melee strikes — and no city has
    # ever changed hands, so "did a city change owner" gets its own detector rather
    # than being inferred from a rising city count. A city count can rise because a
    # settler founded one, which is a completely different event.
    states = []
    for line in (run / "events.jsonl").read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "state":
            states.append(event)
    captures = []
    prev_ours = None
    prev_theirs = None
    for state in states:
        ours = {(c["x"], c["y"]) for c in state.get("cities", [])}
        theirs = set()
        for rival in state.get("rivals", []) or []:
            for city in rival.get("cities", []) or []:
                theirs.add((city["x"], city["y"]))
        if prev_ours is not None:
            # A plot that was THEIRS last turn and is OURS now changed hands. A
            # newly founded city was on nobody's list, so this cannot confuse them.
            gained = (ours - prev_ours) & prev_theirs
            for plot in gained:
                captures.append((state.get("turn"), plot))
        prev_ours, prev_theirs = ours, theirs
    if captures:
        print(f"  *** CITIES CAPTURED: {captures} ***")
    else:
        print("  cities captured: none")
    if states:
        seen = [(s.get("turn"), len(s.get("cities", [])),
                 sum(len(r.get("cities", []) or []) for r in (s.get("rivals") or [])))
                for s in states[-1:]]
        print(f"  last state: turn/our-cities/rival-cities-seen {seen}")

    print("  recent turns:")
    for turn in turns[-args.tail:]:
        print(f"    t{turn.get('turn'):>3} score={turn.get('score')} "
              f"cities={turn.get('cities')} army={turn.get('army')} "
              f"src={turn.get('orders_source')} "
              f"applied={turn.get('orders_applied')}/{turn.get('orders_seen')} "
              f"polls={turn.get('orders_polls')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
