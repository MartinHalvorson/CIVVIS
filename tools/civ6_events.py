#!/usr/bin/env python3
"""What live games recorded, read from their event logs.

Every real finding about the live agent has come from the same two moves: count
an event across recent games, then check it against the next `state` frame to
see what actually happened. This is those two moves, so they are not rewritten
each time.

Usage::

    python3 tools/civ6_events.py kinds              # every event kind, and its fields
    python3 tools/civ6_events.py failures           # failed orders, by type and reason
    python3 tools/civ6_events.py noops              # "the unit did not move", checked
    python3 tools/civ6_events.py strikes            # could each melee attack have landed?
    python3 tools/civ6_events.py plots VERB ...     # what stood on the plot when a unit order failed

Pass ``--runs N`` for more logs, or ``--root DIR`` for another directory. One
game continued from a save writes one log per sitting (``<run>-cont1`` and so
on), so a report says how many logs and how many games it read.
Line numbers in Lua errors refer to the installed agent; see civ6_agent_errors.py.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path
from typing import Iterable, Iterator

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"

# Fragments of a run's log so small they cannot hold a game.
MIN_GAME_BYTES = 1_000_000


def recent_games(root: Path, runs: int) -> list[Path]:
    """The newest event logs that hold a game, newest first."""
    logs = [path for path in root.glob("*/events.jsonl") if path.stat().st_size >= MIN_GAME_BYTES]
    return sorted(logs, key=lambda path: path.stat().st_mtime, reverse=True)[:runs]


def game_of(path: Path) -> str:
    """The game a log belongs to: every sitting of a continued game shares one."""
    return re.sub(r"-cont\d+$", "", path.parent.name)


def events(path: Path, needles: Iterable[str] = ()) -> Iterator[dict]:
    """Parsed events from one log. `needles` skip the JSON parse on other lines."""
    wanted = tuple(needles)
    with path.open(errors="replace") as handle:
        for line in handle:
            if wanted and not any(needle in line for needle in wanted):
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue  # a torn final line
            if isinstance(event, dict):
                yield event


def kinds(logs: list[Path]) -> dict[str, dict]:
    """kind -> {count, games, fields}."""
    table: dict[str, dict] = {}
    for path in logs:
        for event in events(path):
            kind = str(event.get("kind"))
            row = table.setdefault(kind, {"count": 0, "games": set(), "fields": set()})
            row["count"] += 1
            row["games"].add(path.parent.name)
            row["fields"].update(event)
    return table


def _verb_head(verb: object) -> str:
    return re.split(r"[:=,]", str(verb or ""))[0][:24]


def failures(logs: list[Path]) -> list[dict]:
    """Failed orders per (kind, verb), worst rate first.

    Counts orders, not reports. A replan frame re-sends an order and each send
    is checked, so one refusal can file three rows. Raw rows ranked retries
    above real problems: Apostle `no_path`, Great Person `not_activated` and
    religion founding each looked common and were not. An order is one
    (log, turn, subject, verb). It failed if no check of it passed, and
    `recovered` counts the ones that failed and then worked in the same turn.
    """
    checks: dict[tuple, dict] = {}
    for path in logs:
        for event in events(path, ('"order_failed"', '"order_verified"')):
            if event.get("kind") not in ("order_failed", "order_verified"):
                continue
            order = (path, event.get("turn"), str(event.get("subject")), str(event.get("verb")))
            row = checks.setdefault(
                order, {"key": (event.get("order_kind"), _verb_head(event.get("verb"))),
                        "worked": False, "reason": None})
            if event["kind"] == "order_verified":
                row["worked"] = True
            else:
                row["reason"] = re.sub(r"\d+", "N", str(event.get("reason")))
    totals: collections.Counter = collections.Counter()
    recovered: collections.Counter = collections.Counter()
    failed: dict[tuple, collections.Counter] = collections.defaultdict(collections.Counter)
    for row in checks.values():
        totals[row["key"]] += 1
        if row["reason"] is not None:
            if row["worked"]:
                recovered[row["key"]] += 1
            else:
                failed[row["key"]][row["reason"]] += 1
    rows = []
    for key, total in totals.items():
        bad = sum(failed[key].values())
        rows.append(
            {"kind": key[0], "verb": key[1], "total": total, "failed": bad,
             "recovered": recovered[key], "rate": bad / total,
             "reasons": failed[key].most_common(3)}
        )
    return sorted(rows, key=lambda row: (-row["failed"], -row["total"]))


def noop_outcomes(logs: list[Path]) -> dict[str, collections.Counter]:
    """For each `move_noop` reason, where the unit stood at the next state frame."""
    table: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for path in logs:
        pending: list[dict] = []
        for event in events(path, ('"move_noop"', '"kind": "state"')):
            if event.get("kind") == "move_noop":
                pending.append(event)
            elif event.get("kind") == "state" and pending:
                where = {unit.get("id"): (unit.get("x"), unit.get("y")) for unit in event.get("units") or []}
                for noop in pending:
                    now = where.get(noop.get("unit"))
                    if now is None:
                        outcome = "gone"
                    elif now == tuple(noop["want"]):
                        outcome = "reached"
                    elif now == tuple(noop["from"]):
                        outcome = "stayed"
                    else:
                        outcome = "elsewhere"
                    table[str(noop.get("why"))][outcome] += 1
                pending = []
    return table


RELIGIOUS_UNITS = frozenset({"UNIT_APOSTLE", "UNIT_INQUISITOR", "UNIT_MISSIONARY", "UNIT_GURU"})


def _path_verdict(strike: dict) -> str:
    """Where the host's own move path ended this turn, for one melee strike."""
    if not strike.get("native_move_path_known"):
        return "unknown"
    if not strike.get("native_move_path_length"):
        return "no_path"
    end = strike.get("native_move_path_this_turn_end") or {}
    return "reaches_target" if (end.get("x"), end.get("y")) == (strike.get("x"), strike.get("y")) else "stops_short"


