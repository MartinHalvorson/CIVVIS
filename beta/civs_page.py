#!/usr/bin/env python3
"""The Civ Analytics page's figures: the early build orders that win
against Deity, when the second and third cities went down, the faith chosen,
and how CIVVIS does as each civ.

    civs_page.py <source_tree>              print the record as JSON
    civs_page.py <source_tree> <page.html>  write it into the page
    civs_page.py --marks <source_tree> <out.json>
                                            add the games played since to
                                            the snapshot of the cities and
                                            faith
    civs_page.py --self-play <batches_dir> <out.json>
                                            rebuild the self-play snapshot

The Civ Analytics page (`beta/civs.html`, served at
`<lane>/charts/civ-analytics/`) is the third page under Charts/, after ELO
Rankings (operator 2026-11-23: "add another page: Civ Analytics. First can
be the civ records (pulled from win analytics), then early build order").
From the top (operator 2026-11-24: "top chart is build order", "move
winngest civs to the bottom"):

- **build_orders**, the tree of early build orders against Deity: the
  record `research_page.research` builds (`research_page.build_orders`, the
  openings from the run summaries and `data/research/build_orders.json`).
  It is taken from that record rather than rebuilt here, so a change to how
  an opening is read reaches this page with nothing to keep in step.
- **games**, a row per finished game against Civilization VI's AI at Lv 4
  to Lv 8 (`LEVELS`), what the charts of cities and faith count (operator
  2026-11-23: "i also want to see a 2nd city down vs win rate graph by lv
  default lv 8 and 3rd city down turn win rate"; "a religion founded win
  rate and a no religion win rate, filterable by the typical stuff (lvs,
  etc). we should have a win rate for each pantheon belief and a win rate
  for each religion first belief"). A row is the game's level, its
  Diplomatic victory switch (`research_page.switches`), whether CIVVIS won,
  the day it began, the turns its second and third cities were founded, and
  its pantheon and its religion's two founding beliefs, as places in
  `names`; then how the game stood at two marks and what it aimed for
  (operator 2026-11-24: "reflect on the top most important decisions a civ
  player has to make throughout the game. rank them into a list
  chronologically. make a top chart or two that shows monitors the
  problem. merge your charts into the civ analytics chart chronologically
  as well so the whole thing is really one logic timeline of making
  decisions in civ"): its cities at turn 50 and the best rival's, its
  science at turn 100 as a share of the best rival's, and the victory its
  seat was set to aim for (`progress`). The page counts them itself, for
  the level, switch and days its reader picks. A game's cities and faith (`civ6_ladder.founding_reading`)
  are in its runs' summaries (`founding_marks`) from 2026-11-24, and the
  games before that are in a snapshot, `data/research/game_marks.json`,
  which `--marks` fills from the event logs on the ledger branch.
- **self_play**, the CIVVIS self-play seats batch by batch (every seat a
  CIVVIS AI agent): per batch, each seat's second and third city turns, its
  religion, and its civ, with whether it won. Its games are rows on the
  machine that plays them, not in the repository, so they come from a
  snapshot, `data/research/self_play_seats.json`, which `--self-play`
  rebuilds from the batch directories. A self-play seat has no record of its
  pantheon or beliefs.
- **civs**, the winningest civs against Deity: `home_rankings.civs`, off
  `results.jsonl` on the ledger branch, and beside it the self-play seats'
  civs (`self_play_civs`), at the bottom.

Like the site's other pages this runs inside the script that publishes the
whole site: what it cannot read costs the page those figures, said on
stderr, and never the publish.
"""

from __future__ import annotations

import datetime
import json
import pathlib
import re
import sys

import home_rankings as ranks
import research_page

SLOT = re.compile(r'(<script type="application/json" id="civ-data">)(.*?)(</script>)', re.S)

#: The levels the charts of cities and faith count: Prince to Deity, as Win
#: Analytics does, and the one they open on (operator: "default lv 8").
LEVELS = [4, 5, 6, 7, 8]
OPENING_LEVEL = 8

