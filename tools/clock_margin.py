#!/usr/bin/env python3
"""How far our victory lane was from winning when a live game ended: the clock margin.

⭐ WHY THIS EXISTS. At about ninety live games a day a win rate can only show a
large effect: at Deity's 19% a change worth five points needs some 1,200 games a
side to read at 95%. Three in four Deity losses are a rival's Science Victory
landing at t173-t199, so the question every behaviour change there answers is
*how many turns did we gain on our lane, or take from the rival's launch*. Every
game answers it, the losses included, and the boards already carry it.

One reading per run, from the `state` frames the control mod already exports
(no new export):

* **our lane's shortfall** at the last board, in the lane's own units --
  Religious: majors (ourselves included) whose cities do not mostly follow our
  religion, and the cities that would have to turn for each of them to;
  Diplomatic: victory points short of `DIPLOMATIC_VICTORY_POINTS`; Science:
  launches left of four, and the Exoplanet Expedition's distance still to fly;
* **turns short**: that shortfall over our own pace in the last
  `PACE_WINDOW` turns (None when we made no progress: no honest estimate);
* **the rival's clock**: the fewest turns any rival still needed by the lanes
  the boards can time (its expedition, its culture count), 0 when one won;
* **margin** = rival clock - our turns short: positive, we were ahead; at a
  rival's win it is minus our turns short. Both clocks are also given as the
  turn they run out (`finish_turn`, `rival_clock.finish_turn`): the rival's
  is the instrument for a denial change, since it is defined in a loss (the
  turn it won) and in a win (when it would have) alike.

`progress` is the lane's completed share (0..1) and is always defined, so a
batch where nobody gets near a turn estimate still grades.

Usage::

    python3 tools/clock_margin.py RUN_DIR [RUN_DIR ...]      # one line per game
    python3 tools/clock_margin.py --json RUN_DIR ...
    python3 tools/clock_margin.py --root DIR --since 2026-11-22 --difficulty DEITY

A run directory holds `events.jsonl` (live host) or `events.jsonl.gz` (ledger
branch) beside `summary.json`, whose `victory_target` names our lane. The live
harness writes the reading into every new `summary.json` as `clock_margin`
(`civ6_ladder.SUMMARY_MARKS`), so a ledger reader needs only
`lane_margin(summary)`.
"""

from __future__ import annotations

import argparse
import gzip
import json
import statistics
import sys
from pathlib import Path
from typing import Iterable, Iterator

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ6_ladder import MARS_COLONY_PARTS, SPACE_RACE_STAGES  # noqa: E402

#: The Diplomatic Victory's bar (`src/game.rs` `DIPLOMATIC_VICTORY_POINTS`, the
#: Gathering Storm rule the engine models). The board exports each player's
#: points (`congress_dvp`) but not the bar.
DIPLOMATIC_VICTORY_POINTS = 20

#: How far back our pace is read. Twenty turns covers a Missionary's walk and a
#: World Congress session (every 30 turns at Online speed is longer, so a
#: Diplomatic pace reads 0 between sessions, which is the honest answer).
PACE_WINDOW = 20

#: The Exoplanet Expedition's speed with no laser station, light years a turn:
#: the slowest launched rival's `science_victory_points_per_turn` on the live
#: boards (1, e.g. Gran Colombia at t183 in civvis-t8-20261127T041052Z). Used
#: only to time a flight that has not started.
EXPEDITION_BASE_SPEED = 1.0

#: The lanes a seat can be assigned (`summary["victory_target"]`) and the
#: victory each one wins (`victory.victory_type`).
LANE_VICTORY = {"religious": "VICTORY_RELIGIOUS", "diplomatic": "VICTORY_DIPLOMATIC",
                "science": "VICTORY_TECHNOLOGY"}
LANES = tuple(LANE_VICTORY)


def _stages(projects) -> int:
    """Launches landed, of four, from a `science_projects` list."""
    done = {p for p in projects or [] if isinstance(p, str)}
    landed = 0
    for name, spellings in SPACE_RACE_STAGES:
        if spellings <= done or (name == "mars_colony" and MARS_COLONY_PARTS <= done):
            landed += 1
    return landed


