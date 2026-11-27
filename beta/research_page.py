#!/usr/bin/env python3
"""The Research page's figures: when in the game each victory type lands.

    research_page.py <source_tree>              print the record as JSON
    research_page.py <source_tree> <page.html>  write it into the page
    research_page.py --self-play <batches_dir> <out.json>
                                                rebuild the self-play snapshot
    research_page.py --build-orders <source_tree> <out.json> [--again]
                                                add the games played since to
                                                the snapshot of the openings
                                                (with --again, read every game
                                                afresh)

The Research page (`beta/research.html`; Win Analytics since 2026-11-22,
served at `<lane>/charts/win-analytics/`, the address it states itself) is
the site's page between Home and Tournaments. Its first chart (operator
2026-11-21: "a civvis self play and civvis play against deity ai
distribution victory types over online game turn. so when each victory type
spikes in the game") sets two kinds of game side by side, both six players
on a Continents map at Online speed, 250 turns:

- **self-play**: every seat a CIVVIS AI agent, inside the simulator. These
  are the gene-screen tournament batches. Their games are rows on the machine
  that plays them, not in the repository, so the counts come from a snapshot,
  `data/research/self_play_endings.json`, which `--self-play` rebuilds from
  the batch directories (one game per seed: the victory and the turn).
- **against the game's AI**: one CIVVIS AI agent inside Civilization VI
  against the game's own AI. These are the verification games, read off
  `results.jsonl` on the ledger branch at publish, so the figures move with
  every finished game. They are kept apart by the level the AI played at,
  Prince (Lv 4) to Deity (Lv 8), and the page draws one level at a time
  (operator 2026-11-21: "an option to select by level too. so i could for
  example go to level 4 and see the win rate of civvis against level 4";
  2026-11-23: "ai level in charts can go from lv 4 to lv 8", so a game
  against a lower level is not counted), opening on Deity, where most of
  them were played. A game counts once however many runs it took, and only
  a game that ended in a victory, ours or
  a rival's, has a victory type and a turn; an elimination or a stopped game
  has neither and is left out. Some series were played to a shorter limit
  (170, 160 and 120 turns in the Score series of 2026-11-12 to 15), where the
  limit's Score victory and the victories it cut off would bend the chart, so
  a game counts only when its run summary on the ledger
  (`runs/<tag>/summary.json`) says Online speed and a limit of at least 250
  turns. A run with no summary is left out too, and so is a table with more
  than one CIVVIS agent at it, where a win is either agent's.

The record holds, for three windows of each kind (its last 7 days, its last
30 days, every game), the games counted and for each victory type: the games
it ended, the CIVVIS wins among them (against the AI only; in self-play every
winner is a CIVVIS agent), the median turn, and the count on each turn from 1
to 250. The games against the AI are a kind per level, under `levels`, and a
level nobody has played is not in it.
A victory CIVVIS won at least once also holds CIVVIS's wins on each turn
(`won_turns`), so the page can draw a level's games three times (operator
2026-11-21: "start with all victories by type along timeline, then just the
deity ai wins, then the civvis wins"): what is left of a turn's count after
CIVVIS's wins is the AI's.
The page groups the turns into spans itself; a single turn matters, because
the Diplomatic victories land on the World Congress's own turns (185, 205,
225 and 245 at Online speed).
A window is measured back from the kind's own newest game, a level's from
that level's, so a snapshot left unrefreshed, or a level last played weeks
ago, still shows its last week rather than an empty one, and the page says
which days each window holds.

Self-play has two shorter windows as well (operator 2026-11-24: "to win
analytics civvis self play, add 3 days and 1 day options please. 1 day
covers last 24 hrs"): the 24 hours and the 72 hours before its newest game
finished, counted game by game. The longer windows count whole batches by
the day each ended, which a day cannot be read from: a batch runs for six
hours to a day and a half. A game's row carries no clock, so `--self-play`
works out when each game of the last three days finished (`part_finishes`)
and the snapshot holds those two windows already counted, under `last`.

Over the charts the page opens on the benchmark against the game's AI
(operator 2026-11-22: "to the top of win analytics roughly, please list our
win rate against lv 4... lv 8 deity ai in tournament games"; and 2026-11-23:
"at the top of win analytics, we should have a last 1 day win rate col, last
3 days, last 7 days, last 30 days. this will help us track win rates. order
the cols lv 8 down to level 4. this will be our primary Firaxis Civ 6 AI
benchmark - win rate on each level. half the games can be diplo, half no
diplo. we should have a table on the left the is with diplo excluded and a
table on the right that allows diplo wins. we can drop the wins, losses.
games (n) should be listed after each value. so like 100% (5) or 14% (181).
at the top, we should have win rate (total games)"): two tables, the games
played with the Diplomatic victory on and the games played with it switched
off, each a row per level from Deity (Lv 8) down to Prince (Lv 4) and a column
per window, the last 1, 3, 7 and 30 days, a cell being CIVVIS's win rate and
the games it is a share of. The games with it on are the left table (later on
2026-11-23, reversing the sides first asked for: "put diplo victory allowed
to the left of no diplo victory"). Its games are the tournaments' own, every
finished game of every tournament the Tournaments page lists
(`data/tournaments.json`, counted by `home_rankings.tournament_series`), so
the two pages agree: a win is CIVVIS's victory and anything else a finished
game ended in is a loss, a rival's victory, our elimination or a game
conceded early. That is a wider count than the charts', which have no place
for a game without a victory. Which table a game stands in is the game's own
setup, read off its run summary on the ledger (`tables_of`): what the game
itself reported of its Diplomatic victory, or what a hotseat table was asked
to switch off. A game whose summary does not say is in neither table.
A third table holds the Lv 8 Deity games played with the Religious victory
off as well (operator 2026-11-27: "add a third category to the tournament
for lv 8 deity no diplo no religion. lets grind this hard. self improve from
our mistakes."), a row of its own and no game of the Diplo Off table's: the
host's bench conductor plays them as benchmark games, tagged as the others
are, so the game's own Religious switch is all that tells them apart. The
record holds it under `tournament_rates`: `tables.on`, `tables.off` and
`tables.religion_off`, each with its levels and every window, and `to`, the
newest tournament game's start, which every window is measured back from in
whole days of 24 hours, so every cell of every table is the same days'
games.

The record also holds the tree of build orders
(operator 2026-11-23: "add a tree for the early build order win rates for
deity games. meaning, when scout was built first, what percentage of those
games won? when settler was secnod, what percentage of scout-settler won? a
person should be able to click thourgh and see the win rates of all the
units. we should sort the tree by win rates descending ... add this chart to
the top of win analytics"): for the games against Deity, the capital's first
step, then the second after each first, and so on, each with CIVVIS's win
rate over the games that opened that way and those games, listed best win
rate first. A game's opening is what `civ6_ladder.opening_builds` reads off
its first run's events: the capital's build order, what its production
finished and, marked, what it dropped unfinished (operator 2026-11-24:
"capital cant make a settler first - needs 2 pop": a Scout taken up on turn
1 and dropped at pop 2 for a Settler is "Scout, dropped", then "Settler"), a
bought or handed-over item not among them. The tree the page shows leaves
the dropped steps out (operator 2026-11-26: "lets also not list the dropped
in the trie for Capital Build ORder"); the record keeps them. A run's summary on the ledger
carries it (`build_order`, and `build_order_reading`, the way it was read),
and the games before the harness wrote it the way the reader reads now are
in a snapshot, `data/research/build_orders.json`, which `--build-orders`
fills from the event logs on the ledger branch: a 5 MB log a game is too
much to read at every publish, and a finished game's opening never changes.
A publish reads off their logs the few openings neither holds
(`BUILD_READ_AT_PUBLISH`): the games played since the snapshot was filled by
a harness still on an older reader. Its games are
every finished game against Deity on the run ledger with one CIVVIS agent in
it, counted as the civ table counts them (a win, or a loss: a rival's
victory, our elimination, a game conceded early), less the games whose
opening is in no log (a game reloaded from a save whose first run was never
published). The record holds it under `build_orders`.

The tree stood on this page for one day. It moved to Civ Analytics
(`beta/civs.html`) on 2026-11-23 (operator: "add another page: Civ
Analytics. First can be the civ records (pulled from win analytics), then
early build order"), whose filler (`beta/civs_page.py`) takes it from the
record built here, so it is read one way for both. A page with no place for
it, Win Analytics since then, is not sent it (`for_page`).

Like the home page's tables this runs inside the script that publishes the
whole site, so what it cannot read costs the page its figures, said on
stderr, and never costs the site its publish. A lane is filled by its own
revision's copy of this script from its own revision's snapshot (`fill` in
`beta/publish.sh`), and both lanes count the games against the AI off the
run ledger as of the build. Until 2026-11-22 both lanes were sent head's
figures by head's script, while the page a lane served was its own
revision's: a page from before the level picker reads the Deity games under
`deity`, and `for_page` still hands such a page that.
"""

