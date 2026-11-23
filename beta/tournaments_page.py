#!/usr/bin/env python3
"""The Tournaments page's record, read from the definitions and the run ledger.

    tournaments_page.py <source_tree>              print the record as JSON
    tournaments_page.py <source_tree> <page.html>  write it into the page

The Tournaments page (`beta/tournaments.html`; Auto-Tourneys since 2026-11-22,
served at `<lane>/auto-tourneys/`, the address it states itself)
is the site's page between Home and CIVVIS (operator 2026-11-21: "This is my
Simulator Experiments page essentially. I have an idea I want to run. I
schedule in a tournament for it. The AI models just keep working away at
games."). It carries one empty slot,
`<script type="application/json" id="tournaments-data">null</script>`, and
`beta/publish.sh` fills it with what this prints.

A tournament is one idea played out: a fixed number of verification games
under fixed conditions, our AI inside Civilization VI against the game's own
AI. `data/tournaments.json` defines them, in the order they are to be played,
and the games are counted off the ledger's `results.jsonl` exactly as
`home_rankings.tournament_series` counts them for the live mirror's Mode
line. The page shows three lists:

- **running**: a series that has games and no end yet.
- **scheduled**: a definition none of whose games has finished, in the order
  the file lists them. Scheduling a tournament is adding its definition.
- **finished**: every series that has ended, newest first.

Each series carries its games in the order they were played (the civ, how
the game ended, the turn), the wins by victory type, the losses by how they
were lost, and a record per level when its games were not all at one level.

A date is written `YYYY-MM-DD HH:MM`, on the 24-hour clock, with no `Z`
after it (operator 2026-11-21: "for all start and end dates, lets use
YYYY-MM-DD 24 hr time with no z"). It is still UTC, which the page says once.
A series also carries two durations (the same order: "add a wall time and
run time duration cols"). Its wall time is the clock time it took, from its
first game starting to its last game ending. Its run time is the time its
games were in play, each from its start to its result, added up; the time
between games (a restart, a wait, a night the machine was off) is in the
first and not in the second.

A tournament whose result video is on the CIVVIS YouTube channel links it
(operator 2026-11-21: "we can link to the youtube tournament result
videos"). The definitions file lists them under `videos`, by the
tournament's number as the page shows it, and a series carries its link as
`video`. Only a YouTube watch link is carried; anything else is left out and
said on stderr.

Every tournament has a short description, a few words on what it was
(operator 2026-11-21: "for each tournament, we should have a short
description ... the descriptions can be on the title line"). The
definitions file lists them under `descriptions`, by the tournament's
number like the videos, and a tournament carries its own as `description`;
the page writes it on the tournament's title line, "Tournament 4: 50 game
series". A definition may carry a `description` of its own, used when the
list has no line for its number.

Every tournament spells out its hyperparameters, the settings all its
games were played under (operator 2026-11-23: "please spell out the
tourney hyperparameters for each tourney clearly"): how many games, the
AI level, the players at the table, the victory types, how the civ and
its strategy were picked, the map, the game speed, the turn limit, the
ruleset, where the games were played, how a game ended and which build
of the agent played it. The definitions file lists them under
`hyperparameters`, by the tournament's number like the descriptions, and
a tournament carries them as `hyperparameters`, a list of `[name, value]`
pairs in the order of `HYPERPARAMETERS`, the same for every tournament so
two of them read side by side. A definition may carry its own, used when
the list has no line for its number.

Like the home page's civ table this reads a branch every finished game
writes to, from inside the script that publishes the whole site. So a
definition or a line it cannot count costs the page its record, said on
stderr, and never costs the site its publish; the page then says it has no
record. A lane is filled by its own revision's copy of this script from its
own revision's definitions (`fill` in `beta/publish.sh`), and both lanes
count the games off the run ledger as of the build. So a tournament added on
main is listed on /test at once and on the front page once it is promoted,
while the games of a tournament the front page already lists keep counting
there.
"""

from __future__ import annotations

import datetime
import json
import math
import pathlib
import re
import sys

import home_rankings as ranks

SLOT = re.compile(
    r'(<script type="application/json" id="tournaments-data">)(.*?)(</script>)', re.S
)

