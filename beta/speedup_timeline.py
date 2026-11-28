#!/usr/bin/env python3
"""The speed timeline of CIVVIS Performance: five ways one game of
Civilization VI gets played, and how many times faster each is than a person
playing it by hand. It was the home page's, over the videos, until 2026-11-24
(operator: "move the civvis performance timeline thing from home/ to the
civvis performance section. middle of the page or so"); it is now a part of
`beta/performance.html`, under the tables this module works out for it.

    speedup_timeline.py <source_tree>            print the timeline as JSON
    speedup_timeline.py --worked <source_tree>   print the worked tables as JSON
    speedup_timeline.py --history <source_tree>  print each stop over time as JSON

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
reading for every run that set a record, so a new record there moves the
page with no second edit.

A stop's multiplier is the first stop's seconds over its own, and the page's
one number is the largest of them. Nothing derived is written in the ledger:
the multipliers, the texts and each stop's place on the line are worked out
here when the site is published (`beta/performance_page.py` writes the result
into the page's slot as the record's `timeline`), so the page cannot say a
figure the ledger does not.

The line is a scale of multipliers by tenfolds, 1x at its right end and its
end at its left: a stop stands at `log(multiplier) / log(end)` of the way
across, which is its `at`. The ledger's `goals` are marked on it, each at its
own place (operator, 2026-11-22, of the second: "Addd 1 billion x to the
scaling scale as well"). The end is the largest goal, or the ledger's
`line_to` where that is further out (operator, 2026-11-24: "maybe extend
timeline out to 10,000,000x"): the line then runs on past its last goal, which
keeps its place, and a stop past that goal stands on the line (see `end`).

Past the fastest stop the line has stops nothing has played yet
(`projected`; operator, 2026-11-22: "add to out speedup timeline: 10x
theoretical scale up from local hardware ... hardware scaleup is to the left
of m5 max. potential distributed RL fleet: up to 1000+x"). Each is a measured
stop's multiplier (`over`) times a figure of the operator's, so it moves when
that stop does, and it is never the page's number: that is the fastest stop
that was measured.

Where the games play many at once, a time is amortized: the time from one
finished game to the next, which no single game takes (operator, 2026-11-24:
"starting left of CIVVIS self-play (but not including that), we should note
that estimates are amortized games"). The ledger says which stops and goals
those are (`amortized`), and a projected stop is one when the stop it
multiplies is. Each has its games per second beside its time and leads with
them. Past a game a second that was so from the first (operator, the same
day: "on the left side of the timeline, at more than 1 game / second, we
should also provide the games per second as the standard measure and (time
per game amoritized as the second measure)"), and under it since he asked it
of the M5 Max's stop, the one amortized entry under a game a second, later
that day: "lest have m5 max blurb list amortiized games per second as the
primary unit. as this is really amortiization at play". So such a stop's goal
is worded in games a second as well.

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
import sys

import control_times
import self_play_hours

#: The stops and their readings, in the source tree.
LEDGER = pathlib.Path("data/performance/speedup_timeline.json")

#: The processors the Performance page lists the benchmarks across, with the
#: website's two scores for each, in the source tree.
PROCESSORS = pathlib.Path("data/performance/processors.json")

#: The self-play tournament's finished batches, a snapshot in the source
#: tree (`beta/performance_page.py --self-play` rebuilds it).
SELF_PLAY = pathlib.Path("data/performance/self_play_times.json")

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


def per_second(games: float) -> str:
    """Games a second as the line words them: `0.41 games/s`, `1 game/s`,
    `4.1 games/s`, `56 games/s`, `1,200 games/s`."""
    if games < 0.995:
        return f"{games:.2f} games/s"
    if games < 9.95:
        text = f"{games:.1f}"
        return "1 game/s" if text == "1.0" else f"{text[:-2] if text.endswith('.0') else text} games/s"
    return f"{round(games):,} games/s"


def amortize(row: dict, seconds: float, more: str = "") -> None:
    """Mark a row of the line as amortized: its games play many at once, so
    its `time` is the time from one finished game to the next (`seconds`)
    and not what a game takes. It gains its games a second (`rate`, with
    `more` after the figure when the row is open at its top), and `lead`
    says `rate`: the page leads with the games a second and gives the time
    per game as the second measure, at any rate. Until the operator asked
    it of the M5 Max's stop too (0.41 games a second), a row under one game
    a second led with its time."""
    figure, unit = per_second(1 / seconds).split(" ", 1)
    row["amortized"] = True
    row["rate"] = f"{figure}{more} {unit}"
    row["lead"] = "rate"


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


#: The benchmarks that keep a record of their own, under the name a stop
#: reads one by (`benchmark`): each gives the record for a source tree, or
#: `None` in a clone that does not hold it.
BENCHMARKS = {"control": lambda source: control_times.control(source)}


def benchmark(source: pathlib.Path, stop: dict) -> dict | None:
    """The record of the benchmark a stop reads (`"benchmark": "control"` is
    the computer control games of `beta/control_times.py`), or `None` for a
    stop without one, a clone without the record, or a record without a
    week of games."""
    try:
        record = BENCHMARKS[stop["benchmark"]](source)
        return record if record and record["week"]["p"][1] > 0 else None
    except (OSError, ValueError, KeyError, TypeError, IndexError):
        return None


def benchmarked(source: pathlib.Path, stop: dict) -> dict | None:
    """A benchmark's record as a reading: the middle game of its last seven
    days, taken the day of its newest game. The benchmark is run end to end
    on every game, so its record is newer than any reading written down;
    the readings in the ledger are what a clone without the record shows."""
    record = benchmark(source, stop)
    if record is None:
        return None
    week = record["week"]
    low, middle, high = week["p"]
    return {
        "taken": week["to"],
        "seconds": middle,
        "from": f"The computer control benchmark (`beta/control_times.py`, docs/CONTROL_GAME_TIME.md): the middle game of the {week['games']:,} games"
        f" played to a victory in the seven days to {week['to']}, every level, each timed end to end. A tenth took under {clock(low)} and a tenth over {clock(high)}.",
    }


def readings(source: pathlib.Path, stop: dict) -> list[dict]:
    """A stop's readings, oldest first: its own, or the records of the ledger
    it follows. A stop that reads a benchmark has that benchmark's record as
    its newest reading, where the clone holds the record and it is no older
    than the stop's own."""
    followed = stop.get("follows")
    if followed is None:
        own = sorted(stop["readings"], key=lambda reading: reading["taken"])
        newest = benchmarked(source, stop)
        return own + [newest] if newest and newest["taken"] >= own[-1]["taken"] else own
    runs = json.loads((source / followed).read_text(encoding="utf-8"))["runs"]
    return records(runs, FOLLOWED[followed])


