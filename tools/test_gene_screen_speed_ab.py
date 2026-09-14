#!/usr/bin/env python3
"""Completeness, provenance and equality guards for actual tournament timing."""
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_screen_speed_ab as bench  # noqa: E402


class TournamentTimingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.record = {"commit": "a" * 40, "sha256": "b" * 64}
        self.header = {"kind": "header", "genes": ["example"], "players": 6, "start_seed": 100,
                       "batch": {"target_games": 2, "target_seats": 12, "seed_first": 100, "seed_last": 101},
                       "build": {
            "commit": self.record["commit"], "binary_sha256": self.record["sha256"],
            "genes_sha256": "c" * 64, "dirty": False}}
        self.rows = [{"kind": "game", "game": game, "seat": seat,
                      "seed": 100 + game, "turn": 10 + game, "secs": 1.0,
                      "score": seat + game, "genome": "1", "win": seat == 0}
                     for game in range(2) for seat in range(6)]

    def read(self, records):
        path = self.root / "rows.jsonl"
        path.write_text("".join(json.dumps(row) + "\n" for row in records))
        return bench.read_result(path, self.record, 100, 2)

    def test_counts_each_game_once_and_excludes_only_runtime(self):
        records = [self.header] + self.rows
        before = self.read(records)
        self.assertEqual(before["turns"], 21)
        for row in self.rows:
            row["secs"] += 50
        self.assertEqual(self.read(records), before)
        self.rows[-1]["new_outcome_field"] = {"future_schema": True}
        self.assertNotEqual(self.read(records)["outcome_digest"], before["outcome_digest"])

    def test_refuses_partial_duplicate_or_misidentified_games_and_bad_builds(self):
        original = [self.header] + self.rows
        cases = [original[:-1], original + [self.rows[-1]],
                 [self.header, self.header] + self.rows]
        for field, value in [("seat", 1), ("game", 1), ("seed", 99), ("turn", 0),
                             ("kind", "unexpected")]:
            changed = copy.deepcopy(original)
            changed[1][field] = value
            cases.append(changed)
        for field, value in [("players", 4), ("start_seed", 99), ("batch", {"target_games": 1200})]:
            changed = copy.deepcopy(original)
            changed[0][field] = value
            cases.append(changed)
        for field, value in [("commit", "d" * 40), ("binary_sha256", "e" * 64),
                             ("dirty", True)]:
            changed = copy.deepcopy(original)
            changed[0]["build"][field] = value
            cases.append(changed)
        for index, records in enumerate(cases):
            with self.subTest(case=index), self.assertRaises(ValueError):
                self.read(records)

    def test_requires_full_frozen_basename_and_hashes_the_actual_bytes(self):
        for name in ["a" * 40, "civvis-" + "a" * 39, "civvis-" + "g" * 40]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                bench.binary_record(self.root / name)
        path = self.root / ("civvis-" + "a" * 40)
        path.write_bytes(b"frozen artifact")
        record = bench.binary_record(path)
        self.assertEqual(record["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(record["commit"], "a" * 40)

    def test_weighted_cpu_and_wall_summaries_keep_the_block_as_unit(self):
        pairs = [dict(baseline_cpu=100, candidate_cpu=80, delta_pct=-20,
                      baseline_wall=20, candidate_wall=16, wall_delta_pct=-20,
                      turns=100, games_per_arm=16),
                 dict(baseline_cpu=200, candidate_cpu=180, delta_pct=-10,
                      baseline_wall=40, candidate_wall=38, wall_delta_pct=-5,
                      turns=200, games_per_arm=16)]
        result = bench.summarize(pairs)
        self.assertEqual((result["pairs"], result["total_games_per_arm"], result["turns_per_arm"]),
                         (2, 32, 300))
        self.assertEqual(result["median_pct"], -15)
        self.assertAlmostEqual(result["pooled_pct"], -100 * 40 / 300)
        self.assertEqual(result["median_wall_pct"], -12.5)
        self.assertAlmostEqual(result["pooled_wall_pct"], -10)

    def invoke(self, *, mismatch=None):
        binaries = [self.root / ("civvis-" + letter * 40) for letter in "ab"]
        for path in binaries:
            path.write_bytes(path.name.encode())
        output = self.root / ("results-" + (mismatch or "success"))
        calls = []

        def arm(record, seed, label, directory, games, jobs):
            candidate = record["commit"] == "b" * 40
            calls.append((seed, label, candidate, games, jobs))
            result = dict(cpu=1 if candidate else 2, wall=1 if candidate else 2,
                          turns=21, header={"start_seed": seed}, genes_sha256="same",
                          outcome_digest="same")
            if candidate and mismatch:
                result[mismatch] = "changed"
            return result

        argv = ["tool", "--baseline", str(binaries[0]), "--candidate", str(binaries[1]),
                "--out-dir", str(output), "--seed", "100", "--pairs", "2",
                "--controls", "1", "--games-per-arm", "2", "--jobs", "2"]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(bench, "run_arm", arm), \
                contextlib.redirect_stdout(io.StringIO()):
            if mismatch:
                with self.assertRaises(ValueError):
                    bench.main()
            else:
                bench.main()
        return calls, json.loads((output / "manifest.json").read_text()), output

    def test_interleaves_blocks_with_disjoint_seeds_and_a_same_binary_control(self):
        calls, manifest, _ = self.invoke()
        self.assertEqual(calls, [(98, "control-baseline", False, 2, 2),
                                (98, "control-candidate", False, 2, 2),
                                (100, "candidate-baseline", False, 2, 2),
                                (100, "candidate-candidate", True, 2, 2),
                                (102, "candidate-candidate", True, 2, 2),
                                (102, "candidate-baseline", False, 2, 2)])
        self.assertEqual(manifest["status"], "complete")
        self.assertEqual(manifest["seeds"], [100, 103])
        self.assertEqual(manifest["control_seeds"], [98, 99])
        self.assertEqual(manifest["summary"]["control"]["median_pct"], 0)

    def test_mismatched_arms_cannot_be_published_as_a_completed_timing(self):
        for field in ["outcome_digest", "header", "genes_sha256", "turns"]:
            with self.subTest(field=field):
                _, manifest, output = self.invoke(mismatch=field)
                self.assertEqual(manifest["status"], "failed")
                self.assertNotIn("summary", manifest)
                pairs = [json.loads(line) for line in (output / "pairs.jsonl").read_text().splitlines()]
                self.assertEqual([pair["phase"] for pair in pairs], ["control"])

    def test_rejects_invalid_counts_or_negative_control_seeds_before_execution(self):
        base = ["tool", "--baseline", "unused", "--candidate", "unused", "--out-dir", "unused"]
        for option, value in [("--pairs", "0"), ("--controls", "0"), ("--jobs", "-1"),
                              ("--games-per-arm", "0"), ("--seed", "0")]:
            with self.subTest(option=option), mock.patch.object(sys, "argv", base + [option, value]), \
                    contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                bench.main()
            self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