def _num(value) -> float | None:
    """A host figure, or None for the export's -1 / missing / non-number."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or value < 0:
        return None
    return float(value)


def _majority_short(cities: int | None, ours: int) -> int:
    """Cities that must turn to our religion for it to hold a majority (more
    than half: `GetReligionInMajorityOfCities`) of `cities`; at least 1 when the
    host says the majority is not ours, whatever our partial count says."""
    if not cities:
        return 1
    return max(1, cities // 2 + 1 - ours)


def board_reading(event: dict, us: int | None) -> dict:
    """The few numbers of one `state` frame the clock needs."""
    religion = event.get("founded_religion") or None
    cities = [c for c in event.get("cities") or [] if isinstance(c, dict)]
    dvp = {}
    for row in (event.get("congress_dvp") or {}).get("points") or []:
        if isinstance(row, dict) and type(row.get("player")) is int:
            dvp[row["player"]] = _num(row.get("points"))
    rivals = {}
    for rival in event.get("rivals") or []:
        if not isinstance(rival, dict) or type(rival.get("player")) is not int:
            continue
        known = [c for c in rival.get("cities") or [] if isinstance(c, dict)]
        stats = rival.get("public_stats") or {}
        rivals[rival["player"]] = {
            "civ": rival.get("civ"),
            "cities": _num(stats.get("city_count")),
            "known_ours": sum(1 for c in known if religion and c.get("religion") == religion),
            "majority": rival.get("religion") or None,
            "stages": _stages(rival.get("science_projects")),
            "svp": _num(rival.get("science_victory_points")),
            "svp_rate": _num(rival.get("science_victory_points_per_turn")),
            "culture_turns": _num(rival.get("culture_turns_to_victory")),
            "at_war": rival.get("at_war") is True,
            "dvp": dvp.get(rival["player"]),
        }
    return {
        "turn": event.get("turn"),
        "religion": religion,
        "own_cities": len(cities),
        "own_ours": sum(1 for c in cities if religion and c.get("religion") == religion),
        "stages": _stages(event.get("science_projects")),
        "svp": _num(event.get("science_victory_points")),
        "svp_needed": _num(event.get("science_victory_points_needed")),
        "svp_rate": _num(event.get("science_victory_points_per_turn")),
        "dvp": dvp.get(us) if us is not None else _num(event.get("dvp")),
        "rivals": rivals,
    }


def religious_short(board: dict) -> dict:
    """Majors not converted (ourselves included) and the cities short of it."""
    if not board["religion"]:
        majors = 1 + len(board["rivals"])
        return {"majors_short": majors, "cities_short": None, "progress": 0.0}
    short_majors = cities_short = 0
    credit = 0.0
    own_short = 0 if board["own_cities"] == 0 else (
        0 if board["own_ours"] * 2 > board["own_cities"]
        else _majority_short(board["own_cities"], board["own_ours"]))
    entries = [(own_short, board["own_cities"], board["own_ours"])]
    for rival in board["rivals"].values():
        n = int(rival["cities"]) if rival["cities"] else 0
        short = 0 if rival["majority"] == board["religion"] else _majority_short(n, rival["known_ours"])
        entries.append((short, n, rival["known_ours"]))
    for short, n, ours in entries:
        if short:
            short_majors += 1
            cities_short += short
            need = n // 2 + 1 if n else 1
            credit += min(ours, need - 1) / need
        else:
            credit += 1.0
    return {"majors_short": short_majors, "cities_short": cities_short,
            "progress": round(credit / len(entries), 3)}


def science_short(stages: int, svp, needed, rate) -> dict:
    """Launches left and, once the expedition flies, its distance left."""
    flying = stages >= 4
    left = max(0.0, (needed or 0) - (svp or 0)) if flying and needed else None
    share = (1 - left / needed) if left is not None and needed else 0.0
    return {"launches_short": 4 - stages, "distance_short": left,
            "progress": round((stages + share) / 5, 3)}


def diplomatic_short(points) -> dict:
    have = points or 0.0
    short = max(0.0, DIPLOMATIC_VICTORY_POINTS - have)
    return {"points_short": short, "progress": round(min(1.0, have / DIPLOMATIC_VICTORY_POINTS), 3)}


def _lane_units(board: dict, lane: str) -> tuple[dict, float | None]:
    """(shortfall reading, the one number its pace is read on)."""
    if lane == "religious":
        reading = religious_short(board)
        return reading, reading["cities_short"]
    if lane == "diplomatic":
        reading = diplomatic_short(board["dvp"])
        return reading, reading["points_short"]
    reading = science_short(board["stages"], board["svp"], board["svp_needed"], board["svp_rate"])
    return reading, None


def _pace_board(boards: list[dict], last: dict) -> dict | None:
    """The board `PACE_WINDOW` turns before the last, or the first one."""
    turn = last.get("turn")
    if not isinstance(turn, int):
        return None
    earlier = [b for b in boards if isinstance(b.get("turn"), int) and b["turn"] <= turn - PACE_WINDOW]
    if earlier:
        return earlier[-1]
    return boards[0] if boards and boards[0] is not last else None


def _science_turns(stages: int, svp, needed, rate, landed_turns: list[int]) -> float | None:
    """Turns until an expedition lands: its flight left at its current speed, or
    the launches left at the pace of the ones already landed plus a flight at
    base speed. None with no launch landed and none flying."""
    if not needed:
        return None
    if stages >= 4:
        left = max(0.0, needed - (svp or 0))
        return round(left / max(rate or 0.0, EXPEDITION_BASE_SPEED), 1)
    if len(landed_turns) < 2:
        return None
    gap = (landed_turns[-1] - landed_turns[0]) / (len(landed_turns) - 1)
    return round((4 - stages) * gap + needed / EXPEDITION_BASE_SPEED, 1)


def _landed_turns(boards: list[dict], read) -> list[int]:
    """The first turn each launch count was seen, from per-board stage counts."""
    turns, seen = [], 0
    for board in boards:
        stages = read(board)
        if stages is None:
            continue
        while seen < stages:
            seen += 1
            turns.append(board["turn"])
    return turns


def read_events(events: Iterable) -> tuple[int | None, dict | None, list[dict]]:
    """(our player id, the victory/defeat outcome, the opening board of each
    turn plus the last board)."""
    us = None
    outcome = None
    boards: list[dict] = []
    last_event = None
    seen_turns = set()
    for event in events:
        if not isinstance(event, dict):
            continue
        kind = event.get("kind")
        if kind == "seat" and type(event.get("local_player")) is int:
            us = event["local_player"]
        elif kind == "victory":
            outcome = event
        elif kind == "defeat" and event.get("ours") is True:
            # We were eliminated: a loss no lane can finish, and no rival won.
            outcome = {"won": False, "victory_type": None, "eliminated": True}
        elif kind == "state" and isinstance(event.get("turn"), int):
            last_event = event
            if event["turn"] not in seen_turns:
                seen_turns.add(event["turn"])
                boards.append(board_reading(event, us))
    if last_event is not None:
        final = board_reading(last_event, us)
        if boards and boards[-1]["turn"] == final["turn"]:
            boards[-1] = final
        else:
            boards.append(final)
    return us, outcome, boards


def _rival_clock(boards: list[dict], outcome: dict | None, us: int | None) -> dict:
    """The fewest turns a rival still needed, by the lanes a board can time."""
    last = boards[-1]
    if outcome and outcome.get("won") is False and outcome.get("victory_type"):
        team = outcome.get("team")
        winner = team if type(team) is int and team in last["rivals"] else None
        return {"turns": 0.0, "player": winner,
                "civ": last["rivals"].get(winner, {}).get("civ") if winner is not None else None,
                "lane": outcome.get("victory_type")}
    best = {"turns": None, "player": None, "civ": None, "lane": None}
    for player, rival in last["rivals"].items():
        landed = _landed_turns(boards, lambda b, p=player: (b["rivals"].get(p) or {}).get("stages"))
        candidates = [("VICTORY_TECHNOLOGY", _science_turns(
            rival["stages"], rival["svp"], last["svp_needed"], rival["svp_rate"], landed))]
        if rival["culture_turns"]:
            candidates.append(("VICTORY_CULTURE", rival["culture_turns"]))
        for lane, turns in candidates:
            if turns is not None and (best["turns"] is None or turns < best["turns"]):
                best = {"turns": turns, "player": player, "civ": rival["civ"], "lane": lane}
    return best


def clock_margin(events: Iterable, lane: str | None = None) -> dict | None:
    """The clock reading of one game (see the module header), for `lane`, or
    for every lane when None. None without a board."""
    us, outcome, boards = read_events(events)
    if not boards:
        return None
    last = boards[-1]
    pace = _pace_board(boards, last)
    won = bool(outcome and outcome.get("won") is True)
    lanes = {}
    for name in ([lane] if lane else LANES):
        if name not in LANES:
            continue
        reading, units = _lane_units(last, name)
        turns = None
        if won:
            # A win by any victory is the top of the scale: a Science seat that
            # wins the Diplomatic Victory is not "two launches short". The
            # shortfall units stay as the boards read them.
            turns = 0.0
            reading["progress"] = 1.0
        elif name == "science":
            landed = _landed_turns(boards, lambda b: b["stages"])
            turns = _science_turns(last["stages"], last["svp"], last["svp_needed"], last["svp_rate"], landed)
        elif units is not None and pace is not None:
            before, _ = _lane_units(pace, name)
            then = before.get("cities_short") if name == "religious" else before.get("points_short")
            span = last["turn"] - pace["turn"]
            if then is not None and span > 0 and then > units:
                turns = round(units / ((then - units) / span), 1)
        reading["turns_short"] = turns
        lanes[name] = reading
    rival = _rival_clock(boards, outcome, us)
    rival["finish_turn"] = (round(last["turn"] + rival["turns"], 1)
                            if rival["turns"] is not None else None)
    for reading in lanes.values():
        short, clock = reading["turns_short"], rival["turns"]
        reading["finish_turn"] = round(last["turn"] + short, 1) if short is not None else None
        reading["margin"] = round(clock - short, 1) if short is not None and clock is not None else None
    return {
        "turn": last["turn"],
        "won": won if outcome else None,
        "victory": outcome.get("victory_type") if outcome else None,
        "eliminated": bool(outcome and outcome.get("eliminated")),
        "pace_window": PACE_WINDOW,
        "rival_clock": rival,
        "lanes": lanes,
    }


def open_events(path: Path) -> Iterator[dict]:
    """Events from `events.jsonl` or `events.jsonl.gz`; only the kinds read here
    are parsed, the rest skipped on a substring test."""
    opener = gzip.open if path.suffix == ".gz" else open
    wanted = ('"kind": "state"', '"kind": "seat"', '"kind": "victory"',
              '"kind":"state"', '"kind":"seat"', '"kind":"victory"')
    with opener(path, "rt", errors="replace") as handle:
        for line in handle:
            if not any(needle in line for needle in wanted):
                continue
            try:
                yield json.loads(line)
            except ValueError:
                continue  # a torn final line


def lane_margin(summary: dict) -> dict | None:
    """Our assigned lane's reading off a summary, with the rival's clock and the
    game's turn beside it; None when the summary has no reading or no lane.

    This is the call a ledger reader (`tools/live_status.py`) makes per game."""
    reading = summary.get("clock_margin")
    lane = summary.get("victory_target")
    if not isinstance(reading, dict) or lane not in (reading.get("lanes") or {}):
        return None
    return {"lane": lane, "turn": reading.get("turn"), "won": reading.get("won"),
            "rival_clock": reading.get("rival_clock"), **reading["lanes"][lane]}


def run_margin(run_dir: Path) -> dict | None:
    """The reading of one run directory: from its summary when the harness
    wrote one, else read from its event log."""
    summary_path = run_dir / "summary.json"
    summary = json.loads(summary_path.read_text()) if summary_path.exists() else {}
    if "clock_margin" not in summary:
        for name in ("events.jsonl", "events.jsonl.gz"):
            if (run_dir / name).exists():
                summary["clock_margin"] = clock_margin(open_events(run_dir / name))
                break
    margin = lane_margin(summary)
    if margin is not None:
        margin["run"] = run_dir.name
    return margin


def batch_summary(margins: list[dict]) -> dict:
    """What a batch of games says: progress and margin, median and mean, and
    how many losses fell within 10 and 20 turns."""
    def stats(values):
        values = [v for v in values if v is not None]
        if not values:
            return None
        return {"n": len(values), "median": round(statistics.median(values), 2),
                "mean": round(statistics.fmean(values), 2)}
    losses = [m for m in margins if m.get("won") is False]
    return {
        "games": len(margins),
        "progress": stats(m.get("progress") for m in margins),
        "margin": stats(m.get("margin") for m in margins),
        "loss_progress": stats(m.get("progress") for m in losses),
        "loss_turns_short": stats(m.get("turns_short") for m in losses),
        "losses_within_10": sum(1 for m in losses if m.get("turns_short") is not None and m["turns_short"] <= 10),
        "losses_within_20": sum(1 for m in losses if m.get("turns_short") is not None and m["turns_short"] <= 20),
        "losses": len(losses),
    }


def _line(m: dict) -> str:
    unit = {"religious": f"{m.get('majors_short')} majors / {m.get('cities_short')} cities short",
            "diplomatic": f"{m.get('points_short')} VP short",
            "science": f"{m.get('launches_short')} launches"
                       + (f" + {m['distance_short']:.0f} ly" if m.get("distance_short") else "") + " short"}[m["lane"]]
    rival = m.get("rival_clock") or {}
    return (f"{m['run']:<40} {m['lane']:<10} t{m.get('turn')} {'WIN ' if m.get('won') else 'loss'} "
            f"progress {m.get('progress'):.2f}  {unit}  turns short {m.get('turns_short')}  "
            f"rival {rival.get('civ') or '?'} {rival.get('turns')}  margin {m.get('margin')}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("runs", nargs="*", type=Path, help="run directories")
    ap.add_argument("--root", type=Path, help="read every run directory under DIR")
    ap.add_argument("--since", help="with --root: runs whose summary finished on or after this UTC date")
    ap.add_argument("--difficulty", help="with --root: e.g. DEITY")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    runs = list(args.runs)
    if args.root:
        for summary_path in sorted(args.root.glob("*/summary.json")):
            summary = json.loads(summary_path.read_text())
            if args.since and str(summary.get("finished_utc") or "") < args.since:
                continue
            if args.difficulty and args.difficulty.upper() not in str(summary.get("difficulty")):
                continue
            runs.append(summary_path.parent)
    margins = [m for m in map(run_margin, runs) if m is not None]
    if args.json:
        print(json.dumps({"games": margins, "summary": batch_summary(margins)}, indent=1))
        return 0
    for m in margins:
        print(_line(m))
    print(json.dumps(batch_summary(margins)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
