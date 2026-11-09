#!/usr/bin/env python3
"""Did the last batch of Civilization VI games play better than the one before?

Wins are too rare at the top levels to grade a change, so this reads the marks
every ledger row carries: the economy against the best rival at fixed turns, the
tech gap, the culture and religious races, and conquest. It prints the median
and the game count of each mark, per group of games.

    python3 tools/civ6_progress_scorecard.py --since 20261108
    python3 tools/civ6_progress_scorecard.py --leader LEADER_PETER_GREAT --group code_rev
    python3 tools/civ6_progress_scorecard.py --backfill --since 20261108

A group always holds one difficulty and one game speed; those are never pooled.
Seats can share a group, so each group names the seats in it: pass `--leader` to
keep one seat. The `-contN` segments of a parked game count as one game.

`--backfill` reads `control/<tag>/events.jsonl` for rows that predate the marks
and appends the result to a side file. It never rewrites the ledger. The run logs
are pruned after a few days, so a row whose log is gone stays blank.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
import multiprocessing
import os
from pathlib import Path
import re
import statistics
import sys

import civ6_ladder

RUNS = Path.home() / "civvis-civ6-runs"
LEDGER = RUNS / "civvis_ladder.jsonl"
CONTROL = RUNS / "control"
SIDE_FILE = RUNS / "analysis" / "progress-marks-backfill.jsonl"

#: The live game shares this machine; a backfill stays under this many workers
#: and runs them at `BACKFILL_NICENESS`.
MAX_JOBS = 6
BACKFILL_NICENESS = 15

PROGRESS_FIELDS = tuple(field for field, _ in civ6_ladder.PROGRESS_MARKS)

ECONOMY_FIELDS = (
    "cities", "population", "science", "production",
    "rival_cities", "rival_population", "rival_science", "rival_production",
    "districts", "campuses", "trade_routes", "trade_capacity",
)


def read_jsonl(path: Path) -> list[dict]:
    """Every JSON object in a file; a damaged line is skipped, a missing file
    is empty."""
    if not path.exists():
        return []
    rows = []
    with path.open(errors="replace") as handle:
        for line in handle:
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if isinstance(row, dict):
                rows.append(row)
    return rows


def day_of(tag: str | None) -> str | None:
    """The UTC date (YYYYMMDD) in a run tag."""
    found = re.search(r"(\d{8})T\d{6}Z", tag or "")
    return found.group(1) if found else None


def latest_rows(rows: list[dict]) -> list[dict]:
    """One row per tag, the newest."""
    return list({row.get("tag"): row for row in rows}.values())


def with_backfill(rows: list[dict], side: list[dict]) -> list[dict]:
    """The rows with side-file marks filling only what each row lacks."""
    by_tag = {record.get("tag"): record for record in side}
    filled = []
    for row in rows:
        record = by_tag.get(row.get("tag"), {})
        filled.append({**row, **{
            field: record[field] for field in PROGRESS_FIELDS
            if row.get(field) is None and record.get(field) is not None}})
    return filled


def _first_per_mark(readings: list[dict]) -> dict:
    """Each mark from the earliest segment holding it.

    The older `tech_marks` and `culture_marks` read "at or after" their turn, so
    a continuation opening past a mark files a late board under it; the earliest
    segment is the one that was really there.
    """
    merged: dict = {}
    for reading in readings:
        for mark, value in reading.items():
            merged.setdefault(str(mark), value)
    return merged


def _merged_race(segments: list[dict], field: str) -> dict | None:
    """One game's `religion_marks` or `conquest_marks` across its segments."""
    readings = [segment[field] for segment in segments if segment.get(field)]
    if not readings:
        return None
    spreads: Counter = Counter()
    for reading in readings:
        spreads.update(reading.get("spreads") or {})
    return {
        "religion": next((r["religion"] for r in readings if r.get("religion")), None),
        "founded_turn": next((r["founded_turn"] for r in readings
                              if r.get("founded_turn") is not None), None),
        "spreads": dict(spreads),
        "at": _first_per_mark([reading.get("at") or {} for reading in readings]),
        "last": readings[-1].get("last"),
    }


def merge_game(segments: list[dict]) -> dict:
    """One game from its ledger rows, which are in segment order."""
    def first(field: str):
        return next((segment[field] for segment in segments
                     if segment.get(field) is not None), None)

    def marks(field: str) -> dict:
        return _first_per_mark([segment[field] for segment in segments
                                if isinstance(segment.get(field), dict)])

    tag = civ6_ladder.game_tag(segments[0].get("tag"))
    return {
        "tag": tag,
        "day": day_of(tag),
        "difficulty": first("difficulty"),
        "speed": first("speed"),
        "leader": first("leader"),
        "code_rev": first("code_rev"),
        "cities_at_60": first("cities_at_60"),
        "economy": marks("economy_marks"),
        "tech": marks("tech_marks"),
        "culture": marks("culture_marks"),
        "religion": _merged_race(segments, "religion_marks"),
        "conquest": _merged_race(segments, "conquest_marks"),
    }


