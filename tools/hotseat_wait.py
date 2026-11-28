#!/usr/bin/env python3
"""Where a hotseat person's wait goes: every round and every CIVVIS seat, split by stage.

    python3 tools/hotseat_wait.py RUN_DIR [--json] [--rounds 5] [--band 10]

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

By turn band (the last table; `--band` turns wide, 10 by default), a seat-turn
is cut at every mark that carries a clock, so its pieces add up to its total:

    open    shown      previous turn end -> this seat's hand-over is shown
            press      shown -> Start Turn pressed
            to_export  pressed -> the opening board is exported (unpause,
                       the controller's first tick, the moves-restored wait)
            export     exported -> the board is at the relay (export work,
                       the log write, the relay's pass)
            brain      at the relay -> the answer is written
            seen       written -> the mod applies it (the peek)
    frames  settle     an answer applied -> the next frame asked (units
                       walking, a strike's fight, the next tick)
            export, brain, seen   as above, for every extra frame
    close   settle     last answer applied -> end turn requested
            host       requested -> the turn ended

and a round adds the rollover: the last seat's end -> the person's hand-over
shown (the game's own between-turn processing and its autosave), then that
hand-over pressed and the cover lifted. Per seat-turn the band prints medians;
per round, means, which add up: their sum is the band's mean round. A piece
between a mark on the UI clock and one on the wall clock (`open.export`,
`open.seen` and their frame twins) carries the offset's error (the header's
+/-); a piece between two marks on the same clock does not.
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
#: The pieces of one seat-turn, in the order they happen (see the header).
PIECES = (("open", "shown"), ("open", "press"), ("open", "to_export"), ("open", "export"),
          ("open", "brain"), ("open", "seen"), ("frames", "settle"), ("frames", "export"),
          ("frames", "brain"), ("frames", "seen"), ("close", "settle"), ("close", "host"))
#: What a band also counts per seat-turn, beside the time: the reasons it grows,
#: and the pulse ticks of `--fast-turns` (`pulse_tick`, the agent's
#: `CivvisQueue.pulseTickDue`) with the unpause they waited for.
DRIVERS = ("extra_frames", "strike_frames", "empty_frames", "fights", "fight_s",
           "board_kb", "orders", "units", "pulse_open", "pulse_settle", "unpause_ms")
#: Drivers a band gives as a median, not a mean.
MEDIAN_DRIVERS = ("board_kb", "unpause_ms")


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
                if row.get("kind") == "state":
                    # How big the board was as the relay carried it.
                    row["_bytes"] = len(raw)
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


#: A timescale switch's record (`--fast-animations`): the agent's
#: `fast_animations` with these phases carries the raw UI clock at the switch
#: (`ui`) and the scale after it (`scale`).
SWITCH_PHASES = ("on", "off", "loaded")
#: The keys a record carries the UI clock under (`at`: the cover's records'
#: only; elsewhere `at` is a plot or another clock).
UI_KEYS = ("ui", "ui_now", "at")


class RealClock:
    """The UI clock back into seconds.

    `UI.GetElapsedTime` runs `scale` times faster under the console's
    `timescale`, and `--fast-animations` switches it between 1 and N around
    the CIVVIS seats' turns, so a span of the raw clock is not a span of time.
    Each switch (`ui`, `scale`) starts a stretch at its scale; before the first
    the clock is itself, so a run without a switch reads exactly as before."""

    def __init__(self, switches: list[tuple[float, float]]):
        self.knots: list[tuple[float, float, float]] = []  # (raw ui, real, scale from it)
        real_at, last_ui, scale = None, None, 1.0
        for ui, new in sorted((float(ui), float(new)) for ui, new in switches
                              if isinstance(ui, (int, float)) and isinstance(new, (int, float))
                              and 1 <= new <= 8):
            real_at = ui if real_at is None else real_at + (ui - last_ui) / scale
            last_ui, scale = ui, new
            self.knots.append((ui, real_at, scale))

    def __bool__(self) -> bool:
        return bool(self.knots)

    def real(self, ui: float) -> float:
        out = ui
        for start, real_at, scale in self.knots:
            if ui < start:
                break
            out = real_at + (ui - start) / scale
        return out

    def scale_at(self, ui: float) -> float:
        scale = 1.0
        for start, _, after in self.knots:
            if ui < start:
                break
            scale = after
        return scale


def switches(rows: list[dict]) -> list[tuple[float, float]]:
    """The timescale switches in a journal: the agent's commands, and the
    HUD's own (`fast_animations_guard` put back `by` the heartbeat)."""
    out = []
    for row in rows:
        kind, ui = row.get("kind"), row.get("ui")
        if not isinstance(ui, (int, float)):
            continue
        if kind == "fast_animations" and row.get("phase") in SWITCH_PHASES \
                and isinstance(row.get("scale"), (int, float)):
            out.append((float(ui), float(row["scale"])))
        elif kind == "fast_animations_guard" and row.get("by") == "heartbeat":
            out.append((float(ui), 1.0))
    return out


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
        self.config = config if isinstance(config, dict) else {}
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
        # `--fast-animations`: every UI clock stamp into seconds before anything
        # is placed or subtracted (the raw stamp kept as `_raw_<key>`). The test
        # stand-in's context (`-p0`) switches it too.
        stub = read_jsonl(self.run_dir / f"{self.tag}-p0" / "events.jsonl") if 0 not in self.seats else []
        self.stub_events = stub
        every = [self.base, stub] + list(self.seat_events.values())
        self.clock = RealClock([switch for rows in every for switch in switches(rows)])
        if self.clock:
            for rows in every:
                for row in rows:
                    for key in UI_KEYS:
                        # `at` is the UI clock on the cover's records only.
                        if key == "at" and row.get("ctx") != "cover" \
                                and not str(row.get("kind", "")).startswith("hotseat_cover"):
                            continue
                        if isinstance(row.get(key), (int, float)):
                            row[f"_raw_{key}"] = row[key]
                            row[key] = self.clock.real(float(row[key]))
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
            marks: dict[int, dict] = {}
            for row in events:
                turn = row.get("turn")
                if not isinstance(turn, int):
                    continue
                if isinstance(row.get("t"), (int, float)):
                    reference.setdefault(turn, float(row["t"]))
                kind = row.get("kind")
                frame = int(row.get("frame") or 0)
                if kind == "end_turn_wait" and self.fit \
                        and isinstance(row.get("ui_now"), (int, float)):
                    ends[turn] = row
                elif kind == "state":
                    at = parse_stamp(row.get("relay_utc"))
                    if at is not None:
                        boards[(turn, frame)] = at
                    if frame == 0 and isinstance(row.get("_bytes"), int):
                        self._seat_marks(marks, turn).setdefault("board_kb", row["_bytes"] / 1000)
                self._note(marks, turn, frame, kind, row)
            answers = brain_answers(self.run_dir / f"{self.tag}-p{seat}" / "brain.log", reference)
            for turn, end in ends.items():
                mine = sorted((frame, at, count) for (t, frame), (at, count) in answers.items()
                              if t == turn)
                at_end = self.fit.wall(turn, end["ui_now"])
                wait = end.get("wait")
                mark = self._seat_marks(marks, turn)
                mark["request"] = (at_end - float(wait) if isinstance(wait, (int, float))
                                   and at_end is not None else None)
                rows.append({
                    "turn": turn, "seat": seat,
                    "end": at_end,
                    "end_turn_wait": wait,
                    "answers": [(frame, at) for frame, at, _ in mine],
                    "orders": sum(count for _, _, count in mine),
                    "boards": {frame: boards[(turn, frame)] for frame, _, _ in mine
                               if (turn, frame) in boards},
                    "marks": mark,
                })
        rows.sort(key=lambda row: (row["turn"], row["seat"]))
        return rows

    @staticmethod
    def _seat_marks(marks: dict[int, dict], turn: int) -> dict:
        return marks.setdefault(turn, {"export": {}, "ask": {}, "applied": {}, "count": {},
                                       "reason": {}, "fights": 0, "fight_s": 0.0})

    def _note(self, marks: dict[int, dict], turn: int, frame: int, kind: object,
              row: dict) -> None:
        """One seat record's part in its turn's marks (wall seconds)."""
        mark = None
        if kind in ("export_timing", "replan_frame", "combat_frame", "orders"):
            at = self.wall(row) if isinstance(row.get("ui"), (int, float)) else None
            if at is None:
                return
            mark = self._seat_marks(marks, turn)
            if kind == "export_timing":
                mark["export"].setdefault(frame, at)
            elif kind == "orders":
                # The answer the mod applied; a fallback's record carries no clock.
                mark["applied"].setdefault(frame, at)
                mark["count"].setdefault(frame, row.get("applied"))
            else:
                mark["ask"].setdefault(frame, at)
                mark["reason"].setdefault(frame, row.get("reason"))
        elif kind == "combat" and row.get("ours") is True and row.get("our_turn") is not False:
            mark = self._seat_marks(marks, turn)
            mark["fights"] += 1
            if isinstance(row.get("vis_seconds"), (int, float)):
                mark["fight_s"] += float(row["vis_seconds"])
        elif kind == "turn" and isinstance(row.get("units"), int):
            self._seat_marks(marks, turn).setdefault("units", row["units"])
        elif kind == "pulse_tick" and row.get("why") in ("open", "settle"):
            mark = self._seat_marks(marks, turn)
            mark["pulses"] = mark.get("pulses", {})
            mark["pulses"][row["why"]] = mark["pulses"].get(row["why"], 0) + 1
            at = self.wall(row) if isinstance(row.get("ui"), (int, float)) else None
            if row["why"] == "open" and at is not None:
                # The opening's pulse tick fires once the game reads unpaused.
                mark.setdefault("unpaused", at)

    def base_marks(self) -> dict:
        """Wall times of the hand-over records, where they carry a clock."""
        marks = {"press": {}, "end": {}, "lift": {}, "waited": {}, "shown": {}, "played": set()}
        for row in self.base:
            kind, turn, player = row.get("kind"), row.get("turn"), row.get("player")
            if not isinstance(turn, int):
                continue
            if kind == "hotseat_turn" and isinstance(player, int):
                # Who played, clock or no clock: a seat after a person whose
                # turn end has no clock has no start either.
                marks["played"].add((turn, player))
            if kind == "hotseat_cover" and row.get("up") is False and row.get("reason") == "person_start":
                # The first lift of the turn is the wait: a turn handed back to
                # the person (hotseat-20261128T045523Z t33, an end the game
                # returned) lifts it again 1.5 s later.
                if isinstance(row.get("waited"), (int, float)):
                    marks["waited"].setdefault((turn, player), float(row["waited"]))
                # `at` is the cover's own UI clock; the relay's stamp waits for
                # the game's log flush.
                at = (self.fit.wall(turn, float(row["at"]))
                      if isinstance(row.get("at"), (int, float)) and self.fit else self.wall(row))
                if at is not None:
                    marks["lift"].setdefault((turn, player), at)
                continue
            at = self.wall(row)
            if at is None:
                continue
            if kind == "hotseat_start_turn":
                # The first press; a retry (`again`) two seconds on is not the hand-over.
                marks["press"].setdefault((turn, player), at)
            elif kind == "hotseat_turn" and row.get("phase") == "end":
                marks["end"][(turn, player)] = at
            elif kind == "hotseat_turn" and row.get("phase") == "begin" \
                    and isinstance(row.get("ui"), (int, float)):
                marks["shown"].setdefault((turn, player), at)
        return marks