#: The snapshots, in the source tree.
MARKS = pathlib.Path("data/research/game_marks.json")
SELF_PLAY = pathlib.Path("data/research/self_play_seats.json")

#: The lines of a run's log `founding_reading` reads: the seat, each city we
#: founded, the pantheon and religion orders verified, and the founding.
MARKS_READ = ('"kind": "seat"', '"kind": "found"', '"religion_founded"', '"order_kind": "pantheon"', '"order_kind": "religion"')

#: Words a belief's name keeps in lower case, as the game writes them:
#: "God of the Forge", "Lady of the Reeds and Marshes".
SMALL_WORDS = {"of", "the", "and", "to"}
BELIEF_NAMES = {"BELIEF_CROSS_CULTURAL_DIALOGUE": "Cross-Cultural Dialogue"}


def belief_name(belief: str) -> str:
    """A belief as the game names it: `BELIEF_GOD_OF_THE_FORGE` is God of
    the Forge."""
    if belief in BELIEF_NAMES:
        return BELIEF_NAMES[belief]
    words = belief.removeprefix("BELIEF_").lower().split("_")
    return " ".join(w if i and w in SMALL_WORDS else w.capitalize() for i, w in enumerate(words))


# --- A game's cities and faith ----------------------------------------------


def merged(readings: list[dict | None]) -> dict | None:
    """A game's cities and faith from its runs' readings, the game's first
    run leading: the first run's cities, and after them a later run's cities
    founded past the last of them (a run reloaded from a save plays a few
    turns over again); the first pantheon and religion any run saw. `None`
    when the first run has no reading: its opening is in no log."""
    if not readings or not readings[0]:
        return None
    cities = list(readings[0].get("cities") or [])
    pantheon, religion = readings[0].get("pantheon"), readings[0].get("religion")
    for reading in readings[1:]:
        if not reading:
            continue
        last = cities[-1] if cities else 0
        cities += [turn for turn in reading.get("cities") or [] if turn > last]
        pantheon = pantheon or reading.get("pantheon")
        religion = religion or reading.get("religion")
    return {"cities": cities, "pantheon": pantheon, "religion": religion}


def held_marks(snapshot: dict) -> dict[str, dict]:
    """The snapshot's games, each under its key as `merged` gives it."""
    out = {}
    for key, cities, pantheon, religion in snapshot.get("games") or []:
        out[str(key)] = {
            "cities": list(cities),
            "pantheon": pantheon,
            "religion": dict(zip(("turn", "founder", "follower"), religion)) if religion else None,
        }
    return out


def summary_marks(source: pathlib.Path, games: list[tuple]) -> dict[str, dict]:
    """Each game's cities and faith off its runs' summaries on the ledger
    branch, for a game whose every run's summary carries `founding_marks`
    (a run from 2026-11-24 on). A game that began before has none."""
    tags = sorted({tag for _, game_tags, _, _ in games for tag in game_tags})
    read = research_page.summaries(source, tags, lambda summary: ("founding_marks" in summary, summary.get("founding_marks")))
    out = {}
    for key, game_tags, _, _ in games:
        if all(read.get(tag, (False, None))[0] for tag in game_tags):
            found = merged([read[tag][1] for tag in game_tags])
            if found:
                out[key] = found
    return out


def read_marks(source: pathlib.Path, tags: list[str]) -> dict | None:
    """A game's cities and faith off its runs' event logs (`founding_reading`)."""
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "tools"))
    import civ6_ladder

    readings = []
    for tag in tags:
        events = research_page.run_events(source, tag, MARKS_READ)
        try:
            readings.append(civ6_ladder.founding_reading(events))
        finally:
            events.close()
    return merged(readings)


