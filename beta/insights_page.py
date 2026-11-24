#!/usr/bin/env python3
"""The AI Insights page's figures: what the Civ games show about AI at large.

    insights_page.py <source_tree>              print the record as JSON
    insights_page.py <source_tree> <page.html>  write it into the page

The AI Insights page (`beta/insights.html`, served at
`<lane>/charts/ai-insights/`) is the first page under Charts/ (operator
2026-11-24: "the old ai interpretability should be in the process of being
renamed civ ai interpretability. we want a new ai insights"). The page that
reads the Civ agent itself keeps its place after it; this one is for what
the games say about AI in general, an insight at a time, each with the chart
it is read off.

Its first insight (operator 2026-11-24: "for the first visualization, we
should show the civvis vs lv 8 deity ai chart, switching between lv 8, 7, 6,
5 to highlight that smarter ai picks up speed. this is the process of
optimization decisions chosen to move fast. as ai gets smarter, expect the
pace of progress to speed up") is under `speed`:

- **levels**, the games against Civilization VI's own AI at Lv 8 Deity, Lv 7
  Immortal, Lv 6 Emperor and Lv 5 King, in that order (`LEVELS`, the order
  the page's switch lists them). Each is the level's every counted game: its
  number and name, the games, CIVVIS's wins among them, the days of its
  first and newest game, and per victory type the count on each turn from 1
  to 250 (a type nobody won at the level is left out). `median` is the turn
  by which half of the level's games were over, whoever won them: the figure
  the insight rests on, which falls as the level rises.

The games are Win Analytics' own, counted by its own functions
(`research_page.level_groups`, `research_page.window`): one CIVVIS AI agent
inside Civilization VI against the game's AI, six players on a Continents
map at Online speed, played to the full 250 turns and ended in a victory,
CIVVIS's or the AI's. So the chart here and the chart of the same name there
are drawn from the same games by construction, read off `results.jsonl` on
the ledger branch at publish. A level nobody has played is not in the
record, and the page says so on that level's button.

The page's heading says the turn falls as the level rises, and the page
draws whatever the games say. So when they stop saying it (`not_falling`), a
publish says so in its log, as a warning, and the heading is somebody's to
look at again.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page its figures, said on stderr,
and never the publish.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

import home_rankings as ranks
import research_page

SLOT = re.compile(r'(<script type="application/json" id="insights-data">)(.*?)(</script>)', re.S)

#: The levels the first insight switches between, in the order the page
#: lists them (operator 2026-11-24: "switching between lv 8, 7, 6, 5").
LEVELS = [8, 7, 6, 5]


def speed(source: pathlib.Path) -> dict | None:
    """The first insight's games: every counted game against the AI at each
    of `LEVELS`, or `None` when none of them has one."""
    rows = ranks.results(source)
    if not rows:
        return None
    played = research_page.level_groups(rows, lambda tags: research_page.limits(source, tags))
    levels = []
    for level in LEVELS:
        games = research_page.window(played.get(level) or [], None)
        if not games or not games["games"]:
            continue
        # The turn half of the level's games were over by: the median over
        # every victory type, off the same counts the page stacks.
        ended: dict[int, int] = {}
        for victory in games["victories"].values():
            for turn, count in enumerate(victory["turns"], start=1):
                if count:
                    ended[turn] = ended.get(turn, 0) + count
        levels.append(
            {
                "level": level,
                "rung": ranks.CIV6_RUNGS[level - 1].title(),
                "games": games["games"],
                "won": sum(victory["won"] for victory in games["victories"].values()),
                "from": games["from"],
                "to": games["to"],
                "median": research_page.median(ended),
                "victories": {
                    name: victory["turns"] for name, victory in games["victories"].items() if victory["games"]
                },
            }
        )
    return {"levels": levels} if levels else None


def not_falling(levels: list[dict]) -> list[str]:
    """Where the first insight does not hold: each pair of levels next to
    each other in the record whose stronger level's games were not over
    sooner, as a sentence. Empty while the turn falls with every step up."""
    ordered = sorted(levels, key=lambda level: level["level"], reverse=True)
    return [
        f"Lv {strong['level']} {strong['rung']} games are half over by turn {strong['median']}, "
        f"no sooner than Lv {weak['level']} {weak['rung']} games (turn {weak['median']})"
        for strong, weak in zip(ordered, ordered[1:])
        if not strong["median"] < weak["median"]
    ]


def record(source: pathlib.Path) -> dict | None:
    """The page's record, or `None` when no insight has figures."""
    out: dict = {"turns": research_page.TURNS, "victories": research_page.VICTORIES, "speed": None}
    try:
        out["speed"] = speed(source)
    except (ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        print(f"insights_page: no games against the game's AI ({error})", file=sys.stderr)
    return out if out["speed"] else None


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
    data = record(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"insights_page: {page} has no insights slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"insights_page: no figures to write into {page}", file=sys.stderr)
    else:
        said = [f"Lv {level['level']} {level['games']}" for level in data["speed"]["levels"]]
        print(f"insights_page: wrote the games against the AI ({', '.join(said)}) into {page}", file=sys.stderr)
        for sentence in not_falling(data["speed"]["levels"]):
            print(f"::warning::insights_page: the page says smarter AI picks up speed, and {sentence}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