def span(a: float | None, b: float | None) -> float | None:
    return None if a is None or b is None else b - a


#: A person's turn whose UI clock ran faster than the wall clock by more than
#: this is one that ran scaled (the relay's stamps are good to ~20 ms a record).
PERSON_RATIO_LIMIT = 1.15
#: ...measured only over turns at least this long (wall seconds).
PERSON_SPAN_MIN_S = 2.0
#: Every person's turn summed: at 1x the owner's 33 turns read 1.00, the test
#: stand-in's 0.1-0.5 s turns 1.10 and 1.14 (live-6p-20261128T0446Zd and
#: ...0614Ze: the relay's stamps weigh on spans that short); a turn at 2x or
#: 3x reads about 2 or 3.
PERSON_ALL_LIMIT = 1.5


def timescale_audit(run: Run) -> dict | None:
    """`--fast-animations`: what switched, what put it back, and how fast the
    UI clock ran on the person's turns and under the cover.

    A person's turn is from the cover's lift for them to its raise as their turn
    ends (`hotseat_cover`, `at` on the raw UI clock, `relay_utc` on the wall
    clock); its UI clock should run at the wall clock's pace. None for a run
    with neither the key nor any of these records."""
    every = [run.base, run.stub_events] + list(run.seat_events.values())
    rows = [row for journal in every for row in journal]
    kinds = ("fast_animations", "fast_animations_guard", "hotseat_scale_hold",
             "hotseat_scale_unconfirmed", "hotseat_cover_scale")
    if not run.config.get("HotseatFastAnimations") and not any(row.get("kind") in kinds for row in rows):
        return None
    out: dict = {"want": run.config.get("HotseatFastAnimations"), "on": 0, "off": {},
                 "reset_failed": 0, "guards": {}, "press_held": 0, "press_unconfirmed": 0,
                 "lift_held": 0, "lift_unconfirmed": 0}
    for row in rows:
        kind = row.get("kind")
        if kind == "fast_animations":
            phase = row.get("phase")
            if phase == "on":
                out["on"] += 1
            elif phase in ("off", "loaded"):
                why = str(row.get("why"))
                out["off"][why] = out["off"].get(why, 0) + 1
            elif phase == "reset_failed":
                out["reset_failed"] += 1
        elif kind == "fast_animations_guard":
            why = str(row.get("why"))
            out["guards"][why] = out["guards"].get(why, 0) + 1
        elif kind == "hotseat_scale_hold":
            out["press_held"] += 1
        elif kind == "hotseat_scale_unconfirmed":
            out["press_unconfirmed"] += 1
        elif kind == "hotseat_cover_scale":
            if row.get("held") is True:
                out["lift_held"] += 1
            elif row.get("confirmed") is False:
                out["lift_unconfirmed"] += 1
    ends = [row for journal in run.seat_events.values() for row in journal
            if row.get("kind") == "end_turn_wait"]
    out["seat_turns"] = len(ends)
    out["scaled_seat_turns"] = sum(1 for row in ends if isinstance(row.get("scale"), (int, float))
                                   and row["scale"] > 1)
    # The cover's records in order: lifts for a person, raises as a turn ends.
    marks = []
    for row in run.base:
        if row.get("kind") != "hotseat_cover":
            continue
        raw = row.get("_raw_at", row.get("at"))
        wall = parse_stamp(row.get("relay_utc"))
        if not isinstance(raw, (int, float)) or wall is None:
            continue
        if row.get("up") is False and row.get("reason") in ("person_start", "person_turn"):
            marks.append(("lift", raw, wall))
        elif row.get("up") is True and row.get("reason") == "turn_end":
            marks.append(("raise", raw, wall))
    person, covered = [], []
    # ...and every person's turn together: the test stand-in's last 0.1-0.5 s
    # each (live-6p-20261128T0614Ze), too short to read one by one against
    # the relay's stamps, but a game's worth of them sums to seconds.
    together = [0.0, 0.0, 0]
    for (a, ui_a, wall_a), (b, ui_b, wall_b) in zip(marks, marks[1:]):
        if a == "lift" and b == "raise" and wall_b > wall_a:
            together[0] += ui_b - ui_a
            together[1] += wall_b - wall_a
            together[2] += 1
        if wall_b - wall_a < PERSON_SPAN_MIN_S:
            continue
        ratio = (ui_b - ui_a) / (wall_b - wall_a)
        if a == "lift" and b == "raise":
            person.append(ratio)
        elif a == "raise" and b == "lift":
            covered.append(ratio)
    out["person_turns"] = len(person)
    out["person_ratio_max"] = max(person) if person else None
    out["person_over_limit"] = sum(1 for ratio in person if ratio > PERSON_RATIO_LIMIT)
    out["person_turns_all"] = together[2]
    out["person_wall_s"] = together[1]
    out["person_ratio_all"] = together[0] / together[1] if together[1] > 0 else None
    out["covered_ratio_median"] = median(covered)
    return out


