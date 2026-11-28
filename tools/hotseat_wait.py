#!/usr/bin/env python3
"""Where a hotseat person's wait goes: every round and every CIVVIS seat, split by stage.

    python3 tools/hotseat_wait.py RUN_DIR [--json] [--rounds 5]

RUN_DIR is a `tools/civ6_hotseat.py` run (`~/civvis-civ6-runs/hotseat/<tag>`).
A round is the person's wait: from their turn end to their next turn, as the
cover measures it (`hotseat_cover` `waited`, the UI clock). Within it, each
CIVVIS seat's turn runs from the previous seat's turn end to its own:

    open    previous turn end -> this seat's first answer is in the orders DB
              = hand-over + Start Turn press + export + log + relay + worker
    frames  each later answer after the one before (a replan or combat frame)
    close   last answer -> this seat's turn ends (apply, units, end turn)

When the records carry the finer stamps, `open` and `frames` split further:
`handover` (turn end -> Start Turn pressed), `start` (press -> opening board at
the relay), `brain` (board at the relay -> answer written) and `remote` (answer
-> the next frame's board at the relay: the mod's apply, export and log).

Clocks, all on the wall clock:
* an answer is `brain.log`'s `at HH:MM:SS.mmmZ`;
* a turn end is `end_turn_wait.ui_now` (`UI.GetElapsedTime`), put on the wall
  clock by the offset that fits every turn end's whole-second `t`;
* a record with `ui` (the same UI clock) is placed by that offset; otherwise by
  `relay_utc`, the relay's receipt stamp (`civ6_control/watch.py`). A run from
  before either has neither, and those columns print `-`.
"""
from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

ANSWER = re.compile(r"\[brain\] turn (\d+)(?: frame (\d+))?: (\d+) orders in ([\d.]+)s "
                    r"at (\d\d):(\d\d):(\d\d(?:\.\d+)?)Z")
STAGES = ("open", "handover", "start", "brain", "frames", "remote", "close", "total")


def parse_stamp(stamp: object) -> float | None:
    """`2026-11-28T03:38:29.044Z` -> Unix seconds; anything else -> None."""
    if not isinstance(stamp, str):
        return None
    for layout in ("%Y-%m-%dT%H:%M:%S.%fZ", "%Y-%m-%dT%H:%M:%SZ"):
        try:
            return datetime.strptime(stamp, layout).replace(tzinfo=timezone.utc).timestamp()
        except ValueError:
            continue
    return None


def read_jsonl(path: Path) -> list[dict]:
    rows = []
    try:
        handle = path.open("r", encoding="utf-8", errors="replace")
    except OSError:
        return rows
    with handle:
        for raw in handle:
            try:
                row = json.loads(raw)
            except ValueError:
                continue
            if isinstance(row, dict):
                rows.append(row)
    return rows


def fit_ui_offset(ends: list[tuple[float, float]]) -> tuple[float, float] | None:
    """The offset C with wall = ui + C, from (t, ui) pairs where `t` is the
    whole-second wall clock the same record was written at: every pair says
    t <= ui + C < t + 1. Returns (C, half-width of the interval that fits every
    pair). Pairs that do not all fit (a clock that drifted, a stray second)
    still give the middle of the bounds, with a negative half-width."""
    if not ends:
        return None
    low = max(t - ui for t, ui in ends)
    high = min(t + 1 - ui for t, ui in ends)
    return (low + high) / 2, (high - low) / 2


class ClockFit:
    """`fit_ui_offset` over the turn ends near each turn.

    One offset for a whole game can fail to fit: over the 119 turns of
    `selfplay-duel-20261127T0808Z` the turn ends' `t - ui` spread over 1.13 s,
    while every window of seven turns but four fit within one second."""

    WINDOW = 3

    def __init__(self, pairs: list[tuple[int, float, float]]):
        self.pairs = pairs
        self._cache: dict[int, tuple[float, float] | None] = {}

    def __bool__(self) -> bool:
        return bool(self.pairs)

    def at(self, turn: int | None) -> tuple[float, float] | None:
        if turn is None:
            return fit_ui_offset([(t, ui) for _, t, ui in self.pairs])
        if turn not in self._cache:
            near = [(t, ui) for other, t, ui in self.pairs if abs(other - turn) <= self.WINDOW]
            self._cache[turn] = fit_ui_offset(near) or self.at(None)
        return self._cache[turn]

    def wall(self, turn: int | None, ui: float) -> float | None:
        fit = self.at(turn)
        return None if fit is None else ui + fit[0]

    def disagreeing(self) -> int:
        """Turns whose window of turn ends does not fit one offset."""
        return sum(1 for turn in sorted({turn for turn, _, _ in self.pairs})
                   if (self.at(turn) or (0, 0))[1] < 0)


