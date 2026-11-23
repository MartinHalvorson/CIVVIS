#!/usr/bin/env python3
"""The home page's speed timeline: five ways one game of Civilization VI gets
played, and how many times faster each is than a person playing it by hand.

    speedup_timeline.py <source_tree>            print the timeline as JSON
    speedup_timeline.py --worked <source_tree>   print the worked tables as JSON

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

The CIVVIS Performance page opens on the same readings as tables (`worked`),
each figure beside the sum that gives it (operator, 2026-11-22: "at the top
of civvis performance we want to calculate a multiplier on the theoretical
and experimetnal throughput of the simualtor. and the calculations. baeline
is 5 hrs for manually played game. please add the deity computer control,
deity self-play, civvis self-play. how much has civvis self-play speed up
over my manual 5 hr games? add this info, maybe in tables to the top"):

- **one game at a time**: every stop with readings of its own, the time one
  game takes and the baseline's seconds over it;
- **one machine, every core**: the experimental throughput, which is the
  every-core record counted by the stopwatch, and under it the theoretical
  throughput, which is a calculation and not a run: every core of that
  machine playing as fast as one core was measured. It is worked out twice
  for self-play, from the one-core reading of the table above (taken on the
  busy host) and from a game played with the machine to itself (the stop's
  `alone` readings), and each says what share of it the measured run reaches.

Both pages read one ledger through this one file, so they cannot disagree.
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
        # What the worked tables need of the run and the timeline does not:
        # the cores that played, the machine they are in, how many turns its
        # games ran and how many cores' worth of time its workers were given.
        "cores": (run.get("machine") or {}).get("cores") or run.get("workers"),
        "chip": (run.get("machine") or {}).get("chip"),
        "turns": run.get("mean_turns"),
        "had": (run.get("conditions") or {}).get("workers_cpu_cores_sampled") or (run.get("conditions") or {}).get("workers_cpu_cores"),
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


def rate(value: float) -> str:
    """Games an hour or a minute as a table words them: `0.2`, `4.8`, `63`,
    `1,100`."""
    if value < 99.95:
        text = f"{value:.1f}"
        return text[:-2] if text.endswith(".0") else text
    return f"{round(value):,}"


def figure(value: float) -> str:
    """A figure inside a sum, to four figures so the sum can be done again
    by hand: `757`, `57.1`, `18.33`, `194.7`, `18,000`."""
    return f"{round(value):,}" if value >= 999.5 else f"{value:.4g}"


def share(part: float) -> str:
    """A share as the tables word it: `97%`, and `0.5%` under a tenth."""
    return f"{100 * part:.1f}%" if part < 0.0995 else f"{round(100 * part)}%"


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


def worked(source: pathlib.Path) -> dict | None:
    """What the CIVVIS Performance page opens on: the readings as two tables,
    every figure with the sum that gives it. `None` when the revision has no
    ledger or one that cannot be read.

    `games` is one game at a time: each stop with readings of its own, in the
    ledger's order, the first the baseline. `throughput` is one machine with
    every core playing: for each stop that plays another's game on every core
    (`of`), the run as it was measured (`kind: "measured"`), and for each
    one-core stop the same machine's cores times that stop's speed (`kind:
    "theory"`), from its one-core reading (`basis: "one_core"`) and, where it
    has one, from a game played with the machine to itself (`basis:
    "alone"`). A theory row of a stop with a measured run says what share of
    it that run `reaches`. Last come the goals the runs are held against
    (`kind: "goal"`): each measured run's own, and the goal the home page's
    line ends at (`key: "goal"`). `answer` is the page's opening sentence in
    figures: the self-play stop one game at a time, on every core as
    measured, and the highest theory there is for it.
    """
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        goal = ledger["goal"]
        measured = [(stop, readings(source, stop)) for stop in ledger["stops"]]
        base = measured[0][1][-1]["seconds"]
        hours = f"{figure(base / 3600)} h"
        newest = {stop["key"]: taken[-1] for stop, taken in measured}

        def common(stop: dict, taken: list[dict]) -> dict:
            now, first = taken[-1], taken[0]
            row = {
                "key": stop["key"],
                "seconds": round(now["seconds"], 3),
                "time": clock(now["seconds"]),
                "an_hour": rate(3600 / now["seconds"]),
                "multiplier": round(base / now["seconds"], 2),
                "times": times(base / now["seconds"]),
                "taken": now["taken"][:10],
            }
            if now.get("from"):
                row["from"] = now["from"]
            if "goal_seconds" in stop:
                row["goal"] = f"{clock(stop['goal_seconds'])}, {times(base / stop['goal_seconds'])}"
            if times(base / first["seconds"]) != row["times"]:
                row["was"] = f"{times(base / first['seconds'])} on {day(first['taken'])}"
            return row

        games = []
        for index, (stop, taken) in enumerate(measured):
            if "follows" in stop:
                continue
            row = common(stop, taken)
            row["sum"] = f"{clock(base)} = {figure(base)} s, the baseline" if index == 0 else f"{figure(base)} s ÷ {figure(row['seconds'])} s"
            games.append(row)

        # The machine is the one the every-core run was played on: its cores
        # are the cores the theory multiplies by.
        runs = [(stop, taken[-1]) for stop, taken in measured if "follows" in stop and taken[-1].get("cores")]
        throughput, machine = [], None
        for stop, run in runs:
            machine = machine or {"cores": run["cores"], "chip": run.get("chip") or ""}
            row = common(stop, [run])
            row.update(
                kind="measured",
                of=stop.get("of") or "",
                a_minute=rate(60 / run["seconds"]),
                sum=f"{figure(60 / run['seconds'])} games a minute × 60 × {hours}",
            )
            if run.get("had"):
                row["had"] = f"{run['had']:g}"
            # The goal is the run's own; where it started is not kept here:
            # a follower's readings are its records, and the first of one
            # machine is not the first of another.
            row.pop("was", None)
            throughput.append(row)
        if machine:
            cores = machine["cores"]
            # A game's rows stand together: the run that was measured, then
            # the theory for the game it plays. The theory for a game no run
            # has played on every core comes after them.
            played = [row["of"] for row in throughput]
            one_core = [(stop, taken) for stop, taken in measured if stop.get("cores") == 1]
            one_core.sort(key=lambda pair: pair[0]["key"] not in played)
            theory = {}
            for stop, taken in one_core:
                held = next((row for row in throughput if row["kind"] == "measured" and row["of"] == stop["key"]), None)
                bases = [("one_core", taken[-1]["seconds"], f"{cores} cores × {figure(base)} s ÷ {figure(taken[-1]['seconds'])} s", taken[-1])]
                run = next((run for other, run in runs if other.get("of") == stop["key"]), None)
                alone = sorted(stop.get("alone") or [], key=lambda reading: reading["taken"])
                if alone and run and run.get("turns"):
                    # A game of the measured run, at the speed of the game
                    # that had the machine to itself.
                    quiet = alone[-1]
                    seconds = run["turns"] * quiet["seconds"] / quiet["turns"]
                    said = (
                        f"{cores} cores × ({figure(quiet['turns'])} turns ÷ {figure(quiet['seconds'])} s)"
                        f" × {figure(base)} s ÷ {figure(run['turns'])} turns a game"
                    )
                    bases.append(("alone", seconds, said, quiet))
                for basis, seconds, said, reading in bases:
                    multiplier = cores * base / seconds
                    row = {
                        "key": stop["key"],
                        "kind": "theory",
                        "basis": basis,
                        "one_game": clock(seconds),
                        "a_minute": rate(cores * 60 / seconds),
                        "an_hour": rate(cores * 3600 / seconds),
                        "multiplier": round(multiplier, 2),
                        "times": times(multiplier),
                        "sum": said,
                        "taken": reading["taken"][:10],
                    }
                    if basis == "alone" and reading.get("from"):
                        row["from"] = reading["from"]
                    if held:
                        row["reaches"] = share(held["multiplier"] / multiplier)
                    theory.setdefault(stop["key"], []).append(row)
            ordered = []
            for row in throughput:
                ordered += [row, *theory.pop(row["of"], [])]
            throughput = ordered + [row for rows in theory.values() for row in rows]
            # The goals, each with the share of it the measured run reaches:
            # a run's own, then the goal of the whole line.
            best = max(row["multiplier"] for row in throughput if row["kind"] == "measured")
            aims = [
                (stop["key"], stop["goal_seconds"], f"{figure(base)} s ÷ {figure(stop['goal_seconds'])} s", base / run["seconds"])
                for stop, run in runs
                if "goal_seconds" in stop
            ]
            aims.append(("goal", base / goal, f"a game every {clock(base / goal)}", best))
            for key, seconds, said, reached in aims:
                throughput.append(
                    {
                        "key": key,
                        "kind": "goal",
                        "one_game": clock(seconds),
                        "a_minute": rate(60 / seconds),
                        "an_hour": rate(3600 / seconds),
                        "multiplier": round(base / seconds, 2),
                        "times": times(base / seconds),
                        "sum": said,
                        "reaches": share(reached * seconds / base),
                    }
                )

        shown = {
            "base": {"time": clock(base), "seconds": figure(base), "an_hour": rate(3600 / base)},
            "games": games,
            "throughput": throughput,
            "goal": {
                "times": times(goal),
                "time": clock(base / goal),
                "a_minute": rate(goal * 60 / base),
                "an_hour": rate(goal * 3600 / base),
            },
        }
        if machine:
            shown["machine"] = machine
        # The opening sentence: the stop a measured run plays on every core
        # (self-play), one game at a time, as measured, and in theory.
        held = next((row for row in throughput if row["kind"] == "measured" and row["of"] in newest), None)
        if held:
            one = next(row for row in games if row["key"] == held["of"])
            theories = [row for row in throughput if row["kind"] == "theory" and row["key"] == held["of"]]
            answer = {
                "stop": held["of"],
                "one": one["times"],
                "one_time": one["time"],
                "all": held["times"],
                "all_time": held["time"],
                "all_an_hour": held["an_hour"],
            }
            if theories:
                top = max(theories, key=lambda row: row["multiplier"])
                answer.update(theory=top["times"], theory_basis=top["basis"], reaches=top["reaches"])
            shown["answer"] = answer
        return shown
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError, StopIteration):
        return None


def main(argv: list[str]) -> int:
    if len(argv) == 3 and argv[1] == "--worked":
        json.dump(worked(pathlib.Path(argv[2])), sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    json.dump(timeline(pathlib.Path(argv[1])), sys.stdout, ensure_ascii=False, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