def audit_report(audit: dict | None) -> str:
    if audit is None:
        return ""
    def counts(table: dict) -> str:
        return ", ".join(f"{key} {value}" for key, value in sorted(table.items())) or "none"
    ratio = audit["person_ratio_max"]
    covered = audit["covered_ratio_median"]
    together = audit["person_ratio_all"]
    return "\n".join([
        "", "",
        f"timescale (--fast-animations {audit['want']}): switched on {audit['on']} times; "
        f"back to 1 by {counts(audit['off'])}; refused {audit['reset_failed']}",
        f"  seat-turns run scaled {audit['scaled_seat_turns']} of {audit['seat_turns']}; "
        f"UI clock / wall under the cover, median {'-' if covered is None else f'{covered:.2f}'}",
        f"  MUST BE NONE: guard {counts(audit['guards'])}; person's press held "
        f"{audit['press_held']} (unconfirmed {audit['press_unconfirmed']}); lift held "
        f"{audit['lift_held']} (unconfirmed {audit['lift_unconfirmed']})",
        f"  person's turns: {audit['person_turns']} of {PERSON_SPAN_MIN_S:.0f} s or more, UI clock / "
        f"wall at most {'-' if ratio is None else f'{ratio:.2f}'}; over {PERSON_RATIO_LIMIT}: "
        f"{audit['person_over_limit']} (MUST BE 0)",
        f"  all {audit['person_turns_all']} person's turns together ({audit['person_wall_s']:.1f} s): "
        f"UI clock / wall {'-' if together is None else f'{together:.2f}'} (MUST BE AT MOST "
        f"{PERSON_ALL_LIMIT})",
    ])


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
        row["split"] = pieces(row, prev_end, marks["shown"].get(key), press)
        row["drivers"] = drivers(row, press)
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
        shown = marks["shown"].get((turn, person))
        rounds.append({
            "turn": turn, "person": person, "waited": waited, "seats": seat_rows,
            "played": played,
            # The person's end -> the first answer, plus the last seat's end
            # -> the person's next turn: what no seat-turn row covers.
            "ends": None if played is None else waited - played,
            "tail": span(seat_rows[-1]["end"], lift),
            # ...and the tail cut where the person's hand-over is shown: the
            # game's own rollover (other players, its autosave), then the
            # hand-over pressed and the cover lifted.
            "rollover": span(seat_rows[-1]["end"], shown),
            "person_handover": span(shown, lift),
        })
    return rounds