def began(taken: list[dict]) -> dict:
    """The reading a stop is said to have started from (`was`), among its
    readings oldest first: the first that was timed. A figure that was worked
    out is not somewhere the stop stood, so a timed reading that takes its
    place did not move from it. A stop with worked-out readings alone starts
    from its first."""
    newest = taken[-1]
    if newest.get("worked"):
        return taken[0]
    return next(reading for reading in taken if not reading.get("worked"))


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
#: as a share of the line: the words are 150px wide, and the line is 1000px
#: long or longer wherever the page draws it. That is the inside of a panel
#: of CIVVIS Performance from 1120px of window, where the line is drawn:
#: 1026px, or 1009px with a scrollbar's 17px taken. It was 1040px while the
#: line stood on Home, in no panel.
ROOM = 150 / 1000


#: A stop this near an end of the line has its words begin or end at its
#: point, where the others' are centred on theirs (the page's `is-end`
#: under this share of the line and `is-start` past one less it).
EDGE = 0.08


def middle(place: float, room: float = ROOM) -> float:
    """Where along the line a stop's words are centred, for a stop at this
    place: on its point, or half their width inside it at either end of the
    line, where they run inward from the point."""
    if place < EDGE:
        return place + room / 2
    if place > 1 - EDGE:
        return place - room / 2
    return place


def rows(places: list[float], chosen: list[str], room: float = ROOM) -> list[bool]:
    """Which stops' words stand a row out from the line (`True`), for stops
    at these places whose words are on these sides. Taken in order along the
    line, a stop's words go in the row next to the line when the last words
    there on its side are `room` away, and otherwise in the row beyond it,
    at the end of a rule from the stop's point. A line that runs to a far
    goal holds the measured stops in a short stretch of itself, and this is
    what keeps their words apart there. The distance is between the middles
    of the words (`middle`), which at an end of the line is not the point."""
    last = {(side, out): -math.inf for side in ("above", "below") for out in (False, True)}
    far = [False] * len(places)
    for index in sorted(range(len(places)), key=lambda i: places[i]):
        side, at = chosen[index], middle(places[index], room)
        far[index] = at - last[(side, False)] < room
        last[(side, far[index])] = at
    return far