def is_selected(game: dict, args) -> bool:
    """Whether a game passes the command line's filters."""
    if args.difficulty and game["difficulty"] != args.difficulty:
        return False
    if args.speed and game["speed"] != args.speed:
        return False
    if args.leader and game["leader"] not in args.leader:
        return False
    if args.since and (game["day"] is None or game["day"] < args.since):
        return False
    if args.until and (game["day"] is None or game["day"] > args.until):
        return False
    return True


def group_key(game: dict, by: str) -> tuple:
    """A game's group. Difficulty and speed are in every key, so they cannot
    be pooled whatever `--group` asks for."""
    extra = {"day": game["day"], "code_rev": game["code_rev"], "none": "all"}[by]
    return (str(game["difficulty"]), str(game["speed"]), str(extra))


def _stat(values) -> dict:
    """The median and count of the numbers among `values`."""
    known = [value for value in values
             if isinstance(value, (int, float)) and not isinstance(value, bool)]
    return {"median": round(statistics.median(known), 1) if known else None,
            "n": len(known)}


def _race_summary(games: list[dict], field: str, columns: tuple[str, ...]) -> dict | None:
    """The medians of one race across a group's games."""
    races = [game[field] for game in games if game[field]]
    if not races:
        return None

    def board(read) -> dict:
        return {column: _stat((read(race) or {}).get(column) for race in races)
                for column in columns}

    summary = {
        "games": len(races),
        "at": {str(mark): board(lambda race: race["at"].get(str(mark)))
               for mark in civ6_ladder.VICTORY_MARK_TURNS},
        "last": board(lambda race: race["last"]),
    }
    if field == "religion":
        summary["founded"] = sum(1 for race in races if race["religion"])
        summary["founded_turn"] = _stat(race["founded_turn"] for race in races)
        bands = [civ6_ladder._spread_band(first)
                 for first, _ in civ6_ladder.SPREAD_BANDS]
        summary["spreads"] = {band: _stat(race["spreads"].get(band) for race in races)
                              for band in bands}
    return summary


def summarize(games: list[dict]) -> dict:
    """Every mark's median and game count for one group."""
    def table(field: str, marks, columns) -> dict:
        return {str(mark): {column: _stat(
            (game[field].get(str(mark)) or {}).get(column) for game in games)
            for column in columns} for mark in marks}

    return {
        "games": len(games),
        "seats": dict(Counter(game["leader"] or "unknown" for game in games).most_common()),
        "cities_at_60": _stat(game["cities_at_60"] for game in games),
        "economy": table("economy", civ6_ladder.ECONOMY_MARK_TURNS, ECONOMY_FIELDS),
        "tech": table("tech", civ6_ladder.BOOST_MARK_TURNS, ("techs", "rival_techs")),
        "culture": table("culture", civ6_ladder.BOOST_MARK_TURNS,
                         ("percent", "rival_percent")),
        "religion": _race_summary(
            games, "religion", ("converted", "rivals", "cities_following")),
        "conquest": _race_summary(games, "conquest", ("capitals", "cities")),
    }


def _cell(stat: dict, width: int) -> str:
    median = stat["median"]
    return ("-" if median is None else f"{median:g}").rjust(width)


def _render_board(label: str, board: dict, columns: tuple[str, ...]) -> str:
    games = max(board[column]["n"] for column in columns)
    return (f"    {label:>5} ({games:>3} games): " + "  ".join(
        f"{column} {_cell(board[column], 1)}" for column in columns))


def render(key: tuple, summary: dict) -> str:
    """One group as text."""
    difficulty, speed, group = key
    seats = ", ".join(f"{seat} {count}" for seat, count in summary["seats"].items())
    lines = [f"{difficulty} {speed} {group}: {summary['games']} games ({seats})"]
    lines.append(f"  turn games | cities  pop    sci   prod | best rival: cities  pop    sci   prod"
                 f" | districts campus routes cap")
    for mark, row in summary["economy"].items():
        lines.append(
            f"  {mark:>4} {row['cities']['n']:>5} | "
            + " ".join(_cell(row[f], w) for f, w in (
                ("cities", 6), ("population", 4), ("science", 6), ("production", 6)))
            + " |             "
            + " ".join(_cell(row[f], w) for f, w in (
                ("rival_cities", 6), ("rival_population", 4),
                ("rival_science", 6), ("rival_production", 6)))
            + " | "
            + " ".join(_cell(row[f], w) for f, w in (
                ("districts", 9), ("campuses", 6), ("trade_routes", 6),
                ("trade_capacity", 3))))
    stat = summary["cities_at_60"]
    lines.append(f"  cities at t60: {_cell(stat, 1)} ({stat['n']} games)")
    for mark, row in summary["tech"].items():
        lines.append(f"  techs t{mark}: {_cell(row['techs'], 1)} vs best rival "
                     f"{_cell(row['rival_techs'], 1)} ({row['techs']['n']} games)")
    for mark, row in summary["culture"].items():
        lines.append(f"  culture victory share t{mark}: ours {_cell(row['percent'], 1)}%, "
                     f"leading rival {_cell(row['rival_percent'], 1)}% "
                     f"({row['percent']['n']} games)")
    religion = summary["religion"]
    if religion:
        columns = ("converted", "rivals", "cities_following")
        turn = religion["founded_turn"]
        lines.append(f"  religion: founded in {religion['founded']} of {religion['games']} games, "
                     f"median turn {_cell(turn, 1)} ({turn['n']} dated)")
        lines.append("    spreads landed per game: " + "  ".join(
            f"t{band} {_cell(stat, 1)}" for band, stat in religion["spreads"].items()))
        for mark, board in religion["at"].items():
            lines.append(_render_board(f"t{mark}", board, columns))
        lines.append(_render_board("last", religion["last"], columns))
    conquest = summary["conquest"]
    if conquest:
        lines.append("  conquest (rival capitals held, foreign-founded cities held):")
        for mark, board in conquest["at"].items():
            lines.append(_render_board(f"t{mark}", board, ("capitals", "cities")))
        lines.append(_render_board("last", conquest["last"], ("capitals", "cities")))
    return "\n".join(lines)