def pieces(row: dict, prev_end: float | None, shown: float | None,
           press: float | None) -> dict[str, float | None]:
    """One seat-turn cut at every mark with a clock (`PIECES`); a piece whose
    two marks are not both on record is None. Present pieces add up to the
    seat-turn's `total`."""
    mark = row.get("marks") or {}
    answers = dict(row["answers"])
    boards = row["boards"]
    export, ask, applied = (mark.get(name, {}) for name in ("export", "ask", "applied"))
    out: dict[str, float | None] = dict.fromkeys(f"{part}.{name}" for part, name in PIECES)
    out["open.shown"] = span(prev_end, shown)
    out["open.press"] = span(shown, press)
    out["open.to_export"] = span(press, export.get(0))
    out["open.export"] = span(export.get(0), boards.get(0))
    out["open.brain"] = span(boards.get(0), answers.get(0))
    out["open.seen"] = span(answers.get(0), applied.get(0))
    frames = sorted(answers)
    # A run whose mod stamped no clock on its applied answers has no frame
    # pieces at all, not a zero for a turn that had no extra frame.
    if frames and frames[0] == 0 and 0 in applied:
        sums = {"settle": 0.0, "export": 0.0, "brain": 0.0, "seen": 0.0}
        for before, frame in zip(frames, frames[1:]):
            parts = {"settle": span(applied.get(before), ask.get(frame)),
                     "export": span(ask.get(frame), boards.get(frame)),
                     "brain": span(boards.get(frame), answers.get(frame)),
                     "seen": span(answers.get(frame), applied.get(frame))}
            for name, value in parts.items():
                sums[name] = None if sums[name] is None or value is None else sums[name] + value
        for name, value in sums.items():
            out[f"frames.{name}"] = value
        out["close.settle"] = span(applied.get(frames[-1]), mark.get("request"))
    out["close.host"] = span(mark.get("request"), row["end"])
    return out