def scale(multiplier: float) -> str:
    """A mark of the line's scale as it is worded: a multiplier, and from a
    million up in words, `1 million x` and `1 billion x`, so that ten marks
    fit along one line and none is read by counting its noughts."""
    for size, name in ((1e12, "trillion"), (1e9, "billion"), (1e6, "million")):
        if multiplier >= size:
            return f"{multiplier / size:g} {name} x"
    return times(multiplier)


def where_played(stop: dict, seconds: float) -> dict:
    """What a stop of the line is played on, as its words on the line say it
    (`where`), and how that was established (`where_from`): the ledger's
    `runs` (`native app`, Civilization VI itself; `native`, the CIVVIS
    program built for the machine), and for a stop with a `browser` entry,
    the same games in the browser, the reading times the measured `ratio`.
    Empty for a stop that says neither."""
    runs, browser = stop.get("runs"), stop.get("browser")
    if not runs:
        return {}
    out = {"where": runs}
    if isinstance(browser, dict) and float(browser.get("ratio") or 0) > 0:
        ratio = float(browser["ratio"])
        out["where"] = f"{runs} · {clock(seconds * ratio)} in the browser"
        out["browser_seconds"] = round(seconds * ratio, 3)
        out["where_from"] = (f"In the browser (the site's WebAssembly build) the same games take {ratio:g} times as long as on the "
                             f"native program: {browser.get('from', '')}").strip()
    return out


def ticks(goal: float, base: float | None = None) -> list[dict]:
    """The marks along the line: every tenfold from 1x up to the multiplier
    the line ends at (`goal`), and that multiplier itself at its end. With
    the time a game by hand takes (`base`, seconds), each mark also says
    what it comes to, for the line's two scales (operator 2026-11-27: "we
    can also have a time / game measure along the top and a games / time
    measure along the bottom"): the time one game takes at that pace
    (`per_game`) and how many games that is in an hour, a minute or a
    second (`per_time`)."""
    marks = [10**power for power in range(math.ceil(math.log10(goal)))] + [goal]
    out = []
    for mark in marks:
        tick = {"times": scale(mark), "at": round(math.log(mark) / math.log(goal), 4)}
        if base:
            tick["per_game"] = f"{fine_clock(base / mark)}/game"
            tick["per_time"] = per_time(mark / base)
        out.append(tick)
    return out


def fine_clock(seconds: float) -> str:
    """A time on the line's top scale: as `clock` words it, but a tenth of a
    millisecond under ten, so that a tenfold scale reads `1.8 ms`, `18 ms`,
    `180 ms`, and not `2 ms` once."""
    if 0.0009995 <= seconds < 0.00995:
        return f"{seconds * 1000:.1f} ms".replace(".0 ", " ")
    return clock(seconds)


def per_time(games: float) -> str:
    """Games in a stretch of time, as the line's bottom scale words them: an
    hour, a minute or a second, the longest that leaves less than a hundred
    or so, and at least one: `0.2 games/h`, `20 games/h`, `3.3 games/min`,
    `33.3 games/min`, `5.6 games/s`, `556 games/s` (`games` a second)."""
    if games * 60 < 1:
        figure, unit = rate(games * 3600), "h"
    elif games < 1:
        figure, unit = rate(games * 60), "min"
    else:
        return per_second(games)
    return f"{figure} {'game' if figure == '1' else 'games'}/{unit}"


def marked(ledger: dict) -> list[dict]:
    """The ledger's goals, smallest first: each the `key` the pages name it by
    and its multiplier (`times`), and `amortized` on one that is reached by
    many games at once. The line ends at the last, or runs on past it (see
    `end`)."""
    kept = ("key", "times", "amortized")
    return sorted(({name: goal[name] for name in kept if name in goal} for goal in ledger["goals"]), key=lambda goal: goal["times"])