def finished_games(rows: list[dict]) -> list[tuple]:
    """Every finished game against the AI at Lv 4 to Lv 8 with one CIVVIS
    agent in it, as `research_page.build_games` counts them, with its level:
    `(key, tags, won, day, level)`."""
    return [(*game, level) for level in LEVELS for game in research_page.build_games(rows, level)]


def marks_snapshot(source: pathlib.Path, snapshot: dict | None = None) -> dict:
    """The snapshot of the cities and faith brought up to the ledger: the
    games it held, and every finished game at Lv 4 to Lv 8 it did not, read
    off the game's event logs. A game whose runs' summaries carry their
    reading is left to them, and a game whose first run is in no log is left
    out, to be asked again next time. A game is its key, the turns of its
    first three cities, its pantheon and its religion's turn and founding
    beliefs (founder, then follower), so a refresh's diff is the games it
    added."""
    from concurrent.futures import ThreadPoolExecutor

    snapshot = snapshot or {}
    held = {str(game[0]): game for game in snapshot.get("games") or []}
    games = [game[:4] for game in finished_games(ranks.results(source) or []) if game[0] not in held]
    carried = summary_marks(source, games)
    games = [game for game in games if game[0] not in carried]
    with ThreadPoolExecutor(8) as readers:
        read = list(readers.map(lambda game: read_marks(source, game[1]), games))
    for (key, _, _, _), found in zip(games, read):
        if not found:
            continue
        religion = found["religion"]
        held[key] = [
            key,
            found["cities"][:3],
            found["pantheon"],
            [religion.get("turn"), religion.get("founder"), religion.get("follower")] if religion else None,
        ]
    return {
        "about": "Each finished game against Lv 4 to Lv 8 AI on the run ledger: its key, the turns its first three "
        "cities were founded, its pantheon, and its religion's turn, founder belief and follower belief. Read off "
        "the game's event logs by `civ6_ladder.founding_reading`; a game whose run summaries carry "
        "`founding_marks` is not here. Filled by `beta/civs_page.py --marks`; read by Civ Analytics.",
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "games": [held[key] for key in sorted(held)],
    }


def written_marks(data: dict) -> str:
    """The snapshot as its file holds it: a line per game."""
    head = {key: value for key, value in data.items() if key != "games"}
    lines = [json.dumps(game, separators=(",", ":")) for game in data["games"]]
    return json.dumps(head, indent=1)[:-2] + ',\n "games": [\n' + ",\n".join(lines) + "\n ]\n}\n"


#: The victories a seat can be set to aim for (a run summary's
#: `victory_target`), in Win Analytics' order, and as the game names them.
TARGETS = research_page.VICTORIES
TARGET_NAMES = ["Science", "Culture", "Diplomatic", "Religious", "Domination", "Score"]