from __future__ import annotations

import datetime
import json
import pathlib
import re
import subprocess
import sys

import home_rankings as ranks

SLOT = re.compile(r'(<script type="application/json" id="research-data">)(.*?)(</script>)', re.S)

#: The snapshot of the self-play games, in the source tree.
SELF_PLAY = pathlib.Path("data/research/self_play_endings.json")

#: Civilization VI's victory types, in the order the page lists them (and so
#: the order of their colours), under the keys the simulator's rows use. The
#: ledger's names map onto them through `LEDGER`.
VICTORIES = ["science", "culture", "diplomatic", "religious", "domination", "score"]
LEDGER = {
    "VICTORY_TECHNOLOGY": "science",
    "VICTORY_CULTURE": "culture",
    "VICTORY_DIPLOMATIC": "diplomatic",
    "VICTORY_RELIGIOUS": "religious",
    "VICTORY_CONQUEST": "domination",
    "VICTORY_SCORE": "score",
}

#: The table both kinds are played at, and the game's length in turns.
TURNS = 250
TABLE = {"players": 6, "map": "Continents.lua"}

#: The level the page opens on: Deity, the level most of the games against
#: the AI were played at.
OPENING_LEVEL = 8

#: The levels the page lists, on its slider and in its tables of win rates:
#: Prince to Deity, the levels the benchmark tournament deals (operator
#: 2026-11-23: "ai level in charts can go from lv 4 to lv 8"). A game against
#: a lower level is in no chart and not in the record.
LEVELS = [4, 5, 6, 7, 8]

#: The windows, newest first: days back from a kind's newest game (`None` is
#: every game).
WINDOWS = [("7", 7), ("30", 30), ("all", None)]

#: Self-play's two shorter windows, in days of 24 hours back from the moment
#: its newest game finished (operator 2026-11-24: "add 3 days and 1 day
#: options please. 1 day covers last 24 hrs"). They are counted when the
#: snapshot is taken, from each game's own finish.
RECENT = [("1", 1), ("3", 3)]

#: The tournaments' definitions, in the source tree, and what the tables
#: of win rates list (operator 2026-11-23): their rows, the levels from Deity
#: down to Prince ("order the cols lv 8 down to level 4"), their columns, the
#: days back from the newest tournament game ("a last 1 day win rate col,
#: last 3 days, last 7 days, last 30 days"), and the tables themselves, the
#: games with the Diplomatic victory on before the games with it off (he
#: first asked for "a table on the left the is with diplo excluded and a
#: table on the right that allows diplo wins", and later that day reversed
#: it: "put diplo victory allowed to the left of no diplo victory"), each
#: with the levels it lists. The third, the games with the Religious victory
#: off as well (operator 2026-11-27: "add a third category to the tournament
#: for lv 8 deity no diplo no religion"), lists Deity alone, the one level
#: they are played at.
TOURNAMENTS = pathlib.Path("data/tournaments.json")
RATE_LEVELS = LEVELS[::-1]
RATE_DAYS = [1, 3, 7, 30]
RATE_TABLES = [("on", RATE_LEVELS), ("off", RATE_LEVELS), ("religion_off", [8])]

#: The snapshot of the openings, in the source tree, and what the tree of
#: build orders lists (operator 2026-11-23: "early build order win rates for
#: deity games"): the games against this level, this many builds deep, and
#: the builds after an opening only when at least this many games share it
#: (under one game stands a single line of 0% or 100%).
BUILD_ORDERS = pathlib.Path("data/research/build_orders.json")
BUILD_LEVEL = 8
BUILD_DEPTH = 8
BUILD_BRANCH = 2

#: A step of a build order the capital dropped unfinished, as the step's
#: item stands among a game's builds and in the snapshot's `items`
#: (`UNIT_SCOUT:dropped`; operator 2026-11-24: show a dropped item, marked).
DROPPED = ":dropped"

#: How many games' openings a publish reads off their logs when neither the
#: snapshot nor a run summary holds them as the reader reads now: the games
#: since the snapshot was last filled whose summaries were written by a
#: harness still on an older reader, and the few whose opening is in no log.
BUILD_READ_AT_PUBLISH = 200

