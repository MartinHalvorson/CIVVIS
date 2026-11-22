#!/usr/bin/env python3
"""The site's two rankings, and the home page's fastest game, read from the
lane's own revision.

    home_rankings.py <source_tree>              print the rankings as JSON
    home_rankings.py <source_tree> <page.html>  write them into the page

The rankings stood on the home page until 2026-11-22, when the operator moved
them to Charts ("move 2 charts on home/ to charts/"): Win Analytics
(`beta/research.html`) carries one empty slot,
`<script type="application/json" id="rankings-data">null</script>`, and
`beta/publish.sh` fills it with what this prints. A home page from before the
move has the same slot and is filled the same way. Two rankings:

- **Winningest civs**: our AI's wins and losses as each civ in the
  verification games against Civilization VI's own AI, best win rate first.
  Only the Lv 8 (Deity) games that began on or after 2026-11-16 count
  (`CIVS_LEVEL`, `CIVS_SINCE`). Read from `results.jsonl` on the `ledger` branch, where
  `tools/civ6_ladder.py publish-run` adds a line as each game finishes, so
  it is the one ranking that is not a property of the lane's revision: both
  lanes show the record as it stood when the site was built.
- **Player strategies**: the Civ AI Agents by published Elo, from
  `data/strategy_elo.json` (written by `strategy_elo`, see `docs/ELO.md`),
  with their handles and tilts from `strategies::ROSTER`, and Civilization
  VI's difficulty rungs in between as the scale's landmarks.

The page once listed the head of the heuristic gene ranking as well. It does
not any more; `GENE_HEURISTIC_RANKING.md` is the place to read that.

It also carried the Tournaments table until 2026-11-21, when the operator gave
tournaments a page of their own between Home and CIVVIS
(`beta/tournaments.html`, `beta/tournaments_page.py`). `tournament_series`
still lives here, because that page, the live mirror's Mode line and a lane
pinned to a home page that still has the card all count the same series.

The home page (`beta/landing.html`) is four videos now, and one of them
follows a record: the fastest game is whichever video leads
`data/performance/fastest_game_videos.json`, the list the Performance page
shows. The page carries a second slot for it,
`<script type="application/json" id="home-videos">null</script>`, which is
filled here too (`videos`), so a new record reaches both pages from its one
line in that file.

Nothing here is committed as a copy. The page once kept a hand-synced battle
catalog and it went the way such copies go; this is derived at publish time
from the same files the engine embeds, so it cannot drift from them.

A revision that predates the Elo file (the stable lane is routinely pinned
to an older one) simply gets that ranking as `null`, and the page shows what
it has. So does a clone that has never fetched the `ledger` branch.
"""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys

import performance_page

#: How many civs the table shows: the ones that have won, best win rate first.
TOP_CIVS = 25

#: The games the civ table counts (operator 2026-11-19: "our winningest civs
#: ranking should be by win %. and just limited to lv 8 games, starting from
#: 11/16"): the ones played at this Civilization VI level, Deity, that began
#: on or after this day. The day is UTC, as every stamp in the ledger is.
CIVS_LEVEL = 8
CIVS_SINCE = "2026-11-16"

#: The verification games' results, one JSON line per recorded run, as the
#: clone last fetched them. The publish workflow's checkout fetches every
#: branch, so the site is built from the record of a few minutes before.
RESULTS_REF = "refs/remotes/origin/ledger:results.jsonl"

#: A recorded run's ending, best first: a game reloaded from its autosave is
#: several runs, and it ended the best way any of them did. The last is a
#: harness or a person stopping the game, which is not a result.
OUTCOMES = ["win", "defeat", "rival_victory", "abandoned", "unfinished"]
LOSSES = {"defeat", "rival_victory", "abandoned"}

#: Civilization VI's victory types under the names its own screens use.
VICTORIES = {
    "VICTORY_TECHNOLOGY": "Science",
    "VICTORY_CULTURE": "Culture",
    "VICTORY_CONQUEST": "Domination",
    "VICTORY_RELIGIOUS": "Religious",
    "VICTORY_DIPLOMATIC": "Diplomatic",
    "VICTORY_SCORE": "Score",
}