def drivers(row: dict, press: float | None = None) -> dict[str, float | None]:
    """What a seat-turn carried: the things its time grows with, and its
    pulse ticks. `unpause_ms`: Start Turn pressed -> the opening's pulse tick
    (the game unpaused), when both are on record."""
    mark = row.get("marks") or {}
    pulses = mark.get("pulses", {})
    extra = [frame for frame, _ in row["answers"] if frame > 0]
    counts = mark.get("count", {})
    return {
        "extra_frames": len(extra),
        "strike_frames": sum(1 for frame in extra if mark.get("reason", {}).get(frame) == "strike"),
        # An extra frame whose answer the mod applied nothing of.
        "empty_frames": sum(1 for frame in extra if counts.get(frame) == 0),
        "fights": mark.get("fights", 0),
        "fight_s": mark.get("fight_s", 0.0),
        "board_kb": mark.get("board_kb"),
        "orders": row["orders"],
        "units": mark.get("units"),
        "pulse_open": pulses.get("open", 0),
        "pulse_settle": pulses.get("settle", 0),
        "unpause_ms": (None if span(press, mark.get("unpaused")) is None
                       else span(press, mark.get("unpaused")) * 1000),
    }


def bands(rounds: list[dict], width: int = 10) -> list[dict]:
    """Rounds grouped by the turn their seats played, `width` turns a band; a
    last band under half as wide joins the one before it."""
    width = max(1, int(width))
    keyed = [(round_["seats"][0]["turn"], round_) for round_ in rounds if round_["seats"]]
    if not keyed:
        return []
    last = max(turn for turn, _ in keyed)
    edges = list(range(1, last + 1, width))
    if len(edges) > 1 and last - edges[-1] + 1 < (width + 1) // 2:
        edges.pop()
    out = []
    for index, low in enumerate(edges):
        high = edges[index + 1] - 1 if index + 1 < len(edges) else last
        mine = [round_ for turn, round_ in keyed if low <= turn <= high]
        if mine:
            out.append(summarise(low, high, mine))
    return out


