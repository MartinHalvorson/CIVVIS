#!/usr/bin/env python3
"""The self-play benchmark hours, as the CIVVIS Performance page draws them.

    self_play_hours.py <source_tree>     print the record as JSON

Operator, 2026-11-24: "we should be benchmarking civvis self-play tournament
games as well. maybe every 6 hrs or so we can do 1 hr of civvis self-play
games on this machine". `tools/self_play_hour.py` takes the hour: it watches
the self-play tournament for sixty minutes and writes what it finished as one
line of `benchmarks/self_play_hours.jsonl` on the `ledger` branch
(`docs/SELF_PLAY_HOUR.md`). This reads those lines when the site is
published and turns them into what a page shows: the hours in order, the
newest, and the last day of them.

An hour is kept when it says when it was taken, ran for `MIN_MINUTES` or
more (a shorter run is a trial of the tool, not an hour) and counted its
games. Every other figure of a line is carried only if it is the kind of
number it should be, so that one odd line costs the page that figure and
nothing else. The figures are the reading's own: `tools/self_play_hour.py`
says what each is.

Like the computer control games' times (`beta/control_times.py`) nothing
here is committed. The ledger moves with every hour taken, and each lane
shows the hours as of its build.
"""

from __future__ import annotations

import datetime
import json
import math
import pathlib
import subprocess
import sys

#: The benchmark hours, one JSON line each, as the clone last fetched them
#: (`tools/self_play_hour.py` `LEDGER_PATH`; a test holds the two equal).
HOURS_REF = "refs/remotes/origin/ledger:benchmarks/self_play_hours.jsonl"

#: A run shorter than this is not an hour.
MIN_MINUTES = 30

#: The hours the page is given, the newest ones: a hundred days of four a day.
KEEP = 400

#: The stretch the page's summary counts, back from the newest hour.
DAY = datetime.timedelta(hours=24)

UTC = datetime.timezone.utc

#: The figures of a line that are plain numbers, each with the least it may
#: be: a count or a time cannot be negative, and a time cannot be nothing.
NUMBERS = {
    "games_an_hour": 0, "turns": 1, "workers": 0, "running": 0, "cores": 0,
    "host_busy": 0, "load": 0, "civ6": 0, "battery": 0, "machine_cores": 1,
}
POSITIVE = ("cpu_a_turn", "cpu_a_game")
WORDS = ("build", "batch", "host", "machine")


def lines(source: pathlib.Path) -> list[dict] | None:
    """The ledger's lines, or `None` when this clone holds no ledger or the
    ledger no hours yet."""
    try:
        shown = subprocess.run(["git", "-C", str(source), "show", HOURS_REF], capture_output=True, check=False)
    except OSError:
        return None
    if shown.returncode != 0:
        return None
    rows = []
    for line in shown.stdout.decode("utf-8", "replace").splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows or None


def number(value: object, least: float = 0) -> float | int | None:
    """A reading that is a real number, `least` or more."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < least:
        return None
    return value


def moment(stamp: object) -> datetime.datetime | None:
    try:
        return datetime.datetime.strptime(str(stamp), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=UTC)
    except ValueError:
        return None


def hour(row: dict) -> dict | None:
    """One line as the page takes it, or `None` for a line that is not an
    hour (see the module's own account)."""
    minutes, games = number(row.get("minutes"), MIN_MINUTES), number(row.get("games"))
    if moment(row.get("taken")) is None or minutes is None or games is None or isinstance(games, float) and games != int(games):
        return None
    out: dict = {"taken": str(row["taken"]), "minutes": minutes, "games": int(games), "reserved": row.get("reserved") is True}
    for name, least in NUMBERS.items():
        out[name] = number(row.get(name), least)
    for name in POSITIVE:
        value = number(row.get(name))
        out[name] = value if value else None
    p = row.get("p")
    out["p"] = list(p) if isinstance(p, list) and len(p) == 3 and all(number(v) for v in p) and p[0] <= p[1] <= p[2] else None
    if out["games_an_hour"] is None:
        out["games_an_hour"] = round(out["games"] * 60 / minutes, 1)
    for name in WORDS:
        out[name] = str(row[name]) if isinstance(row.get(name), str) and row[name] else None
    return out


def mean(values: list[float], places: int = 1) -> float | None:
    return round(sum(values) / len(values), places) if values else None


def record(rows: list[dict]) -> dict | None:
    """The record the page draws, or `None` without an hour.

    `hours` are the hours, oldest first, the newest `KEEP` of them, and one
    per moment and machine: the first line of an hour stands. `day` is the
    last twenty-four hours back from the newest: how many hours it holds,
    their mean `games_an_hour`, the best of them, and the means of the
    processor time a turn and of the workers running. `first` is the oldest
    hour's day, so the page can say since when.
    """
    kept: dict[tuple[str, str], dict] = {}
    for row in rows:
        read = hour(row) if isinstance(row, dict) else None
        if read is not None:
            kept.setdefault((read["taken"], read["host"] or ""), read)
    hours = sorted(kept.values(), key=lambda read: (read["taken"], read["host"] or ""))[-KEEP:]
    if not hours:
        return None
    newest = moment(hours[-1]["taken"])
    lately = [read for read in hours if newest - moment(read["taken"]) < DAY]
    turn = [read["cpu_a_turn"] for read in lately if read["cpu_a_turn"]]
    return {
        "min_minutes": MIN_MINUTES,
        "hours": hours,
        "day": {
            "hours": len(lately),
            "from": lately[0]["taken"],
            "to": lately[-1]["taken"],
            "games": sum(read["games"] for read in lately),
            "games_an_hour": mean([read["games_an_hour"] for read in lately]),
            "best": max(read["games_an_hour"] for read in lately),
            "cpu_a_turn": mean(turn, 4),
            "running": mean([read["running"] for read in lately if read["running"] is not None]),
        },
        "first": hours[0]["taken"][:10],
    }


def self_play(source: pathlib.Path) -> dict | None:
    """The record for a source tree, or `None` when its clone holds no
    ledger or the ledger no hour."""
    rows = lines(source)
    return record(rows) if rows is not None else None


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    json.dump(self_play(pathlib.Path(argv[1])), sys.stdout, ensure_ascii=False, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