def _backfill_one(job: tuple[str, str]) -> dict:
    tag, events = job
    return {"tag": tag, **civ6_ladder.progress_marks(Path(events))}


def backfill(rows: list[dict], control: Path, side_file: Path, jobs: int) -> dict:
    """Append the marks of every row that lacks them and has a log to read.

    A run already in the side file is not read again, including one that
    yielded nothing, so a repeat is cheap. The ledger is only ever read.
    """
    known = {record.get("tag") for record in read_jsonl(side_file)}
    todo, no_log = [], 0
    for row in rows:
        tag = row.get("tag")
        if tag in known or all(row.get(field) is not None for field in PROGRESS_FIELDS):
            continue
        events = control / str(tag) / "events.jsonl"
        if events.exists():
            todo.append((str(tag), str(events)))
        else:
            no_log += 1
    side_file.parent.mkdir(parents=True, exist_ok=True)
    with side_file.open("a") as sink:
        def keep(record: dict) -> None:
            sink.write(json.dumps(record, sort_keys=True) + "\n")
            sink.flush()

        if jobs <= 1:
            for job in todo:
                keep(_backfill_one(job))
        elif todo:
            with multiprocessing.Pool(min(jobs, MAX_JOBS), initializer=os.nice,
                                      initargs=(BACKFILL_NICENESS,)) as pool:
                for record in pool.imap_unordered(_backfill_one, todo):
                    keep(record)
    return {"read": len(todo), "no_log": no_log}


def build_groups(rows: list[dict], side: list[dict], args) -> dict:
    """The selected games, merged and grouped."""
    groups: dict = {}
    for segments in civ6_ladder.games_from_attempts(with_backfill(rows, side)):
        game = merge_game(segments)
        if is_selected(game, args):
            groups.setdefault(group_key(game, args.group), []).append(game)
    return groups


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ledger", type=Path, default=LEDGER)
    parser.add_argument("--control", type=Path, default=CONTROL,
                        help="where each run's events.jsonl lives")
    parser.add_argument("--side-file", type=Path, default=SIDE_FILE,
                        help="backfilled marks, kept apart from the ledger")
    parser.add_argument("--difficulty", help="exact, e.g. DIFFICULTY_DEITY")
    parser.add_argument("--speed", help="exact, e.g. GAMESPEED_ONLINE")
    parser.add_argument("--leader", action="append",
                        help="exact, e.g. LEADER_PETER_GREAT; repeatable")
    parser.add_argument("--since", help="first UTC date, YYYYMMDD")
    parser.add_argument("--until", help="last UTC date, YYYYMMDD")
    parser.add_argument("--group", choices=("day", "code_rev", "none"), default="day")
    parser.add_argument("--backfill", action="store_true",
                        help="first fill the marks of selected rows that lack them")
    parser.add_argument("--jobs", type=int, default=4,
                        help=f"backfill workers, at most {MAX_JOBS}")
    parser.add_argument("--json", type=Path, help="also write the summaries here")
    args = parser.parse_args(argv)

    if not args.ledger.exists():
        print(f"no ledger at {args.ledger}", file=sys.stderr)
        return 2
    rows = latest_rows(read_jsonl(args.ledger))
    if args.backfill:
        chosen = [segment
                  for segments in civ6_ladder.games_from_attempts(rows)
                  if is_selected(merge_game(segments), args)
                  for segment in segments]
        result = backfill(chosen, args.control, args.side_file, args.jobs)
        print(f"backfill: read {result['read']} run logs; "
              f"{result['no_log']} rows have no log left", file=sys.stderr)
    groups = build_groups(rows, read_jsonl(args.side_file), args)
    summaries = {key: summarize(games) for key, games in sorted(groups.items())}
    print("\n\n".join(render(key, summary) for key, summary in summaries.items())
          or "no games match")
    if args.json:
        args.json.write_text(json.dumps(
            {" ".join(key): summary for key, summary in summaries.items()},
            indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