def strike_paths(logs: list[Path]) -> dict[tuple[str, str], collections.Counter]:
    """(unit family, host path this turn) -> how those melee strikes came out.

    One strike is one (turn, unit, target) in a log, since a replan re-sends it.
    The host's path is a move path, so a strike that stops short can still land.
    A verdict names the unit and not the plot, so a unit that struck a second
    target that turn counts as landed. `no_path` is the case to watch: 119 of
    126 missed before Apostles stopped aiming into rival cities (2026-11-10).
    """
    table: dict[tuple[str, str], collections.Counter] = collections.defaultdict(collections.Counter)
    for path in logs:
        strikes: dict[tuple, dict] = {}
        worked: dict[tuple, bool] = {}
        for event in events(path, ('"kind": "strike"', '"order_failed"', '"order_verified"')):
            kind, verb = event.get("kind"), event.get("verb")
            if verb != "ATTACK":
                continue
            if kind == "strike":
                strikes.setdefault((event.get("turn"), event.get("unit"), event.get("x"), event.get("y")), event)
            elif kind in ("order_failed", "order_verified"):
                order = (event.get("turn"), event.get("subject"))
                worked[order] = worked.get(order, False) or kind == "order_verified"
        for (turn, unit, _, _), strike in strikes.items():
            if (turn, unit) in worked:
                family = "religious" if strike.get("unit_kind") in RELIGIOUS_UNITS else "military"
                outcome = "landed" if worked[(turn, unit)] else "missed"
                table[(family, _path_verdict(strike))][outcome] += 1
    return table