def end(ledger: dict) -> float:
    """The multiplier the line runs out to: the largest goal, or the
    ledger's `line_to` where that is further out. A line that runs on past
    its last goal has room for the stops past that goal, which otherwise
    stand at its end as running on from it. `line_to` is no goal: nothing is
    aimed at it and nothing is costed to it."""
    last = marked(ledger)[-1]["times"]
    return max(last, ledger.get("line_to") or last)


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
    (`one_game`, as the processors' table works a processor out), and the
    machine is its cores times that (`times`, with the games it plays in a
    minute and an hour and what a million of them cost). An entry whose
    `scaling` is `measured` does not take every core to play as fast as one:
    it takes the share of that our own every-core run reached on the machine
    it was played on (`scaling`, with `scaling_sum` the sum for it), because
    that run is the only evidence there is of what busy cores deliver. An
    entry without it takes the whole. What is short of a goal is the goal less the
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
        # The share of cores-times-one-core that busy cores deliver: the
        # whole, or what the every-core run of the same game reached.
        part, scaling = 1.0, None
        if entry.get("scaling") == "measured":
            run = next(taken[-1] for stop, taken in measured if "follows" in stop and stop.get("of") == entry["of"])
            part = min(1.0, (one["seconds"] / run["seconds"]) / run["cores"])
            scaling = {
                "scaling": share(part),
                "scaling_sum": f"{figure(60 / run['seconds'])} games a minute ÷ ({run['cores']} cores × 60 s ÷ {figure(one['seconds'])} s)",
            }
        a_second = part * cores / seconds
        each = a_second * base
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
            "times_sum": f"{cores} cores × {figure(base)} s ÷ {figure(seconds)} s" + (f" × {figure(part)}" if scaling else ""),
            "a_minute": rate(a_second * 60),
            "an_hour": rate(a_second * 3600),
            "a_million": dollars(price / (a_second * 3600) * 1000000),
            "a_year": dollars(price * HOURS_A_YEAR),
            "hours_a_year": f"{HOURS_A_YEAR:,}",
            "from": str(entry.get("from") or ""),
            "rows": rows,
        }
        if scaling:
            shown.update(scaling)
        if spot:
            shown.update(
                spot=dollars(spot),
                spot_a_million=dollars(spot / (a_second * 3600) * 1000000),
                spot_a_year=dollars(spot * HOURS_A_YEAR),
            )
        return shown
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError, StopIteration):
        return None


def projected(ledger: dict, measured: list[tuple[dict, list[dict]]], base: float, goal: float) -> list[dict]:
    """The stops past what was measured, in the ledger's order, for the stops
    as read (`measured`), the baseline's seconds (`base`) and the multiplier
    the line ends at (`goal`; see `end`). None when the ledger lists none.

    A projected stop is a measured stop's multiplier (`over` names the stop)
    times the figure of its newest estimate (`times`), and nothing has run at
    it. `over` may also name a projected stop listed before it, and then it
    is that stop's multiplier it multiplies: a guess that stands on a guess. A row has that figure as it is worded (`by`), the multiplier it comes
    to (`times`) with the time from one finished game to the next at it
    (`time`), the sum that gives it (`sum`), its place on the line (`at`) and
    the estimate's own account (`from`). An estimate that is `or_more` is
    open at its top, and both figures end in a `+`. A row over a stop that
    is `amortized` is amortized too, and so is one the ledger says is (see
    `amortize`).
    """
    reached = {stop["key"]: base / taken[-1]["seconds"] for stop, taken in measured}
    shared = {stop["key"] for stop, _ in measured if stop.get("amortized")}
    rows = []
    for stop in ledger.get("projected") or []:
        estimate = sorted(stop["estimates"], key=lambda entry: entry["taken"])[-1]
        under = reached[stop["over"]]
        multiplier = estimate["times"] * under
        reached[stop["key"]] = multiplier
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
        # So many times a stop whose games play many at once is that many
        # more of them at once.
        if stop.get("amortized") or stop["over"] in shared:
            shared.add(stop["key"])
            amortize(row, base / multiplier, more)
        # Past where the line ends: it stands at the line's end and the
        # page says it runs on from there.
        if multiplier > goal:
            row["past"] = True
        rows.append(row)
    return rows