#: A build's name where Civilization VI's own is not its type's words: the
#: buildings and districts the game names otherwise, and the units that
#: replace another for one civ, whose type leads with the civ.
BUILD_NAMES = {
    "BUILDING_WALLS": "Ancient Walls",
    "BUILDING_GOV_TALL": "Audience Chamber",
    "BUILDING_GOV_WIDE": "Ancestral Hall",
    "BUILDING_GOV_CONQUEST": "Warlord\u2019s Throne",
    "BUILDING_GOV_CITYSTATES": "Foreign Ministry",
    "BUILDING_GOV_SPIES": "Intelligence Agency",
    "BUILDING_GOV_FAITH": "Grand Master\u2019s Chapel",
    "DISTRICT_THEATER": "Theater Square",
    "DISTRICT_GOVERNMENT": "Government Plaza",
    "UNIT_SUMERIAN_WAR_CART": "War-Cart",
    "UNIT_MAYAN_HULCHE": "Hul\u2019che",
    "UNIT_EGYPTIAN_CHARIOT_ARCHER": "Maryannu Chariot Archer",
    "PROJECT_ENHANCE_DISTRICT_HOLY_SITE": "Holy Site Prayers",
    "PROJECT_ENHANCE_DISTRICT_CAMPUS": "Campus Research Grants",
    "PROJECT_ENHANCE_DISTRICT_THEATER": "Theater Square Festival",
    "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB": "Commercial Hub Investment",
    "PROJECT_ENHANCE_DISTRICT_HARBOR": "Harbor Shipping",
    "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT": "Encampment Training",
    "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE": "Industrial Zone Logistics",
}
BUILD_CIVS = (
    "AMERICAN", "ARABIAN", "AZTEC", "BABYLONIAN", "BRAZILIAN", "BYZANTINE", "CHINESE", "COLOMBIAN", "CREE",
    "DUTCH", "EGYPTIAN", "ENGLISH", "ETHIOPIAN", "FRENCH", "GAUL", "GEORGIAN", "GERMAN", "GREEK", "INCA",
    "INDIAN", "INDONESIAN", "JAPANESE", "KHMER", "KONGO", "KOREAN", "MACEDONIAN", "MALI", "MAORI", "MAPUCHE",
    "MAYAN", "MONGOLIAN", "NORWEGIAN", "NUBIAN", "OTTOMAN", "PERSIAN", "PHOENICIA", "POLISH", "PORTUGUESE",
    "ROMAN", "RUSSIAN", "SCOTTISH", "SCYTHIAN", "SPANISH", "SUMERIAN", "SWEDISH", "VIETNAMESE", "ZULU",
)


def per_turn(turns: dict[int, int]) -> list[int]:
    """The count on each turn of the game, turn 1 first.

    A victory the simulator finds as the last turn ends is stamped with the
    turn after it (a handful of Culture victories read 251), so that turn is
    the last one's. Any other turn outside the game is dropped."""
    out = [0] * TURNS
    for turn, count in turns.items():
        if turn == TURNS + 1:
            turn = TURNS
        if 1 <= turn <= TURNS:
            out[turn - 1] += count
    return out


def median(turns: dict[int, int]) -> int | None:
    """The middle turn of a set of endings (the lower middle of an even set)."""
    total = sum(turns.values())
    if not total:
        return None
    seen = 0
    for turn in sorted(turns):
        seen += turns[turn]
        if 2 * seen >= total:
            return turn
    return None


def window(groups: list[dict], days: int | None) -> dict | None:
    """One window of one kind, from its groups (a batch or a day each, with a
    `day`, a `games` count and per victory a turn -> count map in `turns`,
    and CIVVIS wins the same way in `won`)."""
    if not groups:
        return None
    newest = max(g["day"] for g in groups)
    since = None
    if days is not None:
        since = (datetime.date.fromisoformat(newest) - datetime.timedelta(days=days - 1)).isoformat()
    kept = [g for g in groups if since is None or g["day"] >= since]
    games = sum(g["games"] for g in kept)
    victories = {}
    for victory in VICTORIES:
        turns: dict[int, int] = {}
        won: dict[int, int] = {}
        for g in kept:
            for turn, count in g["turns"].get(victory, {}).items():
                turns[int(turn)] = turns.get(int(turn), 0) + count
            for turn, count in g.get("won", {}).get(victory, {}).items():
                won[int(turn)] = won.get(int(turn), 0) + count
        victories[victory] = {
            "games": sum(turns.values()),
            "won": sum(won.values()),
            "median": median(turns),
            "turns": per_turn(turns),
        }
        if won:
            # Only where there is a win to place: self-play has none, and a
            # row of 250 zeros per victory and window is weight the page
            # reads as the same thing when it is missing.
            victories[victory]["won_turns"] = per_turn(won)
    return {
        "from": min(g["day"] for g in kept),
        "to": newest,
        "games": games,
        "victories": victories,
    }


#: Where a run's summary sits on the ledger branch.
SUMMARY_REF = "refs/remotes/origin/ledger:runs/{tag}/summary.json"


def summaries(source: pathlib.Path, tags: list[str], said) -> dict[str, object]:
    """What each run's summary on the ledger branch says (`said`, asked of the
    summary), read in one pass. A run whose summary is missing or unreadable
    is not in the answer."""
    asked = "".join(SUMMARY_REF.format(tag=tag) + "\n" for tag in tags).encode()
    try:
        shown = subprocess.run(
            ["git", "-C", str(source), "cat-file", "--batch"], input=asked, capture_output=True, check=False
        )
    except OSError:
        return {}
    out, at, found = shown.stdout, 0, {}
    for tag in tags:
        end = out.find(b"\n", at)
        if end < 0:
            break
        head = out[at:end].split()
        at = end + 1
        if len(head) != 3 or not head[2].isdigit():
            continue  # "<name> missing"
        size = int(head[2])
        body, at = out[at : at + size], at + size + 1
        try:
            summary = json.loads(body)
        except ValueError:
            continue
        if isinstance(summary, dict):
            found[tag] = said(summary)
    return found


def limits(source: pathlib.Path, tags: list[str]) -> dict[str, tuple]:
    """Each run's speed and turn limit, `(speed, max_turns)`, from its summary
    on the ledger branch."""
    return summaries(source, tags, lambda summary: (summary.get("speed"), summary.get("max_turns")))


def victory_on(summary: dict, victory: str) -> bool | None:
    """Whether a run was played with a victory (`diplomatic`, `religious`)
    on, as its summary says. A hotseat table's names what the table was
    asked to switch off (`victories_off`); a single-player game's carries
    what the game itself reported once the board was up (`seat.victories`, a
    switch per victory type). `None` when it says neither: a launch that
    never reached a board, or a run from before the game reported its
    switches."""
    off = summary.get("victories_off")
    if isinstance(off, list):
        return f"VICTORY_{victory.upper()}" not in off
    seat = summary.get("seat")
    reported = seat.get("victories") if isinstance(seat, dict) else None
    on = reported.get(victory) if isinstance(reported, dict) else None
    return on if isinstance(on, bool) else None


def diplomatic(summary: dict) -> bool | None:
    """Whether a run was played with the Diplomatic victory on (`victory_on`)."""
    return victory_on(summary, "diplomatic")


def religious(summary: dict) -> bool | None:
    """Whether a run was played with the Religious victory on (`victory_on`)."""
    return victory_on(summary, "religious")


def switches(source: pathlib.Path, tags: list[str]) -> dict[str, bool]:
    """Each run's Diplomatic victory, on or off, from its summary on the
    ledger branch. A run whose summary does not say is not in the answer."""
    return {tag: on for tag, on in summaries(source, tags, diplomatic).items() if on is not None}