def _count(value) -> int:
    """A count off a board, or -1 when the board does not hold one."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value < 0:
        return -1
    return round(value)


def progress(source: pathlib.Path, games: list[tuple]) -> dict[str, dict]:
    """How each game stood at two marks, and what it aimed for, off its
    runs' summaries on the ledger branch (`civ6_ladder.economy_marks`, read
    on the first board of the mark's turn, and `victory_target`):

    - `cities50`, our cities at turn 50, and `rival_cities50`, the most any
      rival we had met held then. At Deity the gap opens in the first fifty
      turns (`civ6_ladder.ECONOMY_MARK_TURNS`).
    - `science100`, our science a turn at turn 100 as a whole percent of the
      best rival's.
    - `victory`, the victory the seat was set to aim for, a place in
      `TARGETS` counted from 1.

    A game's first run leads: a mark comes from the first of its runs whose
    summary holds it (a run reloaded from a save holds only the marks it
    played through). A figure no summary holds is -1, a target 0; a game
    with no summary at all is not in the answer."""
    tags = sorted({tag for game in games for tag in game[1]})
    read = research_page.summaries(source, tags, lambda summary: (summary.get("economy_marks"), summary.get("victory_target")))
    out: dict[str, dict] = {}
    for key, game_tags, *_ in games:
        if not any(tag in read for tag in game_tags):
            continue
        boards: dict[str, dict] = {}
        target = None
        for tag in game_tags:
            economy, aim = read.get(tag, (None, None))
            if isinstance(economy, dict):
                for mark, board in economy.items():
                    if isinstance(board, dict):
                        boards.setdefault(str(mark), board)
            target = target or (aim if aim in TARGETS else None)
        at50, at100 = boards.get("50") or {}, boards.get("100") or {}
        ours, best = _count(at100.get("science")), at100.get("rival_science")
        share = round(100 * at100["science"] / best) if ours >= 0 and _count(best) > 0 else -1
        out[key] = {
            "cities50": _count(at50.get("cities")),
            "rival_cities50": _count(at50.get("rival_cities")),
            "science100": share,
            "victory": TARGETS.index(target) + 1 if target else 0,
        }
    return out


def game_rows(source: pathlib.Path, rows: list[dict], held: dict[str, dict]) -> dict | None:
    """The games the charts of cities and faith count, a row each (`FIELDS`),
    or `None` when none has a reading. `diplo` is 1 for a game played with
    the Diplomatic victory on, 0 off, and 2 when its summary does not say;
    `day` is days after `first`; a city never founded is 0, and so is a
    pantheon or belief the game never had, the others being places in
    `names` counted from 1, and after them what `progress` read (`PROGRESS`:
    -1 for a figure no summary holds, 0 for no target), the targets being
    places in `targets` counted from 1. `finished` is the finished games of
    each level, those without a reading among them."""
    games = finished_games(rows)
    if not games:
        return None
    known = {**summary_marks(source, [game[:4] for game in games if game[0] not in held]), **held}
    marks = progress(source, games)
    switched = research_page.switches(source, sorted({tag for game in games for tag in game[1]}))
    names: list[str] = []

    def place(belief: str | None) -> int:
        if not belief:
            return 0
        name = belief_name(belief)
        if name not in names:
            names.append(name)
        return names.index(name) + 1

    counted = []
    finished: dict[str, int] = {}
    for key, tags, won, day, level in games:
        finished[str(level)] = finished.get(str(level), 0) + 1
        found = known.get(key)
        reached = marks.get(key) or {}
        if not found or not day:
            continue
        cities = found.get("cities") or []
        religion = found.get("religion") or {}
        on = next((switched[tag] for tag in tags if tag in switched), None)
        counted.append((day, [
            level,
            2 if on is None else int(on),
            int(won),
            day,
            cities[1] if len(cities) > 1 else 0,
            cities[2] if len(cities) > 2 else 0,
            place(found.get("pantheon")),
            place(religion.get("founder")),
            place(religion.get("follower")),
            *(reached.get(field, unread) for field, unread in PROGRESS),
        ]))
    if not counted:
        return None
    first = min(day for day, _ in counted)
    start = datetime.date.fromisoformat(first)
    out = []
    for day, row in sorted(counted, key=lambda item: item[0]):
        row[3] = (datetime.date.fromisoformat(day) - start).days
        out.append(row)
    return {
        "fields": FIELDS,
        "first": first,
        "names": names,
        "rows": out,
        "finished": finished,
        "level": OPENING_LEVEL,
        "targets": TARGET_NAMES,
    }


#: The fields `progress` adds to a row after the cities and the faith, each
#: with what it reads when no summary holds it.
PROGRESS = [("cities50", -1), ("rival_cities50", -1), ("science100", -1), ("victory", 0)]
FIELDS = ["level", "diplo", "won", "day", "city2", "city3", "pantheon", "founder", "follower", *(field for field, _ in PROGRESS)]


# --- The self-play seats ------------------------------------------------------


def batch_seats(directory: pathlib.Path) -> dict | None:
    """One batch's seats: per seat-turn of its second and third cities (`0`
    for never), per religion founded or not, and per civ, the seats and
    their wins, `[seats, wins]`. A batch whose header is not the table
    `research_page.SETUP` names is `None`; a field no row of the batch
    carries (a batch from before the rows wrote it) is left out."""
    tallies: dict[str, dict] = {"city2": {}, "city3": {}, "religion": {}, "civs": {}}
    seats = 0
    recorded = False
    for rows in sorted(directory.glob("rows*.jsonl")):
        with rows.open(encoding="utf-8") as lines:
            for line in lines:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("kind") == "header":
                    if any(row.get(key) != value for key, value in research_page.SETUP.items()):
                        return None
                    continue
                if row.get("kind") != "game" or not isinstance(row.get("win"), bool):
                    continue
                seats += 1
                won = int(row["win"])
                # A seat that never had a second city writes no turn for it,
                # and neither does any seat of a batch from before the field:
                # a batch counts its cities only when one of its rows has one.
                cities = "city_two_turn" in row or "city_three_turn" in row
                recorded = recorded or cities
                marks = {
                    "city2": str(row.get("city_two_turn") or 0),
                    "city3": str(row.get("city_three_turn") or 0),
                    "religion": ("1" if row["founded_religion"] else "0") if isinstance(row.get("founded_religion"), bool) else None,
                    "civs": row.get("civ") or None,
                }
                for field, value in marks.items():
                    if value is None:
                        continue
                    tally = tallies[field].setdefault(value, [0, 0])
                    tally[0] += 1
                    tally[1] += won
    if not seats:
        return None
    if not recorded:
        del tallies["city2"], tallies["city3"]
    return {"seats": seats, **{field: tally for field, tally in tallies.items() if tally}}


def self_play_snapshot(batches: pathlib.Path) -> dict:
    """Every batch under a gene-screen state directory's `batches/`, oldest
    first, as the snapshot file holds them."""
    out = []
    for directory in sorted(p for p in batches.iterdir() if p.is_dir() and research_page.BATCH.fullmatch(p.name)):
        read = batch_seats(directory)
        if not read:
            continue
        m = research_page.BATCH.fullmatch(directory.name)
        began = f"{m[1]}-{m[2]}-{m[3]}"
        newest = max((p.stat().st_mtime for p in directory.glob("rows*.jsonl")), default=None)
        ended = datetime.datetime.fromtimestamp(newest, datetime.timezone.utc).date().isoformat() if newest else began
        ordered = {}
        for field in ("city2", "city3"):
            if field in read:
                ordered[field] = dict(sorted(read[field].items(), key=lambda item: int(item[0])))
        if "religion" in read:
            ordered["religion"] = dict(sorted(read["religion"].items()))
        if "civs" in read:
            ordered["civs"] = dict(sorted(read["civs"].items()))
        out.append({"batch": directory.name, "began": began, "ended": max(began, ended), "seats": read["seats"], **ordered})
    return {
        "about": "The self-play seats batch by batch: per turn of the seat's second and third cities (0 for never), "
        "per religion founded (1) or not (0), and per civ, the seats and their wins, [seats, wins]. Every seat a "
        "CIVVIS AI agent. Rebuilt by `beta/civs_page.py --self-play`; read by Civ Analytics.",
        "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent",
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "batches": out,
    }


def written_self_play(data: dict) -> str:
    """The snapshot as its file holds it: a line per batch."""
    head = {key: value for key, value in data.items() if key != "batches"}
    lines = [json.dumps(batch, separators=(",", ":")) for batch in data["batches"]]
    return json.dumps(head, indent=1)[:-2] + ',\n "batches": [\n' + ",\n".join(lines) + "\n ]\n}\n"


def self_play(snapshot: dict) -> dict | None:
    """What the page draws of the self-play seats: each batch's days and its
    tallies of cities and religion, and the civs over every batch, best win
    rate first (`[name, seats, wins]`, a tie going to the civ with more
    seats). `None` when no batch has a tally."""
    batches = [b for b in snapshot.get("batches") or [] if any(k in b for k in ("city2", "city3", "religion"))]
    civs: dict[str, list[int]] = {}
    held = [b for b in snapshot.get("batches") or [] if b.get("civs")]
    for batch in held:
        for civ, (seats, wins) in batch["civs"].items():
            tally = civs.setdefault(civ, [0, 0])
            tally[0] += seats
            tally[1] += wins
    if not batches and not civs:
        return None
    ranked = sorted(civs.items(), key=lambda item: (-item[1][1] / item[1][0], -item[1][0], item[0]))
    return {
        "batches": [
            {"from": b["began"], "to": b["ended"], "seats": b["seats"], **{k: b[k] for k in ("city2", "city3", "religion") if k in b}}
            for b in batches
        ],
        "civs": {
            "rows": [[civ, seats, wins] for civ, (seats, wins) in ranked],
            "seats": sum(seats for seats, _ in civs.values()),
            "from": min((b["began"] for b in held), default=None),
            "to": max((b["ended"] for b in held), default=None),
            "batches": len(held),
        }
        if civs
        else None,
    }


# --- The record ---------------------------------------------------------------


def record(source: pathlib.Path) -> dict | None:
    """The record, or `None` when no figure can be read."""
    out: dict = {"build_orders": None, "games": None, "self_play": None, "civs": None}
    try:
        out["build_orders"] = (research_page.research(source) or {}).get("build_orders")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"civs_page: no tree of build orders ({error})", file=sys.stderr)
    try:
        rows = ranks.results(source)
        if rows:
            try:
                held = held_marks(json.loads((source / MARKS).read_text(encoding="utf-8")))
            except OSError as error:
                print(f"civs_page: no snapshot of the cities and faith ({error})", file=sys.stderr)
                held = {}
            out["games"] = game_rows(source, rows, held)
    except (ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        print(f"civs_page: no games' cities and faith ({error})", file=sys.stderr)
    try:
        out["self_play"] = self_play(json.loads((source / SELF_PLAY).read_text(encoding="utf-8")))
    except (OSError, ValueError, KeyError, TypeError, AttributeError, ZeroDivisionError) as error:
        print(f"civs_page: no self-play seats ({error})", file=sys.stderr)
    try:
        out["civs"] = ranks.civs(source)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"civs_page: no civ records ({error})", file=sys.stderr)
    if not any(out.values()):
        return None
    return out


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "--marks":
        out = pathlib.Path(argv[3])
        try:
            before = json.loads(out.read_text(encoding="utf-8"))
        except OSError:
            before = None
        data = marks_snapshot(pathlib.Path(argv[2]), before)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written_marks(data), encoding="utf-8")
        added = len(data["games"]) - len((before or {}).get("games") or [])
        print(f"civs_page: {len(data['games'])} games' cities and faith in {out}, {added} of them new", file=sys.stderr)
        return 0
    if len(argv) == 4 and argv[1] == "--self-play":
        data = self_play_snapshot(pathlib.Path(argv[2]))
        out = pathlib.Path(argv[3])
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written_self_play(data), encoding="utf-8")
        seats = sum(b["seats"] for b in data["batches"])
        print(f"civs_page: {seats} self-play seats in {len(data['batches'])} batches into {out}", file=sys.stderr)
        return 0
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
        print(f"civs_page: {page} has no civ slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"civs_page: no figures to write into {page}", file=sys.stderr)
    else:
        said = []
        if data.get("build_orders"):
            said.append(f"{data['build_orders'].get('games')} build orders")
        if data.get("games"):
            said.append(f"{len(data['games']['rows'])} games' cities and faith")
        if data.get("self_play"):
            said.append(f"{sum(b['seats'] for b in data['self_play']['batches'])} self-play seats")
        if data.get("civs"):
            said.append(f"{data['civs'].get('games')} games' civ records")
        print(f"civs_page: wrote {', '.join(said)} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