def gap(ledger: dict, measured: list[tuple[dict, list[dict]]], base: float, goal: float) -> dict | None:
    """Why one stop of the line is so many times faster than another, for
    the stops as read (`measured`), the baseline's seconds (`base`) and the
    multiplier the line ends at (`goal`): the ledger's `gap` names the two
    (`slow` and `fast`; operator, 2026-11-24: "what is the main reason civvis
    is so much faster than firaxis civ 6? please try to explain that gap in
    the chart/timeline"). `None` for a ledger that names none, a stop it
    names that is not on the line, or two that are the wrong way round.

    What it gives is how many times faster (`times`) and where on the line
    the stretch between the two runs (`from` and `to`, the two stops' own
    places), and then as much as the readings say of where the slower
    game's time goes:

    - `bars`, the two games drawn to one scale, the slower first, each with
      its time and the `share` of the slower game's time it is. The slower
      game is in parts where its newest reading has `parts` (its seconds as
      `setup`, `play` and `ending`): the time outside its turns (`outside`),
      and its play. The play is split where the ledger's `gap` has `threads`
      (what `tools/civ6_thread_time.py` read of that game's process as it
      played) into the time its game thread was on a processor (`works`)
      and the time it was not (`waits`), by that thread's share of the play
      it was read over. Each part has the factor of the gap it accounts for
      (`times`): the time outside the turns is taken away, then the
      waiting, and the work that is left is so many times the faster game's
      whole time, so the factors multiply to the gap. The faster game is
      one part, at work: its reading's `cpu_seconds` are all but all of it.
    - `facts`, the same two games as pairs of figures: what one turn takes
      (`turn`, where both readings have their `turns`), and where there are
      `threads` and the faster reading has its `cpu_seconds`, how many
      processors each keeps busy as it plays (`cores`) and what share of
      the play each one's game thread is on a processor (`busy`).
    - `counted`, the account of those figures a visitor can open, which ends
      on the few words the ledger keeps for the page of how the threads were
      read (`note`; the whole account is the ledger's `from`).
    """
    asked = ledger.get("gap")
    if not asked:
        return None
    shown = {stop["key"]: taken[-1] for stop, taken in measured if stop.get("line") is not False}
    slow, fast = shown.get(asked.get("slow")), shown.get(asked.get("fast"))
    if not slow or not fast or not fast["seconds"] < slow["seconds"]:
        return None

    def place(seconds: float) -> float:
        return round(min(1.0, max(0.0, math.log(base / seconds) / math.log(goal))), 4)

    whole, quick = slow["seconds"], fast["seconds"]
    row = {"slow": asked["slow"], "fast": asked["fast"], "times": times(whole / quick), "from": place(whole), "to": place(quick)}
    parts, threads = slow.get("parts"), asked.get("threads")
    if not parts:
        return row
    play = parts["play"]
    busy = threads["game_seconds"] / threads["play_seconds"] if threads else None
    # The slower game's time, a part taken away at a time until the faster
    # game's is left: each part's seconds, and the seconds left once it is
    # gone.
    steps = [("outside", whole - play, play)]
    steps += [("waits", play * (1 - busy), play * busy), ("works", play * busy, quick)] if threads else [("play", play, quick)]
    cut, left = [], whole
    for key, seconds, after in steps:
        cut.append({"key": key, "time": clock(seconds), "share": round(seconds / whole, 4), "times": times(left / after)})
        left = after
    row["bars"] = [
        # From the bars' shared start, the work first: the faster game's one
        # part stands under it and is held against it.
        {"key": asked["slow"], "time": clock(whole), "share": 1, "parts": cut[::-1]},
        {"key": asked["fast"], "time": clock(quick), "share": round(quick / whole, 4), "parts": [{"key": cut[-1]["key"], "time": clock(quick), "share": round(quick / whole, 4)}]},
    ]
    counted = [
        f"The slower game, {clock(whole)}: {clock(parts['setup'])} from launching it to its first turn, {clock(play)} of play and {clock(parts['ending'])} of ending."
    ]
    facts = []
    if slow.get("turns") and fast.get("turns"):
        one, other = play / slow["turns"], quick / fast["turns"]
        facts.append({"key": "turn", "slow": clock(one), "fast": clock(other), "times": times(one / other)})
        counted.append(f"Its play was {figure(slow['turns'])} turns, and the faster game's {clock(quick)} were {figure(fast['turns'])}.")
    if threads:
        cores = threads["cpu_seconds"] / threads["play_seconds"]
        if fast.get("cpu_seconds"):
            alone = fast["cpu_seconds"] / quick
            facts.append({"key": "cores", "slow": f"{cores:.1f}", "fast": f"{alone:.1f}".replace(".0", "")})
            facts.append({"key": "busy", "slow": share(busy), "fast": share(alone)})
        counted.append(
            f"Its threads were read over {clock(threads['play_seconds'])} of a game's play: the game thread was on a processor for {share(busy)} of that,"
            f" and the whole process for {cores:.1f} processors' worth."
        )
        if threads.get("note"):
            counted.append(threads["note"])
    if facts:
        row["facts"] = facts
    row["counted"] = " ".join(counted)
    return row


