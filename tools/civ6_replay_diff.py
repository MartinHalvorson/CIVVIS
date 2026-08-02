#!/usr/bin/env python3
"""Compare recorded live CIVVIS decisions with a corrected persistent replay."""

from __future__ import annotations

import argparse
import json
import re
import sqlite3
from collections import Counter
from pathlib import Path


PLAN = re.compile(
    r"plan strategy=(?P<strategy>\w+) .*?target_player=(?P<target>\S+) "
    r"desired_cities=(?P<cities>\d+)"
)


def plan(note: str) -> tuple[str, str, int] | None:
    found = PLAN.search(note or "")
    if not found:
        return None
    return found["strategy"], found["target"], int(found["cities"])


def recorded_notes(run: Path) -> dict[int, dict]:
    out = {}
    path = run / "civvis_notes.jsonl"
    for line in path.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") == "civvis_note" and isinstance(event.get("turn"), int):
            out[event["turn"]] = event
    return out


def replay_notes(path: Path) -> dict[int, dict]:
    out = {}
    for line in path.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if isinstance(event.get("turn"), int):
            out[event["turn"]] = event
    return out


def recorded_orders(run: Path) -> dict[int, Counter]:
    out: dict[int, Counter] = {}
    connection = sqlite3.connect(run / "orders.sqlite")
    try:
        rows = connection.execute(
            "SELECT turn, kind, subject, verb, x, y FROM orders ORDER BY turn, seq"
        )
        for turn, kind, subject, verb, x, y in rows:
            out.setdefault(turn, Counter())[(kind, subject, verb, x, y)] += 1
    finally:
        connection.close()
    return out


def replay_orders(events: dict[int, dict]) -> dict[int, Counter]:
    return {
        turn: Counter(
            (row.get("kind"), row.get("subject"), row.get("verb"), row.get("x"), row.get("y"))
            for row in event.get("orders") or []
        )
        for turn, event in events.items()
    }


def segments(turns: list[int], values: dict[int, str]) -> list[str]:
    if not turns:
        return []
    out, start, prior, value = [], turns[0], turns[0], values[turns[0]]
    for turn in turns[1:]:
        current = values[turn]
        if current != value or turn != prior + 1:
            out.append(f"t{start}-{prior}:{value}")
            start, value = turn, current
        prior = turn
    out.append(f"t{start}-{prior}:{value}")
    return out


def counter_total(values: Counter) -> int:
    return sum(values.values())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path)
    parser.add_argument("replay", type=Path)
    args = parser.parse_args()

    old = recorded_notes(args.run)
    new = replay_notes(args.replay)
    turns = sorted(set(old) & set(new))
    old_plans = {turn: plan(old[turn].get("note", "")) for turn in turns}
    new_plans = {turn: plan(new[turn].get("note", "")) for turn in turns}
    comparable = [turn for turn in turns if old_plans[turn] and new_plans[turn]]
    strategy_old = {turn: old_plans[turn][0] for turn in comparable}
    strategy_new = {turn: new_plans[turn][0] for turn in comparable}
    changed = [turn for turn in comparable if old_plans[turn] != new_plans[turn]]
    strategy_changed = [turn for turn in comparable if strategy_old[turn] != strategy_new[turn]]

    old_orders = recorded_orders(args.run)
    new_orders = replay_orders(new)
    order_changed = [turn for turn in turns if old_orders.get(turn, Counter()) != new_orders.get(turn, Counter())]
    added = sum(counter_total(new_orders.get(turn, Counter()) - old_orders.get(turn, Counter()))
                for turn in turns)
    removed = sum(counter_total(old_orders.get(turn, Counter()) - new_orders.get(turn, Counter()))
                  for turn in turns)

    print(f"turns compared       {len(turns)}")
    print(f"plan changed         {len(changed)}; first {changed[0] if changed else 'none'}")
    print(f"strategy changed     {len(strategy_changed)}; "
          f"first {strategy_changed[0] if strategy_changed else 'none'}")
    print(f"orders changed       {len(order_changed)}; first {order_changed[0] if order_changed else 'none'}")
    print(f"order delta          +{added} corrected / -{removed} recorded")
    print("recorded strategy    " + "  ".join(segments(comparable, strategy_old)))
    print("corrected strategy   " + "  ".join(segments(comparable, strategy_new)))
    transitions = Counter((strategy_old[turn], strategy_new[turn]) for turn in strategy_changed)
    if transitions:
        print("strategy transitions " + ", ".join(
            f"{old_name}->{new_name}:{count}"
            for (old_name, new_name), count in transitions.most_common()
        ))
    for turn in order_changed[:12]:
        before, after = old_orders.get(turn, Counter()), new_orders.get(turn, Counter())
        print(
            f"t{turn} orders          +{counter_total(after - before)} / "
            f"-{counter_total(before - after)} "
            f"strategy {strategy_old.get(turn, '?')}->{strategy_new.get(turn, '?')}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