def brain_answers(path: Path, reference: dict[int, float]) -> dict[tuple[int, int], tuple[float, int]]:
    """(turn, frame) -> (wall seconds the answer was written, order count).

    `brain.log` stamps the time of day only; the date is the one that puts the
    answer nearest its turn's own `t` (`reference`), which survives midnight."""
    answers: dict[tuple[int, int], tuple[float, int]] = {}
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return answers
    for line in lines:
        match = ANSWER.search(line)
        if not match:
            continue
        turn, frame, count, _, hh, mm, ss = match.groups()
        turn, frame = int(turn), int(frame or 0)
        near = reference.get(turn)
        if near is None:
            continue
        day = datetime.fromtimestamp(near, timezone.utc).replace(hour=0, minute=0, second=0,
                                                                 microsecond=0)
        clock = timedelta(hours=int(hh), minutes=int(mm), seconds=float(ss))
        at = min(((day + shift + clock).timestamp() for shift in
                  (timedelta(days=-1), timedelta(0), timedelta(days=1))),
                 key=lambda candidate: abs(candidate - near))
        # A worker restarted mid-turn answers its turn again; the last answer
        # is the one the game acted on.
        answers[(turn, frame)] = (at, int(count))
    return answers


class Run:
    """One hotseat run directory, read once."""

    def __init__(self, run_dir: Path):
        self.run_dir = Path(run_dir)
        config = {}
        try:
            config = json.loads((self.run_dir / "config.json").read_text())
        except (OSError, ValueError):
            pass
        self.tag = config.get("RunTag") or self.run_dir.name
        self.people = [int(seat) for seat in config.get("HotseatHumanSeats") or []]
        seats = config.get("HotseatSeats")
        if not seats:
            seats = sorted(int(path.name.rsplit("-p", 1)[1]) for path in
                           self.run_dir.glob(f"{self.tag}-p*") if path.name.rsplit("-p", 1)[1].isdigit())
        self.seats = [int(seat) for seat in seats]
        self.base = read_jsonl(self.run_dir / "events.jsonl")
        self.seat_events = {seat: read_jsonl(self.run_dir / f"{self.tag}-p{seat}" / "events.jsonl")
                            for seat in self.seats}
        self.fit = ClockFit([
            (row["turn"], float(row["t"]), float(row["ui_now"]))
            for rows in self.seat_events.values() for row in rows
            if row.get("kind") == "end_turn_wait" and isinstance(row.get("turn"), int)
            and isinstance(row.get("t"), (int, float))
            and isinstance(row.get("ui_now"), (int, float))])

    def wall(self, row: dict) -> float | None:
        """When the mod wrote `row` (its `ui`), else when the relay got it."""
        ui = row.get("ui")
        if isinstance(ui, (int, float)) and self.fit:
            turn = row.get("turn")
            return self.fit.wall(turn if isinstance(turn, int) else None, ui)
        return parse_stamp(row.get("relay_utc"))

    def seat_turns(self) -> list[dict]:
        """Every CIVVIS seat-turn with a recorded end, in play order."""
        rows = []
        for seat in self.seats:
            events = self.seat_events[seat]
            reference: dict[int, float] = {}
            ends: dict[int, dict] = {}
            boards: dict[tuple[int, int], float] = {}
            for row in events:
                turn = row.get("turn")
                if not isinstance(turn, int):
                    continue
                if isinstance(row.get("t"), (int, float)):
                    reference.setdefault(turn, float(row["t"]))
                kind = row.get("kind")
                if kind == "end_turn_wait" and self.fit \
                        and isinstance(row.get("ui_now"), (int, float)):
                    ends[turn] = row
                elif kind == "state":
                    at = parse_stamp(row.get("relay_utc"))
                    if at is not None:
                        boards[(turn, int(row.get("frame") or 0))] = at
            answers = brain_answers(self.run_dir / f"{self.tag}-p{seat}" / "brain.log", reference)
            for turn, end in ends.items():
                mine = sorted((frame, at, count) for (t, frame), (at, count) in answers.items()
                              if t == turn)
                rows.append({
                    "turn": turn, "seat": seat,
                    "end": self.fit.wall(turn, end["ui_now"]),
                    "end_turn_wait": end.get("wait"),
                    "answers": [(frame, at) for frame, at, _ in mine],
                    "orders": sum(count for _, _, count in mine),
                    "boards": {frame: boards[(turn, frame)] for frame, _, _ in mine
                               if (turn, frame) in boards},
                })
        rows.sort(key=lambda row: (row["turn"], row["seat"]))
        return rows

    def base_marks(self) -> dict:
        """Wall times of the hand-over records, where they carry a clock."""
        marks = {"press": {}, "end": {}, "lift": {}, "waited": {}, "played": set()}
        for row in self.base:
            kind, turn, player = row.get("kind"), row.get("turn"), row.get("player")
            if not isinstance(turn, int):
                continue
            if kind == "hotseat_turn" and isinstance(player, int):
                # Who played, clock or no clock: a seat after a person whose
                # turn end has no clock has no start either.
                marks["played"].add((turn, player))
            if kind == "hotseat_cover" and row.get("up") is False and row.get("reason") == "person_start":
                if isinstance(row.get("waited"), (int, float)):
                    marks["waited"][(turn, player)] = float(row["waited"])
                at = self.wall(row)
                if at is not None:
                    marks["lift"][(turn, player)] = at
                continue
            at = self.wall(row)
            if at is None:
                continue
            if kind == "hotseat_start_turn" and not row.get("again"):
                marks["press"].setdefault((turn, player), at)
            elif kind == "hotseat_turn" and row.get("phase") == "end":
                marks["end"][(turn, player)] = at
        return marks