#: Civilization VI's own ladder, lowest first: the order pooled rungs are named
#: in, and a rung's place in it is its level number (Settler is Lv 1, Deity
#: Lv 8). `home_rankings_test.py` holds it equal to `strategies::CIV6_RUNGS`.
# Half-width of the 95% interval in standard deviations (`strategies::MARGIN_Z`).
MARGIN_Z = 1.96

CIV6_RUNGS = ["settler", "chieftain", "warlord", "prince", "king", "emperor", "immortal", "deity"]

SLOT = re.compile(
    r'(<script type="application/json" id="rankings-data">)(.*?)(</script>)', re.S
)


def roster(source: pathlib.Path) -> dict[str, dict[str, str]]:
    """`strategies::ROSTER` by id: the handle and tilt a person is shown."""
    path = source / "src" / "strategies.rs"
    if not path.is_file():
        return {}
    rows = re.finditer(
        r'id:\s*"([^"]+)",\s*handle:\s*"([^"]+)",\s*tilt:\s*"([^"]+)"',
        path.read_text(encoding="utf-8"),
    )
    return {m[1]: {"name": m[2], "tilt": m[3]} for m in rows}


def strategies(source: pathlib.Path) -> dict | None:
    path = source / "data" / "strategy_elo.json"
    if not path.is_file():
        return None
    published = json.loads(path.read_text(encoding="utf-8"))
    ratings = published.get("ratings") or {}
    names = roster(source)
    rows = []
    for key, rating in ratings.items():
        if key.startswith("civ6:"):
            continue
        shown = names.get(key, {"name": key, "tilt": ""})
        rows.append(
            {
                "kind": "agent",
                "id": key,
                "name": shown["name"],
                "tilt": shown["tilt"],
                "elo": rating["elo"],
                "sd": rating.get("sd", 0.0),
                # The 95% margin of error: the file's own, or 1.96 sd for a
                # file written before `strategy_elo` published one.
                "margin": rating.get("margin95", round(MARGIN_Z * rating.get("sd", 0.0))),
                "games": rating.get("games", 0),
                "wins": rating.get("wins", 0),
                "bound": rating.get("bound", ""),
            }
        )
    # The rungs as a person is shown them: `ladder` is the monotone pooling of
    # the raw fit, and rungs the tables cannot tell apart share one Elo, so
    # they share one row here.
    pooled: dict[float, list[str]] = {}
    for rung, elo in (published.get("ladder") or {}).items():
        pooled.setdefault(elo, []).append(rung)
    for elo, rungs in pooled.items():
        rungs.sort(key=lambda rung: CIV6_RUNGS.index(rung) if rung in CIV6_RUNGS else len(CIV6_RUNGS))
        bounds = {ratings.get(f"civ6:{rung}", {}).get("bound", "") for rung in rungs}
        rows.append(
            {
                "kind": "rung",
                "name": " · ".join(rung.title() for rung in rungs),
                "levels": [CIV6_RUNGS.index(rung) + 1 for rung in rungs if rung in CIV6_RUNGS],
                "elo": elo,
                "bound": bounds.pop() if len(bounds) == 1 else "",
            }
        )
    # Strongest first; at a tie the agent stands above the rung it equals.
    rows.sort(key=lambda row: (-row["elo"], row["kind"] != "agent", row["name"]))
    runs = [run for run in published.get("runs") or [] if isinstance(run, dict)]
    return {
        "tables": published.get("tables"),
        "players": published.get("players"),
        "turns": published.get("turns"),
        # Rating runs in the window; their labels begin with the run's date.
        "runs": len(runs),
        "latest_run": (runs[-1].get("run") or "")[:10] if runs else "",
        "rows": rows,
    }


def results(source: pathlib.Path) -> list[dict] | None:
    """The verification games' result lines, or `None` when this clone holds
    no ledger (or its ledger predates the results file)."""
    try:
        shown = subprocess.run(
            ["git", "-C", str(source), "show", RESULTS_REF], capture_output=True, check=False
        )
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