#: A result video is a video on YouTube, named by its eleven-character id in
#: either of the two addresses YouTube gives for one.
VIDEO = re.compile(r"https://(?:www\.youtube\.com/watch\?v=|youtu\.be/)([\w-]{11})(?:[?&#].*)?")


def video(videos: dict, number: int) -> str:
    """A tournament's result video as its YouTube watch page, or `""`."""
    listed = videos.get(str(number))
    if not listed:
        return ""
    found = VIDEO.fullmatch(str(listed))
    if not found:
        print(f"tournaments_page: Tournament {number}'s video is not a YouTube link; left out", file=sys.stderr)
        return ""
    return f"https://www.youtube.com/watch?v={found[1]}"


def description(descriptions: dict, number: int, spec: dict) -> str:
    """A tournament's short description, the few words after its number on
    its title line, or `""`. The line listed for its number is the one
    shown; without one, its definition's own `description` is."""
    for listed in (descriptions.get(str(number)), spec.get("description")):
        if isinstance(listed, str) and listed.strip():
            return " ".join(listed.split())
    return ""


#: A tournament's hyperparameters, in the order the page lists them. A
#: name the definitions file adds outside this list follows them, in the
#: file's order.
HYPERPARAMETERS = (
    "Games",
    "AI level",
    "Players",
    "Victory types",
    "Civ",
    "Strategy",
    "Map",
    "Game speed",
    "Turn limit",
    "Ruleset",
    "Played in",
    "Game end",
    "CIVVIS agent",
)


def hyperparameters(listed: dict, number: int, spec: dict) -> list[list[str]]:
    """A tournament's hyperparameters as `[name, value]` pairs in the order
    of `HYPERPARAMETERS`, or `[]`. The ones listed for its number are the
    ones shown; without them, its definition's own are. A name or a value
    that is not a few words is left out."""
    rank = {name: place for place, name in enumerate(HYPERPARAMETERS)}
    for given in (listed.get(str(number)), spec.get("hyperparameters")):
        if not isinstance(given, dict):
            continue
        pairs = [[" ".join(name.split()), " ".join(value.split())] for name, value in given.items()
                 if isinstance(name, str) and name.strip() and isinstance(value, str) and value.strip()]
        if pairs:
            return sorted(pairs, key=lambda pair: rank.get(pair[0], len(rank)))
    return []


#: A date as `home_rankings` writes one: `2026-11-19 18:45Z`.
STAMPED = re.compile(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}Z")

#: The moment a run tag names, to the second: `civvis-20261119T184500Z`.
TAGGED = re.compile(r"\d{8}T\d{6}Z")


def plain(stamp: str | None) -> str | None:
    """A date as the page shows it: `2026-11-19 18:45`, 24-hour, no `Z`.
    A start that is not a date (an old tag with no time in it) is left as
    it is, and a series with no end has none here either."""
    return stamp[:-1] if isinstance(stamp, str) and STAMPED.fullmatch(stamp) else stamp


def began(tag: str) -> datetime.datetime | None:
    """When a game started: the moment its tag names."""
    found = TAGGED.search(tag)
    try:
        return datetime.datetime.strptime(found[0], "%Y%m%dT%H%M%SZ") if found else None
    except ValueError:
        return None


