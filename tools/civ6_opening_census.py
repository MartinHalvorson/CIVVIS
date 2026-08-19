#!/usr/bin/env python3
"""The opening of every recorded Civilization VI run, in the turns that decide it.

The ladder is decided early: `cities_at_60` is a gate (0 wins in 22 recorded games
below four cities) and two-thirds of the losses are behind by turn 75. Reading an
opening out of a run means grepping `events.jsonl`, `orders.sqlite` and `why.log`
by hand, and the same questions get asked of every game:

* When was the capital founded, and when did it reach the host's Settler floor
  (population 2)?
* When was the first Settler ORDERED, and what were the capital's first builds?
  (The book's Settler slot burns when the capital is still population 1 at Scout
  completion — `SCOUT,BUILDER,SETTLER…` — see `BasicAi::opening_settler_waits`.)
* When were cities 2, 3, 4, 5 founded; how many stood at turns 30/45/60, against
  the best rival's count?
* When did the pantheon land and which belief — Religious Settlements is a free
  Settler in the capital, and the seat took Divine Spark 40 of 40 times at median
  turn 22 before `AdvancedAi::expansion_pantheon` — and for how long was God-King,
  the only early Faith, slotted?
* How many Settlers after the first were built and how many never founded a city?

    python3 tools/civ6_opening_census.py                       # every recorded run
    python3 tools/civ6_opening_census.py <run-dir> [<run-dir>…]
    python3 tools/civ6_opening_census.py --since 20260819 --json census.json

⚠ It reads and prints. It starts no game, changes no controller, and asks nothing of
the host — safe against a game still being played.

⚠ Every row is ONE game; the footer's medians are a description of the recorded
population, not a treatment result. Use it to see whether a merged opening repair
did what it said on the next game, and to pick runs worth a closer look.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import sqlite3
import statistics
import sys
from pathlib import Path
from typing import Iterable, Sequence

DEFAULT_ROOT = "~/civvis-civ6-runs/control"
#: The turn by which the whole opening has played out; nothing past it is read.
HORIZON = 80
#: The population Civilization VI asks of a city before it starts a Settler.
SETTLER_FLOOR = 2
#: The turn-count checkpoints reported beside the foundings.
CHECKPOINTS = (30, 45, 60)


class CensusError(RuntimeError):
    """A refusal that names its cause rather than printing an empty table."""


def _events(run: Path) -> Iterable[dict]:
    path = run / "events.jsonl"
    if not path.exists():
        raise CensusError(f"{run}: no events.jsonl")
    with path.open(errors="ignore") as handle:
        for line in handle:
            try:
                yield json.loads(line)
            except ValueError:
                continue


def _capital_orders(run: Path) -> tuple[list[tuple[int, str]], list[tuple[int, str]]]:
    """(capital's `produce` orders, every city's `produce` orders), turn-ordered."""
    path = run / "orders.sqlite"
    if not path.exists():
        return [], []
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        every = connection.execute(
            "select turn, verb from orders where kind='produce' and turn<=? order by turn, seq",
            (HORIZON,),
        ).fetchall()
        capital = connection.execute(
            "select subject from orders where kind='produce' order by turn, seq limit 1"
        ).fetchone()
        mine = []
        if capital:
            mine = connection.execute(
                "select turn, verb from orders where kind='produce' and subject=? and turn<=? "
                "order by turn, seq",
                (capital[0], HORIZON),
            ).fetchall()
    finally:
        connection.close()
    return [(int(t), v) for t, v in mine], [(int(t), v) for t, v in every]


def _short(verb: str) -> str:
    for prefix in ("UNIT_", "BUILDING_", "DISTRICT_"):
        if verb.startswith(prefix):
            return verb[len(prefix):][:8]
    return verb[:8]


def census(run: Path) -> dict:
    """The opening of one recorded run."""
    founded: list[dict] = []
    per_turn: dict[int, dict] = {}
    settlers_first: dict[int, int] = {}
    settlers_last: dict[int, int] = {}
    settler_founded: set[int] = set()
    god_king: list[int] = []
    pantheon: tuple[int, str] | None = None
    for event in _events(run):
        kind = event.get("kind")
        turn = event.get("turn")
        if not isinstance(turn, int):
            continue
        if kind == "found":
            founded.append({"turn": turn, "unit": event.get("unit"), "x": event.get("x"), "y": event.get("y")})
            if isinstance(event.get("unit"), int):
                settler_founded.add(event["unit"])
        elif kind == "state":
            if turn > HORIZON:
                break
            cities = event.get("cities") or []
            capital = next((c for c in cities if c.get("capital")), None)
            rivals = event.get("rivals") or []
            per_turn[turn] = {
                "cities": len(cities),
                "pop": capital.get("pop") if capital else None,
                "producing": capital.get("producing") if capital else None,
                "rival_cities": max(
                    (r.get("public_stats", {}).get("city_count", 0) or 0 for r in rivals), default=0
                ),
            }
            for unit in event.get("units") or []:
                if unit.get("kind") == "UNIT_SETTLER" and isinstance(unit.get("id"), int):
                    settlers_first.setdefault(unit["id"], turn)
                    settlers_last[unit["id"]] = turn
            if "POLICY_GOD_KING" in (event.get("policies") or []):
                god_king.append(turn)
            if pantheon is None and event.get("pantheon"):
                pantheon = (turn, str(event["pantheon"]))
    if not per_turn:
        raise CensusError(f"{run}: no state records")

    def at(turn: int, key: str):
        earlier = [t for t in per_turn if t <= turn]
        return per_turn[max(earlier)][key] if earlier else None

    capital_orders, all_orders = _capital_orders(run)
    # What the host actually built in the capital, from its own `producing`
    # field: an order can be replaced by a hint or the mod's ladder, and a hint
    # (`produce_next`) never appears as a `produce` order at all.
    built: list[tuple[int, str]] = []
    for turn in sorted(per_turn):
        item = per_turn[turn]["producing"]
        if item and (not built or built[-1][1] != item):
            built.append((turn, item))
    if not built:
        built = capital_orders
    first_settler = next((t for t, v in built if v == "UNIT_SETTLER"), None)
    pop2 = min((t for t, s in per_turn.items() if (s["pop"] or 0) >= SETTLER_FLOOR), default=None)
    foundings = [f["turn"] for f in founded]
    walkers = sorted(settlers_first, key=settlers_first.get)[1:]  # the first is the capital's
    lost = [uid for uid in walkers if uid not in settler_founded and settlers_last[uid] < HORIZON]
    row = {
        "run": run.name,
        "capital_turn": foundings[0] if foundings else None,
        "pop2_turn": pop2,
        "first_settler_turn": first_settler,
        "first_builds": [_short(v) for _, v in built[:4]],
        "book_settler_held": None,
        "city_turns": foundings[1:6],
        "settler_orders": sum(1 for _, v in all_orders if v == "UNIT_SETTLER"),
        "walkers": len(walkers),
        "walkers_lost": len(lost),
        "pantheon_turn": pantheon[0] if pantheon else None,
        "pantheon": pantheon[1] if pantheon else None,
        "god_king": [min(god_king), max(god_king)] if god_king else None,
    }
    for checkpoint in CHECKPOINTS:
        row[f"cities_at_{checkpoint}"] = at(checkpoint, "cities")
    row["rival_cities_at_60"] = at(60, "rival_cities")
    why = run / "why.log"
    if why.exists():
        with why.open(errors="ignore") as handle:
            for line in handle:
                if "holds the opening book's settler" in line:
                    try:
                        row["book_settler_held"] = int(line.split(" t", 1)[1].split()[0])
                    except (IndexError, ValueError):
                        row["book_settler_held"] = -1
                    break
    return row


def rows_for(runs: Sequence[Path]) -> list[dict]:
    rows = []
    for run in runs:
        try:
            rows.append(census(run))
        except CensusError as error:
            print(f"skip: {error}", file=sys.stderr)
    return rows


def _fmt(value) -> str:
    if value is None:
        return "-"
    if isinstance(value, list):
        return "/".join(str(v) for v in value)
    return str(value)


def render(rows: Sequence[dict]) -> str:
    head = (
        f"{'run':17} {'cap':>3} {'pop2':>4} {'s1':>3} {'held':>4} {'first builds':30} "
        f"{'c2/c3/c4/c5':14} {'c30':>3} {'c45':>3} {'c60':>3} {'r60':>3} {'so':>3} {'lost':>4} "
        f"{'panth':>5} belief          godking"
    )
    lines = [head]
    for r in rows:
        lines.append(
            f"{r['run'][7:]:17} {_fmt(r['capital_turn']):>3} {_fmt(r['pop2_turn']):>4} "
            f"{_fmt(r['first_settler_turn']):>3} {_fmt(r['book_settler_held']):>4} "
            f"{','.join(r['first_builds']):30} {_fmt(r['city_turns'][:4]):14} "
            f"{_fmt(r['cities_at_30']):>3} {_fmt(r['cities_at_45']):>3} {_fmt(r['cities_at_60']):>3} "
            f"{_fmt(r['rival_cities_at_60']):>3} {_fmt(r['settler_orders']):>3} "
            f"{r['walkers_lost']}/{r['walkers']:<3} {_fmt(r['pantheon_turn']):>5} "
            f"{(r['pantheon'] or '-').replace('BELIEF_', '')[:15]:15} {_fmt(r['god_king'])}"
        )
    if len(rows) > 1:
        lines.append("")
        lines.append(f"{len(rows)} runs; medians (n):")
        for key, label in (
            ("capital_turn", "capital founded"),
            ("pop2_turn", "population 2"),
            ("first_settler_turn", "first Settler ordered"),
            ("cities_at_30", "cities at 30"),
            ("cities_at_45", "cities at 45"),
            ("cities_at_60", "cities at 60"),
            ("pantheon_turn", "pantheon"),
        ):
            values = [r[key] for r in rows if r.get(key) is not None]
            if values:
                lines.append(f"  {label:22} {statistics.median(values):>5g} ({len(values)})")
        for index, label in ((0, "city 2"), (1, "city 3"), (2, "city 4"), (3, "city 5")):
            values = [r["city_turns"][index] for r in rows if len(r["city_turns"]) > index]
            if values:
                lines.append(f"  {label:22} {statistics.median(values):>5g} ({len(values)})")
        beliefs: dict[str, int] = {}
        for r in rows:
            if r["pantheon"]:
                beliefs[r["pantheon"]] = beliefs.get(r["pantheon"], 0) + 1
        if beliefs:
            lines.append("  pantheons: " + ", ".join(f"{k.replace('BELIEF_', '')} {v}" for k, v in sorted(beliefs.items(), key=lambda kv: -kv[1])))
        lost = sum(r["walkers_lost"] for r in rows)
        walkers = sum(r["walkers"] for r in rows)
        if walkers:
            lines.append(f"  walkers never founding    {lost}/{walkers}")
    return "\n".join(lines)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("runs", nargs="*", help="recorded run directories (default: every run under --root)")
    parser.add_argument("--root", default=DEFAULT_ROOT, help="where the recorded runs live")
    parser.add_argument("--since", help="only runs whose tag date is >= this (YYYYMMDD)")
    parser.add_argument("--json", help="also write the rows here")
    args = parser.parse_args(argv)
    if args.runs:
        runs = [Path(r).expanduser() for r in args.runs]
    else:
        root = os.path.expanduser(args.root)
        runs = [Path(p) for p in sorted(glob.glob(os.path.join(root, "civvis-*Z")))]
    if args.since:
        runs = [r for r in runs if r.name[7:15] >= args.since]
    rows = rows_for(runs)
    if not rows:
        print("no readable runs", file=sys.stderr)
        return 1
    print(render(rows))
    if args.json:
        Path(args.json).write_text(json.dumps(rows, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