def civ_name(civ: str, names: list[str]) -> str:
    """A civ under the engine's own name for it (`data/civs.json`), from the
    host's type: `CIVILIZATION_GRAN_COLOMBIA` is Gran Colombia. Two types do
    not title-case into the name, `CIVILIZATION_OTTOMAN` (Ottomans) and
    `CIVILIZATION_BABYLON_STK` (Babylon); both differ only past the name's
    end, and a type with no such match keeps its plain reading."""
    plain = civ.removeprefix("CIVILIZATION_").replace("_", " ").title()
    if plain in names:
        return plain
    near = [name for name in names if name.startswith(plain) or plain.startswith(name + " ")]
    return near[0] if len(near) == 1 else plain


def level(difficulty: str | None) -> int | None:
    """A difficulty's level number: `DIFFICULTY_DEITY` is 8."""
    rung = (difficulty or "").removeprefix("DIFFICULTY_").lower()
    return CIV6_RUNGS.index(rung) + 1 if rung in CIV6_RUNGS else None


def began(key: str, runs: list[dict]) -> str:
    """The day a game began, `YYYY-MM-DD` in UTC: the stamp in its tag, which
    is when it was launched (`civvis-20261116T050606Z`), and for a tag with
    no stamp the earliest day any of its runs was recorded."""
    m = re.search(r"(\d{4})(\d{2})(\d{2})T\d{6}Z", key)
    if m:
        return f"{m[1]}-{m[2]}-{m[3]}"
    return min((str(row["utc"])[:10] for row in runs if row.get("utc")), default="")


def civ_ranking(
    rows: list[dict], names: list[str], civ6_level: int | None = CIVS_LEVEL, since: str | None = CIVS_SINCE
) -> dict | None:
    """Wins and losses per civ over the recorded games, best win rate first.

    Only the games played at `civ6_level` that began on or after `since`
    count; `None` for either counts every level, or every day. A game counts
    once, however many runs it took, and counts as a win, a loss (a rival's
    victory, our elimination, or a game the harness conceded early), or
    nothing: a game somebody stopped has no result. Ties in win rate go to
    the civ with more wins, which is the same rate over more games.
    """
    games: dict[str, list[dict]] = {}
    for row in rows:
        if civ6_level is not None and level(row.get("difficulty")) != civ6_level:
            continue
        key = row.get("game") or row.get("tag")
        if key and row.get("outcome") in OUTCOMES:
            games.setdefault(key, []).append(row)
    civs: dict[str, dict] = {}
    total = {"wins": 0, "losses": 0, "unfinished": 0, "unattributed_wins": 0}
    through = ""
    for key, runs in games.items():
        if since and began(key, runs) < since:
            continue
        ending = min(runs, key=lambda row: OUTCOMES.index(row["outcome"]))
        outcome = ending["outcome"]
        if outcome == "unfinished":
            total["unfinished"] += 1
            continue
        through = max(through, max(str(row.get("utc") or "")[:10] for row in runs))
        total["wins" if outcome == "win" else "losses"] += 1
        civ = next((row["civ"] for row in runs if row.get("civ")), None)
        if civ is None:
            total["unattributed_wins"] += outcome == "win"
            continue
        tally = civs.setdefault(civ, {"wins": 0, "losses": 0, "won": {}})
        if outcome in LOSSES:
            tally["losses"] += 1
            continue
        tally["wins"] += 1
        won = (level(ending.get("difficulty")) or 0, VICTORIES.get(ending.get("victory"), ""))
        tally["won"][won] = tally["won"].get(won, 0) + 1
    if not total["wins"] + total["losses"]:
        return None
    ranked = []
    for civ, tally in civs.items():
        # Highest level first, then the victory's name: "Lv 8 Culture", and
        # "Lv 5 Diplomatic ×2" for a pairing won more than once.
        won = [
            " ".join(part for part in (f"Lv {lv}" if lv else "", victory) if part)
            + (f" \u00d7{count}" if count > 1 else "")
            for (lv, victory), count in sorted(tally["won"].items(), key=lambda item: (-item[0][0], item[0][1]))
        ]
        ranked.append(
            {
                "civ": civ,
                "name": civ_name(civ, names),
                "wins": tally["wins"],
                "losses": tally["losses"],
                "best": max((lv for lv, _ in tally["won"]), default=0) or None,
                "won": won,
            }
        )
    ranked.sort(key=lambda row: (-row["wins"] / (row["wins"] + row["losses"]), -row["wins"], row["name"]))
    winners = [row for row in ranked if row["wins"]]
    return {
        "level": civ6_level,
        "since": since,
        "games": total["wins"] + total["losses"],
        "wins": total["wins"],
        "unfinished": total["unfinished"],
        "unattributed_wins": total["unattributed_wins"],
        "played": len(ranked),
        "winners": len(winners),
        "through": through or None,
        "rows": winners[:TOP_CIVS],
    }


