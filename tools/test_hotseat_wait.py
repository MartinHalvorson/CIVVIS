#!/usr/bin/env python3
"""`hotseat_wait.py` splits a person's wait into its stages from a run's records."""

from __future__ import annotations

import contextlib
import io
import json
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import hotseat_wait  # noqa: E402

TAG = "hs"
#: wall = ui + OFFSET for every record of the fixture.
OFFSET = 1795835700.0


def tod(wall: float) -> str:
    """`brain.log`'s time of day for a wall time."""
    return datetime.fromtimestamp(wall, timezone.utc).strftime("%H:%M:%S.%f")[:-3]


def stamp(wall: float) -> str:
    return datetime.fromtimestamp(wall, timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


class Fixture:
    """A two-CIVVIS-seat table, the person on seat 0, written as the mod and
    the relay write it. `stamped` adds the relay's receipt stamps."""

    def __init__(self, root: Path, stamped: bool = False):
        self.root, self.stamped = root, stamped
        self.base: list[dict] = []
        self.seat: dict[int, list[dict]] = {1: [], 2: []}
        self.brain: dict[int, list[str]] = {1: [], 2: []}
        (root / "config.json").write_text(json.dumps(
            {"RunTag": TAG, "HotseatSeats": [1, 2], "HotseatHumanSeats": [0]}))

    def relay(self, row: dict, wall: float) -> dict:
        if self.stamped:
            row["relay_utc"] = stamp(wall)
        return row

    def person(self, turn: int, end: float | None, lift: float | None, waited: float | None):
        self.base.append({"kind": "hotseat_turn", "phase": "begin", "player": 0, "turn": turn})
        if end is not None:
            self.base.append(self.relay({"kind": "hotseat_turn", "phase": "end", "player": 0,
                                         "turn": turn}, end))
        if waited is not None:
            self.base.append(self.relay({"kind": "hotseat_cover", "up": False, "player": 0,
                                         "reason": "person_start", "turn": turn + 1,
                                         "waited": waited}, lift or 0))

    def play(self, seat: int, turn: int, press: float, boards: list[float],
             answers: list[float], end: float, etw: float = 0.05) -> None:
        self.base.append({"kind": "hotseat_turn", "phase": "begin", "player": seat, "turn": turn})
        self.base.append(self.relay({"kind": "hotseat_start_turn", "player": seat, "turn": turn,
                                     "again": False}, press))
        for frame, (board, answer) in enumerate(zip(boards, answers)):
            self.seat[seat].append(self.relay({"kind": "state", "turn": turn, "frame": frame,
                                               "t": int(board)}, board))
            suffix = f" frame {frame}" if frame else ""
            self.brain[seat].append(f"[brain] turn {turn}{suffix}: 3 orders in 0.01s at "
                                    f"{tod(answer)}Z")
        self.seat[seat].append({"kind": "end_turn_wait", "turn": turn, "t": int(end),
                                "ui_now": end - OFFSET, "wait": etw})
        self.base.append({"kind": "hotseat_turn", "phase": "end", "player": seat, "turn": turn})

    def write(self) -> Path:
        (self.root / "events.jsonl").write_text(
            "".join(json.dumps(row) + "\n" for row in self.base))
        for seat in (1, 2):
            folder = self.root / f"{TAG}-p{seat}"
            folder.mkdir(exist_ok=True)
            (folder / "events.jsonl").write_text(
                "".join(json.dumps(row) + "\n" for row in self.seat[seat]))
            (folder / "brain.log").write_text("\n".join(self.brain[seat]) + "\n")
        return self.root


def table(root: Path, stamped: bool = False) -> Path:
    """Turn 7: the person ends at 100.000; seat 1 plays one frame, seat 2 two;
    the person's turn 8 begins at 104.000 (waited 4.0). Every turn end's
    fractional second is .005 or .995, so the offset fits within 5 ms."""
    base = OFFSET + 1400
    fx = Fixture(root, stamped)
    fx.person(7, end=base + 100.0, lift=base + 104.0, waited=4.0)
    fx.play(1, 7, press=base + 100.350, boards=[base + 100.500], answers=[base + 100.540],
            end=base + 100.995)
    fx.play(2, 7, press=base + 101.400, boards=[base + 101.600, base + 102.300],
            answers=[base + 101.620, base + 102.330], end=base + 103.005)
    return fx.write()


def timed_table(root: Path, handed_back: bool = False) -> Path:
    """Turn 7 with every clock the 2026-11-28 records carry (`ui` on the
    hand-over, the exports, the frames and the applied answers; the cover's
    `at`). The person ends at 100.000; seat 1 plays an opening and one strike
    frame that applies nothing, seat 2 an opening only; the person's turn 8 is
    shown at 103.000 and the cover lifts at 103.017 (waited 3.017).
    `handed_back`: the game returns turn 8 to the person and lifts again."""
    base = OFFSET + 1400
    fx = Fixture(root, stamped=True)
    ui = lambda at: base + at - OFFSET  # noqa: E731 - the UI clock of a wall time
    fx.base.append({"kind": "hotseat_turn", "phase": "end", "player": 0, "turn": 7,
                    "ui": ui(100.0)})

    def seat(number: int, shown: float, press: float, frames: list[dict], request: float,
             end: float) -> None:
        fx.base.append({"kind": "hotseat_turn", "phase": "begin", "player": number, "turn": 7,
                        "ui": ui(shown)})
        fx.base.append({"kind": "hotseat_start_turn", "player": number, "turn": 7,
                        "again": False, "ui": ui(press)})
        rows = fx.seat[number]
        for index, frame in enumerate(frames):
            if index:
                rows.append({"kind": frame.get("kind", "replan_frame"), "turn": 7,
                             "frame": index, "reason": frame.get("reason", "revealed"),
                             "ui": ui(frame["ask"])})
            rows.append({"kind": "export_timing", "turn": 7, "frame": index,
                         "ui": ui(frame.get("export", frame.get("ask", 0.0)))})
            rows.append({"kind": "state", "turn": 7, "frame": index, "t": int(base + frame["board"]),
                         "pad": "x" * (2000 if index == 0 else 10),
                         "relay_utc": stamp(base + frame["board"])})
            suffix = f" frame {index}" if index else ""
            fx.brain[number].append(f"[brain] turn 7{suffix}: 3 orders in 0.01s at "
                                    f"{tod(base + frame['answer'])}Z")
            rows.append({"kind": "orders", "turn": 7, "frame": index,
                         "applied": frame.get("applied", 2), "ui": ui(frame["applied_at"])})
        rows.append({"kind": "turn", "turn": 7, "units": 4})
        rows.append({"kind": "end_turn_wait", "turn": 7, "t": int(base + end),
                     "ui_now": ui(end), "wait": round(end - request, 6)})
        fx.base.append({"kind": "hotseat_turn", "phase": "end", "player": number, "turn": 7,
                        "ui": ui(end)})

    seat(1, shown=100.200, press=100.217,
         frames=[{"export": 100.330, "board": 100.380, "answer": 100.410, "applied_at": 100.440},
                 {"kind": "combat_frame", "reason": "strike", "ask": 100.600, "board": 100.650,
                  "answer": 100.670, "applied_at": 100.700, "applied": 0}],
         request=100.850, end=100.995)
    fx.seat[1].append({"kind": "combat", "turn": 7, "ours": True, "our_turn": True,
                       "vis_seconds": 0.7})
    # `--fast-turns` pulse ticks (#6053): seat 1's opening 13 ms after its
    # press, and the settle of its last frame; seat 2 has none.
    fx.seat[1].append({"kind": "pulse_tick", "turn": 7, "frame": 0, "why": "open", "n": 1,
                       "ui": ui(100.230)})
    fx.seat[1].append({"kind": "pulse_tick", "turn": 7, "frame": 1, "why": "settle", "n": 1,
                       "ui": ui(100.720)})
    fx.seat[1].append({"kind": "pulse_tick", "turn": 7, "frame": 1, "why": "settle", "n": 2,
                       "ui": ui(100.740)})
    fx.seat[1].append({"kind": "combat", "turn": 7, "ours": False, "vis_seconds": 0.5})
    # Seat 2's only pulse tick is a settle: no opening pulse, no unpause.
    fx.seat[2].append({"kind": "pulse_tick", "turn": 7, "frame": 0, "why": "settle", "n": 1,
                       "ui": ui(101.300)})
    seat(2, shown=100.995, press=101.012,
         frames=[{"export": 101.100, "board": 101.150, "answer": 101.170, "applied_at": 101.200}],
         request=101.400, end=102.005)
    fx.base.append({"kind": "hotseat_turn", "phase": "begin", "player": 0, "turn": 8,
                    "ui": ui(103.0)})
    fx.base.append({"kind": "hotseat_cover", "up": False, "player": 0, "reason": "person_start",
                    "turn": 8, "waited": 3.017, "at": ui(103.017),
                    "relay_utc": stamp(base + 103.4)})
    if handed_back:
        fx.base.append({"kind": "hotseat_cover", "up": False, "player": 0,
                        "reason": "person_start", "turn": 8, "waited": 1.5,
                        "at": ui(160.0), "relay_utc": stamp(base + 160.1)})
    return fx.write()


class ClockTest(unittest.TestCase):
    def test_the_offset_is_the_middle_of_what_every_turn_end_allows(self) -> None:
        fit = hotseat_wait.fit_ui_offset([(1000, 99.995), (1001, 100.005)])
        # t <= ui + C < t + 1: the first pair allows [900.005, 901.005), the
        # second [900.995, 901.995); together [900.995, 901.005).
        self.assertAlmostEqual(fit[0], 901.0, places=6)
        self.assertAlmostEqual(fit[1], 0.005, places=6)

    def test_turn_ends_that_disagree_still_give_an_offset_and_say_so(self) -> None:
        fit = hotseat_wait.fit_ui_offset([(1000, 100.0), (1002, 100.0)])
        self.assertLess(fit[1], 0)
        self.assertAlmostEqual(fit[0], 901.5, msg="the middle of the two bounds")

    def test_each_turn_is_placed_by_the_turn_ends_near_it(self) -> None:
        # The UI clock runs 1.3 s behind by turn 20: no single offset fits.
        def pair(turn: int, offset: float) -> tuple[int, float, float]:
            wall = 5000 + 10 * turn + (0.1 if turn % 2 else 0.9)
            return turn, float(int(wall)), wall - offset
        pairs = [pair(turn, 1000.0) for turn in range(1, 10)]
        pairs += [pair(turn, 1001.3) for turn in range(20, 30)]
        fit = hotseat_wait.ClockFit(pairs)
        self.assertLess(fit.at(None)[1], 0, "one offset for the game does not fit")
        self.assertAlmostEqual(fit.at(5)[0], 1000.0, places=6)
        self.assertAlmostEqual(fit.at(25)[0], 1001.3, places=6)
        self.assertAlmostEqual(fit.wall(25, 24.2), 1025.5, places=6)
        self.assertEqual(fit.disagreeing(), 0, "every seven-turn window fits")

    def test_an_answer_just_after_midnight_lands_on_the_next_day(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "brain.log"
            path.write_text("[brain] turn 4: 2 orders in 0.00s at 00:00:00.250Z\n")
            near = datetime(2026, 11, 27, 23, 59, 59, tzinfo=timezone.utc).timestamp()
            answers = hotseat_wait.brain_answers(path, {4: near})
            self.assertAlmostEqual(answers[(4, 0)][0], near + 1.25, places=3)


class SplitTest(unittest.TestCase):
    def rounds(self, stamped: bool):
        with tempfile.TemporaryDirectory() as temporary:
            run = hotseat_wait.Run(table(Path(temporary), stamped))
            return hotseat_wait.split(run), run

    def test_without_stamps_a_seat_turn_is_open_frames_and_close(self) -> None:
        rounds, _ = self.rounds(stamped=False)
        self.assertEqual(len(rounds), 1)
        round_ = rounds[0]
        self.assertEqual(round_["turn"], 8)
        self.assertEqual([row["seat"] for row in round_["seats"]], [1, 2])
        first, second = (row["stages"] for row in round_["seats"])
        self.assertIsNone(first["open"], "the person's turn end has no clock here")
        self.assertAlmostEqual(first["close"], 0.455, places=2)
        self.assertAlmostEqual(first["frames"], 0.0)
        self.assertAlmostEqual(second["open"], 101.620 - 100.995, places=2)
        self.assertAlmostEqual(second["frames"], 0.710, places=2)
        self.assertAlmostEqual(second["close"], 103.005 - 102.330, places=2)
        self.assertAlmostEqual(second["total"], 103.005 - 100.995, places=2)
        for name in ("handover", "start", "brain", "remote"):
            self.assertIsNone(second[name], name)
        self.assertEqual(round_["seats"][1]["extra_frames"], 1)
        self.assertAlmostEqual(round_["played"], 103.005 - 100.540, places=2)
        self.assertAlmostEqual(round_["ends"], 4.0 - (103.005 - 100.540), places=2)

    def test_with_the_relays_stamps_open_and_frames_split_further(self) -> None:
        rounds, _ = self.rounds(stamped=True)
        first, second = (row["stages"] for row in rounds[0]["seats"])
        self.assertAlmostEqual(first["open"], 0.540, places=2)
        self.assertAlmostEqual(first["handover"], 0.350, places=2)
        self.assertAlmostEqual(first["start"], 0.150, places=2)
        self.assertAlmostEqual(first["brain"], 0.040, places=2)
        self.assertAlmostEqual(second["handover"], 101.400 - 100.995, places=2)
        self.assertAlmostEqual(second["start"], 0.200, places=2)
        self.assertAlmostEqual(second["brain"], 0.020 + 0.030, places=2)
        self.assertAlmostEqual(second["remote"], 102.300 - 101.620, places=2)
        self.assertAlmostEqual(rounds[0]["tail"], 104.0 - 103.005, places=2)

    def test_the_hand_over_is_the_first_press_not_a_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = table(Path(temporary), stamped=True)
            with (root / "events.jsonl").open("a") as output:
                output.write(json.dumps({"kind": "hotseat_start_turn", "player": 1, "turn": 7,
                                         "again": True,
                                         "relay_utc": stamp(OFFSET + 1400 + 102.35)}) + "\n")
            rounds = hotseat_wait.split(hotseat_wait.Run(root))
            self.assertAlmostEqual(rounds[0]["seats"][0]["stages"]["handover"], 0.350, places=2)

    def test_a_seat_after_a_turn_with_no_clock_gets_no_start(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = table(Path(temporary), stamped=True)
            seat1 = root / f"{TAG}-p1" / "events.jsonl"
            seat1.write_text("".join(line for line in seat1.read_text().splitlines(True)
                                     if "end_turn_wait" not in line))
            run = hotseat_wait.Run(root)
            rounds = hotseat_wait.split(run)
            self.assertEqual([row["seat"] for row in rounds[0]["seats"]], [2])
            self.assertIsNone(rounds[0]["seats"][0]["stages"]["open"],
                              "not measured from the person's turn, which has a clock")


class PiecesTest(unittest.TestCase):
    """The split by mark: every piece of a seat-turn, the rollover, the bands."""

    def rounds(self, handed_back: bool = False):
        with tempfile.TemporaryDirectory() as temporary:
            return hotseat_wait.split(hotseat_wait.Run(timed_table(Path(temporary), handed_back)))

    def test_each_piece_is_the_gap_between_its_two_marks(self) -> None:
        first, second = (row["split"] for row in self.rounds()[0]["seats"])
        want_first = {
            "open.shown": 0.200, "open.press": 0.017, "open.to_export": 0.113,
            "open.export": 0.050, "open.brain": 0.030, "open.seen": 0.030,
            "frames.settle": 0.160, "frames.export": 0.050, "frames.brain": 0.020,
            "frames.seen": 0.030, "close.settle": 0.150, "close.host": 0.145,
        }
        for name, want in want_first.items():
            self.assertAlmostEqual(first[name], want, places=2, msg=name)
        self.assertAlmostEqual(second["open.shown"], 0.0, places=2,
                               msg="a CIVVIS seat's hand-over shows in the frame the last ended")
        self.assertAlmostEqual(second["frames.settle"], 0.0, places=6, msg="no extra frame")
        self.assertAlmostEqual(second["close.settle"], 0.200, places=2)
        self.assertAlmostEqual(second["close.host"], 0.605, places=2)

    def test_the_pieces_add_up_to_the_seat_turn(self) -> None:
        for row in self.rounds()[0]["seats"]:
            self.assertAlmostEqual(sum(row["split"].values()), row["stages"]["total"], places=6)

    def test_the_tail_is_cut_where_the_persons_hand_over_shows(self) -> None:
        round_ = self.rounds()[0]
        self.assertAlmostEqual(round_["rollover"], 103.0 - 102.005, places=2)
        self.assertAlmostEqual(round_["person_handover"], 0.017, places=2)
        self.assertAlmostEqual(round_["tail"], 103.017 - 102.005, places=2,
                               msg="the lift is placed by the cover's own clock, not the relay")

    def test_a_turn_handed_back_to_the_person_keeps_its_first_wait(self) -> None:
        round_ = self.rounds(handed_back=True)[0]
        self.assertAlmostEqual(round_["waited"], 3.017, places=6)
        self.assertAlmostEqual(round_["tail"], 103.017 - 102.005, places=2)

    def test_what_a_seat_turn_carried(self) -> None:
        first, second = (row["drivers"] for row in self.rounds()[0]["seats"])
        self.assertEqual((first["extra_frames"], first["strike_frames"], first["empty_frames"]),
                         (1, 1, 1))
        self.assertEqual(first["fights"], 1, "only our own fights on our turn")
        self.assertAlmostEqual(first["fight_s"], 0.7)
        self.assertEqual(first["units"], 4)
        self.assertGreater(first["board_kb"], 2.0, "the opening board's size, not a frame's")
        self.assertEqual(second["extra_frames"], 0)
        self.assertEqual((first["pulse_open"], first["pulse_settle"]), (1, 2))
        self.assertAlmostEqual(first["unpause_ms"], 13, delta=2,
                               msg="Start Turn pressed -> the opening's pulse tick")
        self.assertEqual((second["pulse_open"], second["pulse_settle"]), (0, 1))
        self.assertIsNone(second["unpause_ms"], "a settle pulse is not the unpause")

    def test_a_band_adds_up_to_the_wait(self) -> None:
        table = hotseat_wait.bands(self.rounds())
        self.assertEqual(len(table), 1)
        band = table[0]
        self.assertEqual(band["turns"], [1, 7], "the first band, as far as the game went")
        self.assertAlmostEqual(band["round_sum"], band["waited_mean"], places=2)
        self.assertAlmostEqual(band["round_mean"]["close.host"], 0.145 + 0.605, places=2,
                               msg="per round: the seat-turns' sum")
        self.assertAlmostEqual(band["seat_turn_median"]["close.host"], (0.145 + 0.605) / 2,
                               places=2)
        self.assertAlmostEqual(band["drivers"]["extra_frames"], 0.5)
        self.assertAlmostEqual(band["drivers"]["pulse_settle"], 1.5, msg="a mean per seat-turn")
        self.assertAlmostEqual(band["drivers"]["unpause_ms"], 13, delta=2,
                               msg="a median over the seat-turns that have one")

    def test_the_unpause_is_a_median_and_frames_a_mean(self) -> None:
        def seat_row(unpause: float, frames: int) -> dict:
            drivers = dict.fromkeys(hotseat_wait.DRIVERS, 0)
            drivers.update(unpause_ms=unpause, extra_frames=frames)
            return {"turn": 3, "stages": {"total": 1.0}, "drivers": drivers, "split": dict.fromkeys(
                f"{part}.{name}" for part, name in hotseat_wait.PIECES)}
        round_ = {"seats": [seat_row(10, 0), seat_row(20, 0), seat_row(90, 3)], "waited": 1.0,
                  "rollover": None, "person_handover": None}
        band = hotseat_wait.summarise(1, 10, [round_])
        self.assertEqual(band["drivers"]["unpause_ms"], 20)
        self.assertEqual(band["drivers"]["extra_frames"], 1)

    def test_bands_are_width_turns_and_a_short_last_one_joins_the_one_before(self) -> None:
        def fake(turn: int) -> dict:
            row = {"turn": turn, "stages": {"total": 1.0}, "drivers": dict.fromkeys(
                hotseat_wait.DRIVERS, 0), "split": dict.fromkeys(
                f"{part}.{name}" for part, name in hotseat_wait.PIECES)}
            return {"seats": [row], "waited": 1.0, "rollover": None, "person_handover": None}
        rounds = [fake(turn) for turn in range(1, 33)]
        self.assertEqual([band["turns"] for band in hotseat_wait.bands(rounds, 10)],
                         [[1, 10], [11, 20], [21, 32]])
        rounds += [fake(turn) for turn in range(33, 37)]
        self.assertEqual([band["turns"] for band in hotseat_wait.bands(rounds, 10)],
                         [[1, 10], [11, 20], [21, 30], [31, 36]])

    def test_a_run_without_the_finer_clocks_leaves_the_pieces_empty(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            rounds = hotseat_wait.split(hotseat_wait.Run(table(Path(temporary), stamped=True)))
        second = rounds[0]["seats"][1]["split"]
        self.assertIsNone(second["open.to_export"])
        self.assertIsNone(second["close.settle"])
        first = rounds[0]["seats"][0]["split"]
        self.assertIsNone(first["frames.settle"], "no frame clock is not a frame of zero")
        self.assertIsNone(hotseat_wait.bands(rounds)[0]["round_sum"],
                          "no sum of pieces that are not all there")

    def test_the_report_prints_the_band_table(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(hotseat_wait.main([str(timed_table(Path(temporary)))]), 0)
        text = out.getvalue()
        self.assertIn("by turn band", text)
        band_row = next(line for line in text.splitlines() if line.startswith("  close host"))
        self.assertEqual(band_row.split()[2:], ["375", "|", "750"])
        self.assertIn("round, sum of pieces", text)


def scaled(root: Path, scale: float = 3.0, on: float = 100.1, off: float = 102.005,
           person_scale: float = 1.0) -> Path:
    """`timed_table` as a `--fast-animations` run records it: the engine at
    `scale` from `on` to `off` (wall seconds after the fixture's base), so every
    UI clock stamp in between runs `scale` times fast, and the agent's two
    switch records say when. `person_scale`: the person's next turn (from the
    lift) runs at that scale instead of 1, as a reset that did not take would."""
    timed_table(root)
    base = OFFSET + 1400
    u_on = base + on - OFFSET
    u_off = u_on + (off - on) * scale

    def raw(real: float) -> float:
        if real < u_on:
            return real
        if real < u_on + (off - on):
            return u_on + (real - u_on) * scale
        return u_off + (real - (u_on + off - on))

    for path in [root / "events.jsonl"] + sorted(root.glob(f"{TAG}-p*/events.jsonl")):
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        for row in rows:
            for key in ("ui", "ui_now", "at"):
                if isinstance(row.get(key), (int, float)):
                    row[key] = raw(row[key])
        if path.parent.name == f"{TAG}-p1":
            rows.append({"kind": "fast_animations", "phase": "on", "scale": scale, "ui": u_on,
                         "turn": 7})
        if path.parent.name == f"{TAG}-p2":
            rows.append({"kind": "fast_animations", "phase": "off", "why": "last_seat_end",
                         "scale": 1, "ui": u_off, "turn": 7})
        if path.name == "events.jsonl" and path.parent == root:
            # The cover's raise as the person's turn 8 ends, 6 s after the lift.
            lift = next(row for row in rows if row.get("kind") == "hotseat_cover")
            rows.append({"kind": "hotseat_cover", "ctx": "cover", "up": True, "reason": "turn_end",
                         "player": 0, "turn": 8, "at": lift["at"] + 6.0 * person_scale,
                         "relay_utc": stamp(base + 103.4 + 6.0)})
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
    config = json.loads((root / "config.json").read_text())
    config["HotseatFastAnimations"] = scale
    (root / "config.json").write_text(json.dumps(config))
    return root


class TimescaleTest(unittest.TestCase):
    """`--fast-animations`: the UI clock runs N times fast under the cover, and
    the split still reads seconds."""

    def test_the_clock_is_itself_without_a_switch(self) -> None:
        clock = hotseat_wait.RealClock([])
        self.assertFalse(clock)
        self.assertEqual(clock.real(123.25), 123.25)

    def test_each_stretch_runs_at_its_scale(self) -> None:
        clock = hotseat_wait.RealClock([(130.0, 1), (100.0, 3), (140.0, 99)])
        self.assertEqual(clock.real(90.0), 90.0, "before the first switch: itself")
        self.assertAlmostEqual(clock.real(103.0), 101.0)
        self.assertAlmostEqual(clock.real(130.0), 110.0)
        self.assertAlmostEqual(clock.real(140.0), 120.0, msg="a scale out of range is no switch")
        self.assertEqual((clock.scale_at(99.0), clock.scale_at(101.0), clock.scale_at(131.0)), (1, 3, 1))

    def test_a_run_at_three_splits_into_the_same_seconds_as_at_one(self) -> None:
        with tempfile.TemporaryDirectory() as plain, tempfile.TemporaryDirectory() as fast:
            want = hotseat_wait.split(hotseat_wait.Run(timed_table(Path(plain))))
            got = hotseat_wait.split(hotseat_wait.Run(scaled(Path(fast))))
        for mine, theirs in zip(got[0]["seats"], want[0]["seats"]):
            for name, value in theirs["split"].items():
                if value is None:
                    self.assertIsNone(mine["split"][name], name)
                else:
                    self.assertAlmostEqual(mine["split"][name], value, places=3, msg=name)
        self.assertAlmostEqual(got[0]["rollover"], want[0]["rollover"], places=3)
        self.assertAlmostEqual(got[0]["tail"], want[0]["tail"], places=3)

    def test_the_raw_clock_would_not_have(self) -> None:
        # The same run read with no switch records: three times too long.
        with tempfile.TemporaryDirectory() as fast:
            root = scaled(Path(fast))
            for path in root.glob(f"{TAG}-p*/events.jsonl"):
                path.write_text("".join(line + "\n" for line in path.read_text().splitlines()
                                        if "fast_animations" not in line))
            rows = hotseat_wait.split(hotseat_wait.Run(root))
        self.assertGreater(rows[0]["seats"][0]["split"]["frames.settle"], 0.4,
                           "0.16 s of settle reads 0.48 s on the raw clock")

    def test_the_audit_reads_the_switches_and_the_persons_turn(self) -> None:
        with tempfile.TemporaryDirectory() as fast:
            audit = hotseat_wait.timescale_audit(hotseat_wait.Run(scaled(Path(fast))))
        self.assertEqual(audit["on"], 1)
        self.assertEqual(audit["off"], {"last_seat_end": 1})
        self.assertEqual(audit["person_turns"], 1)
        self.assertAlmostEqual(audit["person_ratio_max"], 1.0, places=2)
        self.assertEqual(audit["person_over_limit"], 0)
        self.assertEqual(audit["guards"], {})

    def test_a_persons_turn_at_three_is_flagged(self) -> None:
        with tempfile.TemporaryDirectory() as fast:
            audit = hotseat_wait.timescale_audit(hotseat_wait.Run(scaled(Path(fast), person_scale=3)))
        self.assertEqual(audit["person_over_limit"], 1)
        self.assertAlmostEqual(audit["person_ratio_max"], 3.0, places=2)

    def test_what_must_never_appear_is_counted(self) -> None:
        with tempfile.TemporaryDirectory() as fast:
            root = scaled(Path(fast))
            with (root / "events.jsonl").open("a") as handle:
                for row in ({"kind": "fast_animations_guard", "why": "scale", "by": "heartbeat",
                             "ui": 1.0, "turn": 8},
                            {"kind": "hotseat_scale_hold", "player": 0, "turn": 8},
                            {"kind": "hotseat_cover_scale", "held": True, "turn": 8},
                            {"kind": "hotseat_cover_scale", "held": False, "confirmed": False,
                             "turn": 8}):
                    handle.write(json.dumps(row) + "\n")
            run = hotseat_wait.Run(root)
            audit = hotseat_wait.timescale_audit(run)
            text = hotseat_wait.audit_report(audit)
        self.assertEqual(audit["guards"], {"scale": 1})
        self.assertEqual((audit["press_held"], audit["lift_held"], audit["lift_unconfirmed"]), (1, 1, 1))
        self.assertIn("MUST BE NONE: guard scale 1", text)
        self.assertIn("(MUST BE 0)", text)

    def test_a_run_without_the_key_has_no_audit(self) -> None:
        with tempfile.TemporaryDirectory() as plain:
            run = hotseat_wait.Run(timed_table(Path(plain)))
            self.assertIsNone(hotseat_wait.timescale_audit(run))
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                hotseat_wait.main([str(Path(plain))])
        self.assertNotIn("timescale", out.getvalue())


class ReportTest(unittest.TestCase):
    def test_the_report_names_every_stage_and_marks_what_has_no_clock(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(hotseat_wait.main([str(table(Path(temporary)))]), 0)
        text = out.getvalue()
        self.assertIn("turn 8: the person waited 4000", text)
        second = next(row for row in text.splitlines() if row.startswith("  p2 "))
        self.assertEqual(second.split()[1:3], ["625", "-"], "open measured, handover not")
        for name in hotseat_wait.STAGES:
            self.assertIn(f"  {name:<9}", text)
        self.assertIn("one extra frame 710 (n=1)", text)

    def test_json_carries_the_rounds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                hotseat_wait.main([str(table(Path(temporary), stamped=True)), "--json"])
        payload = json.loads(out.getvalue())
        self.assertEqual(payload["rounds"][0]["seats"][0]["stages"]["handover"] is not None, True)

    def test_a_missing_directory_is_refused(self) -> None:
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(hotseat_wait.main(["/nonexistent/hotseat-run"]), 2)


if __name__ == "__main__":
    unittest.main()