def mean(values: list[float | None]) -> float | None:
    present = [value for value in values if value is not None]
    return statistics.mean(present) if present else None


def median(values: list[float | None]) -> float | None:
    present = [value for value in values if value is not None]
    return statistics.median(present) if present else None


def summarise(low: int, high: int, rounds: list[dict]) -> dict:
    seats = [row for round_ in rounds for row in round_["seats"]]
    per_round = len(seats) / len(rounds)
    names = [f"{part}.{name}" for part, name in PIECES] + ["total"]

    def values(name: str) -> list[float | None]:
        if name == "total":
            return [row["stages"]["total"] for row in seats]
        return [row["split"][name] for row in seats]

    # Per round, a piece's mean over the seat-turns that have it, times the
    # seat-turns a round has: means add up where medians do not.
    round_means = {name: (None if mean(values(name)) is None else mean(values(name)) * per_round)
                   for name in names}
    for name in ("rollover", "person_handover"):
        round_means[name] = mean([round_[name] for round_ in rounds])
    present = [round_means[name] for name in names[:-1] + ["rollover", "person_handover"]]
    complete = all(value is not None for value in present)
    return {
        "turns": [low, high], "rounds": len(rounds), "seat_turns": len(seats),
        "seat_turn_median": {name: median(values(name)) for name in names},
        "round_mean": round_means,
        # The pieces' sum: the band's mean round, when every piece has a clock.
        "round_sum": sum(present) if complete else None,
        "waited_mean": mean([round_["waited"] for round_ in rounds]),
        "waited_median": median([round_["waited"] for round_ in rounds]),
        "drivers": {name: (median if name in MEDIAN_DRIVERS else mean)(
            [row["drivers"][name] for row in seats]) for name in DRIVERS},
    }


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