def civs(source: pathlib.Path) -> dict | None:
    """The civ table, or `None` when there is no record to draw it from.

    The other ranking reads files of the revision being published, which
    CI has already held to their shape. This one reads a branch every
    finished game writes to and nothing tests, from inside the script that
    publishes the whole site. A line it cannot count therefore costs the
    page its civ table, said on stderr, and never costs the site its publish.
    """
    rows = results(source)
    if rows is None:
        return None
    try:
        path = source / "data" / "civs.json"
        names = list(json.loads(path.read_text(encoding="utf-8"))) if path.is_file() else []
        return civ_ranking(rows, names)
    except Exception as error:  # noqa: BLE001 - see above
        print(f"home_rankings: the ledger's results could not be ranked ({error!r}); no civ table", file=sys.stderr)
        return None


def game_key(row: dict) -> str | None:
    """The game a run belongs to: its root tag, without a `-contN` resume."""
    key = row.get("game") or row.get("tag")
    return re.sub(r"-cont\d+$", "", key) if isinstance(key, str) else None


def when(tag: str) -> str:
    """A run tag's start as `YYYY-MM-DD HH:MMZ`: `civvis-20261119T184500Z`."""
    m = re.search(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})\d{2}Z", tag)
    return f"{m[1]}-{m[2]}-{m[3]} {m[4]}:{m[5]}Z" if m else tag


def tournament_series(rows: list[dict], tournaments: list[dict], detail: bool = False) -> list[dict] | None:
    """Each tournament's series: consecutive blocks of `series_size` finished games.

    A tournament counts every game whose tag sorts at or after its `start` (and
    before its `end`, if it has one) and whose result line matches every `match`
    field (difficulty, players, map). A series has an end once it holds
    `series_size` games, or once its definition's `end` closes it short.
    A game counts once, by its best run (`OUTCOMES`); a game whose every run is
    `unfinished` (a launch that never reached a board) is not a game. Each series is a
    tournament with its own number (`id`), counted from 1 in the order the
    series began across every definition; the live mirror's Mode reads
    "Tournament <id>: <wins>-<losses>" from the same count. Newest first.

    With `detail` each series also carries what the Tournaments page
    (`beta/tournaments_page.py`) draws beyond the table: the place of its
    definition in the list (`definition`) and its games in the order they were
    played (`played`, each a game's tag and the result line of its best run).
    The home page's table never asks for it.
    """
    out = []
    for place, spec in enumerate(tournaments):
        size = int(spec.get("series_size") or 10)
        start = str(spec.get("start") or "")
        end = str(spec.get("end") or "")
        match = spec.get("match") or {}
        games: dict[str, list[dict]] = {}
        for row in rows:
            key = game_key(row)
            if (key and key >= start and (not end or key < end) and row.get("outcome") in OUTCOMES
                    and all(row.get(field) == value for field, value in match.items())):
                games.setdefault(key, []).append(row)
        finished = []
        for key in sorted(games):
            ending = min(games[key], key=lambda row: OUTCOMES.index(row["outcome"]))
            if ending["outcome"] != "unfinished":
                finished.append((key, ending))
        blocks = list(range(0, len(finished), size))
        for number, first in enumerate(blocks, 1):
            block = finished[first:first + size]
            wins = sum(1 for _, ending in block if ending["outcome"] == "win")
            last = max(str(ending.get("utc") or "") for _, ending in block)
            # A definition closed by its `end` closes its last series too, full
            # or not (the operator restarted the tournament), once a game past
            # that end has been played after the series' last one. An `end`
            # that is a tag prefix another table owns (`civvis-t`) closes
            # nothing on its own: the games played after that table still
            # sort before it and form the next series.
            closed = len(block) == size or bool(
                end and first == blocks[-1] and any(
                    (game_key(row) or "") >= end and str(row.get("utc") or "") > last
                    for row in rows))
            out.append({
                "tournament": spec.get("name") or "",
                "series": number,
                "start": when(block[0][0]),
                "end": (last[:16].replace("T", " ") + "Z") if closed and last else None,
                "games": len(block),
                "size": size,
                "wins": wins,
                "losses": len(block) - wins,
                "conditions": spec.get("conditions") or "",
                # The operator's word on the definition (Tournament 2 scrapped,
                # operator 2026-11-20), the same on each of its series.
                "note": spec.get("note") or "",
                **({"definition": place, "played": block} if detail else {}),
            })
    if not out:
        return None
    out.sort(key=lambda row: (row["start"], row["tournament"]))
    for number, row in enumerate(out, 1):
        row["id"] = number
    out.reverse()
    return out


