#!/usr/bin/env python3
"""Live verification results per exact `LEADER_*` id and victory lane.

Since 2026-10-31 the live seat plays whichever leader Civilization VI's Random
Leader row deals, and the lane operator may later deal the leader itself, as
a (leader, lane) pair from a table. This ledger is the evidence for that
table, read from the runs themselves:

- the leader, civilization, difficulty and map come from the mod's own `seat`
  event (`summary.json` `seat`), never from a requested leader. The seat check
  compares the `LEADER_*` id exactly, so the table must too;
- one game is a run and its `-contN` continuations, scored by its last part;
- the result is the game's own `victory` event, or the harness's retire
  verdict (`abandoned`), or neither (stopped, timed out or still running);
- our share of the leading rival's Production at turn 80 comes from the agent
  `turn` records (`major_production`), the operator's retire rule's reading.

    python3 tools/leader_ledger.py                       # 6p Continents seats
    python3 tools/leader_ledger.py --since 20261031T0000 --json
    python3 tools/leader_ledger.py --map any --players 0  # every seat
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"
TAG = re.compile(r"^(civvis-\d{8}T\d{6}Z)(?:-cont(\d+))?$")
SHARE_TURN = 80


def game_parts(root: Path, since: str = "") -> dict[str, list[Path]]:
    """Run directories grouped by game: the base tag, then each continuation."""
    games: dict[str, list[tuple[int, Path]]] = defaultdict(list)
    for path in root.iterdir() if root.is_dir() else ():
        match = TAG.match(path.name)
        if not match or not path.is_dir():
            continue
        if since and match.group(1) < f"civvis-{since}":
            continue
        games[match.group(1)].append((int(match.group(2) or 0), path))
    return {base: [p for _, p in sorted(parts)] for base, parts in sorted(games.items())}


def _summary(path: Path) -> dict:
    try:
        value = json.loads((path / "summary.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return value if isinstance(value, dict) else {}


def production_share_at(events: Path, turn: int = SHARE_TURN) -> float | None:
    """Our Production over the best rival's on the first agent turn record at `turn`."""
    try:
        handle = events.open(encoding="utf-8")
    except OSError:
        return None
    with handle:
        for line in handle:
            if '"kind": "turn"' not in line or f'"turn": {turn}' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("ctx") != "agent" or event.get("turn") != turn:
                continue
            production = event.get("major_production")
            us = event.get("local_player")
            if not isinstance(production, dict) or not isinstance(us, int):
                continue
            ours = production.get(str(us))
            rivals = [v for k, v in production.items()
                      if k != str(us) and isinstance(v, (int, float))]
            if not isinstance(ours, (int, float)) or not rivals or max(rivals) <= 0:
                continue
            return round(ours / max(rivals), 3)
    return None


def game_record(parts: list[Path]) -> dict | None:
    """One game's ledger row, or None when no part carries a seat."""
    seat: dict = {}
    for part in parts:
        seat = _summary(part).get("seat") or seat
        if seat:
            break
    if not seat.get("leader"):
        return None
    last = _summary(parts[-1])
    outcome = last.get("outcome") if isinstance(last.get("outcome"), dict) else {}
    abandoned = last.get("abandoned") if isinstance(last.get("abandoned"), dict) else {}
    if outcome.get("victory_type"):
        result = ("win:" if outcome.get("won") else "loss:") + outcome["victory_type"].removeprefix("VICTORY_").lower()
    elif abandoned:
        result = "retired"
    else:
        result = "unfinished"
    lane = next((s.get("victory_target") for s in map(_summary, reversed(parts))
                 if s.get("victory_target")), None)
    return {
        "game": parts[0].name,
        "leader": seat.get("leader"),
        "civ": seat.get("civ"),
        "difficulty": seat.get("difficulty"),
        "map": seat.get("map"),
        "players": seat.get("players"),
        "lane": lane,
        "result": result,
        "last_turn": last.get("last_turn"),
        "share_t80": production_share_at(parts[0] / "events.jsonl"),
    }


def ledger(records: list[dict]) -> list[dict]:
    """Rows per (leader, lane), most games first."""
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for record in records:
        groups[(record["leader"], record["lane"])].append(record)
    rows = []
    for (leader, lane), games in groups.items():
        results = [g["result"] for g in games]
        shares = [g["share_t80"] for g in games if g["share_t80"] is not None]
        rows.append({
            "leader": leader,
            "civ": games[0]["civ"],
            "lane": lane,
            "games": len(games),
            "wins": sum(r.startswith("win:") for r in results),
            "losses": sorted(r.removeprefix("loss:") for r in results if r.startswith("loss:")),
            "retired": results.count("retired"),
            "unfinished": results.count("unfinished"),
            "median_share_t80": round(statistics.median(shares), 2) if shares else None,
            "tags": [g["game"] for g in games],
        })
    return sorted(rows, key=lambda r: (-r["games"], -r["wins"], r["leader"] or "", r["lane"] or ""))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=RUN_ROOT)
    parser.add_argument("--since", default="", help="first tag stamp, e.g. 20261031T0029")
    parser.add_argument("--map", default="Continents.lua", help="seat map, or 'any'")
    parser.add_argument("--players", type=int, default=6, help="majors, or 0 for any")
    parser.add_argument("--json", action="store_true", help="per-game records and rows as JSON")
    args = parser.parse_args(argv)
    records = [r for r in (game_record(parts) for parts in game_parts(args.root, args.since).values())
               if r is not None
               and (args.map == "any" or r["map"] == args.map)
               and (args.players == 0 or r["players"] == args.players)]
    rows = ledger(records)
    if args.json:
        json.dump({"games": records, "leaders": rows}, sys.stdout, indent=1)
        print()
        return 0
    wins = sum(r["wins"] for r in rows)
    print(f"{len(records)} games, {wins} won, {len(rows)} (leader, lane) rows")
    print(f"{'leader':<26} {'civ':<28} {'lane':<11} games wins retired open  t80   losses")
    for r in rows:
        share = "-" if r["median_share_t80"] is None else f"{r['median_share_t80']:.2f}"
        print(f"{r['leader']:<26} {(r['civ'] or '-'):<28} {(r['lane'] or '-'):<11} "
              f"{r['games']:>5} {r['wins']:>4} {r['retired']:>7} {r['unfinished']:>4} {share:>5}   "
              f"{' '.join(r['losses']) or '-'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
