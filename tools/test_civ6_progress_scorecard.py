#!/usr/bin/env python3
"""The scorecard compares batches of games: one game per game, one difficulty
and speed per group, and the ledger left alone."""
from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import civ6_progress_scorecard as scorecard


def economy(turn: str, cities: int) -> dict:
    return {turn: {"cities": cities}}


def row(tag: str, **extra) -> dict:
    return {"tag": tag, "difficulty": "DIFFICULTY_DEITY",
            "speed": "GAMESPEED_ONLINE", "leader": "LEADER_A",
            "code_rev": "abc", **extra}


def jsonl(path: Path, rows: list[dict]) -> Path:
    path.write_text("".join(json.dumps(r) + "\n" for r in rows))
    return path


class Scorecard(unittest.TestCase):
    def setUp(self):
        directory = TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.ledger = self.root / "ledger.jsonl"
        self.side = self.root / "side.jsonl"
        self.control = self.root / "control"
        self.control.mkdir()

    def run_card(self, *flags: str) -> tuple[int, dict, str]:
        out = self.root / "out.json"
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = scorecard.main([
                "--ledger", str(self.ledger), "--side-file", str(self.side),
                "--control", str(self.control), "--json", str(out), "--jobs", "1",
                *flags])
        return code, json.loads(out.read_text()), stdout.getvalue()

    def test_the_median_and_the_count_of_each_mark(self):
        jsonl(self.ledger, [
            row("civvis-20261109T010101Z", economy_marks=economy("50", 3)),
            row("civvis-20261109T020101Z", economy_marks=economy("50", 4)),
            row("civvis-20261109T030101Z", economy_marks=economy("50", 8)),
            row("civvis-20261109T040101Z"),
        ])
        _, summaries, text = self.run_card()
        group = summaries["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261109"]
        self.assertEqual(group["games"], 4)
        self.assertEqual(group["economy"]["50"]["cities"], {"median": 4, "n": 3})
        self.assertIn("4 games (LEADER_A 4)", text)

    def test_difficulties_and_speeds_are_never_pooled(self):
        jsonl(self.ledger, [
            row("civvis-20261109T010101Z", economy_marks=economy("50", 3)),
            row("civvis-20261109T020101Z", difficulty="DIFFICULTY_KING",
                economy_marks=economy("50", 9)),
            row("civvis-20261109T030101Z", speed="GAMESPEED_STANDARD",
                economy_marks=economy("50", 5)),
        ])
        _, summaries, _ = self.run_card("--group", "none")
        self.assertEqual(sorted(summaries), [
            "DIFFICULTY_DEITY GAMESPEED_ONLINE all",
            "DIFFICULTY_DEITY GAMESPEED_STANDARD all",
            "DIFFICULTY_KING GAMESPEED_ONLINE all",
        ])

    def test_the_segments_of_a_parked_game_are_one_game(self):
        tag = "civvis-20261109T010101Z"
        jsonl(self.ledger, [
            row(tag, economy_marks=economy("50", 3),
                # The older reader files a late board under an early mark.
                tech_marks={"100": {"techs": 25}},
                religion_marks={"religion": "RELIGION_X", "founded_turn": 18,
                                "spreads": {"1-50": 2}, "at": {},
                                "last": {"converted": 0}}),
            row(tag + "-cont1", economy_marks=economy("150", 7),
                tech_marks={"100": {"techs": 44}},
                religion_marks={"religion": "RELIGION_X", "founded_turn": None,
                                "spreads": {"1-50": 1, "151+": 5}, "at": {},
                                "last": {"converted": 2}}),
        ])
        _, summaries, _ = self.run_card()
        group = summaries["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261109"]
        self.assertEqual(group["games"], 1)
        self.assertEqual(group["economy"]["50"]["cities"]["median"], 3)
        self.assertEqual(group["economy"]["150"]["cities"]["median"], 7)
        self.assertEqual(group["tech"]["100"]["techs"]["median"], 25)
        self.assertEqual(group["religion"]["founded_turn"]["median"], 18)
        self.assertEqual(group["religion"]["spreads"]["1-50"]["median"], 3)
        self.assertEqual(group["religion"]["last"]["converted"]["median"], 2)

    def test_the_filters(self):
        jsonl(self.ledger, [
            row("civvis-20261108T010101Z"),
            row("civvis-20261109T010101Z"),
            row("civvis-20261110T010101Z", leader="LEADER_B"),
        ])
        _, summaries, _ = self.run_card("--since", "20261109", "--until", "20261109")
        self.assertEqual(sorted(summaries),
                         ["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261109"])
        _, summaries, _ = self.run_card("--leader", "LEADER_B")
        self.assertEqual(sorted(summaries),
                         ["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261110"])

    def test_grouping_by_code_revision(self):
        jsonl(self.ledger, [row("civvis-20261109T010101Z", code_rev="aaa"),
                            row("civvis-20261109T020101Z", code_rev="bbb")])
        _, summaries, _ = self.run_card("--group", "code_rev")
        self.assertEqual(sorted(summaries), [
            "DIFFICULTY_DEITY GAMESPEED_ONLINE aaa",
            "DIFFICULTY_DEITY GAMESPEED_ONLINE bbb"])

    def backfillable(self, tag: str) -> None:
        run = self.control / tag
        run.mkdir()
        jsonl(run / "events.jsonl", [
            {"kind": "seat", "local_player": 0},
            {"kind": "state", "turn": 50, "public_stats": {"population": 9},
             "cities": [{"original_owner": 0}], "cities_following_religion": 0}])

    def test_backfill_fills_a_side_file_and_never_touches_the_ledger(self):
        tag = "civvis-20261109T010101Z"
        jsonl(self.ledger, [row(tag), row("civvis-20261109T020101Z")])
        self.backfillable(tag)
        before = self.ledger.read_bytes()
        _, summaries, _ = self.run_card("--backfill")
        self.assertEqual(self.ledger.read_bytes(), before)
        group = summaries["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261109"]
        self.assertEqual(group["economy"]["50"]["population"], {"median": 9, "n": 1})
        self.assertEqual(len(scorecard.read_jsonl(self.side)), 1)

    def test_backfill_reads_each_run_once(self):
        tag = "civvis-20261109T010101Z"
        jsonl(self.ledger, [row(tag)])
        self.backfillable(tag)
        self.run_card("--backfill")
        self.run_card("--backfill")
        self.assertEqual(len(scorecard.read_jsonl(self.side)), 1)

    def test_a_row_s_own_marks_beat_the_side_file(self):
        tag = "civvis-20261109T010101Z"
        jsonl(self.ledger, [row(tag, economy_marks=economy("50", 3))])
        jsonl(self.side, [{"tag": tag, "economy_marks": economy("50", 99)}])
        _, summaries, _ = self.run_card()
        group = summaries["DIFFICULTY_DEITY GAMESPEED_ONLINE 20261109"]
        self.assertEqual(group["economy"]["50"]["cities"]["median"], 3)

    def test_a_missing_ledger_is_an_error(self):
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(scorecard.main(["--ledger", str(self.ledger)]), 2)

    def test_the_text_report_renders_every_section(self):
        jsonl(self.ledger, [row(
            "civvis-20261109T010101Z", economy_marks=economy("50", 3),
            religion_marks={"religion": "R", "founded_turn": 18, "spreads": {},
                            "at": {"100": {"converted": 1, "rivals": 5,
                                           "cities_following": 6}},
                            "last": {"converted": 1, "rivals": 5,
                                     "cities_following": 6}},
            conquest_marks={"at": {}, "last": {"capitals": 2, "cities": 3}})])
        _, _, text = self.run_card()
        self.assertIn("religion: founded in 1 of 1 games, median turn 18", text)
        self.assertIn("converted 1  rivals 5  cities_following 6", text)
        self.assertIn("capitals 2  cities 3", text)


if __name__ == "__main__":
    unittest.main()
