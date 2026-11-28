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
