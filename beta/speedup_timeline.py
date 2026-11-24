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
largest goal at its left: a stop stands at `log(multiplier) / log(goal)` of
the way across, which is its `at`. The ledger's `goals` are marked on it, each
at its own place (operator, 2026-11-22, of the second: "Addd 1 billion x to
the scaling scale as well").

Past the fastest stop the line has stops nothing has played yet
(`projected`; operator, 2026-11-22: "add to out speedup timeline: 10x
theoretical scale up from local hardware ... hardware scaleup is to the left
of m5 max. potential distributed RL fleet: up to 1000+x"). Each is a measured
stop's multiplier (`over`) times a figure of the operator's, so it moves when
that stop does, and it is never the page's number: that is the fastest stop
that was measured.

What renting the rest of the way to a goal would cost stands with each goal
(`cloud`; operator, 2026-11-22: "we should also estimate the cloud cost in $
to fill i nthe remaining amount (note that this is a Fable 5.2 best estimate
of the numbers)"). It is an estimate from the ledger's `cloud` entry, and the
pages say whose: a rented machine's cores, each taken to play a game as fast
as the one-core reading says, slower by the two processors' single thread
scores; the whole machines it takes to make up what the fastest stop is short
of the goal; and their price for an hour. Nothing was run in a cloud.

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

Under them stands a third table, **processor by processor** (`processors`;
operator, 2026-11-22: "Add in a theoretical 9980, performance and m5 ultra,
m6. we should have a table in civvis performance that has our benchmarks (and
age) across each processor. we should also list the single thread score and
multithread score from the website"). Its rows are the processors of
`data/performance/processors.json`, each with the website's two scores for
it. The processor the readings were taken on shows them as they were
measured. Every other row is a calculation from the scores, not a run: one
game on one core is the measured time, times the measured processor's single
thread score, over the row's own, and the games a minute on every core are
the measured record, times the row's multithread score, over the measured
processor's. Each cell carries the day of the reading it rests on, so the
page can say how old a benchmark is on the day it is read.

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

#: The processors the Performance page lists the benchmarks across, with the
#: website's two scores for each, in the source tree.
PROCESSORS = pathlib.Path("data/performance/processors.json")

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
    """A time as the page words it: `18 µs`, `18 ms`, `3.3 s`, `35 s`,
    `12.6 min`, `5 h`."""
    if seconds < 0.0009995:
        return f"{seconds * 1000000:.0f} µs"
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
    """A share as the tables word it: `97%`, `0.5%` under a tenth, and its
    first figure under a thousandth, `0.0005%`."""
    if part < 0.000995:
        return f"{100 * part:.1g}%" if part > 0 else "0%"
    return f"{100 * part:.1f}%" if part < 0.0995 else f"{round(100 * part)}%"


def dollars(value: float) -> str:
    """A sum of money as the pages word it. A price is to the cent, `$2.85`
    and `$10.35`; a larger sum is an estimate and is worded to three figures:
    `$74`, `$269`, `$1,340`, `$267,000`, `$2.36 million`, `$2.34 billion`."""
    if value < 19.995:
        return f"${value:.2f}"
    if value < 999.5:
        return f"${round(value):,}"
    for size, name in ((1e12, " trillion"), (1e9, " billion"), (1e6, " million")):
        if value >= 0.9995 * size:
            return f"${float(f'{value / size:.3g}'):g}{name}"
    return f"${round(float(f'{value:.3g}')):,}"


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


#: How far apart along the line two stops' words must stand to share a row,
#: as a share of the line: the words are 150px wide, and the line is 1040px
#: long or longer wherever the page draws it.
ROOM = 150 / 1040


def rows(places: list[float], chosen: list[str], room: float = ROOM) -> list[bool]:
    """Which stops' words stand a row out from the line (`True`), for stops
    at these places whose words are on these sides. Taken in order along the
    line, a stop's words go in the row next to the line when the last words
    there on its side are `room` away, and otherwise in the row beyond it,
    at the end of a rule from the stop's point. A line that runs to a far
    goal holds the measured stops in a short stretch of itself, and this is
    what keeps their words apart there."""
    last = {(side, out): -math.inf for side in ("above", "below") for out in (False, True)}
    far = [False] * len(places)
    for index in sorted(range(len(places)), key=lambda i: places[i]):
        side = chosen[index]
        far[index] = places[index] - last[(side, False)] < room
        last[(side, far[index])] = places[index]
    return far


def scale(multiplier: float) -> str:
    """A mark of the line's scale as it is worded: a multiplier, and from a
    million up in words, `1 million x` and `1 billion x`, so that ten marks
    fit along one line and none is read by counting its noughts."""
    for size, name in ((1e12, "trillion"), (1e9, "billion"), (1e6, "million")):
        if multiplier >= size:
            return f"{multiplier / size:g} {name} x"
    return times(multiplier)


def ticks(goal: float) -> list[dict]:
    """The marks along the line: every tenfold from 1x up to the largest
    goal, and that goal itself at its end."""
    marks = [10**power for power in range(math.ceil(math.log10(goal)))] + [goal]
    return [{"times": scale(mark), "at": round(math.log(mark) / math.log(goal), 4)} for mark in marks]


def marked(ledger: dict) -> list[dict]:
    """The ledger's goals, smallest first: each the `key` the pages name it by
    and its multiplier (`times`). The line ends at the last."""
    return sorted(({"key": goal["key"], "times": goal["times"]} for goal in ledger["goals"]), key=lambda goal: goal["times"])


#: The hours a rented machine is paid for in a year of playing without a stop.
HOURS_A_YEAR = 24 * 365


def rented(source: pathlib.Path, ledger: dict, measured: list[tuple[dict, list[dict]]], base: float) -> dict | None:
    """What renting the rest of the way to each goal would cost, for the
    stops as read (`measured`) and the baseline's seconds (`base`). An
    estimate, not a measurement: `None` when the ledger has no `cloud` entry,
    or the revision no list of processors to read the measured processor's
    score from.

    The newest entry of the ledger's `cloud` names a machine to rent. One
    game on one of its cores is the one-core reading of the stop it plays
    (`of`), times the measured processor's single thread score, over its own
    (`one_game`, as the processors' table works a processor out), and every
    core is taken to play as fast as one, so the machine is its cores times
    that (`times`, with the games it plays in a minute and an hour and what a
    million of them cost). What is short of a goal is the goal less the
    fastest stop's multiplier (`have`), and `rows` has for each goal the
    whole machines that makes (`machines`) and their price for an hour and
    for a year without a stop, at the list price and, where the entry has
    one, at the spot price. Every figure has the sum that gives it beside
    it. `by` is who made the estimate, and `from` its own account.
    """
    try:
        estimates = sorted(ledger.get("cloud") or [], key=lambda entry: entry["taken"])
        if not estimates:
            return None
        entry = estimates[-1]
        one = next(taken[-1] for stop, taken in measured if stop["key"] == entry["of"])
        listed = json.loads((source / PROCESSORS).read_text(encoding="utf-8"))
        own = next(chip for chip in listed["processors"] if chip["key"] == listed["measured"])
        single = sorted(own["scores"], key=lambda reading: reading["read"])[-1]["single_thread"]
        cores, price, spot = entry["cores"], entry["dollars_an_hour"], entry.get("spot_dollars_an_hour")
        seconds = one["seconds"] * single / entry["single_thread"]
        each = cores * base / seconds
        fastest, reached = min(measured, key=lambda pair: pair[1][-1]["seconds"])
        have = base / reached[-1]["seconds"]
        rows = []
        for goal in marked(ledger):
            rest = max(0.0, goal["times"] - have)
            machines = math.ceil(rest / each - 1e-9)
            row = {
                "key": goal["key"],
                "times": times(goal["times"]),
                "rest": times(rest),
                "rest_sum": f"{figure(goal['times'])} − {figure(have)}",
                "machines": f"{machines:,}",
                "machines_sum": f"{figure(rest)} ÷ {figure(each)} = {rest / each:,.1f}, in whole machines",
                "cores": f"{machines * cores:,}",
                "an_hour": dollars(machines * price),
                "an_hour_sum": f"{machines:,} × {dollars(price)}",
                "a_year": dollars(machines * price * HOURS_A_YEAR),
            }
            if spot:
                row.update(
                    spot_an_hour=dollars(machines * spot),
                    spot_an_hour_sum=f"{machines:,} × {dollars(spot)}",
                    spot_a_year=dollars(machines * spot * HOURS_A_YEAR),
                )
            rows.append(row)
        shown = {
            "by": str(entry["by"]),
            "taken": entry["taken"][:10],
            "machine": str(entry["machine"]),
            "cores": cores,
            "of": entry["of"],
            "have": times(have),
            "have_stop": fastest["key"],
            "price": dollars(price),
            "seconds": round(seconds, 3),
            "one_game": clock(seconds),
            "one_game_sum": f"{figure(one['seconds'])} s × {figure(single)} ÷ {figure(entry['single_thread'])}",
            "multiplier": round(each, 2),
            "times": times(each),
            "times_sum": f"{cores} cores × {figure(base)} s ÷ {figure(seconds)} s",
            "a_minute": rate(cores * 60 / seconds),
            "an_hour": rate(cores * 3600 / seconds),
            "a_million": dollars(price * seconds / (cores * 3600) * 1000000),
            "a_year": dollars(price * HOURS_A_YEAR),
            "hours_a_year": f"{HOURS_A_YEAR:,}",
            "from": str(entry.get("from") or ""),
            "rows": rows,
        }
        if spot:
            shown.update(
                spot=dollars(spot),
                spot_a_million=dollars(spot * seconds / (cores * 3600) * 1000000),
                spot_a_year=dollars(spot * HOURS_A_YEAR),
            )
        return shown
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError, StopIteration):
        return None


def projected(ledger: dict, measured: list[tuple[dict, list[dict]]], base: float, goal: float) -> list[dict]:
    """The stops past what was measured, in the ledger's order, for the stops
    as read (`measured`), the baseline's seconds (`base`) and the multiplier
    the line ends at (`goal`). None when the ledger lists none.

    A projected stop is a measured stop's multiplier (`over` names the stop)
    times the figure of its newest estimate (`times`), and nothing has run at
    it. A row has that figure as it is worded (`by`), the multiplier it comes
    to (`times`) with the time from one finished game to the next at it
    (`time`), the sum that gives it (`sum`), its place on the line (`at`) and
    the estimate's own account (`from`). An estimate that is `or_more` is
    open at its top, and both figures end in a `+`.
    """
    newest = {stop["key"]: taken[-1] for stop, taken in measured}
    rows = []
    for stop in ledger.get("projected") or []:
        estimate = sorted(stop["estimates"], key=lambda entry: entry["taken"])[-1]
        under = base / newest[stop["over"]]["seconds"]
        multiplier = estimate["times"] * under
        more = "+" if estimate.get("or_more") else ""
        row = {
            "key": stop["key"],
            "over": stop["over"],
            "by": times(estimate["times"]) + more,
            "seconds": round(base / multiplier, 6),
            "time": clock(base / multiplier),
            "multiplier": round(multiplier, 2),
            "times": times(multiplier) + more,
            "sum": f"{figure(estimate['times'])} × {figure(under)}",
            "taken": day(estimate["taken"]),
            "at": round(min(1.0, max(0.0, math.log(multiplier) / math.log(goal))), 4),
        }
        if estimate.get("from"):
            row["from"] = estimate["from"]
        rows.append(row)
    return rows


def timeline(source: pathlib.Path) -> dict | None:
    """What the home page shows: the stops in the ledger's order, each with
    its newest reading as a time and a multiplier, its goal when it has one,
    where it started when it has moved, and its place on the line; the largest
    multiplier, which is the page's number; and the `goals`, smallest first,
    each with the time a game would take at it, its place on the line, which
    ends at the last, and what renting the rest of the way to it is
    estimated to cost (`cost`, with `cloud` saying whose estimate it is and
    of what machine at what price). A stop or a goal whose words would meet
    its neighbour's is `far`: its words stand a row out from the line (see
    `rows`). `projected` are the stops past what was measured (see
    `projected`), in a ledger that lists any: their words stand under the
    line, and none of them is the page's number. `None` when the revision
    has no ledger or one that cannot be read."""
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        marks = marked(ledger)
        goal = marks[-1]["times"]
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
        goals = [
            {
                "key": mark["key"],
                "times": times(mark["times"]),
                "time": clock(base / mark["times"]),
                "at": round(math.log(mark["times"]) / math.log(goal), 4),
            }
            for mark in marks
        ]
        ahead = projected(ledger, measured, base, goal)
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError):
        return None
    for row, side in zip(stops, sides([row["at"] for row in stops])):
        row["side"] = side
    # The goals' words stand over the line and the projected stops' under
    # it, and both take their rows with the stops' words there.
    for mark in goals:
        mark["side"] = "above"
    for row in ahead:
        row["side"] = "below"
    entries = stops + ahead + goals
    for row, far in zip(entries, rows([row["at"] for row in entries], [row["side"] for row in entries])):
        if far:
            row["far"] = True
    fastest = max(stops, key=lambda row: row["multiplier"])
    shown = {
        "speedup": fastest["times"],
        "fastest": fastest["key"],
        "reach": fastest["at"],
        "goals": goals,
        "ticks": ticks(goal),
        "stops": stops,
    }
    if "was" in fastest:
        shown["was"] = fastest["was"]
    if ahead:
        shown["projected"] = ahead
    cloud = rented(source, ledger, measured, base)
    if cloud:
        costs = {row["key"]: row["an_hour"] for row in cloud["rows"]}
        for mark in goals:
            mark["cost"] = f"about {costs[mark['key']]} an hour in the cloud"
        shown["cloud"] = {"by": cloud["by"], "machine": cloud["machine"], "price": cloud["price"], "taken": day(cloud["taken"])}
    return shown


def processors(source: pathlib.Path, measured: list[tuple[dict, list[dict]]], base: float) -> dict | None:
    """The benchmarks processor by processor, for the stops as `worked` read
    them (`measured`) and the baseline's seconds (`base`). `None` when the
    revision has no list of processors, or one that cannot be read.

    `rows` are the processors in the list's order. A row has the website's
    two scores as the newest reading gives them, with the `samples` behind
    them, the `margin` for error, the day they were `read` and the `page`;
    `games`, one cell for every one-core stop; and `every_core`, the games a
    minute with a worker on every core, with the multiplier that gives
    (`times`, and `times_sum` the sum for it). A cell's `kind` is `measured`
    when the benchmark was run on that processor and `theory` when it is
    worked out from the scores, and then its `sum` shows how. A row's own
    `kind` is `measured` when the benchmarks were run on it, and its `taken`
    is the oldest day among the readings its figures rest on. `site` is the
    website the scores are read from, with the newest day any was read.

    The processor the one-core readings were taken on is the list's
    `measured`. A processor's every-core figure is measured when the
    benchmark's ledger has a run on its `chip`: the record among those runs.
    """
    try:
        ledger = json.loads((source / PROCESSORS).read_text(encoding="utf-8"))
        site, listed, own = ledger["scores"], ledger["processors"], ledger["measured"]
        newest = {entry["key"]: sorted(entry["scores"], key=lambda reading: reading["read"])[-1] for entry in listed}
        anchor = next(entry for entry in listed if entry["key"] == own)
        single, multi = newest[own]["single_thread"], newest[own]["multithread"]
        hours = f"{figure(base / 3600)} h"
        one_core = [(stop, taken[-1]) for stop, taken in measured if stop.get("cores") == 1]
        # The stop that plays a one-core stop's game on every core, and the
        # runs of the ledger it follows: a processor's record is the record
        # among the runs on its own chip.
        runner = next((stop for stop, _ in measured if "follows" in stop and stop.get("of")), None)
        runs = json.loads((source / runner["follows"]).read_text(encoding="utf-8"))["runs"] if runner else []

        def record(entry: dict) -> dict | None:
            chip = entry.get("chip")
            mine = [run for run in runs if chip and (run.get("machine") or {}).get("chip") == chip]
            took = records(mine, FOLLOWED[runner["follows"]]) if mine else []
            return took[-1] if took else None

        held = record(anchor)
        address = str(site["address"])
        rows = []
        for entry in listed:
            scores, ours = newest[entry["key"]], entry["key"] == own
            row = {
                "key": entry["key"],
                "name": entry["name"],
                "cores": entry["cores"],
                "threads": entry["threads"],
                "single_thread": f"{scores['single_thread']:,}",
                "multithread": f"{scores['multithread']:,}",
                "samples": scores["samples"],
                "read": scores["read"],
            }
            if scores.get("margin"):
                row["margin"] = scores["margin"]
            # A score links the page it was read on, when that page is the
            # website's own.
            if address.startswith("https://") and str(scores.get("page") or "").startswith(address):
                row["page"] = scores["page"]
            if entry.get("fleet"):
                row["fleet"] = entry["fleet"]
            days, games = [], []
            for stop, reading in one_core:
                cell = {"key": stop["key"], "kind": "measured" if ours else "theory", "taken": reading["taken"][:10]}
                seconds = reading["seconds"]
                if not ours:
                    cell["sum"] = f"{figure(seconds)} s × {figure(single)} ÷ {figure(scores['single_thread'])}"
                    seconds = seconds * single / scores["single_thread"]
                cell.update(seconds=round(seconds, 3), time=clock(seconds))
                games.append(cell)
                days.append(cell["taken"])
            row["games"] = games
            run = record(entry) if runner else None
            every = None
            if run:
                every = {"kind": "measured", "a_minute": 60 / run["seconds"], "taken": run["taken"][:10]}
            elif held and not ours:
                every = {
                    "kind": "theory",
                    "a_minute": 60 / held["seconds"] * scores["multithread"] / multi,
                    "taken": held["taken"][:10],
                    "sum": f"{figure(60 / held['seconds'])} games a minute × {figure(scores['multithread'])} ÷ {figure(multi)}",
                }
            if every:
                a_minute = every["a_minute"]
                multiplier = a_minute * base / 60
                every.update(of=runner["of"], games_a_minute=round(a_minute, 3), a_minute=rate(a_minute), an_hour=rate(60 * a_minute))
                row.update(
                    every_core=every,
                    multiplier=round(multiplier, 2),
                    times=times(multiplier),
                    times_sum=f"{figure(a_minute)} games a minute × 60 × {hours}",
                )
                days.append(every["taken"])
            row["kind"] = "measured" if run or (ours and games) else "theory"
            if days:
                row["taken"] = min(days)
            rows.append(row)
        return {
            "site": {
                "name": site["site"],
                "address": address,
                "single_thread": site["single_thread"],
                "multithread": site["multithread"],
                "from": str(site.get("from") or ""),
                "read": max(row["read"] for row in rows),
            },
            "rows": rows,
        }
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError, StopIteration):
        return None


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
    (`kind: "goal"`): each measured run's own, and the goals the home page's
    line is marked with (`whole`, under the goal's own key). `goals` are
    those goals by themselves, smallest first. `answer` is the page's opening
    sentence in figures: the self-play stop one game at a time, on every
    core as measured, and the highest theory there is for it. `processors`
    is the same benchmarks processor by processor (see `processors`), in a
    revision that lists them, and `cloud` what renting the rest of the way to
    each goal is estimated to cost (see `rented`), in a revision with such an
    estimate.
    """
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        marks = marked(ledger)
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
            # a run's own, then the goals of the whole line.
            best = max(row["multiplier"] for row in throughput if row["kind"] == "measured")
            aims = [
                (stop["key"], stop["goal_seconds"], f"{figure(base)} s ÷ {figure(stop['goal_seconds'])} s", base / run["seconds"], False)
                for stop, run in runs
                if "goal_seconds" in stop
            ]
            aims += [(mark["key"], base / mark["times"], f"a game every {clock(base / mark['times'])}", best, True) for mark in marks]
            for key, seconds, said, reached, whole in aims:
                row = {
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
                if whole:
                    row["whole"] = True
                throughput.append(row)

        shown = {
            "base": {"time": clock(base), "seconds": figure(base), "an_hour": rate(3600 / base)},
            "games": games,
            "throughput": throughput,
            "goals": [
                {
                    "key": mark["key"],
                    "times": times(mark["times"]),
                    "words": scale(mark["times"]),
                    "time": clock(base / mark["times"]),
                    "a_minute": rate(mark["times"] * 60 / base),
                    "an_hour": rate(mark["times"] * 3600 / base),
                }
                for mark in marks
            ],
        }
        if machine:
            shown["machine"] = machine
        across = processors(source, measured, base)
        if across:
            shown["processors"] = across
        cloud = rented(source, ledger, measured, base)
        if cloud:
            shown["cloud"] = cloud
            costs = {row["key"]: row["an_hour"] for row in cloud["rows"]}
            for mark in shown["goals"]:
                mark["cost"] = costs[mark["key"]]
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
