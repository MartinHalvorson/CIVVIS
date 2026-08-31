#!/usr/bin/env python3
"""Tests for tools/live_divergence.py: the comparison arithmetic and the report.

A synthetic two-frame fixture — the JSON the Rust bin emits for a run whose
frames at t=10 and t=11 pair two cities — pins MAE, median, worst-turn
selection and coverage, then the generated report, the scoreboard ordering and
the threshold seeding / ``--check`` rule.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path

import live_divergence as ld


def two_frame_report() -> dict:
    """What the bin prints for three frames (t=10, 11, 12): two comparable turns.

    The second comparable turn (11 -> 12) has no city pairs at all, so the city
    subsystems cover 1 of 2 turns while the empire deltas cover both.
    """
    return {
        "run": "synthetic-run",
        "events": "/nowhere/events.jsonl",
        "mode": "projection",
        "frames": 3,
        "frame_turns": [10, 11, 12],
        "comparable_turns": 2,
        "compared_turns": 2,
        "skipped": [],
        "subsystems": {
            "city_science": {
                "unit": "yield/turn",
                "note": "per-city science",
                "pairs": [
                    {"turn": 11, "key": "Rome", "live": 6.5, "sim": 5.0},
                    {"turn": 11, "key": "Antium", "live": 1.5, "sim": 1.5},
                ],
            },
            "empire_gold_delta": {
                "unit": "gold",
                "note": "treasury delta",
                "pairs": [
                    {"turn": 11, "key": "gold", "live": 9.0, "sim": 11.0},
                    {"turn": 12, "key": "gold", "live": 10.0, "sim": 10.5},
                ],
            },
            "combat_damage": {"unit": "hp", "note": "needs the ledger", "pairs": []},
        },
    }


class Arithmetic(unittest.TestCase):
    def test_two_frame_fixture_mae_median_worst_coverage(self):
        summary = ld.summarize(two_frame_report(), processed_at="2026-08-24T00:00:00Z")
        science = summary["subsystems"]["city_science"]
        self.assertEqual(science["n"], 2)
        self.assertAlmostEqual(science["mae"], 0.75)          # (1.5 + 0.0) / 2
        self.assertAlmostEqual(science["median"], 0.75)
        self.assertEqual(science["turns"], 1)
        self.assertAlmostEqual(science["coverage"], 0.5)      # 1 of 2 comparable turns
        self.assertEqual(science["worst"][0]["key"], "Rome")
        self.assertAlmostEqual(science["worst"][0]["abs"], 1.5)
        self.assertEqual(len(science["worst"]), 1, "one worst entry per turn, not per pair")

        gold = summary["subsystems"]["empire_gold_delta"]
        self.assertAlmostEqual(gold["mae"], 1.25)             # (2.0 + 0.5) / 2
        self.assertAlmostEqual(gold["median"], 1.25)
        self.assertAlmostEqual(gold["coverage"], 1.0)
        self.assertEqual([w["turn"] for w in gold["worst"]], [11, 12], "worst first")

        combat = summary["subsystems"]["combat_damage"]
        self.assertEqual(combat["n"], 0)
        self.assertIsNone(combat["mae"], "no pairs is no MAE, never zero")
        self.assertEqual(combat["coverage"], 0.0)
        self.assertEqual((summary["first_turn"], summary["last_turn"]), (10, 12))

    def test_worst_keeps_five_turns(self):
        report = two_frame_report()
        report["comparable_turns"] = 8
        report["subsystems"]["city_science"]["pairs"] = [
            {"turn": t, "key": f"c{t}", "live": float(t), "sim": 0.0} for t in range(20, 28)
        ]
        science = ld.summarize(report)["subsystems"]["city_science"]
        self.assertEqual([w["turn"] for w in science["worst"]], [27, 26, 25, 24, 23])
        self.assertAlmostEqual(science["median"], 23.5)


class Report(unittest.TestCase):
    def test_run_report_format(self):
        summary = ld.summarize(two_frame_report(), processed_at="2026-08-24T00:00:00Z", source_commit="abcdef0123456789")
        text = ld.render_run_report(summary, {"city_science": {"mae": 0.5}})
        self.assertTrue(text.startswith(ld.GENERATED), "a generated file says so on line 1")
        self.assertIn("# Divergence replay: `synthetic-run`", text)
        self.assertIn("**Mode: projection.**", text)
        self.assertIn("| city_science | yield/turn | 2 | 1/2 | 50% | 0.750 | 0.750 | 0.500 BREACH |", text)
        self.assertIn("| empire_gold_delta | gold | 2 | 2/2 | 100% | 1.250 | 1.250 | - |", text)
        self.assertIn("| combat_damage | hp | 0 | 0/2 | 0% | - | - | - |", text)
        self.assertIn("| 11 | Rome | 6.500 | 5.000 | 1.500 |", text)
        self.assertIn("No pair could be formed.", text)

    def test_scoreboard_newest_first_and_replaces_reprocessed_run(self):
        old = ld.summarize(two_frame_report(), processed_at="2026-08-20T00:00:00Z")
        newer = ld.summarize(two_frame_report(), processed_at="2026-08-24T00:00:00Z")
        newer["run"] = "other-run"
        entries = ld.upsert_scoreboard([], old)
        entries = ld.upsert_scoreboard(entries, newer)
        reprocessed = ld.summarize(two_frame_report(), processed_at="2026-08-25T00:00:00Z")
        entries = ld.upsert_scoreboard(entries, reprocessed)
        self.assertEqual([e["run"] for e in entries], ["synthetic-run", "other-run"])
        text = ld.render_scoreboard(entries, {"city_science": {"mae": 1.0}})
        rows = [line for line in text.splitlines() if line.startswith("| [")]
        self.assertEqual(len(rows), 6, "one row per run per subsystem")
        self.assertTrue(rows[0].startswith("| [synthetic-run](synthetic-run.md) | 2026-08-25 | city_science |"))
        self.assertIn("| OK |", rows[0])
        self.assertIn("| unwaived |", rows[1])
        self.assertIn("| no pairs |", rows[2])


class FidelityDoc(unittest.TestCase):
    def test_section_is_inserted_once_and_replaced_in_place(self):
        entries = ld.upsert_scoreboard([], ld.summarize(two_frame_report()))
        section = ld.render_fidelity_section(entries, {"city_science": {"mae": 1.0}})
        self.assertIn("## Measured divergence", section)
        self.assertIn("| city_science | [synthetic-run](fidelity/synthetic-run.md) | 2 | 50% | 0.750 | 0.750 | 1.000 |", section)
        self.assertIn("`combat_damage`", section, "unmeasured subsystems are named, not dropped")
        doc = "# Fidelity\n\nbody\n"
        once = ld.update_fidelity_doc(doc, section)
        self.assertTrue(once.startswith("# Fidelity\n\nbody\n\n" + ld.DOC_BEGIN))
        twice = ld.update_fidelity_doc(once + "\n## After\n", section.replace("0.750", "0.100"))
        self.assertEqual(twice.count(ld.DOC_BEGIN), 1)
        self.assertIn("0.100", twice)
        self.assertNotIn("0.750", twice)
        self.assertTrue(twice.endswith("## After\n"), "text after the block survives")


class Waivers(unittest.TestCase):
    def _summary(self, run: str, mae: float) -> dict:
        report = two_frame_report()
        report["run"] = run
        report["subsystems"]["city_science"]["pairs"] = [
            {"turn": 11, "key": "Rome", "live": mae, "sim": 0.0},
        ]
        return ld.summarize(report)

    def test_threshold_is_max_of_first_three_then_frozen(self):
        waivers = {"thresholds": {}}
        for run, mae in (("a", 0.4), ("b", 0.9), ("c", 0.6)):
            ld.seed_thresholds(waivers, self._summary(run, mae))
        entry = waivers["thresholds"]["city_science"]
        self.assertAlmostEqual(entry["mae"], 0.9)
        self.assertTrue(entry["frozen"])
        self.assertEqual([s["run"] for s in entry["seeds"]], ["a", "b", "c"])
        ld.seed_thresholds(waivers, self._summary("d", 5.0))
        self.assertAlmostEqual(entry["mae"], 0.9, "a fourth run does not move a frozen threshold")
        self.assertNotIn("combat_damage", waivers["thresholds"], "no pairs seeds nothing")

    def test_check_fails_only_above_threshold(self):
        waivers = {"thresholds": {"city_science": {"mae": 0.9}}}
        self.assertEqual(ld.breaches(self._summary("d", 0.9), waivers), [])
        failed = ld.breaches(self._summary("e", 1.2), waivers)
        self.assertEqual(failed, ["e: city_science MAE 1.200 > waived 0.900"])

    def test_scoreboard_check_reads_newest_row_per_subsystem(self):
        entries = ld.upsert_scoreboard([], self._summary("old", 3.0))
        entries[0]["processed_at"] = "2026-08-01T00:00:00Z"
        entries = ld.upsert_scoreboard(entries, self._summary("new", 0.5))
        pseudo = ld.newest_per_subsystem(entries)
        self.assertEqual(pseudo["runs"]["city_science"], "new")
        self.assertEqual(ld.breaches(pseudo, {"thresholds": {"city_science": {"mae": 0.9}}}), [])


class BinaryLookup(unittest.TestCase):
    """`--bin`, then the env var, then the cargo dirs, then the published runtimes.

    Until 2026-08-26 only `target/release/live_divergence` was consulted, no
    host had built it, and every scoreboard row read "no pairs" for the ledger
    subsystems. The published runtime layout is a directory per sha holding
    the bins, so a directory names the bin inside it."""

    def test_explicit_then_env_then_cargo_then_published(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            published = root / "published"
            older = published / "aaaa" / "live_divergence"
            newer = published / "bbbb" / "live_divergence"
            for path in (older, newer):
                path.parent.mkdir(parents=True)
                path.write_text("")
            import os
            os.utime(older, (1, 1))
            os.utime(newer, (2, 2))
            env_dir = root / "env-runtime"
            env_dir.mkdir()
            (env_dir / "live_divergence").write_text("")
            env = {ld.BIN_ENV: str(env_dir), "CARGO_TARGET_DIR": str(root / "cargo")}
            found = ld.binary_candidates("/x/explicit", env=env, home_published=published)
            self.assertEqual(found[0], Path("/x/explicit"))
            self.assertEqual(found[1], env_dir / "live_divergence",
                             "a directory in the env var names the bin inside it")
            self.assertEqual(found[2], root / "cargo" / "release" / "live_divergence")
            self.assertEqual(found[3], ld.REPO / "target" / "release" / "live_divergence")
            self.assertEqual(found[4:], [newer, older], "newest published runtime first")

    def test_nothing_named_means_only_the_cargo_dirs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            found = ld.binary_candidates(None, env={}, home_published=Path(tmp) / "absent")
            self.assertEqual(found, [ld.REPO / "target" / "release" / "live_divergence"])

    def test_fallback_build_enables_developer_tools(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "target" / "release" / "live_divergence"

            def build(command, **_kwargs):
                self.assertEqual(
                    command,
                    [
                        "cargo",
                        "build",
                        "--release",
                        "--features",
                        "developer-tools",
                        "--bin",
                        "live_divergence",
                    ],
                )
                target.parent.mkdir(parents=True)
                target.write_text("", encoding="utf-8")

            with mock.patch.object(ld, "binary_candidates", return_value=[target]), \
                    mock.patch.object(ld.subprocess, "run", side_effect=build) as run:
                self.assertEqual(ld.find_binary(None), target)
            run.assert_called_once()


class CheckCommand(unittest.TestCase):
    def test_check_exit_codes_from_scoreboard_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            fid = Path(tmp)
            entries = ld.upsert_scoreboard([], ld.summarize(two_frame_report()))
            (fid / "scoreboard.json").write_text(json.dumps({"runs": entries}), encoding="utf-8")
            (fid / "waivers.json").write_text(json.dumps({"thresholds": {"city_science": {"mae": 1.0}}}), encoding="utf-8")
            self.assertEqual(ld.main(["--check", "--fidelity-dir", tmp]), 0)
            (fid / "waivers.json").write_text(json.dumps({"thresholds": {"city_science": {"mae": 0.5}}}), encoding="utf-8")
            self.assertEqual(ld.main(["--check", "--fidelity-dir", tmp]), 1)


if __name__ == "__main__":
    unittest.main()
