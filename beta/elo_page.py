#!/usr/bin/env python3
"""The ELO Rankings page's figures: the Civ AI Agents and Civilization VI's
own AI on one Elo scale, now and over time.

    elo_page.py <source_tree>              print the record as JSON
    elo_page.py <source_tree> <page.html>  write it into the page

The ELO Rankings page (`beta/elo.html`, served at
`<lane>/charts/elo-rankings/`) is the second page under Charts/, after Win
Analytics (operator 2026-11-23: "add that to a new page: Charts/ELO
Rankings. this should be after win analytics in the ordering. at the top of
this page is the player straetgies, pulled from win analytics"). It answers
two requests of the same evening: "we should have our benchmarking games
update our elo ... so elo is always staying fresh" and "we should have a
running graph over time of the top 10 elo civvis ai agents, as well as our
elo estimations benchmarks for the lv 4 - 8 firaxis civ ai".

**The agents** are rated by `strategy_elo` in the simulator (`docs/ELO.md`):
`data/strategy_elo.json` is the newest fit and
`data/strategy_elo_history.json` every fit before it
(`tools/strategy_elo_history.py`). The graph draws the top `TOP_AGENTS` by
the newest fit, each as the step its ratings took from fit to fit.

**Civilization VI's own AI, Lv 4 Prince to Lv 8 Deity**, is rated from the
benchmark: every finished game of every tournament on Auto-Tourneys, the
games the Win Analytics benchmark counts (`research_page.tournament_games`'s
set, through `home_rankings.tournament_series`), each game a table of six:
our one seat against five Firaxis AIs at the game's level. A table is fitted
on the model `strategy_elo` fits (`strategies::fit`): a seat wins with
probability proportional to `10^(elo / 400)`, so our seat wins a game against
five AIs rated `r` with probability `s / (s + 5 * 10^(r / 400))`, `s` being
our seat's strength.

Our seat in Civilization VI is the live decider playing one victory lane,
and the agent that plays that lane in the simulator is its twin
(`TWINS`: Science is MoonshotMaven, Religion HolyRoller, and so on; a seat
with no lane is JackOfAllTrades, the deployment genome). The twin's rating as
published when the game began (the history's point then in force) stands for
our seat. That one assumption, the twin plays in Civilization VI as it plays
in the simulator, is what puts the Firaxis levels on the agents' scale; the
spacing of the levels and our seat's distance to each is measured.

Because each Firaxis level meets only our seat, the joint fit of these
tables with the simulator's reduces to one equation per level, solved here:
the level's rating is the one at which our seat's expected wins equal its
wins, plus the pair of imaginary games `strategy_elo` gives every entrant (one
won and one lost against the window's average twin) so a level we have
always beaten, or never, still has a finite rating, shown as a bound. Its
95% margin is the fit's own (from the curvature of the likelihood) and the
twins' margins, added in quadrature.

The figure is read off the run ledger whenever the site is built, so it
moves with every finished game: no rating run is waited on. Each level is
fitted over the games of the last `WINDOW_DAYS` days back from the newest
benchmark game (its rating now), and once for each day the benchmark has
games (its line on the graph), a point when its window holds at least
`MIN_GAMES` games.

What the figure does not say: the simulator's own Difficulty dial is a
different thing, our stand-in agent with each level's handicap
(`civ6:<rung>` in `data/strategy_elo.json`, `strategies::monotone_ladder`).
The page shows that ladder beside the measurement, by name, and the game
keeps using it.
"""

from __future__ import annotations

import bisect
import datetime
import json
import math
import pathlib
import re
import sys

import home_rankings as ranks
import research_page

SLOT = re.compile(r'(<script type="application/json" id="elo-data">)(.*?)(</script>)', re.S)

#: The fits of the strategy Elo, in the source tree.
HISTORY = pathlib.Path("data/strategy_elo_history.json")
PUBLISHED = pathlib.Path("data/strategy_elo.json")

#: How many agents the graph draws (operator: "the top 10 elo civvis ai agents").
TOP_AGENTS = 10

#: The levels rated from the benchmark: the levels it deals.
LEVELS = [4, 5, 6, 7, 8]

#: Our seat's lane to the roster agent that plays it in the simulator.
TWINS = {
    "science": "moonshot",
    "culture": "tourist",
    "religious": "holy",
    "religion": "holy",
    "diplomatic": "swing",
    "diplomacy": "swing",
    "domination": "siege",
}
#: The twin of a seat with no lane, or a lane no agent plays.
ADAPTIVE = "jack"