def span(a: float | None, b: float | None) -> float | None:
    return None if a is None or b is None else b - a


def split(run: Run) -> list[dict]:
    """Rounds, each with its seat-turns and their stages (seconds)."""
    turns = run.seat_turns()
    marks = run.base_marks()
    # Who played just before whom: every seat-turn of the table in (turn, seat)
    # order, CIVVIS seats from their records and people from the base journal.
    # A seat-turn whose end has no clock is still in the order, so the next one
    # gets no start rather than an earlier seat's.
    ends: dict[tuple[int, int], float | None] = dict.fromkeys(marks["played"])
    ends.update({key: at for key, at in marks["end"].items() if key[1] in run.people})
    ends.update({(row["turn"], row["seat"]): row["end"] for row in turns})
    order = sorted(ends)
    previous = {later: ends[earlier] for earlier, later in zip(order, order[1:])}
    by_key = {(row["turn"], row["seat"]): row for row in turns}
    for row in turns:
        key = (row["turn"], row["seat"])
        prev_end = previous.get(key)
        answers = [at for _, at in row["answers"]]
        boards = row["boards"]
        frames = [frame for frame, _ in row["answers"]]
        first, last = (answers[0], answers[-1]) if answers else (None, None)
        press = marks["press"].get(key)
        stages = {
            "open": span(prev_end, first),
            "handover": span(prev_end, press),
            "start": span(press, boards.get(0)) if frames and frames[0] == 0 else None,
            "frames": (sum(b - a for a, b in zip(answers, answers[1:])) if len(answers) > 1
                       else 0.0 if answers else None),
            "close": span(last, row["end"]),
            "total": span(prev_end, row["end"]),
        }
        # Only from a run whose relay stamped every one of this turn's boards.
        stamped = bool(answers) and all(frame in boards for frame in frames)
        stages["brain"] = (sum(at - boards[frame] for frame, at in row["answers"])
                           if stamped else None)
        stages["remote"] = (sum(boards[frame] - before for (_, before), (frame, _) in
                                zip(row["answers"], row["answers"][1:]))
                            if stamped else None)
        row["stages"] = stages
        row["extra_frames"] = max(0, len(answers) - 1)
    rounds = []
    for (turn, person), waited in sorted(marks["waited"].items()):
        if person not in run.people:
            continue
        seat_rows = [by_key[key] for key in order
                     if (turn - 1, person) < key < (turn, person) and key in by_key]
        if not seat_rows:
            continue
        lift = marks["lift"].get((turn, person))
        first = seat_rows[0]["answers"][0][1] if seat_rows[0]["answers"] else None
        played = span(first, seat_rows[-1]["end"])
        rounds.append({
            "turn": turn, "person": person, "waited": waited, "seats": seat_rows,
            "played": played,
            # The person's end -> the first answer, plus the last seat's end
            # -> the person's next turn: what no seat-turn row covers.
            "ends": None if played is None else waited - played,
            "tail": span(seat_rows[-1]["end"], lift),
        })
    return rounds


