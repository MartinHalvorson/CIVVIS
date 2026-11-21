#!/usr/bin/env python3
"""The Research page's figures: when in the game each victory type lands.

    research_page.py <source_tree>              print the record as JSON
    research_page.py <source_tree> <page.html>  write it into the page
    research_page.py --self-play <batches_dir> <out.json>
                                                rebuild the self-play snapshot

The Research page (`beta/research.html`, served at `<lane>/research/`) is
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
- **against Deity**: one CIVVIS AI agent inside Civilization VI against the
  game's own AI at Deity (Lv 8). These are the verification games, read off
  `results.jsonl` on the ledger branch at publish, so the figures move with
  every finished game. A game counts once however many runs it took, and only
  a game that ended in a victory, ours or a rival's, has a victory type and a
  turn; an elimination or a stopped game has neither and is left out. Some
  series were played to a shorter limit (170, 160 and 120 turns in the Score
  series of 2026-11-12 to 15), where the limit's Score victory and the
  victories it cut off would bend the chart, so a game counts only when its
  run summary on the ledger (`runs/<tag>/summary.json`) says Online speed and
  a limit of at least 250 turns. A run with no summary is left out too.

The record holds, for three windows of each kind (its last 7 days, its last
30 days, every game), the games counted and for each victory type: the games
it ended, the CIVVIS wins among them (Deity only; in self-play every winner is
a CIVVIS agent), the median turn, and the count on each turn from 1 to 250.
A victory CIVVIS won at least once also holds CIVVIS's wins on each turn
(`won_turns`), so the page can draw the Deity games three times (operator
2026-11-21: "start with all victories by type along timeline, then just the
deity ai wins, then the civvis wins"): what is left of a turn's count after
CIVVIS's wins is the Deity AI's.
The page groups the turns into spans itself; a single turn matters, because
the Diplomatic victories land on the World Congress's own turns (185, 205,
225 and 245 at Online speed).
A window is measured back from the kind's own newest game, so a snapshot left
unrefreshed still shows its last week rather than an empty one, and the page
says which days each window holds.

Like the home page's tables this runs inside the script that publishes the
whole site, so what it cannot read costs the page its figures, said on
stderr, and never costs the site its publish. Both lanes are sent head's
figures: they are facts about the games played, not about a revision.
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
DEITY = {"players": 6, "map": "Continents.lua"}

#: The windows, newest first: days back from a kind's newest game (`None` is
#: every game).
WINDOWS = [("7", 7), ("30", 30), ("all", None)]


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


def limits(source: pathlib.Path, tags: list[str]) -> dict[str, tuple]:
    """Each run's speed and turn limit, `(speed, max_turns)`, from its summary
    on the ledger branch, read in one pass. A run whose summary is missing or
    unreadable is not in the answer."""
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
            found[tag] = (summary.get("speed"), summary.get("max_turns"))
    return found


def full_length(limit: tuple | None) -> bool:
    """Whether a run was played at Online speed to at least the game's 250 turns."""
    if not limit:
        return False
    speed, max_turns = limit
    return speed == "GAMESPEED_ONLINE" and isinstance(max_turns, int) and max_turns >= TURNS


def deity_groups(rows: list[dict], limit=lambda tags: {}) -> list[dict]:
    """The Deity verification games that ended in a victory, a group per day
    the game began. `limit` answers each ending run's speed and turn limit
    (`limits`); a game not played to the full length is left out."""
    games: dict[str, list[dict]] = {}
    for row in rows:
        if ranks.level(row.get("difficulty")) != 8:
            continue
        if any(row.get(key) != value for key, value in DEITY.items()):
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
    days: dict[str, dict] = {}
    for key, (ending, victory, turn) in endings.items():
        if not full_length(known.get(ending.get("tag") or key)):
            continue
        day = ranks.began(key, games[key])
        group = days.setdefault(day, {"day": day, "games": 0, "turns": {}, "won": {}})
        group["games"] += 1
        tally = group["turns"].setdefault(victory, {})
        tally[turn] = tally.get(turn, 0) + 1
        if ending["outcome"] == "win":
            tally = group["won"].setdefault(victory, {})
            tally[turn] = tally.get(turn, 0) + 1
    return sorted(days.values(), key=lambda g: g["day"])


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


def kind(name: str, groups: list[dict]) -> dict | None:
    windows = {key: window(groups, days) for key, days in WINDOWS}
    if not windows["all"]:
        return None
    return {"name": name, "windows": windows}


def research(source: pathlib.Path) -> dict | None:
    """The record, or `None` when neither kind of game can be read."""
    out: dict = {"turns": TURNS, "victories": VICTORIES}
    try:
        snapshot = json.loads((source / SELF_PLAY).read_text(encoding="utf-8"))
        out["self_play"] = kind("CIVVIS self-play", self_play_groups(snapshot))
        out["self_play_taken"] = str(snapshot.get("taken") or "")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"research_page: no self-play figures ({error})", file=sys.stderr)
        out["self_play"] = None
    try:
        rows = ranks.results(source)
        groups = deity_groups(rows, lambda tags: limits(source, tags)) if rows else []
        out["deity"] = kind("CIVVIS vs Deity AI", groups)
    except (ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"research_page: no Deity figures ({error})", file=sys.stderr)
        out["deity"] = None
    if not out["self_play"] and not out["deity"]:
        return None
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


def snapshot(batches: pathlib.Path) -> dict:
    """Every batch under a gene-screen state directory's `batches/`, oldest
    first, as the snapshot file holds them."""
    out = []
    for directory in sorted(p for p in batches.iterdir() if p.is_dir() and BATCH.fullmatch(p.name)):
        read = batch_endings(directory)
        if not read:
            continue
        m = BATCH.fullmatch(directory.name)
        began = f"{m[1]}-{m[2]}-{m[3]}"
        ended = began
        newest = max((p.stat().st_mtime for p in directory.glob("rows*.jsonl")), default=None)
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
    return {
        "about": "When each self-play game ended and by which victory: a batch per entry, a game per seed. "
        "Rebuilt by `beta/research_page.py --self-play`; read by the Research page.",
        "setup": "6 players, Continents, Online speed, 250 turns, every seat a CIVVIS AI agent",
        "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
        "batches": out,
    }


def written(data: dict) -> str:
    """The snapshot as its file holds it: a line per batch, so a refresh's
    diff is the batches that changed."""
    head = {key: value for key, value in data.items() if key != "batches"}
    lines = [json.dumps(batch, separators=(",", ":")) for batch in data["batches"]]
    return json.dumps(head, indent=1)[:-2] + ',\n "batches": [\n' + ",\n".join(lines) + "\n ]\n}\n"


# --- The page ---------------------------------------------------------------


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
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = research(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"research_page: {page} has no research slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"research_page: no figures to write into {page}", file=sys.stderr)
    else:
        said = [
            f"{data[key]['windows']['all']['games']} {data[key]['name']} games"
            for key in ("self_play", "deity")
            if data.get(key)
        ]
        print(f"research_page: wrote {' and '.join(said)} into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