#: The Firaxis AIs at a benchmark table.
RIVALS = 5

#: The window a level's rating is fitted over, in days back from the newest
#: benchmark game, and the fewest games a point on its line is drawn from.
WINDOW_DAYS = 7
MIN_GAMES = 3

#: Half-width of the 95% interval in standard deviations (`strategies::MARGIN_Z`).
MARGIN_Z = 1.96

LN = math.log(10) / 400


def strength(elo: float) -> float:
    return 10 ** (elo / 400)


def history(source: pathlib.Path) -> list[dict]:
    """Every published fit, oldest first, the published file last when the
    history does not end at it yet."""
    try:
        points = json.loads((source / HISTORY).read_text(encoding="utf-8"))["points"]
    except (OSError, ValueError, KeyError, TypeError):
        points = []
    try:
        published = json.loads((source / PUBLISHED).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return points
    newest = {
        key: [round(float(r["elo"]), 1), round(float(r.get("margin95", MARGIN_Z * float(r.get("sd") or 0)))),
              r.get("bound") or ""]
        for key, r in (published.get("ratings") or {}).items()
    }
    if not points or points[-1].get("ratings") != newest:
        at = points[-1]["at"] if points else ""
        points = [*points, {"at": at, "tables": published.get("tables"), "ratings": newest,
                            "ladder": published.get("ladder") or {}}]
    return points


def stamp(text: str) -> datetime.datetime | None:
    try:
        return datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")
    except (TypeError, ValueError):
        return None


def in_force(points: list[dict], at: datetime.datetime) -> dict:
    """The fit published last before `at`: the first one for a game older than every fit."""
    times = [stamp(p.get("at")) or datetime.datetime.min for p in points]
    return points[max(0, bisect.bisect_right(times, at) - 1)]


def twin(target: object) -> str:
    return TWINS.get(str(target or "").lower(), ADAPTIVE)


def benchmark_games(source: pathlib.Path, rows: list[dict] | None = None) -> list[dict]:
    """The benchmark's games at the levels rated: when each began, its level,
    whether our seat won, and the twin of the lane it played. A table with
    two of our agents at it (its row names both, `A+B`) is left out: either
    winning was a win there."""
    rows = ranks.results(source) if rows is None else rows
    if not rows:
        return []
    specs = json.loads((source / research_page.TOURNAMENTS).read_text(encoding="utf-8")).get("tournaments") or []
    games: dict[str, dict] = {}
    for series in ranks.tournament_series(rows, specs, detail=True) or []:
        for key, ending in series["played"]:
            level = ranks.level(ending.get("difficulty"))
            at = research_page.started(key, ending)
            if level in LEVELS and at and "+" not in str(ending.get("civ") or ""):
                games[key] = {"at": at, "level": level, "won": ending["outcome"] == "win",
                              "twin": twin(ending.get("target"))}
    return sorted(games.values(), key=lambda g: g["at"])


def fit_level(games: list[tuple[float, float, bool]]) -> dict | None:
    """One level's rating from its games, each `(twin elo, twin margin, won)`."""
    if not games:
        return None
    prior = sum(a for a, _, _ in games) / len(games)
    wins = sum(1 for _, _, won in games if won)

    def slope(r: float) -> float:
        # d(log likelihood)/dr, in units of ln(10)/400: expected wins less
        # wins, and the imaginary pair against the average twin.
        expected = sum(1 / (1 + RIVALS * strength(r - a)) for a, _, _ in games)
        return expected - wins + (1 - strength(r - prior)) / (1 + strength(r - prior))

    low, high = prior - 2000, prior + 2000
    for _ in range(80):
        mid = (low + high) / 2
        if slope(mid) > 0:
            low = mid
        else:
            high = mid
    r = (low + high) / 2
    info = sum(q * (1 - q) for q in (1 / (1 + RIVALS * strength(r - a)) for a, _, _ in games))
    pair = strength(r - prior)
    info += 2 * pair / (1 + pair) ** 2
    own = MARGIN_Z / (LN * math.sqrt(info))
    twins = sum(m for _, m, _ in games) / len(games)
    return {
        "elo": round(r, 1),
        "margin": round(math.hypot(own, twins)),
        "bound": "upper" if wins == len(games) else "lower" if wins == 0 else "",
        "games": len(games),
        "wins": wins,
    }


def rated(games: list[dict], points: list[dict]) -> dict[int, list[tuple[float, float, bool]]]:
    """Each game as its level's fit reads it, by level."""
    out: dict[int, list] = {}
    for game in games:
        ratings = in_force(points, game["at"]).get("ratings") or {}
        elo = ratings.get(game["twin"]) or ratings.get(ADAPTIVE)
        if elo:
            out.setdefault(game["level"], []).append((float(elo[0]), float(elo[1] or 0), game["won"]))
    return out


def firaxis(games: list[dict], points: list[dict]) -> dict | None:
    """The Firaxis levels: rated now, and a point a day."""
    if not games or not points:
        return None
    to = games[-1]["at"]
    window = datetime.timedelta(days=WINDOW_DAYS)
    first = games[0]["at"].date()
    # The end of each day the benchmark played, and the newest game itself.
    ends = []
    day = first
    while datetime.datetime.combine(day, datetime.time()) <= to:
        ends.append(min(datetime.datetime.combine(day + datetime.timedelta(days=1), datetime.time()), to))
        day += datetime.timedelta(days=1)
    levels = []
    for level in LEVELS[::-1]:
        mine = [g for g in games if g["level"] == level]
        series = []
        for end in ends:
            inside = [g for g in mine if end - window < g["at"] <= end]
            if len(inside) < MIN_GAMES:
                continue
            fitted = fit_level(rated(inside, points).get(level, []))
            if fitted:
                series.append([end.strftime("%Y-%m-%dT%H:%M:%SZ"), fitted["elo"], fitted["margin"],
                               fitted["bound"], fitted["games"]])
        now = fit_level(rated([g for g in mine if to - window < g["at"] <= to], points).get(level, []))
        if not now and not series:
            continue
        levels.append({
            "level": level,
            "name": ranks.CIV6_RUNGS[level - 1].title(),
            "now": now,
            "series": series,
            "twins": sorted({g["twin"] for g in mine if to - window < g["at"] <= to}),
        })
    if not levels:
        return None
    return {
        "to": to.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "days": WINDOW_DAYS,
        "games": sum(1 for g in games if to - window < g["at"] <= to),
        "all_games": len(games),
        "rivals": RIVALS,
        "levels": levels,
        "twins": TWINS | {"": ADAPTIVE},
    }


def agents(source: pathlib.Path, points: list[dict]) -> list[dict]:
    """The top agents by the newest fit, each with its rating at every fit."""
    if not points:
        return []
    names = ranks.roster(source)
    newest = points[-1].get("ratings") or {}
    ids = [key for key in newest if not key.startswith("civ6:")]
    ids.sort(key=lambda key: -newest[key][0])
    out = []
    for key in ids[:TOP_AGENTS]:
        shown = names.get(key, {"name": key, "tilt": ""})
        series = [[p["at"], *p["ratings"][key]] for p in points if key in (p.get("ratings") or {}) and p.get("at")]
        out.append({"id": key, "name": shown["name"], "tilt": shown["tilt"], "series": series})
    return out


def simulator(points: list[dict]) -> list[dict]:
    """The simulator's Difficulty dial at the levels rated: the stand-in's
    ladder in the newest fit."""
    if not points:
        return []
    ladder = points[-1].get("ladder") or {}
    ratings = points[-1].get("ratings") or {}
    out = []
    for level in LEVELS[::-1]:
        rung = ranks.CIV6_RUNGS[level - 1]
        if rung in ladder:
            margin = (ratings.get(f"civ6:{rung}") or [0, 0])[1]
            out.append({"level": level, "name": rung.title(), "elo": ladder[rung], "margin": margin})
    return out


def record(source: pathlib.Path) -> dict | None:
    """The record, or `None` when there are no ratings to show."""
    points = history(source)
    if not points:
        return None
    out: dict = {
        "strategies": ranks.strategies(source),
        "agents": agents(source, points),
        "simulator": simulator(points),
        "fits": len(points),
        "firaxis": None,
    }
    try:
        out["firaxis"] = firaxis(benchmark_games(source), points)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"elo_page: no Firaxis levels from the benchmark ({error})", file=sys.stderr)
    return out


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = record(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"elo_page: {page} has no elo slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"elo_page: no figures to write into {page}", file=sys.stderr)
    else:
        levels = (data.get("firaxis") or {}).get("levels") or []
        print(f"elo_page: wrote {len(data['agents'])} agents over {data['fits']} fits and "
              f"{len(levels)} Firaxis levels into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
