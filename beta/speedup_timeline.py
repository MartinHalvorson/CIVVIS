#!/usr/bin/env python3
"""The home page's speed timeline: five ways one game of Civilization VI gets
played, and how many times faster each is than a person playing it by hand.

    speedup_timeline.py <source_tree>    print the timeline as JSON

Operator, 2026-11-22: "Above the 6 seconds video, lets have a timeline: right
to left: 5 hrs for me to play a Civ 6 game, 10-15 minutes with CIVVIS computer
control, how long with Firaxis Civ 6 Deity Sim? CIVVIS self-play should be
down to 10s or so. Can scale across M5 Max cores to 1 game per second. which
is on the left. We should track the multipliers for each one though and see
how much we can push things. 1,000,000x speed imrpove would be a huge
achievement over playing Civ 6 games. Provide a speedup number: Like 123,654x."

The measurements are `data/performance/speedup_timeline.json`: a list of
stops, slowest first, each with its dated `readings` (the seconds one game
takes and how that was measured), oldest first. The newest reading is the one
the page shows and the older ones are the track of how far a stop has been
pushed. A stop with `follows` keeps no readings of its own: it is read from
the games-per-minute ledger (`tools/games_per_minute.py --record`), one
reading for every run that set a record, so a new record there moves the home
page with no second edit.

A stop's multiplier is the first stop's seconds over its own, and the page's
one number is the largest of them. Nothing derived is written in the ledger:
the multipliers, the texts and each stop's place on the line are worked out
here when the site is published (`beta/home_rankings.py` writes the result
into the home page's `home-speedup` slot), so the page cannot say a figure the
ledger does not.

The line is a scale of multipliers by tenfolds, 1x at its right end and the
goal at its left: a stop stands at `log(multiplier) / log(goal)` of the way
across, which is its `at`.
"""

from __future__ import annotations

import json
import math
import pathlib
import re
import sys

#: The stops and their readings, in the source tree.
LEDGER = pathlib.Path("data/performance/speedup_timeline.json")

#: The home page's slot for the timeline.
SLOT = re.compile(r'(<script type="application/json" id="home-speedup">)(.*?)(</script>)', re.S)

#: The ledgers a stop may follow, by the name it follows them under, each
#: with what turns one of its runs into a reading's seconds and its `from`.
FOLLOWED = {
    "data/performance/games_per_minute.json": lambda run: {
        "seconds": 60 / run["games_per_minute"],
        "from": f"{run['games_per_minute']:g} games a minute, the record of `tools/games_per_minute.py --record`"
        " (docs/GAMES_PER_MINUTE.md).",
    },
}


def clock(seconds: float) -> str:
    """A time as the page words it: `18 ms`, `3.3 s`, `35 s`, `12.6 min`,
    `5 h`."""
    if seconds < 0.9995:
        return f"{seconds * 1000:.0f} ms"
    if seconds < 9.95:
        return f"{seconds:.1f} s".replace(".0 ", " ")
    if seconds < 99.5:
        return f"{seconds:.0f} s"
    if seconds < 5970:
        return f"{seconds / 60:.1f} min".replace(".0 ", " ")
    return f"{seconds / 3600:.1f} h".replace(".0 ", " ")


def times(multiplier: float) -> str:
    """A multiplier as the page words it: `1x`, `2.5x`, `24x`, `5,499x`."""
    if multiplier < 9.95 and abs(multiplier - round(multiplier)) >= 0.05:
        return f"{multiplier:.1f}x"
    return f"{round(multiplier):,}x"


def day(stamp: str) -> str:
    """A reading's day as the page words it, `11-22`, from `2026-11-22` or a
    full stamp that begins with one."""
    return stamp[5:10]


def records(runs: list[dict], reading) -> list[dict]:
    """The runs of a followed ledger that set a record, oldest first, as
    readings (`reading` gives a run's seconds and its `from`): a run slower
    than one before it is not a reading, so the newest reading is the record
    that stands."""
    readings: list[dict] = []
    for run in sorted(runs, key=lambda run: run["taken"]):
        read = reading(run)
        if not readings or read["seconds"] < readings[-1]["seconds"]:
            readings.append({"taken": run["taken"][:10], **read})
    return readings


def readings(source: pathlib.Path, stop: dict) -> list[dict]:
    """A stop's readings, oldest first: its own, or the records of the ledger
    it follows."""
    followed = stop.get("follows")
    if followed is None:
        return sorted(stop["readings"], key=lambda reading: reading["taken"])
    runs = json.loads((source / followed).read_text(encoding="utf-8"))["runs"]
    return records(runs, FOLLOWED[followed])


def sides(places: list[float]) -> list[str]:
    """Which side of the line each stop's words stand on, `above` or `below`,
    for stops at these places along it. Taken in order along the line, a stop
    goes to the side whose last stop is further off, so two stops that stand
    close together (self-play and the Deity sim are one engine and always do)
    never share a side."""
    last = {"below": -math.inf, "above": -math.inf}
    chosen = [""] * len(places)
    for index in sorted(range(len(places)), key=lambda i: places[i]):
        side = "below" if places[index] - last["below"] >= places[index] - last["above"] else "above"
        chosen[index] = side
        last[side] = places[index]
    return chosen


def ticks(goal: float) -> list[dict]:
    """The marks along the line: every tenfold from 1x up to the goal, and the
    goal itself at its end."""
    marks = [10**power for power in range(math.ceil(math.log10(goal)))] + [goal]
    return [{"times": times(mark), "at": round(math.log(mark) / math.log(goal), 4)} for mark in marks]


def timeline(source: pathlib.Path) -> dict | None:
    """What the home page shows: the stops in the ledger's order, each with
    its newest reading as a time and a multiplier, its goal when it has one,
    where it started when it has moved, and its place on the line; the largest
    multiplier, which is the page's number; and the goal, with the time a
    game would take at it. `None` when the revision has no ledger or one that
    cannot be read."""
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        goal = ledger["goal"]
        measured = [(stop, readings(source, stop)) for stop in ledger["stops"]]
        base = measured[0][1][-1]["seconds"]
        stops = []
        for stop, taken in measured:
            now, first = taken[-1], taken[0]
            multiplier = base / now["seconds"]
            row = {
                "key": stop["key"],
                "seconds": round(now["seconds"], 3),
                "time": clock(now["seconds"]),
                "multiplier": round(multiplier, 2),
                "times": times(multiplier),
                "taken": day(now["taken"]),
                "at": round(min(1.0, max(0.0, math.log(multiplier) / math.log(goal))), 4),
            }
            if now.get("from"):
                row["from"] = now["from"]
            if "goal_seconds" in stop:
                row["goal"] = f"goal {clock(stop['goal_seconds'])}, {times(base / stop['goal_seconds'])}"
            if times(base / first["seconds"]) != row["times"]:
                row["was"] = f"{times(base / first['seconds'])} on {day(first['taken'])}"
            stops.append(row)
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError):
        return None
    for row, side in zip(stops, sides([row["at"] for row in stops])):
        row["side"] = side
    fastest = max(stops, key=lambda row: row["multiplier"])
    shown = {
        "speedup": fastest["times"],
        "fastest": fastest["key"],
        "reach": fastest["at"],
        "goal": times(goal),
        "goal_time": clock(base / goal),
        "ticks": ticks(goal),
        "stops": stops,
    }
    if "was" in fastest:
        shown["was"] = fastest["was"]
    return shown


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    json.dump(timeline(pathlib.Path(argv[1])), sys.stdout, ensure_ascii=False, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