def timeline(source: pathlib.Path) -> dict | None:
    """What the page's line shows: the stops in the ledger's order, each with
    its newest reading as a time and a multiplier, its goal when it has one,
    where it started when it has moved, and its place on the line; the largest
    multiplier, which is the page's number; and the `goals`, smallest first,
    each with the time a game would take at it, its place on the line, which
    ends at the last or runs on past it to the ledger's `line_to` (see
    `end`), and what renting the rest of the way to it is
    estimated to cost (`cost`, with `cloud` saying whose estimate it is and
    of what machine at what price). A stop or a goal whose words would meet
    its neighbour's is `far`: its words stand a row out from the line (see
    `rows`). `projected` are the stops past what was measured (see
    `projected`), in a ledger that lists any: none of them is the page's
    number. Every entry has the `side` of the line its words stand on,
    `above` or `below`, taken in turn along the line (see `sides`). A stop, a goal or a
    projected stop whose games play many at once is `amortized`, with its
    games a second (`rate`), which it leads with (`lead`; see `amortize`),
    and its goal, when it has one, is worded in games a second too. `None` when the revision has no ledger or one
    that cannot be read."""
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        marks = marked(ledger)
        goal = end(ledger)
        measured = [(stop, readings(source, stop)) for stop in ledger["stops"]]
        base = measured[0][1][-1]["seconds"]
        stops = []
        for stop, taken in measured:
            # A stop kept for the Performance page's tables alone.
            if stop.get("line") is False:
                continue
            now, first = taken[-1], began(taken)
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
            # What the stop is played on (operator 2026-11-27: "note that
            # firaxis (native app) and civvis (browser)", "if thats true",
            # "if we have different speeds for browser and native speeds too
            # we should note that"): Civilization VI is a native app, and
            # CIVVIS's own games are timed on the native program, which the
            # browser's WebAssembly build plays more slowly; where the
            # ledger has measured by how much, the browser's time is given
            # too, worked out from this reading.
            row.update(where_played(stop, now["seconds"]))
            # A reading worked out from other measurements, not timed itself.
            if now.get("worked"):
                row["worked"] = True
            # Many games at once: the time is from one finished game to
            # the next.
            if stop.get("amortized"):
                amortize(row, now["seconds"])
            if "goal_seconds" in stop:
                # In the measure the stop leads with: games a second where
                # the games play many at once, and a game's time elsewhere.
                aim = per_second(1 / stop["goal_seconds"]) if stop.get("amortized") else clock(stop["goal_seconds"])
                row["goal"] = f"goal {aim}, {times(base / stop['goal_seconds'])}"
            if times(base / first["seconds"]) != row["times"]:
                row["was"] = f"{times(base / first['seconds'])} on {day(first['taken'])}"
            stops.append(row)
        goals = [
            {
                "key": mark["key"],
                "times": times(mark["times"]),
                "multiplier": mark["times"],
                "time": clock(base / mark["times"]),
                "at": round(math.log(mark["times"]) / math.log(goal), 4),
            }
            for mark in marks
        ]
        for row, mark in zip(goals, marks):
            if mark.get("amortized"):
                amortize(row, base / mark["times"])
        ahead = projected(ledger, measured, base, goal)
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError):
        return None
    # Every entry of the line takes its side with the others, a stop, a
    # projected stop or a goal: the line holds more of them than one side
    # has room for, and two that stand close together go one over the line
    # and one under it (see `sides`).
    entries = stops + ahead + goals
    for row, side in zip(entries, sides([row["at"] for row in entries])):
        row["side"] = side
    for row, far in zip(entries, rows([row["at"] for row in entries], [row["side"] for row in entries])):
        if far:
            row["far"] = True
    fastest = max(stops, key=lambda row: row["multiplier"])
    shown = {
        "speedup": fastest["times"],
        "fastest": fastest["key"],
        "reach": fastest["at"],
        "goals": goals,
        "ticks": ticks(goal, base),
        "stops": stops,
    }
    if "was" in fastest:
        shown["was"] = fastest["was"]
    if ahead:
        shown["projected"] = ahead
    # Why one stop is so many times faster than another, where the ledger
    # asks for it and the two are on the line.
    try:
        between = gap(ledger, measured, base, goal)
    except (KeyError, TypeError, ValueError, ZeroDivisionError):
        between = None
    if between:
        shown["gap"] = between
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
    (`kind: "goal"`): each measured run's own, and the goals the page's
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
            now, first = taken[-1], began(taken)
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
            if now.get("worked"):
                row["worked"] = True
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