def tournaments(source: pathlib.Path) -> list[dict] | None:
    """The Tournaments table, or `None` without definitions or a ledger.

    Like the civ table it reads the ledger branch, so a line it cannot count
    costs the page this table, said on stderr, never the site its publish.
    """
    path = source / "data" / "tournaments.json"
    if not path.is_file():
        return None
    rows = results(source)
    if rows is None:
        return None
    try:
        specs = json.loads(path.read_text(encoding="utf-8")).get("tournaments") or []
        return tournament_series(rows, specs)
    except Exception as error:  # noqa: BLE001 - see above
        print(f"home_rankings: the tournaments could not be read ({error!r}); no tournament table", file=sys.stderr)
        return None


def rankings(source: pathlib.Path) -> dict:
    return {"civs": civs(source), "strategies": strategies(source), "tournaments": tournaments(source)}


#: The home page's Tournaments card as pages up to 2026-11-21 carried it. The
#: table lives on the Tournaments page now (`beta/tournaments.html`, filled by
#: `beta/tournaments_page.py`), and only a page that still has the card is
#: sent its rows: a lane pinned to an older revision keeps the table it shows.
TOURNAMENTS_CARD = 'id="rank-tournaments"'


def for_page(page: str, data: dict) -> dict:
    """The rankings this page has a place for."""
    if TOURNAMENTS_CARD in page:
        return data
    return {key: value for key, value in data.items() if key != "tournaments"}


#: The home page's own slot: the videos of its four that follow a record.
VIDEOS_SLOT = re.compile(
    r'(<script type="application/json" id="home-videos">)(.*?)(</script>)', re.S
)


def videos(source: pathlib.Path) -> dict | None:
    """The home page's videos that follow a record: `fastest`, the fastest
    game on video, which is the record the Performance page leads with
    (`performance_page.videos`, fewest seconds first). `None` when the
    revision has no list of records or the list has no video."""
    try:
        listed = json.loads((source / performance_page.VIDEOS).read_text(encoding="utf-8"))["videos"]
    except (OSError, ValueError, KeyError, TypeError):
        return None
    records = performance_page.videos(listed)
    if not records:
        return None
    return {"fastest": {key: records[0][key] for key in ("id", "title", "time")}}


def fill(page: str, data: dict | None, slot: re.Pattern[str] = SLOT) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = slot.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    source = pathlib.Path(argv[1])
    if len(argv) == 2:
        json.dump(rankings(source), sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    text = page.read_text(encoding="utf-8")
    shown = []
    # The rankings, on the page that has their slot: Win Analytics, or a
    # home page from before they moved there. They are not read for a page
    # with no place for them.
    if SLOT.search(text):
        data = for_page(text, rankings(source))
        text = fill(text, data)
        shown += [name for name, value in data.items() if value] or ["no rankings"]
    # The home page's fastest game, on the page that has its slot.
    if VIDEOS_SLOT.search(text):
        records = videos(source)
        text = fill(text, records, VIDEOS_SLOT)
        shown.append("the fastest game's video" if records else "no video")
    if not shown:
        # A page with neither slot: nothing to fill.
        print(f"home_rankings: {page} has no rankings slot and no videos slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(text, encoding="utf-8")
    print(f"home_rankings: wrote {', '.join(shown)} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
