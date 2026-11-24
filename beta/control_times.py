#!/usr/bin/env python3
"""The computer control benchmark: how long one game takes with CIVVIS at the
controls of Civilization VI, end to end, day by day.

    control_times.py <source_tree>     print the record as JSON

Operator, 2026-11-24: "we should be benchmarking our civvis computer control
games time end to end and that number show flow through to a performance
chart (civvis comptuer control games speed over time) and be a top chart for
civvis performance". This is the number and its track. The CIVVIS Performance
page (`beta/performance.html`) opens on it as a chart: `performance_page.py`
puts the record in the page's slot under `control`. `docs/CONTROL_GAME_TIME.md`
is the write-up.

**What is timed.** A game of the verification games: CIVVIS playing the real
Civilization VI through its own controls against the game's AI. Its time is
end to end, everything the machine spends on the game: launching Civilization
VI, setting the game up, playing it to a victory, holding the final screen,
quitting, and writing the result.

**How.** Every run of the harness (`tools/civ6_play.py`) is one line of
`results.jsonl` on the `ledger` branch, and a game is the lines that share a
`game`: one, or several when a game was reloaded from its autosave.

- A run since 2026-11-24 carries `seconds`, read off the harness's own clock
  (`civ6_play.RUN_CLOCK`), with the part of it that was setup and the part
  that was play. A game whose every run has one is the sum of them: `clocked`.
- A run before that has no clock, and its time is read off two stamps: the
  moment in its tag, which is when it was launched, and the moment of its
  result. That holds for a game recorded in one run only, because a reloaded
  game's later runs carry the first run's stamp. A reloaded game from before
  the clock is left out.

**Which games.** The ones played to a victory, ours or a rival's. A game we
were eliminated from, one the harness conceded early and one somebody stopped
all end sooner than a game does, and would read as speed.

**The figures.** A day is the UTC day a game ended on. A day with at least
`MIN_GAMES` games is a point of the chart: the middle game's time and the
time one game in ten beats and one in ten runs past (`PERCENTILES`, nearest
rank on the games themselves, as the page's self-play times are), the same
three for the time of one turn (a game's seconds over the turn it ended on),
which does not move with how long the games happened to run, the middle
game's length in turns, and the Civilization VI levels played. The benchmark's
reading is the same summary over the last seven days (`week`), set against
the first seven days of the record (`first`).

Like every page's figures this is worked out when the site is published, from
the ledger as it stands then, so a finished game is on the chart with the
next publish and nothing here is committed as a copy.
"""

from __future__ import annotations

import collections
import datetime
import json
import math
import pathlib
import re
import subprocess
import sys

#: The verification games' results, one JSON line per recorded run, as the
#: clone last fetched them (`home_rankings.RESULTS_REF`; a test holds the two
#: equal).
RESULTS_REF = "refs/remotes/origin/ledger:results.jsonl"

#: The readings the speed timeline keeps, for the seconds of a game played by
#: hand: its first stop is the one every other is measured against.
LEDGER = pathlib.Path("data/performance/speedup_timeline.json")

#: The percentiles of a day: the time one game in ten beats, the middle game,
#: and the time one game in ten runs past.
PERCENTILES = [10, 50, 90]

#: A day is a point of the chart from this many games: under it, one slow
#: game is the day.
MIN_GAMES = 3

#: The days the benchmark's reading counts, back from the newest game's.
WINDOW_DAYS = 7

#: How a game that counts ended: in a victory, ours or a rival's.
COMPLETE = {"win", "rival_victory"}

#: A recorded run's ending, best first (`home_rankings.OUTCOMES`): a game
#: reloaded from its autosave ended the best way any of its runs did.
OUTCOMES = ["win", "defeat", "rival_victory", "abandoned", "unfinished"]

#: Civilization VI's ladder, lowest first (`home_rankings.CIV6_RUNGS`): a
#: difficulty's place in it is its level number.
CIV6_RUNGS = ["settler", "chieftain", "warlord", "prince", "king", "emperor", "immortal", "deity"]

