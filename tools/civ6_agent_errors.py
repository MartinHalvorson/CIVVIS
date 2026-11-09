#!/usr/bin/env python3
"""What the live Lua agent's guards caught, across recent games.

The agent wraps its work in guards so one bad unit cannot end a turn, which also
means a bug in a handler is a decision that quietly never happens. It reports
each distinct message as an ``agent_error`` event (``CivvisK.noteError``). This
adds those up.

Usage::

    python3 tools/civ6_agent_errors.py              # the last 40 games
    python3 tools/civ6_agent_errors.py --runs 200
"""

from __future__ import annotations

import argparse
import collections
import json
from dataclasses import dataclass, field
from pathlib import Path

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"


@dataclass
class Site:
    message: str
    where: str
    games: set[str] = field(default_factory=set)
    worst: int = 0
    first_turn: int | None = None


def read_game(path: Path, run: str, sites: dict[str, Site]) -> int:
    """Fold one game's reports into ``sites``; return how many it held."""
    held = 0
    with path.open(errors="replace") as handle:
        for line in handle:
            if "agent_error" not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("kind") != "agent_error":
                continue
            held += 1
            message = str(event.get("message", ""))
            site = sites.setdefault(message, Site(message, str(event.get("where", "?"))))
            site.games.add(run)
            site.worst = max(site.worst, int(event.get("count") or 0))
            turn = event.get("turn")
            if isinstance(turn, int) and (site.first_turn is None or turn < site.first_turn):
                site.first_turn = turn
    return held


def census(root: Path, runs: int) -> tuple[int, int, list[Site]]:
    """(games read, games reporting, sites worst-first)."""
    logs = sorted(root.glob("*/events.jsonl"), key=lambda path: path.stat().st_mtime, reverse=True)[:runs]
    sites: dict[str, Site] = collections.OrderedDict()
    reporting = sum(1 for path in logs if read_game(path, path.parent.name, sites))
    ranked = sorted(sites.values(), key=lambda site: (-len(site.games), -site.worst, site.message))
    return len(logs), reporting, ranked


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--runs", type=int, default=40, help="newest games to read")
    parser.add_argument("--root", type=Path, default=RUN_ROOT)
    args = parser.parse_args()
    read, reporting, sites = census(args.root, args.runs)
    print(f"{read} games read, {reporting} reported errors, {len(sites)} distinct messages")
    for site in sites:
        turn = "?" if site.first_turn is None else site.first_turn
        print(f"{len(site.games):4} games  {site.worst:>7}+ times  t{turn:<4} {site.where:6} {site.message}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