def spoken(days: float) -> str:
    """A length of time in days as the page words it: `2 days`, `6 weeks`."""
    if days < 13.5:
        whole = max(1, round(days))
        return f"{whole} day" + ("" if whole == 1 else "s")
    whole = round(days / 7)
    return f"{whole} weeks"


def pace(points: list[dict]) -> dict:
    """How fast a run of readings has moved, from its first to its last
    (`points` oldest first, each a day `taken` and the `seconds` of one
    game). `readings` is how many there are. With two or more on different
    days: the `days` between the first and the last, how many times faster
    the last is than the first (`factor`, and `times` as it is worded; under
    1 when it has slowed), `over` the span in words, and from a week's span
    up `halves`, the days in which a game's time halves at that pace
    (`doubles` when it has slowed). A pace within two parts in a hundred is
    `steady`, and one that has slowed has `slower`, how many times slower."""
    shown: dict = {"readings": len(points)}
    if len(points) < 2:
        return shown
    first, last = points[0], points[-1]
    days = (_day(last["taken"]) - _day(first["taken"])).days
    if days <= 0 or first["seconds"] <= 0 or last["seconds"] <= 0:
        return shown
    factor = first["seconds"] / last["seconds"]
    shown.update(days=days, over=spoken(days), factor=round(factor, 3), times=_fold(factor))
    shown["first"], shown["last"] = clock(first["seconds"]), clock(last["seconds"])
    # Within two parts in a hundred either way it has not moved; slower is
    # worded by how many times slower.
    if abs(math.log(factor)) < math.log(1.02):
        shown["steady"] = True
    elif factor < 1:
        shown["slower"] = _fold(1 / factor)
    if days >= 7 and abs(math.log(factor)) > 1e-9:
        turn = days * math.log(2) / abs(math.log(factor))
        shown["halves" if factor > 1 else "doubles"] = spoken(turn)
    return shown


def _fold(factor: float) -> str:
    """How many times over, to two places and no trailing nought: `1.66x`,
    `6.8x`, `2x`."""
    return f"{factor:.2f}".rstrip("0").rstrip(".") + "x"


def _day(stamp: str):
    import datetime

    return datetime.date.fromisoformat(stamp[:10])