def ms(value: float | None) -> str:
    return "-" if value is None else f"{value * 1000:.0f}"


def median_ms(values: list[float | None]) -> str:
    present = [value for value in values if value is not None]
    return ms(statistics.median(present)) if present else "-"


def report(run: Run, rounds: list[dict], show: int | None = None) -> str:
    lines = []
    if not run.fit:
        lines.append(f"{run.tag}: no end_turn_wait records with a UI clock; nothing to place")
        return "\n".join(lines)
    _, width = run.fit.at(None)
    off = run.fit.disagreeing()
    fit = (f"+/-{width * 1000:.0f} ms" if width >= 0 else "turn by turn")
    if off:
        fit += f" ({off} turns' ends disagree by up to a tenth of a second: approximate there)"
    lines.append(f"{run.tag}: {len(rounds)} rounds; CIVVIS seats {run.seats}; person seats "
                 f"{run.people}; UI clock placed on the wall clock {fit}")
    lines.append("all times in ms; '-' = this run's records carry no clock for that stage")
    shown = rounds if not show else rounds[-show:]
    for round_ in shown:
        lines.append("")
        lines.append(f"turn {round_['turn']}: the person waited {ms(round_['waited'])} "
                     f"(seats played {ms(round_['played'])}; their end -> first answer + "
                     f"last end -> their turn {ms(round_['ends'])}; rollover tail "
                     f"{ms(round_['tail'])})")
        lines.append("  seat  " + "  ".join(f"{name:>8}" for name in STAGES)
                     + "   extra  orders  etw")
        for row in round_["seats"]:
            stages = row["stages"]
            lines.append(f"  p{row['seat']:<4}" + "  ".join(f"{ms(stages[name]):>8}" for name in STAGES)
                         + f"  {row['extra_frames']:>6}  {row['orders']:>6}  "
                         + ms(row["end_turn_wait"]))
    every = [row for round_ in rounds for row in round_["seats"]]
    lines.append("")
    lines.append(f"medians over {len(every)} seat-turns in {len(rounds)} rounds:")
    for name in STAGES:
        lines.append(f"  {name:<9}{median_ms([row['stages'][name] for row in every]):>8}")
    gaps = [b - a for row in every for (_, a), (_, b) in zip(row["answers"], row["answers"][1:])]
    lines.append(f"  one extra frame {median_ms(gaps)} (n={len(gaps)}); extra frames per "
                 f"seat-turn {sum(row['extra_frames'] for row in every) / max(1, len(every)):.2f}")
    lines.append(f"  per round: waited {median_ms([r['waited'] for r in rounds])}, seats played "
                 f"{median_ms([r['played'] for r in rounds])}, ends "
                 f"{median_ms([r['ends'] for r in rounds])}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--json", action="store_true", help="the rounds as JSON")
    parser.add_argument("--rounds", type=int, default=None,
                        help="print only the last N rounds (the medians cover all)")
    args = parser.parse_args(argv)
    if not args.run_dir.is_dir():
        print(f"{args.run_dir} is not a run directory", file=sys.stderr)
        return 2
    run = Run(args.run_dir)
    rounds = split(run)
    if args.json:
        print(json.dumps({"tag": run.tag, "fit": run.fit.at(None), "rounds": rounds}, indent=1))
    else:
        print(report(run, rounds, args.rounds))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