def order_plots(logs: list[Path], verbs: Iterable[str] = ()) -> dict[tuple, collections.Counter]:
    """(verb, improvement, feature, resource) under the unit -> orders that worked or failed.

    A rule the simulator and the host disagree on shows up as one order failing
    on one kind of plot. That is how Builders chopping under Lumber Mills were
    found. An order is one (turn, unit, verb) in a log, read as in `failures`.
    The unit stands where the last frame of its turn left it, and the plot is as
    it was before that turn, so an order that worked does not show its own result.
    """
    wanted = set(verbs)
    table: dict[tuple, collections.Counter] = collections.defaultdict(collections.Counter)
    for path in logs:
        where: dict[int, dict] = collections.defaultdict(dict)
        chunks: collections.deque = collections.deque()
        plots: dict[tuple, dict] = {}
        orders: dict[tuple, dict] = {}
        for event in events(path, ('"kind": "state"', '"kind": "tiles"', '"order_failed"', '"order_verified"')):
            kind = event.get("kind")
            if kind == "state":
                where[event.get("turn")] = {unit.get("id"): (unit.get("x"), unit.get("y")) for unit in event.get("units") or []}
            elif kind == "tiles":
                chunks.append(event)
            elif kind in ("order_failed", "order_verified") and event.get("order_kind") == "unit":
                verb = _verb_head(event.get("verb"))
                if wanted and verb not in wanted:
                    continue
                turn = event.get("turn")
                while chunks and isinstance(turn, int) and chunks[0].get("turn", turn) < turn:
                    for plot in chunks.popleft().get("plots") or []:
                        plots[(plot.get("x"), plot.get("y"))] = plot
                stood = where[turn].get(event.get("subject"))
                if stood is None:
                    continue
                plot = plots.get(stood, {})
                order = orders.setdefault(
                    (turn, event.get("subject"), verb),
                    {"key": (verb, plot.get("im") or "-", plot.get("f") or "-", plot.get("r") or "-"), "worked": False})
                order["worked"] = order["worked"] or kind == "order_verified"
        for order in orders.values():
            table[order["key"]]["worked" if order["worked"] else "failed"] += 1
    return table


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("report", choices=("kinds", "failures", "noops", "strikes", "plots"))
    parser.add_argument("verbs", nargs="*", help="plots only: the unit verbs to read, or every one")
    parser.add_argument("--runs", type=int, default=8, help="newest games to read")
    parser.add_argument("--root", type=Path, default=RUN_ROOT)
    args = parser.parse_intermixed_args()
    logs = recent_games(args.root, args.runs)
    print(f"{len(logs)} logs read, {len({game_of(path) for path in logs})} games")
    if args.report == "kinds":
        for kind, row in sorted(kinds(logs).items(), key=lambda item: -item[1]["count"]):
            fields = ",".join(sorted(row["fields"] - {"kind", "ctx", "run", "t", "utc"}))
            print(f"{row['count']:8} in {len(row['games']):3} games  {kind:28} {fields[:110]}")
    elif args.report == "failures":
        for row in failures(logs):
            if row["failed"] or row["recovered"]:
                reasons = ", ".join(f"{name} {count}" for name, count in row["reasons"])
                print(f"{row['failed']:5} of {row['total']:6} ({row['rate']:4.0%}) "
                      f"recovered {row['recovered']:4}  "
                      f"{str(row['kind']):14} {row['verb']:24} {reasons}")
    elif args.report == "plots":
        for (verb, improvement, feature, resource), outcomes in sorted(
                order_plots(logs, args.verbs).items(), key=lambda item: (-item[1]["failed"], item[0])):
            print(f"failed {outcomes['failed']:5}  worked {outcomes['worked']:5}  "
                  f"{verb:20} {improvement:30} {feature:26} {resource}")
    elif args.report == "strikes":
        for (family, verdict), outcomes in sorted(strike_paths(logs).items()):
            print(f"{family:10} {verdict:15} landed {outcomes['landed']:5}  missed {outcomes['missed']:5}")
    else:
        for why, outcomes in sorted(noop_outcomes(logs).items(), key=lambda item: -sum(item[1].values())):
            print(f"{why:18} {dict(outcomes)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