def rate_table(summary: dict) -> str | None:
    """The table of win rates a run's game stands in (`RATE_TABLES`), as its
    summary says: `on` with the Diplomatic victory on, `off` with it off,
    and `religion_off` with the Religious victory off as well, which is
    never also `off`. A summary silent on the Religious victory is a game
    with it on, as every game was before the third table. `None` when it
    says nothing of the Diplomatic victory, and for a game with only the
    Religious victory off, which no table lists."""
    on = diplomatic(summary)
    if on is None:
        return None
    if religious(summary) is False:
        return None if on else "religion_off"
    return "on" if on else "off"


def tables_of(source: pathlib.Path, tags: list[str]) -> dict[str, str]:
    """Each run's table of win rates (`rate_table`), from its summary on the
    ledger branch. A run whose summary names none is not in the answer."""
    return {tag: name for tag, name in summaries(source, tags, rate_table).items() if name}


def full_length(limit: tuple | None) -> bool:
    """Whether a run was played at Online speed to at least the game's 250 turns."""
    if not limit:
        return False
    speed, max_turns = limit
    return speed == "GAMESPEED_ONLINE" and isinstance(max_turns, int) and max_turns >= TURNS


def level_name(level: int) -> str:
    """A level as the site writes it: `Lv 4 Prince`."""
    return f"Lv {level} {ranks.CIV6_RUNGS[level - 1].title()}"


def level_groups(rows: list[dict], limit=lambda tags: {}) -> dict[int, list[dict]]:
    """The verification games that ended in a victory, by the level the
    game's AI played at (Deity is 8), of the levels the page lists
    (`LEVELS`): for each level a group per day a game began.
    `limit` answers each ending run's speed and turn limit (`limits`);
    a game not played to the full length is left out. So is a table with more
    than one CIVVIS agent at it (its row names them all, `A+B`): either agent
    winning is a win there, which is not one agent's chance against the AI.
    A level with no game counted is not in the answer."""
    games: dict[str, list[dict]] = {}
    for row in rows:
        if ranks.level(row.get("difficulty")) not in LEVELS or "+" in str(row.get("civ") or ""):
            continue
        if any(row.get(key) != value for key, value in TABLE.items()):
            continue
        key = row.get("game") or row.get("tag")
        if key and row.get("outcome") in ranks.OUTCOMES:
            games.setdefault(key, []).append(row)
    endings = {}
    for key, runs in games.items():
        ending = min(runs, key=lambda row: ranks.OUTCOMES.index(row["outcome"]))
        victory = LEDGER.get(ending.get("victory"))
        turn = ending.get("turns")
        if ending["outcome"] in ("win", "rival_victory") and victory and isinstance(turn, int) and 1 <= turn <= TURNS:
            endings[key] = (ending, victory, turn)
    known = limit([ending.get("tag") or key for key, (ending, _, _) in endings.items()])
    levels: dict[int, dict[str, dict]] = {}
    for key, (ending, victory, turn) in endings.items():
        if not full_length(known.get(ending.get("tag") or key)):
            continue
        day = ranks.began(key, games[key])
        days = levels.setdefault(ranks.level(ending.get("difficulty")), {})
        group = days.setdefault(day, {"day": day, "games": 0, "turns": {}, "won": {}})
        group["games"] += 1
        tally = group["turns"].setdefault(victory, {})
        tally[turn] = tally.get(turn, 0) + 1
        if ending["outcome"] == "win":
            tally = group["won"].setdefault(victory, {})
            tally[turn] = tally.get(turn, 0) + 1
    return {level: sorted(days.values(), key=lambda g: g["day"]) for level, days in sorted(levels.items())}


def self_play_groups(snapshot: dict) -> list[dict]:
    """The self-play batches of the snapshot, a group per batch, dated by the
    day its last game finished."""
    groups = []
    for batch in snapshot.get("batches", []):
        groups.append(
            {
                "day": str(batch["ended"]),
                "games": int(batch["games"]),
                "turns": {v: {int(t): int(n) for t, n in batch["turns"].get(v, {}).items()} for v in VICTORIES},
            }
        )
    return groups