def ended(result: dict) -> datetime.datetime | None:
    """When a game ended: the moment its result line was written."""
    try:
        return datetime.datetime.strptime(str(result.get("utc") or ""), "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        return None


def lasted(seconds: float) -> str:
    """A duration in hours and minutes, `22h 03m`, or `46m` under an hour."""
    minutes = int(seconds // 60 + (seconds % 60 >= 30))
    return f"{minutes // 60}h {minutes % 60:02d}m" if minutes >= 60 else f"{minutes}m"


def durations(played: list) -> tuple[str, str]:
    """A series' wall time and run time, each `""` when no game of it has
    both a start and an end to measure.

    The wall time runs from the first game's start to the last game's end.
    The run time adds up each game's own time, from its start to its
    result. A game that was resumed (`-cont1`) is one game from the start
    its tag names to the result that counted."""
    spans = [(start, end) for start, end in ((began(key), ended(result)) for key, result in played)
             if start and end and end >= start]
    if not spans:
        return "", ""
    wall = max(end for _, end in spans) - min(start for start, _ in spans)
    run = sum(((end - start) for start, end in spans), datetime.timedelta())
    return lasted(wall.total_seconds()), lasted(run.total_seconds())


def ending(row: dict) -> tuple[bool, str]:
    """Whether a game was won, and how it ended in a few words."""
    victory = ranks.VICTORIES.get(row.get("victory"), "")
    outcome = row.get("outcome")
    if outcome == "win":
        return True, f"{victory} victory" if victory else "Victory"
    if outcome == "rival_victory":
        return False, f"Rival {victory} victory" if victory else "Rival victory"
    if outcome == "defeat":
        return False, "Eliminated"
    return False, "Conceded"


def game(row: dict, names: list[str]) -> dict:
    """One game as the page draws it. A table two CIVVIS agents sat at
    records both civs joined by `+`, and reads "Phoenicia & Australia"."""
    won, how = ending(row)
    civs = [ranks.civ_name(civ, names) for civ in str(row.get("civ") or "").split("+") if civ]
    turns = row.get("turns")
    return {
        "civ": " & ".join(civs),
        "won": won,
        "how": how,
        "turns": turns if isinstance(turns, int) else None,
        "level": ranks.level(row.get("difficulty")),
    }


def tally(labels: list[str]) -> list[list]:
    """Each label with its count, most first, then by name."""
    counts: dict[str, int] = {}
    for label in labels:
        counts[label] = counts.get(label, 0) + 1
    return [[label, count] for label, count in sorted(counts.items(), key=lambda item: (-item[1], item[0]))]


def wilson(wins: int, games: int, z: float = 1.96) -> list[int] | None:
    """The 95% range of a win rate, in whole percent (Wilson's interval,
    which stays honest at 0 or 100% and on a handful of games)."""
    if not games:
        return None
    p = wins / games
    d = 1 + z * z / games
    centre = (p + z * z / (2 * games)) / d
    half = z * math.sqrt(p * (1 - p) / games + z * z / (4 * games * games)) / d
    return [round(100 * max(0.0, centre - half)), round(100 * min(1.0, centre + half))]


def benchmark(played: list, days: int = 7) -> dict:
    """An indefinite tournament's win rate by level (operator 2026-11-22:
    "benchmark our win rates roughly for these games over time"): over every
    game, with its 95% range, and day by day (UTC, the day a game ended) over
    the last `days` days that had games. A level's day is `[wins, games]`."""
    by_level: dict[int, list[tuple[str, bool]]] = {}
    for _, result in played:
        lv = ranks.level(result.get("difficulty"))
        if lv:
            by_level.setdefault(lv, []).append((str(result.get("utc") or "")[:10], result.get("outcome") == "win"))
    recent = sorted({day for games in by_level.values() for day, _ in games if day})[-days:]
    levels = []
    for lv in sorted(by_level):
        games = by_level[lv]
        wins = sum(won for _, won in games)
        levels.append({
            "level": lv, "wins": wins, "games": len(games), "range": wilson(wins, len(games)),
            "days": [[sum(won for d, won in games if d == day), sum(1 for d, _ in games if d == day)] for day in recent],
        })
    return {"days": [day[5:] for day in recent], "levels": levels}


def series_view(row: dict, spec: dict, names: list[str], videos: dict, descriptions: dict, listed: dict) -> dict:
    """A series as the page draws it: the table's row and its games."""
    games = [game(result, names) for _, result in row["played"]]
    levels = sorted({g["level"] for g in games if g["level"]})
    wall, run = durations(row["played"])
    return {
        "id": row["id"],
        "description": description(descriptions, row["id"], spec),
        "name": row["tournament"],
        "idea": spec.get("idea") or "",
        "hyperparameters": hyperparameters(listed, row["id"], spec),
        "conditions": row["conditions"],
        "note": row["note"],
        "start": plain(row["start"]),
        "end": plain(row["end"]),
        # A running series' two durations run to its last finished game.
        "wall": wall,
        "run": run,
        "video": video(videos, row["id"]),
        "games": row["games"],
        "size": row["size"],
        "wins": row["wins"],
        "losses": row["losses"],
        # "Diplomatic victory" is a win's ending; the tally names the victory.
        "victories": tally([g["how"].removesuffix(" victory") for g in games if g["won"]]),
        "endings": tally([g["how"] for g in games if not g["won"]]),
        # A record per level, lowest first, only when the games span levels.
        "levels": [
            [lv, sum(g["won"] for g in games if g["level"] == lv),
             sum(not g["won"] for g in games if g["level"] == lv)]
            for lv in levels
        ] if len(levels) > 1 else [],
        "results": games,
        **({"benchmark": benchmark(row["played"])} if spec.get("indefinite") else {}),
    }


def passed_over(spec: dict, rows: list[dict]) -> bool:
    """Whether a definition with no finished game has had its turn and lost it.

    Its `end` closes it once a game tagged at or past that end is played
    after the games that came before its `start`: the queue moved on without
    it. Tags alone cannot say so. The ledger holds runs from older tag
    families (`settler-`, `civvis-tinywar-`) that sort after any prefix a
    scheduled table owns, played months before it was defined, and they close
    nothing. `tournament_series` reads a cut-short series the same way.
    """
    end = str(spec.get("end") or "")
    if not end:
        return False
    start = str(spec.get("start") or "")
    played = [(ranks.game_key(row) or "", str(row.get("utc") or "")) for row in rows]
    before = max((utc for key, utc in played if key < start), default="")
    return any(key >= end and utc > before for key, utc in played)


def record(rows: list[dict], specs: list[dict], names: list[str], videos: dict | None = None,
           descriptions: dict | None = None, listed: dict | None = None) -> dict:
    """The page's three lists and the totals over every game counted.
    `videos`, `descriptions` and `listed` are the definitions file's own: a
    result video's address, a short description and the hyperparameters, by
    the number of their tournament."""
    series = ranks.tournament_series(rows, specs, detail=True) or []
    started = {row["definition"] for row in series}
    videos = videos if isinstance(videos, dict) else {}
    descriptions = descriptions if isinstance(descriptions, dict) else {}
    listed = listed if isinstance(listed, dict) else {}
    views = [series_view(row, specs[row["definition"]], names, videos, descriptions, listed) for row in series]
    number = max((view["id"] for view in views), default=0)
    scheduled = []
    for place, spec in enumerate(specs):
        # A definition with no finished game is still to be played, unless
        # the queue has moved on past its `end`.
        if place in started or passed_over(spec, rows):
            continue
        number += 1
        scheduled.append({
            "id": number,
            "description": description(descriptions, number, spec),
            "name": spec.get("name") or "",
            "idea": spec.get("idea") or "",
            "hyperparameters": hyperparameters(listed, number, spec),
            "conditions": spec.get("conditions") or "",
            "note": spec.get("note") or "",
            "size": None if spec.get("indefinite") else int(spec.get("series_size") or 10),
        })
    last = max((str(result.get("utc") or "") for row in series for _, result in row["played"]), default="")
    return {
        "through": last[:16].replace("T", " ") if last else None,
        "running": [view for view in views if view["end"] is None],
        "scheduled": scheduled,
        "finished": [view for view in views if view["end"] is not None],
        "totals": {
            "tournaments": len(views),
            "games": sum(view["games"] for view in views),
            "wins": sum(view["wins"] for view in views),
            "losses": sum(view["losses"] for view in views),
        },
    }


def tournaments(source: pathlib.Path) -> dict | None:
    """The page's record, or `None` without definitions or a ledger."""
    path = source / "data" / "tournaments.json"
    if not path.is_file():
        return None
    rows = ranks.results(source)
    if rows is None:
        return None
    try:
        defined = json.loads(path.read_text(encoding="utf-8"))
        specs = defined.get("tournaments") or []
        civs = source / "data" / "civs.json"
        names = list(json.loads(civs.read_text(encoding="utf-8"))) if civs.is_file() else []
        return record(rows, specs, names, defined.get("videos"), defined.get("descriptions"),
                      defined.get("hyperparameters"))
    except Exception as error:  # noqa: BLE001 - see the module's last paragraph
        print(f"tournaments_page: the tournaments could not be read ({error!r}); no record", file=sys.stderr)
        return None


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = tournaments(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"tournaments_page: {page} has no tournaments slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"tournaments_page: no record to write into {page}", file=sys.stderr)
    else:
        print(
            f"tournaments_page: wrote {len(data['running'])} running, {len(data['scheduled'])} scheduled "
            f"and {len(data['finished'])} finished into {page}",
            file=sys.stderr,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