def band_report(table: list[dict]) -> str:
    """The split by turn band: per seat-turn medians, per round means (which
    add up), and what each band's seat-turns carried."""
    if not table:
        return ""
    heads = [f"t{band['turns'][0]}-{band['turns'][1]}" for band in table]

    def row(label: str, cells: list[str]) -> str:
        return f"  {label:<22}" + "".join(f"{cell:>10}" for cell in cells)

    lines = ["", "", "by turn band (ms): a seat-turn's pieces, median | a round's, mean "
             "(they add up)"]
    lines.append(row("", heads) + "   |" + "".join(f"{head:>10}" for head in heads))
    for part, name in PIECES:
        key = f"{part}.{name}"
        lines.append(row(f"{part} {name}", [ms(band["seat_turn_median"][key]) for band in table])
                     + "   |" + "".join(f"{ms(band['round_mean'][key]):>10}" for band in table))
    lines.append(row("seat-turn total", [ms(band["seat_turn_median"]["total"]) for band in table])
                 + "   |" + "".join(f"{ms(band['round_mean']['total']):>10}" for band in table))
    for key, label in (("rollover", "rollover (game)"), ("person_handover", "person's hand-over")):
        lines.append(row(label, [""] * len(table))
                     + "   |" + "".join(f"{ms(band['round_mean'][key]):>10}" for band in table))
    lines.append(row("round, sum of pieces", [""] * len(table))
                 + "   |" + "".join(f"{ms(band['round_sum']):>10}" for band in table))
    lines.append(row("waited (cover), mean", [""] * len(table))
                 + "   |" + "".join(f"{ms(band['waited_mean']):>10}" for band in table))
    lines.append(row("waited (cover), median", [""] * len(table))
                 + "   |" + "".join(f"{ms(band['waited_median']):>10}" for band in table))
    lines.append(row("rounds / seat-turns", [f"{band['rounds']}/{band['seat_turns']}"
                                             for band in table]))
    lines.append("  per seat-turn, mean (board_kb and unpause_ms: median)")
    for name in DRIVERS:
        cells = []
        for band in table:
            value = band["drivers"][name]
            cells.append("-" if value is None else f"{value:.2f}")
        lines.append(row(name, cells))
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--json", action="store_true", help="the rounds as JSON")
    parser.add_argument("--rounds", type=int, default=None,
                        help="print only the last N rounds (the medians cover all)")
    parser.add_argument("--band", type=int, default=10,
                        help="turns per band in the split by turn band (default 10)")
    args = parser.parse_args(argv)
    if not args.run_dir.is_dir():
        print(f"{args.run_dir} is not a run directory", file=sys.stderr)
        return 2
    run = Run(args.run_dir)
    rounds = split(run)
    table = bands(rounds, args.band)
    if args.json:
        print(json.dumps({"tag": run.tag, "fit": run.fit.at(None), "rounds": rounds,
                          "bands": table, "timescale": timescale_audit(run)}, indent=1))
    else:
        print(report(run, rounds, args.rounds) + band_report(table)
              + audit_report(timescale_audit(run)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