#: A run tag's stamp, the moment the game was launched, and a result line's.
STAMP = re.compile(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z")
STAMPED = re.compile(r"(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z")


def started(key: str, ending: dict) -> datetime.datetime | None:
    """When a game began, to the second: the stamp in its tag
    (`civvis-t7-20261123T042112Z`), and for a tag with no stamp the moment
    its ending was recorded."""
    m = STAMP.search(key) or STAMPED.fullmatch(str(ending.get("utc") or ""))
    try:
        return datetime.datetime(*map(int, m.groups())) if m else None
    except ValueError:
        return None


def tournament_games(rows: list[dict], specs: list[dict], switch=lambda tags: {}) -> list[tuple]:
    """Every finished game of every tournament, at a level the tables list:
    when it began, the level the game's AI played at, whether CIVVIS won it,
    and the table it stands in (`on`, `off` or `religion_off`). The games and
    their endings are `home_rankings.tournament_series`'s own, the count the
    Tournaments page shows; a game counts once, whatever series it stands
    in. `switch` answers each run's table (`tables_of`), asked of the run a
    game ended in and, where that run does not say, of the game's first; a
    game neither says it for stands in no table and is left out."""
    games: dict[str, tuple] = {}
    for series in ranks.tournament_series(rows, specs, detail=True) or []:
        for key, ending in series["played"]:
            at = started(key, ending)
            if ranks.level(ending.get("difficulty")) in RATE_LEVELS and at:
                games[key] = (at, ending)
    known = switch(sorted({tag for key, (_, ending) in games.items() for tag in (ending.get("tag") or key, key)}))
    out = []
    for key, (at, ending) in games.items():
        table = known.get(ending.get("tag") or key, known.get(key))
        if table is not None:
            out.append((at, ranks.level(ending.get("difficulty")), ending["outcome"] == "win", table))
    return out


def win_rates(games: list[tuple]) -> dict | None:
    """The tables of win rates (`RATE_TABLES`), the games with the Diplomatic
    victory on, the games with it off, and the Deity games with the Religious
    victory off as well: in each, every level it lists with its games and
    wins in each window of `RATE_DAYS`. A game at a level its table does not
    list is in none. A window is that many days of 24 hours back from the
    newest game of them all, in any table, so every cell is the same days'
    games, and a level with no game in a window is listed with none. `games`
    is what the widest window holds, in a table and in all. `None` without a
    game."""
    listed = dict(RATE_TABLES)
    games = [game for game in games if game[1] in listed.get(game[3], ())]
    if not games:
        return None
    newest = max(at for at, _, _, _ in games)
    tables = {}
    for name, rows in RATE_TABLES:
        levels = []
        for level in rows:
            windows = []
            for days in RATE_DAYS:
                since = newest - datetime.timedelta(days=days)
                won = [won for at, played, won, table in games if played == level and table == name and at > since]
                windows.append({"days": days, "games": len(won), "wins": sum(won)})
            levels.append({"level": level, "windows": windows})
        tables[name] = {"games": sum(row["windows"][-1]["games"] for row in levels), "levels": levels}
    return {
        "to": newest.strftime("%Y-%m-%d %H:%MZ"),
        "days": RATE_DAYS,
        "games": sum(table["games"] for table in tables.values()),
        "tables": tables,
    }


def build_name(item: str) -> str:
    """A build as the page writes it: `UNIT_SCOUT` is Scout, `DISTRICT_HOLY_SITE`
    Holy Site, and `UNIT_GREEK_HOPLITE` Hoplite, as the game names it; an
    item the capital dropped (`UNIT_SCOUT:dropped`, `DROPPED`) is
    "Scout, dropped"."""
    if item.endswith(DROPPED):
        return build_name(item[: -len(DROPPED)]) + ", dropped"
    if item in BUILD_NAMES:
        return BUILD_NAMES[item]
    kind, _, words = item.partition("_")
    if kind == "UNIT":
        civ, _, rest = words.partition("_")
        if civ in BUILD_CIVS and rest:
            words = rest
    return words.replace("_", " ").title() if words else item


def build_games(rows: list[dict], level: int = BUILD_LEVEL) -> list[tuple]:
    """Every finished game against the AI at `level` with one CIVVIS agent in
    it: its key, its runs' tags with the game's first run leading (the run
    whose log holds the opening), whether CIVVIS won it, and the day it
    began. A game counts once however many runs it took and ended the best
    way any of them did, as `home_rankings.civ_ranking` counts it: a win, or
    a loss (a rival's victory, our elimination, a game conceded early); a
    game somebody stopped has no result and is not in the answer."""
    games: dict[str, list[dict]] = {}
    for row in rows:
        if ranks.level(row.get("difficulty")) != level or "+" in str(row.get("civ") or ""):
            continue
        key = row.get("game") or row.get("tag")
        if key and row.get("outcome") in ranks.OUTCOMES:
            games.setdefault(key, []).append(row)
    out = []
    for key, runs in games.items():
        ending = min(runs, key=lambda row: ranks.OUTCOMES.index(row["outcome"]))
        if ending["outcome"] == "unfinished":
            continue
        tags = [key] + sorted({str(row["tag"]) for row in runs if row.get("tag")} - {key})
        out.append((key, tags, ending["outcome"] == "win", ranks.began(key, runs)))
    return out


def ladder():
    """`tools/civ6_ladder.py`, which reads a game's opening off its log
    (`opening_builds`) and says how it reads (`BUILD_ORDER_READING`)."""
    tools = str(pathlib.Path(__file__).resolve().parent.parent / "tools")
    if tools not in sys.path:
        sys.path.insert(0, tools)
    import civ6_ladder

    return civ6_ladder


def builds_named(order: object) -> list[str] | None:
    """A run's `build_order` (`[[item, turn], ...]`, a step the capital
    dropped `[item, turn, "dropped"]`) as its steps alone, a dropped one
    marked (`UNIT_SCOUT:dropped`), or `None` when it holds none."""
    if not isinstance(order, list):
        return None
    dropped = ladder().BUILD_DROPPED
    names = [
        step[0] + (DROPPED if step[2:3] == [dropped] else "")
        for step in order
        if isinstance(step, list) and step and isinstance(step[0], str)
    ]
    return names or None


def openings(source: pathlib.Path, tags: list[str]) -> dict[str, list[str]]:
    """Each run's opening, the capital's build order, from its summary on
    the ledger branch, when the summary's was read the way the reader reads
    now (`build_order_reading`). A run whose summary carries none (a run
    from before the harness wrote it, or one that began from a save), or
    one read another way, is not in the answer."""
    reading = ladder().BUILD_ORDER_READING

    def said(summary: dict) -> list[str] | None:
        if summary.get("build_order_reading") != reading:
            return None
        return builds_named(summary.get("build_order"))

    read = summaries(source, tags, said)
    return {tag: names for tag, names in read.items() if names}


def held_openings(snapshot: dict) -> dict[str, list[str]]:
    """The snapshot's games, each under its key with its build order by
    name; none when the snapshot was read another way than the reader reads
    now (its `reading`)."""
    if snapshot.get("reading") != ladder().BUILD_ORDER_READING:
        return {}
    items = snapshot.get("items") or []
    return {str(key): [items[at] for at in builds] for key, builds in snapshot.get("games") or []}


def build_orders(
    games: list[tuple], known: dict[str, list[str]], depth: int = BUILD_DEPTH, branch: int = BUILD_BRANCH
) -> dict | None:
    """The tree of build orders over `games` (`build_games`), `known` being
    each opening on record under its run's tag or its game's key.

    A node is `[item, games, wins]`, and `[item, games, wins, [nodes]]` where
    the builds after it are listed: `item` is its place in `items`, the
    builds' names, and `games` the games whose capital built exactly the
    builds down to this one, in this order. The nodes of one list stand best
    win rate first (operator: "we should sort the tree by win rates
    descending. so the very first thing is: the winngest first build,
    underneath the second winnest first bulid"), a tie going to the build
    with more games, the same rate over more of them, then to its name. A
    game with fewer builds than the tree is deep ends at its last, so the
    nodes under a build can hold fewer games than it does. A build the
    capital dropped unfinished (`DROPPED`) is not a node: the game's builds
    close up over it (operator 2026-11-26: "lets also not list the dropped in
    the trie for Capital Build ORder"). `None` when no game's opening is on
    record."""
    root: dict = {"games": 0, "wins": 0, "next": {}}
    days = []
    for key, tags, won, day in games:
        builds = next((known[tag] for tag in tags if known.get(tag)), None)
        builds = [item for item in builds or [] if not item.endswith(DROPPED)]
        if not builds:
            continue
        days.append(day)
        node = root
        node["games"] += 1
        node["wins"] += won
        for item in builds[:depth]:
            node = node["next"].setdefault(item, {"games": 0, "wins": 0, "next": {}})
            node["games"] += 1
            node["wins"] += won
    if not root["games"]:
        return None
    seen: list[str] = []

    def listed(node: dict) -> list:
        if node["games"] < branch:
            return []
        rows = sorted(
            node["next"].items(),
            key=lambda row: (-row[1]["wins"] / row[1]["games"], -row[1]["games"], build_name(row[0])),
        )
        out = []
        for item, child in rows:
            if item not in seen:
                seen.append(item)
            below = listed(child)
            out.append([seen.index(item), child["games"], child["wins"], *([below] if below else [])])
        return out

    tree = listed(root)
    return {
        "level": BUILD_LEVEL,
        "games": root["games"],
        "wins": root["wins"],
        "finished": len(games),
        "from": min(days),
        "to": max(days),
        "depth": depth,
        "items": [build_name(item) for item in seen],
        "tree": tree,
    }


def kind(name: str, groups: list[dict], recent: dict | None = None) -> dict | None:
    """One kind of game as the page reads it: its windows, the shorter ones
    (`recent`, self-play's alone) before the rest."""
    windows = {key: window(groups, days) for key, days in WINDOWS}
    if not windows["all"]:
        return None
    return {"name": name, "windows": {**(recent or {}), **windows}}


def recent_windows(snapshot: dict) -> dict:
    """Self-play's windows of `RECENT` as the page reads them, from the
    snapshot's `last`: each a window like any other, with the hours it
    covers (`hours`) and the moment they are counted back from (`until`, the
    newest game's finish, to the second). A snapshot taken before these
    windows were kept has none, and the page then offers none."""
    last = snapshot.get("last") or {}
    until = STAMPED.fullmatch(str(last.get("to") or ""))
    out = {}
    for key, days in RECENT:
        held = (last.get("windows") or {}).get(key)
        if not until or not held or not held.get("games"):
            continue
        group = {
            "day": last["to"][:10],
            "games": int(held["games"]),
            "turns": {v: {int(t): int(n) for t, n in held["turns"].get(v, {}).items()} for v in VICTORIES},
        }
        counted = window([group], None)
        counted["from"] = str(held["from"])[:10]
        counted["hours"] = 24 * days
        counted["until"] = last["to"]
        out[key] = counted
    return out


def tournament_rates(source: pathlib.Path, rows: list[dict] | None) -> dict | None:
    """The Firaxis Civ 6 AI benchmark: CIVVIS's win rate at each level over
    the tournaments' games (`win_rates`), for the run ledger's result lines
    (`rows`, `home_rankings.results`) and the definitions the Tournaments
    page reads. `None` when the lines or the definitions cannot be read, or
    hold no game. Win Analytics shows it as its first thing, and the home
    page shows the same record over its videos (operator 2026-11-24: "above
    the videos, lets copy the Firaxis Civ 6 Benchmarks showing our win rate
    (both diplo on and off), same as the chart is now"), so both are worked
    out here."""
    try:
        if not rows:
            return None
        specs = json.loads((source / TOURNAMENTS).read_text(encoding="utf-8")).get("tournaments") or []
        return win_rates(tournament_games(rows, specs, lambda tags: tables_of(source, tags)))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"research_page: no win rates over the tournaments' games ({error})", file=sys.stderr)
        return None