#: The moment a run was launched, in its tag: `civvis-t7-20261124T150035Z`.
STAMP = re.compile(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z")

UTC = datetime.timezone.utc


def percentile(ordered: list[float], p: int) -> float:
    """The nearest-rank percentile of a sorted list
    (`performance_page.percentile`)."""
    rank = max(1, math.ceil(p * len(ordered) / 100))
    return ordered[min(len(ordered), rank) - 1]


def results(source: pathlib.Path) -> list[dict] | None:
    """The result lines, or `None` when this clone holds no ledger."""
    try:
        shown = subprocess.run(["git", "-C", str(source), "show", RESULTS_REF], capture_output=True, check=False)
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


def moment(stamp: object) -> datetime.datetime | None:
    """A result's `utc` as a moment: `2026-11-24T15:13:11Z`."""
    try:
        return datetime.datetime.strptime(str(stamp), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=UTC)
    except ValueError:
        return None


def launched(tag: object) -> datetime.datetime | None:
    """The moment in a run's tag, which is when it was launched."""
    found = STAMP.search(str(tag))
    if not found:
        return None
    try:
        return datetime.datetime(*(int(part) for part in found.groups()), tzinfo=UTC)
    except ValueError:
        return None


def number(value: object) -> float | None:
    """A reading that is a real, positive time."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
        return None
    return float(value)


def level(difficulty: object) -> int | None:
    """A difficulty's level number: `DIFFICULTY_DEITY` is 8."""
    rung = str(difficulty or "").removeprefix("DIFFICULTY_").lower()
    return CIV6_RUNGS.index(rung) + 1 if rung in CIV6_RUNGS else None


def game_times(rows: list[dict]) -> tuple[list[dict], int]:
    """Every game played to a victory with a time, oldest first, and how many
    such games have none (reloaded games from before the clock).

    A game is `day` (the UTC day it ended), `seconds` end to end, `turns` (the
    turn it ended on, or `None`), `level`, `players`, `clocked` (timed by the
    harness's own clock and not read off two stamps) and, for a clocked game
    whose every run says its parts, `split`: the seconds of setup, of play
    and of the ending.
    """
    games: dict[str, list[dict]] = collections.defaultdict(list)
    for row in rows:
        key = row.get("game") or row.get("tag")
        if key and row.get("utc"):
            games[str(key)].append(row)
    out, untimed = [], 0
    for runs in games.values():
        runs.sort(key=lambda row: str(row["utc"]))
        ending = min((str(row.get("outcome")) for row in runs), key=lambda o: OUTCOMES.index(o) if o in OUTCOMES else len(OUTCOMES))
        last = runs[-1]
        ended = moment(last["utc"])
        if ending not in COMPLETE or ended is None:
            continue
        clock = [number(row.get("seconds")) for row in runs]
        split = None
        if all(clock):
            seconds, clocked = sum(clock), True
            setup = [number(row.get("setup_seconds")) for row in runs]
            play = [number(row.get("play_seconds")) for row in runs]
            if all(setup) and all(play) and sum(setup) + sum(play) <= seconds:
                split = (sum(setup), sum(play), seconds - sum(setup) - sum(play))
        else:
            began = launched(last.get("tag")) if len(runs) == 1 else None
            seconds, clocked = (ended - began).total_seconds() if began else 0, False
        if not seconds > 0:
            untimed += 1
            continue
        turns = last.get("turns")
        players = last.get("players")
        out.append(
            {
                "day": ended.date().isoformat(),
                "seconds": seconds,
                "turns": turns if isinstance(turns, int) and not isinstance(turns, bool) and turns >= 1 else None,
                "level": level(last.get("difficulty")),
                "players": players if isinstance(players, int) and not isinstance(players, bool) else None,
                "clocked": clocked,
                "split": split,
            }
        )
    out.sort(key=lambda game: game["day"])
    return out, untimed


def summary(games: list[dict]) -> dict:
    """What the page shows of a set of games: how many, the three percentiles
    of a game's seconds (`p`) and of one turn's (`a_turn`), the middle game's
    length in `turns`, the `levels` played (lowest and highest), the number of
    `players` most of them had, how many were `clocked`, and from
    `MIN_GAMES` clocked games with their parts the middle `split` of one:
    setup, play, the ending."""
    seconds = sorted(game["seconds"] for game in games)
    a_turn = sorted(game["seconds"] / game["turns"] for game in games if game["turns"])
    turns = sorted(game["turns"] for game in games if game["turns"])
    levels = sorted(game["level"] for game in games if game["level"])
    players = collections.Counter(game["players"] for game in games if game["players"])
    out = {
        "games": len(games),
        "p": [round(percentile(seconds, p)) for p in PERCENTILES],
        "a_turn": [round(percentile(a_turn, p), 2) for p in PERCENTILES] if a_turn else None,
        "turns": int(percentile(turns, 50)) if turns else None,
        "levels": [levels[0], levels[-1]] if levels else None,
        "players": players.most_common(1)[0][0] if players else None,
        "clocked": sum(1 for game in games if game["clocked"]),
    }
    parts = [game["split"] for game in games if game["split"]]
    if len(parts) >= MIN_GAMES:
        out["split"] = [round(percentile(sorted(part[i] for part in parts), 50)) for i in range(3)]
    return out


def by_hand(source: pathlib.Path) -> float | None:
    """The seconds of one game played by hand: the newest reading of the
    speed timeline's first stop, the one every multiplier is measured
    against. `None` in a revision without those readings."""
    try:
        stop = json.loads((source / LEDGER).read_text(encoding="utf-8"))["stops"][0]
        return number(sorted(stop["readings"], key=lambda reading: reading["taken"])[-1]["seconds"])
    except (OSError, ValueError, KeyError, IndexError, TypeError):
        return None


def record(rows: list[dict], hand: float | None = None) -> dict | None:
    """The record the page draws, from the result lines: `None` when no game
    played to a victory has a time.

    `days` are the days with `MIN_GAMES` games or more, oldest first, each a
    `summary` under its `day`. `week` is the benchmark's reading, the summary
    of the last `WINDOW_DAYS` days back from the newest game's, and `first`
    the summary of the first `WINDOW_DAYS` days of the record, each with the
    days it spans (`from`, `to`). `all` counts every game, and the games left
    out for having no time (`untimed`). `by_hand` is the seconds of a game
    played by hand, for the multiplier.
    """
    games, untimed = game_times(rows)
    if not games:
        return None
    by_day: dict[str, list[dict]] = collections.defaultdict(list)
    for game in games:
        by_day[game["day"]].append(game)
    days = [{"day": day, **summary(played)} for day, played in sorted(by_day.items()) if len(played) >= MIN_GAMES]
    oldest, newest = games[0]["day"], games[-1]["day"]
    span = datetime.timedelta(days=WINDOW_DAYS - 1)
    since = (datetime.date.fromisoformat(newest) - span).isoformat()
    until = (datetime.date.fromisoformat(oldest) + span).isoformat()

    def window(start: str, end: str) -> dict:
        held = [game for game in games if start <= game["day"] <= end]
        return {"from": held[0]["day"], "to": held[-1]["day"], **summary(held)}

    return {
        "percentiles": PERCENTILES,
        "min_games": MIN_GAMES,
        "window_days": WINDOW_DAYS,
        "by_hand": hand,
        "days": days,
        "week": window(since, newest),
        "first": window(oldest, until),
        "all": {"games": len(games), "from": oldest, "to": newest, "untimed": untimed, "clocked": sum(1 for game in games if game["clocked"])},
    }


def control(source: pathlib.Path) -> dict | None:
    """The record for a source tree, or `None` when its clone holds no
    ledger or the ledger no timed game."""
    rows = results(source)
    if rows is None:
        return None
    return record(rows, by_hand(source))


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    json.dump(control(pathlib.Path(argv[1])), sys.stdout, ensure_ascii=False, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