def history(source: pathlib.Path) -> dict | None:
    """How each way of playing has moved over time, for the CIVVIS
    Performance page's charts (operator, 2026-11-24: "in civvis performance,
    we should have charts studying the improvements rates of each of these
    over time"). `None` when the revision has no ledger or one that cannot
    be read.

    `rows` are the stops of the page's line after the baseline, in its
    order. A row has `lines`, each a run of dated points (`taken`, `seconds`,
    and `time` as it is worded) with the `pace` it has kept (see `pace`):

    - `readings`: the stop's own readings, every one of them. A stop that
      follows a ledger of runs shows every run there, and its pace is kept
      over the records among them, so a slower run is drawn and not counted
      as a step back.
    - `days`, in place of `readings` on a stop that reads a benchmark, where
      the clone holds its record: the middle game of every day of the record
      (`beta/control_times.py` draws a day from three games up). One day is
      a noisy thing to measure a pace by, so its pace is from the middle
      game of the record's first seven days to that of its last seven.
    - `tournament`, on the stop whose game the self-play tournament plays:
      the middle game of each finished batch, on the day the batch ended
      (`data/performance/self_play_times.json`). It is the same game with
      seven or more played at a time on a busy machine, so it reads slower
      than a one-core benchmark and it reaches back much further.
    - `hours`, on the same stop: the self-play benchmark hours
      (`beta/self_play_hours.py`, an hour of the tournament watched every
      six hours), each as the processor seconds one game took, which a busy
      machine does not stretch. A point of an hour has `at`, the moment it
      was taken, so that four hours of one day do not stand on one another.
    - `production`, on the stop that plays that game on every core: the same
      hours as the time from one finished game to the next, which is the
      rate the machine keeps while the live game and everything else run
      beside the tournament. It is far under a record run's.

    A row's `now` is the stop's figure as the page's line shows it:
    the `time`, and `worked` when it was worked out and not timed. `base` is
    the baseline's seconds, and `from` and `to` the first and last day any
    line has a point on.
    """
    try:
        ledger = json.loads((source / LEDGER).read_text(encoding="utf-8"))
        base = sorted(ledger["stops"][0]["readings"], key=lambda reading: reading["taken"])[-1]["seconds"]
        # The benchmark hours, where this clone holds the ledger they are on.
        try:
            watched = (self_play_hours.self_play(source) or {}).get("hours") or []
        except (OSError, ValueError, KeyError, TypeError):
            watched = []
        played = {stop["key"] for stop in ledger["stops"] if stop.get("tournament")}

        def hourly(seconds) -> list[dict]:
            points = []
            for read in watched:
                took = seconds(read)
                if took:
                    points.append({"taken": read["taken"][:10], "at": read["taken"], "seconds": round(took, 3), "time": clock(took), "games": read["games"]})
            return points

        rows = []
        for stop in ledger["stops"][1:]:
            if stop.get("line") is False:
                continue

            def point(reading: dict) -> dict:
                made = {"taken": reading["taken"][:10], "seconds": round(reading["seconds"], 3), "time": clock(reading["seconds"])}
                if reading.get("worked"):
                    made["worked"] = True
                return made

            followed = stop.get("follows")
            record = benchmark(source, stop)
            if followed:
                runs = json.loads((source / followed).read_text(encoding="utf-8"))["runs"]
                every = [{"taken": run["taken"], **FOLLOWED[followed](run)} for run in sorted(runs, key=lambda run: run["taken"])]
                kept = pace([point(reading) for reading in records(runs, FOLLOWED[followed])])
            else:
                every = readings(source, stop)
                kept = pace([point(reading) for reading in every])
            lines = [{"kind": "readings", "points": [point(reading) for reading in every], "pace": kept}]
            if record and record.get("days"):
                points = [{"taken": one["day"], "seconds": one["p"][1], "time": clock(one["p"][1]), "games": one["games"]} for one in record["days"]]
                first, week = record["first"], record["week"]
                kept = pace([{"taken": first["to"], "seconds": first["p"][1]}, {"taken": week["to"], "seconds": week["p"][1]}])
                kept["readings"] = len(points)
                lines = [{"kind": "days", "points": points, "pace": kept}]
            if stop.get("tournament"):
                try:
                    batches = json.loads((source / SELF_PLAY).read_text(encoding="utf-8"))["batches"]
                    points = [
                        {"taken": batch["ended"][:10], "seconds": batch["p"][2], "time": clock(batch["p"][2]), "games": batch["games"]}
                        for batch in sorted(batches, key=lambda batch: (batch["ended"], batch["batch"]))
                    ]
                    if points:
                        lines.append({"kind": "tournament", "points": points, "pace": pace(points)})
                except (OSError, ValueError, KeyError, TypeError, IndexError):
                    pass
                points = hourly(lambda read: read.get("cpu_a_game"))
                if points:
                    lines.append({"kind": "hours", "points": points, "pace": pace(points)})
            if followed and stop.get("of") in played:
                points = hourly(lambda read: 3600 / read["games_an_hour"] if read.get("games_an_hour") else None)
                if points:
                    lines.append({"kind": "production", "points": points, "pace": pace(points)})
            newest = readings(source, stop)[-1]
            row = {"key": stop["key"], "now": {"time": clock(newest["seconds"])}, "lines": lines}
            if newest.get("worked"):
                row["now"]["worked"] = True
            if "goal_seconds" in stop:
                row["goal_seconds"] = stop["goal_seconds"]
                row["goal"] = clock(stop["goal_seconds"])
            rows.append(row)
        days = [point["taken"] for row in rows for line in row["lines"] for point in line["points"]]
        return {"base": base, "base_time": clock(base), "from": min(days), "to": max(days), "rows": rows}
    except (OSError, ValueError, KeyError, TypeError, IndexError, ZeroDivisionError):
        return None


def main(argv: list[str]) -> int:
    if len(argv) == 3 and argv[1] == "--history":
        json.dump(history(pathlib.Path(argv[2])), sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
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