def research(source: pathlib.Path) -> dict | None:
    """The record, or `None` when neither kind of game can be read."""
    out: dict = {"turns": TURNS, "victories": VICTORIES}
    try:
        snapshot = json.loads((source / SELF_PLAY).read_text(encoding="utf-8"))
        out["self_play"] = kind("CIVVIS self-play", self_play_groups(snapshot), recent_windows(snapshot))
        out["self_play_taken"] = str(snapshot.get("taken") or "")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"research_page: no self-play figures ({error})", file=sys.stderr)
        out["self_play"] = None
    # The games against the AI, a kind per level under the level's number;
    # the page opens on `level`, or on the highest level played when nobody
    # has played that one.
    out["levels"] = {}
    rows = ranks.results(source)
    try:
        played = level_groups(rows, lambda tags: limits(source, tags)) if rows else {}
        for level, groups in played.items():
            games = kind(f"CIVVIS vs {level_name(level)} AI", groups)
            if games:
                out["levels"][str(level)] = games
    except (ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"research_page: no figures against the game's AI ({error})", file=sys.stderr)
        out["levels"] = {}
    # The win rate at each level over the tournaments' games, off the same
    # result lines and the definitions the Tournaments page reads, each
    # game in the table of its own Diplomatic and Religious switches.
    out["tournament_rates"] = tournament_rates(source, rows)
    # The tree of build orders over the games against Deity: each game's
    # opening from the snapshot, from its run's summary for a game the
    # snapshot does not hold (a game played since the harness began writing
    # it the way the reader reads now), and off its log for the few neither
    # holds.
    out["build_orders"] = None
    try:
        if rows:
            try:
                held = held_openings(json.loads((source / BUILD_ORDERS).read_text(encoding="utf-8")))
            except OSError as error:
                print(f"research_page: no snapshot of the openings ({error})", file=sys.stderr)
                held = {}
            games = build_games(rows)
            asked = sorted({tag for key, tags, _, _ in games if key not in held for tag in tags})
            known = {**openings(source, asked), **held}
            unread = [game for game in games if not any(known.get(tag) for tag in game[1])]
            if len(unread) > BUILD_READ_AT_PUBLISH:
                print(
                    f"research_page: {len(unread)} openings neither in the snapshot nor in a summary; "
                    f"reading {BUILD_READ_AT_PUBLISH} off their logs (refresh with --build-orders)",
                    file=sys.stderr,
                )
            known.update(read_openings(source, unread[:BUILD_READ_AT_PUBLISH]))
            out["build_orders"] = build_orders(games, known)
    except (ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        print(f"research_page: no tree of build orders ({error})", file=sys.stderr)
    if not out["self_play"] and not out["levels"] and not out["tournament_rates"] and not out["build_orders"]:
        return None
    out["level"] = OPENING_LEVEL if str(OPENING_LEVEL) in out["levels"] else max(map(int, out["levels"]), default=OPENING_LEVEL)
    return out


# --- The self-play snapshot -------------------------------------------------

#: A batch directory is named for the moment it began: `20261121T083532Z-db65`.
BATCH = re.compile(r"(\d{4})(\d{2})(\d{2})T\d{6}Z-[0-9a-f]+")

#: The table the snapshot keeps: a batch whose header says anything else is
#: left out, so the chart never mixes game lengths or table sizes.
SETUP = {"speed": "online", "turns": TURNS, "players": 6, "map": "continents", "all_seats": True}


def batch_endings(directory: pathlib.Path) -> dict | None:
    """One batch's games: the victory and turn of each, a game per seed.

    Every seat of a game writes a row naming the same winner, victory and
    turn, so the first row of a seed is the game. A batch whose header is not
    the table in `SETUP` is `None`."""
    endings: dict[int, tuple[str, int]] = {}
    for rows in sorted(directory.glob("rows*.jsonl")):
        with rows.open(encoding="utf-8") as lines:
            for line in lines:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("kind") == "header":
                    if any(row.get(key) != value for key, value in SETUP.items()):
                        return None
                    continue
                if row.get("kind") != "game" or row.get("seed") in endings:
                    continue
                if row.get("victory") in VICTORIES and isinstance(row.get("turn"), int):
                    endings[row["seed"]] = (row["victory"], row["turn"])
    return {"games": len(endings), "endings": endings} if endings else None


def part_finishes(part: pathlib.Path) -> dict[int, float]:
    """When each game in one part of a batch finished, by seed, in seconds
    since 1970.

    A game's row carries no clock. It carries how long the game took
    (`secs`), and a part is one worker's games in the order it played them,
    one after the other, written as each ends. So the part's last game
    finished when the file was last written, and each game before it that
    game's length earlier. Measured on the batches of 2026-11-22 to 24: a
    part's lengths add up to the hours between its worker's launch and its
    last write to within the launch's own minutes, 14 parts of a batch each
    landing 20 seconds after the one before, as the workers are started. A
    worker the CPU governor holds mid-game is no exception: the held time is
    in that game's length. A row with no length is read as none, which dates
    it with the game after it."""
    lengths: dict[int, float] = {}
    with part.open(encoding="utf-8") as lines:
        for line in lines:
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if row.get("kind") != "game" or row.get("seed") in lengths:
                continue
            secs = row.get("secs")
            lengths[row["seed"]] = float(secs) if isinstance(secs, (int, float)) and secs > 0 else 0.0
    at = part.stat().st_mtime
    finishes = {}
    for seed in reversed(list(lengths)):
        finishes[seed] = at
        at -= lengths[seed]
    return finishes


def batch_finishes(directory: pathlib.Path) -> dict[int, float]:
    """When each game of a batch finished, by seed: read off its parts
    (`part_finishes`). A game in no part is not in the answer (a batch from
    before workers wrote parts has none), and `snapshot` counts it as
    finished when the batch's rows were last written."""
    finishes: dict[int, float] = {}
    for part in sorted((directory / "parts").glob("part-*.jsonl")):
        for seed, at in part_finishes(part).items():
            finishes.setdefault(seed, at)
    return finishes


def stamp(at: float) -> str:
    """A moment as the snapshot writes it: `2026-11-24T18:45:41Z`."""
    return datetime.datetime.fromtimestamp(at, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def stamped(text: str) -> datetime.datetime:
    """The moment a snapshot's stamp names."""
    return datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)


def last_windows(played: list[tuple[float, str, int]]) -> dict | None:
    """The windows of `RECENT` over `played`, a `(finished, victory, turn)`
    per game: `to`, the moment the newest game finished, and per window the
    games that finished in the days of 24 hours before it, the oldest's
    finish (`from`) and each victory's count on each turn. `None` when there
    is no game."""
    if not played:
        return None
    to = max(at for at, _, _ in played)
    windows = {}
    for key, days in RECENT:
        since = to - days * 24 * 3600
        kept = [(at, victory, turn) for at, victory, turn in played if at > since]
        turns: dict[str, dict[str, int]] = {}
        for _, victory, turn in kept:
            tally = turns.setdefault(victory, {})
            tally[str(turn)] = tally.get(str(turn), 0) + 1
        windows[key] = {
            "hours": 24 * days,
            "from": stamp(min(at for at, _, _ in kept)),
            "games": len(kept),
            "turns": {v: dict(sorted(turns[v].items(), key=lambda item: int(item[0]))) for v in VICTORIES if v in turns},
        }
    return {"to": stamp(to), "windows": windows}


def snapshot(batches: pathlib.Path) -> dict:
    """Every batch under a gene-screen state directory's `batches/`, oldest
    first, as the snapshot file holds them, and the windows of `RECENT`
    already counted (`last`)."""
    out = []
    read_batches = []
    for directory in sorted(p for p in batches.iterdir() if p.is_dir() and BATCH.fullmatch(p.name)):
        read = batch_endings(directory)
        if not read:
            continue
        m = BATCH.fullmatch(directory.name)
        began = f"{m[1]}-{m[2]}-{m[3]}"
        ended = began
        newest = max((p.stat().st_mtime for p in directory.glob("rows*.jsonl")), default=None)
        read_batches.append((directory, newest, read["endings"]))
        if newest is not None:
            ended = datetime.datetime.fromtimestamp(newest, datetime.timezone.utc).date().isoformat()
        turns: dict[str, dict[str, int]] = {}
        for victory, turn in read["endings"].values():
            tally = turns.setdefault(victory, {})
            tally[str(turn)] = tally.get(str(turn), 0) + 1
        out.append(
            {
                "batch": directory.name,
                "began": began,
                "ended": max(began, ended),
                "games": read["games"],
                "turns": {v: dict(sorted(turns[v].items(), key=lambda item: int(item[0]))) for v in VICTORIES if v in turns},
            }
        )
    # The games of the last days, each by its own finish. Only a batch whose
    # rows were written inside the widest window can hold one, so only those
    # batches' parts are read.
    played: list[tuple[float, str, int]] = []
    written_at = [newest for _, newest, _ in read_batches if newest is not None]
    if written_at:
        widest = max(written_at) - max(days for _, days in RECENT) * 24 * 3600
        for directory, newest, endings in read_batches:
            if newest is None or newest <= widest:
                continue
            finishes = batch_finishes(directory)
            played += [(finishes.get(seed, newest), victory, turn) for seed, (victory, turn) in endings.items()]
    data = {
        "about": "When each self-play game ended and by which victory: a batch per entry, a game per seed, and under "
        "`last` the games that finished in the 24 and the 72 hours before the newest did (`to`), counted the same "
        "way. Rebuilt by `beta/research_page.py --self-play`; read by the Win Analytics page.",
        "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent",
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "batches": out,
    }
    last = last_windows(played)
    if last:
        data["last"] = last
    return data


def written(data: dict) -> str:
    """The snapshot as its file holds it: a line per batch, so a refresh's
    diff is the batches that changed."""
    head = {key: value for key, value in data.items() if key not in ("batches", "last")}
    lines = [json.dumps(batch, separators=(",", ":")) for batch in data["batches"]]
    text = json.dumps(head, indent=1)[:-2] + ',\n "batches": [\n' + ",\n".join(lines) + "\n ]"
    if "last" in data:
        # A line for the moment and a line per window, after the batches.
        last = data["last"]
        windows = [f'   {json.dumps(key)}: {json.dumps(held, separators=(",", ":"))}' for key, held in last["windows"].items()]
        text += ',\n "last": {\n  "to": ' + json.dumps(last["to"]) + ',\n  "windows": {\n' + ",\n".join(windows) + "\n  }\n }"
    return text + "\n}\n"


# --- The snapshot of the openings -------------------------------------------

#: Where a run's events sit on the ledger branch, gzipped, and the lines of
#: them `opening_builds` reads: the boards, the purchases and the map's size.
EVENTS_REF = "refs/remotes/origin/ledger:runs/{tag}/events.jsonl.gz"
EVENTS_READ = ('"kind": "state"', '"order_verified"', '"tiles_done"', '"turn_verified"')


def run_events(source: pathlib.Path, tag: str, read: tuple = EVENTS_READ):
    """A run's events off the ledger branch, in order, the lines holding one
    of `read` (the kinds in `EVENTS_READ` unless asked otherwise) alone. The
    log is read as it is asked for, so a reader that stops after the opening
    has read a few turns of a 5 MB log; closing the generator stops the read.
    A run with no log on the branch has no events."""
    import gzip

    show = subprocess.Popen(
        ["git", "-C", str(source), "cat-file", "blob", EVENTS_REF.format(tag=tag)],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    try:
        with gzip.open(show.stdout, "rt", encoding="utf-8", errors="replace") as lines:
            for line in lines:
                if any(mark in line for mark in read):
                    try:
                        yield json.loads(line)
                    except ValueError:
                        continue
    except (OSError, EOFError):
        return
    finally:
        show.kill()
        show.wait()


def read_opening(source: pathlib.Path, tags: list[str]) -> list[str] | None:
    """A game's opening off its runs' event logs, the first that holds one:
    the game's first run, unless that one was never published."""
    civ6_ladder = ladder()
    for tag in tags:
        events = run_events(source, tag)
        try:
            names = builds_named(civ6_ladder.opening_builds(events))
        finally:
            events.close()
        if names:
            return names
    return None


def read_openings(source: pathlib.Path, games: list[tuple], readers: int = 4) -> dict[str, list[str]]:
    """The openings of `games` (`build_games`) read off their logs, each
    under its game's key: a game whose opening is in no log is not in the
    answer."""
    from concurrent.futures import ThreadPoolExecutor

    with ThreadPoolExecutor(readers) as pool:
        read = list(pool.map(lambda game: read_opening(source, game[1]), games))
    return {key: names for (key, _, _, _), names in zip(games, read) if names}


def openings_snapshot(source: pathlib.Path, snapshot: dict | None = None, again: bool = False) -> dict:
    """The snapshot of the openings brought up to the ledger: the games it
    held, and every finished game against Deity it did not, read off the
    game's event log. A game whose run summary carries its opening as the
    reader reads now (`build_order_reading`) is left to the summary, and a
    game whose opening is in no log is left out, to be asked again next
    time. The steps' names are kept once, in `items`, and a game is its key
    and its steps' places there, so a refresh's diff is the games it added.

    A snapshot read another way than the reader reads now (its `reading`,
    `civ6_ladder.BUILD_ORDER_READING`) is read again whole, and `again`
    reads every game afresh, the games whose summaries carry an opening
    among them."""
    reading = ladder().BUILD_ORDER_READING
    snapshot = {} if again or (snapshot or {}).get("reading") != reading else snapshot
    items = list(snapshot.get("items") or [])
    held = {str(key): list(builds) for key, builds in snapshot.get("games") or []}
    games = [game for game in build_games(ranks.results(source) or []) if game[0] not in held]
    if not again:
        carried = openings(source, sorted({tag for _, tags, _, _ in games for tag in tags}))
        games = [game for game in games if not any(tag in carried for tag in game[1])]
    for key, names in sorted(read_openings(source, games, 8 if again else 4).items()):
        for name in names:
            if name not in items:
                items.append(name)
        held[key] = [items.index(name) for name in names]
    return {
        "about": "The capital's build order in each finished game against Lv 8 Deity AI on the run ledger: what it "
        "finished, and what it dropped unfinished marked `:dropped`, in order; the game's key and its steps as "
        "places in `items`. Read off the game's event log by `civ6_ladder.opening_builds`, the way `reading` names "
        "(`BUILD_ORDER_READING`); a game whose run summary carries `build_order` read that way is not here. "
        "Filled by `beta/research_page.py --build-orders`; read by Civ Analytics.",
        "level": BUILD_LEVEL,
        "reading": reading,
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "items": items,
        "games": [[key, builds] for key, builds in sorted(held.items())],
    }


def written_openings(data: dict) -> str:
    """The snapshot of the openings as its file holds it: a line per game."""
    head = {key: value for key, value in data.items() if key != "games"}
    lines = [json.dumps(game, separators=(",", ":")) for game in data["games"]]
    return json.dumps(head, indent=1)[:-2] + ',\n "games": [\n' + ",\n".join(lines) + "\n ]\n}\n"


# --- The page ---------------------------------------------------------------

#: The level picker as the page carries it. A page without it is a lane
#: pinned to a revision from before the picker (2026-11-21): it draws the
#: Deity games alone and reads them under `deity`, not `levels`.
LEVEL_PICKER = 'id="res-level"'

#: The tree of build orders as a page carries it: Win Analytics did on
#: 2026-11-23 only, and Civ Analytics since.
BUILD_TREE = 'id="res-builds"'


def for_page(page: str, data: dict | None) -> dict | None:
    """The record in the shape this page reads.

    A page is filled by its own revision's script now (`beta/publish.sh`),
    so the two agree by construction; this is for a page older than the
    script filling it, as the stable lane's was while both lanes were filled
    by head's. Sent `levels`, a page from before the picker would find no
    `deity` and show its Deity charts empty, so it is sent the Deity games
    where it looks for them and nothing it has no place for. A page with no
    tree of build orders is not sent one."""
    if data is None:
        return data
    if LEVEL_PICKER in page:
        return data if BUILD_TREE in page else {key: value for key, value in data.items() if key != "build_orders"}
    out = {key: value for key, value in data.items() if key not in ("levels", "level", "tournament_rates", "build_orders")}
    deity = data.get("levels", {}).get(str(ranks.CIV6_RUNGS.index("deity") + 1))
    out["deity"] = {**deity, "name": "CIVVIS vs Deity AI"} if deity else None
    return out


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "--self-play":
        data = snapshot(pathlib.Path(argv[2]))
        out = pathlib.Path(argv[3])
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written(data), encoding="utf-8")
        games = sum(b["games"] for b in data["batches"])
        print(f"research_page: {games} self-play games in {len(data['batches'])} batches into {out}", file=sys.stderr)
        return 0
    if len(argv) in (4, 5) and argv[1] == "--build-orders" and argv[4:] in ([], ["--again"]):
        out = pathlib.Path(argv[3])
        try:
            before = json.loads(out.read_text(encoding="utf-8"))
        except OSError:
            before = None
        data = openings_snapshot(pathlib.Path(argv[2]), before, again=argv[4:] == ["--again"])
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written_openings(data), encoding="utf-8")
        fresh = argv[4:] == ["--again"] or (before or {}).get("reading") != data["reading"]
        added = len(data["games"]) - (0 if fresh else len((before or {}).get("games") or []))
        print(
            f"research_page: {len(data['games'])} openings in {out}, {added} of them "
            + ("read afresh" if fresh else "new"),
            file=sys.stderr,
        )
        return 0
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = research(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    text = page.read_text(encoding="utf-8")
    data = for_page(text, data)
    filled = fill(text, data)
    if filled is None:
        print(f"research_page: {page} has no research slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"research_page: no figures to write into {page}", file=sys.stderr)
    else:
        kinds = [*data.get("levels", {}).values(), data.get("deity"), data.get("self_play")]
        said = [f"{k['windows']['all']['games']} {k['name']} games" for k in kinds if k]
        if data.get("tournament_rates"):
            said.append(f"{data['tournament_rates']['games']} tournament games")
        if data.get("build_orders"):
            said.append(f"{data['build_orders']['games']} build orders")
        print(f"research_page: wrote {' and '.join(said)} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
