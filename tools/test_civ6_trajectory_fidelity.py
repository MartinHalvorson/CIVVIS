#!/usr/bin/env python3
"""Tests for the trajectory fidelity ledger.

The tool's whole value is that it refuses to pool two corpora that played
different games, so most of these are about what it declines to compare.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_trajectory_fidelity as fidelity  # noqa: E402


def live_row(difficulty="DIFFICULTY_EMPEROR", **fields):
    row = {
        "difficulty": difficulty,
        "speed": "GAMESPEED_ONLINE",
        "map_size": "MAPSIZE_SMALL",
        "turns": 200,
    }
    row.update(fields)
    return row


def write_live(rows, directory: Path) -> Path:
    path = directory / "ladder.json"
    path.write_text(json.dumps({"attempts": rows}), encoding="utf-8")
    return path


def write_sim(games, directory: Path, difficulty="emperor") -> Path:
    path = directory / "screen.jsonl"
    lines = [
        json.dumps(
            {
                "difficulty": difficulty,
                "speed": "online",
                "width": 74,
                "height": 46,
            }
        )
    ]
    for index, seats in enumerate(games):
        for seat in seats:
            lines.append(json.dumps({"kind": "game", "game": index, **seat}))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


class TheMapSizeTableIsDiscovered(unittest.TestCase):
    def test_the_shipped_sizes_are_read_out_of_setup_rs(self):
        sizes = fidelity.map_sizes()
        self.assertEqual(sizes.get((74, 46)), "small")
        self.assertGreater(len(sizes), 3, "the size table parsed to almost nothing")


class ItRefusesToPoolDifferentGames(unittest.TestCase):
    def test_a_configuration_only_one_corpus_played_is_named_not_dropped(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row("DIFFICULTY_SETTLER", score=100, rival_best=200)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp, "emperor"),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(report["matched_cells"], [], "settler is not emperor")
        self.assertEqual(
            [entry["cell"] for entry in report["live_only"]],
            [["settler", "online", "small"]],
        )
        self.assertEqual(
            [entry["cell"] for entry in report["sim_only"]],
            [["emperor", "online", "small"]],
        )

    def test_a_matched_configuration_is_compared(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=200)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(len(report["matched_cells"]), 1)
        standing = report["matched_cells"][0]["subsystems"]["standing"]
        self.assertTrue(standing["available"])
        # The live row is OUR seat against the best rival: 100 of 200.
        self.assertAlmostEqual(standing["live"], 0.5)
        # Every screen seat is a CIVVIS seat, so each one contributes its own
        # standing against the best OTHER seat: 100/200 and 200/100. The median
        # of the two is the typical seat, which is the live row's counterpart.
        self.assertAlmostEqual(standing["sim"], 1.25)
        self.assertAlmostEqual(standing["divergence"], 2.5)

    def test_a_shallow_run_is_not_a_trajectory(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            rows = [live_row(turns=fidelity.MIN_TURNS - 1, score=1, rival_best=1)]
            self.assertEqual(fidelity.live_records(write_live(rows, tmp)), [])


class AMissingFieldIsNamed(unittest.TestCase):
    def test_a_subsystem_one_side_cannot_report_says_which_side(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=200, techs_at_150=20)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        pace = report["matched_cells"][0]["subsystems"]["research_pace"]
        self.assertFalse(pace["available"])
        self.assertEqual(pace["why"], "sim", "the live side had the field")


class TheRatchetOnlyTightens(unittest.TestCase):
    def test_divergence_does_not_care_which_side_is_larger(self):
        self.assertAlmostEqual(fidelity.divergence(2.0, 1.0), 2.0)
        self.assertAlmostEqual(fidelity.divergence(1.0, 2.0), 2.0)
        self.assertAlmostEqual(fidelity.divergence(3.0, 3.0), 1.0)

    def test_a_subsystem_past_its_tolerance_fails_the_check(self):
        report = {
            "matched_cells": [
                {"subsystems": {"standing": {"available": True, "divergence": 4.0}}}
            ]
        }
        status, notes = fidelity.check(report, {"standing": 2.0}, 0)
        self.assertEqual(status, 1)
        self.assertTrue(any("exceeds" in note for note in notes))

    def test_a_subsystem_inside_its_tolerance_passes(self):
        report = {
            "matched_cells": [
                {"subsystems": {"standing": {"available": True, "divergence": 1.5}}}
            ]
        }
        status, _ = fidelity.check(report, {"standing": 2.0}, 0)
        self.assertEqual(status, 0)

    def test_a_subsystem_with_no_tolerance_yet_does_not_fail(self):
        report = {
            "matched_cells": [
                {"subsystems": {"standing": {"available": True, "divergence": 9.0}}}
            ]
        }
        status, notes = fidelity.check(report, {}, 0)
        self.assertEqual(status, 0)
        self.assertTrue(any("no tolerance recorded" in note for note in notes))


class ItSkipsLoudlyWithNoCorpus(unittest.TestCase):
    def test_check_passes_with_a_named_skip_when_the_live_corpus_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "nothing.json"
            self.assertEqual(
                fidelity.main(["--check", "--live", str(missing)]),
                0,
            )


class TheCommittedCorpusIsTheRealOne(unittest.TestCase):
    """The evidence ships with the repository, so the ratchet is a real number
    on a hosted runner and not a permanent skip."""

    def test_the_default_live_corpus_is_committed_and_deep(self):
        self.assertTrue(fidelity.LIVE_DEFAULT.exists())
        records = fidelity.live_records(fidelity.LIVE_DEFAULT)
        self.assertGreater(len(records), 100, "the committed corpus went thin")

    def test_the_default_run_finds_a_configuration_both_corpora_played(self):
        live = fidelity.live_records(fidelity.LIVE_DEFAULT)
        sim = fidelity.sim_records(fidelity.SIM_DEFAULT, fidelity.map_sizes())
        report = fidelity.ledger(live, sim)
        self.assertTrue(
            report["matched_cells"],
            "no configuration is shared, so the ratchet measures nothing",
        )


if __name__ == "__main__":
    unittest.main()
