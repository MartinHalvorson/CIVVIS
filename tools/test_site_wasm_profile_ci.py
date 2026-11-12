#!/usr/bin/env python3
"""Profile orchestration guards use synthetic records and never execute a game.

The only real Node subprocess imports the driver's pure settings validator;
the WASM profile process and GitHub API are fakes.
"""
from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import site_wasm_profile_ci as profile
from test_site_wasm_pair import SOURCE, fixture

REPO = "owner/repo"
RUN = 12345
RAW_SAVE = b'{"exact_u64":18446744073709551615}'


def run_record() -> dict:
    return {"id": RUN, "status": "completed", "conclusion": "success",
            "path": ".github/workflows/wasm-qualify.yml", "head_sha": "b" * 40,
            "repository": {"full_name": REPO}, "html_url": "https://example.test/run"}


def frames() -> list[dict]:
    return [{"turn": i, "sequence": i, "finished": i == 2, "hash": str(i) * 64}
            for i in range(3)]


def samples() -> dict:
    return {"nodes": [{"id": 1, "callFrame": {"functionName": "wasm-function[0]"}}],
            "samples": [1, 1], "timeDeltas": [500, 501]}


def game_record(module_sha: str) -> dict:
    return {"harnessVersion": 7, "seed": 7311003, "roster": "six", "finished": True,
            "moduleSha256": module_sha, "frames": 2, "turn": 2,
            "cpuMs": 1000.0, "wallMs": 1100.0, "winner": 0, "victoryType": "science",
            "saveHash": hashlib.sha256(RAW_SAVE).hexdigest(),
            "traceHash": hashlib.sha256("".join(f["hash"] + "\n" for f in frames()).encode()).hexdigest(),
            "settings": {"majors": 6, "cityStates": 9, "speed": "online", "turns": 250,
                         "map": "continents", "shape": "planet", "width": 90, "height": 38,
                         "difficulty": "prince", "turnStructure": "sequential",
                         "victoryConditions": {k: True for k in
                                               ["culture", "diplomatic", "domination",
                                                "religious", "science", "score"]}}}


def write_game(save: Path, evidence: list[dict] | None = None) -> None:
    save.write_bytes(RAW_SAVE)
    save.with_suffix(save.suffix + ".trace.jsonl").write_text(
        "\n".join(json.dumps(f) for f in (frames() if evidence is None else evidence)) + "\n")


class AProfileRequiresQualifiedProvenance(unittest.TestCase):
    def test_successful_normal_publisher_is_accepted(self):
        profile.check_run(run_record(), RUN, REPO, {"publisher_commit": "b" * 40})

    def test_wrong_run_workflow_publisher_or_completion_is_rejected(self):
        for key, value in [("id", RUN + 1), ("status", "in_progress"),
                           ("conclusion", "failure"), ("path", "other.yml"),
                           ("head_sha", "c" * 40), ("repository", {"full_name": "other/repo"})]:
            with self.subTest(key=key):
                record = run_record()
                record[key] = value
                with self.assertRaises(ValueError):
                    profile.check_run(record, RUN, REPO, {"publisher_commit": "b" * 40})

    def test_available_cases_are_the_normal_full_comparison_cases(self):
        self.assertEqual(set(profile.CASES.values()), set(profile.pair.SEEDS))


class AProfileNeedsCompleteGameEvidence(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.save = Path(self.temp.name) / "game.save.json"
        write_game(self.save)
        self.row = game_record("a" * 64)

    def check(self):
        profile.check_game(self.row, self.save, "six", 7311003, "a" * 64)

    def test_original_save_bytes_keep_unsigned_64_bit_values_exact(self):
        self.check()
        self.save.write_bytes(RAW_SAVE.replace(b"18446744073709551615", b"18446744073709552000"))
        with self.assertRaises(ValueError):
            self.check()

    def test_wrong_module_case_or_unfinished_game_is_rejected(self):
        for key, value in [("moduleSha256", "b" * 64), ("seed", 7),
                           ("roster", "default"), ("finished", False), ("frames", 0)]:
            with self.subTest(key=key):
                old = self.row[key]
                self.row[key] = value
                with self.assertRaises(ValueError):
                    self.check()
                self.row[key] = old

    def test_missing_changed_or_unpublished_frame_is_rejected(self):
        variants = []
        variants.append(frames()[1:])
        changed = frames()
        changed[1]["hash"] = "f" * 64
        variants.append(changed)
        changed = frames()
        changed[-1]["finished"] = False
        variants.append(changed)
        changed = frames()
        changed[1]["sequence"] = 7
        variants.append(changed)
        for evidence in variants:
            with self.subTest(evidence=evidence):
                write_game(self.save, evidence)
                with self.assertRaises(ValueError):
                    self.check()

    def test_cpu_samples_must_have_matching_clocks_nodes_and_wasm_frames(self):
        self.assertEqual(profile.check_samples(samples()), 2)
        for key, value in [("samples", []), ("timeDeltas", [500]),
                           ("timeDeltas", [500, float("nan")]),
                           ("timeDeltas", [-1, 1]), ("samples", [99, 1]), ("nodes", [])]:
            with self.subTest(key=key, value=value):
                broken = samples()
                broken[key] = value
                with self.assertRaises(ValueError):
                    profile.check_samples(broken)


class FakeRunner:
    def __init__(self, receipt: dict):
        self.receipt = receipt
        self.calls = []
        self.row = game_record(receipt["engine"]["wasm_sha256"])
        self.profile = samples()
        self.runtime = profile.NODE_VERSION
        self.failed_game = False

    def __call__(self, command, **kwargs):
        self.calls.append(command)
        if command[:2] == ["gh", "api"]:
            return subprocess.CompletedProcess(command, 0, json.dumps(run_record()))
        if "-p" in command:
            return subprocess.CompletedProcess(command, 0, json.dumps({"node": self.runtime}))
        if "--cpu-prof" in command:
            if self.failed_game:
                raise subprocess.CalledProcessError(1, command)
            save = Path(command[-1])
            write_game(save)
            (save.parent / "engine.cpuprofile").write_text(json.dumps(self.profile))
            kwargs["stdout"].write(json.dumps(self.row) + "\n")
            return subprocess.CompletedProcess(command, 0)
        if "--input-type=module" in command:
            # The real driver is imported solely for its pure validator.
            return subprocess.run(command, timeout=10, **kwargs)
        raise AssertionError(command)


@unittest.skipUnless(shutil.which("node"), "Node is needed for the pure settings-check import")
class TheWorkflowProducesASelfContainedDiagnostic(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.input = self.root / "input"
        self.receipt = fixture(self.input)
        self.out = self.root / "output"
        self.runner = FakeRunner(self.receipt)

    def run_profile(self, **overrides):
        options = {"qualified": self.input, "source": SOURCE, "out": self.out,
                   "case": "six-7311003", "run_id": RUN, "repo": REPO, "run": self.runner}
        options.update(overrides)
        return profile.profile_game(**options)

    def test_module_map_save_frames_and_samples_are_bound_without_running_a_game(self):
        result = self.run_profile()
        self.assertEqual(result["phase"], "complete")
        self.assertEqual(result["samples"], 2)
        profile.pair.qualified(self.out, SOURCE)
        self.assertEqual((self.out / "game.save.json").read_bytes(), RAW_SAVE)
        self.assertTrue((self.out / "report.txt").is_file())
        calls = [c for c in self.runner.calls if "--cpu-prof" in c]
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][-3:-1], ["7311003", "six"])
        self.assertIn("not a speed comparison", result["scope"])

    def test_an_existing_output_is_never_overwritten_or_replayed(self):
        self.out.mkdir()
        with self.assertRaises(FileExistsError):
            self.run_profile()
        self.assertEqual(self.runner.calls, [])

    def test_bad_source_or_runtime_never_starts_a_profile_process(self):
        for options in [{"source": "c" * 40}, {"case": "six-7"}, {"run_id": 0}]:
            with self.subTest(options=options):
                with self.assertRaises(ValueError):
                    self.run_profile(**options)
        self.runner.runtime = "22.0.0"
        with self.assertRaises(ValueError):
            self.run_profile()
        self.assertFalse(any("--cpu-prof" in c for c in self.runner.calls))
        self.assertFalse(self.out.exists())

    def test_real_driver_validator_rejects_changed_normal_settings(self):
        self.runner.row["settings"]["cityStates"] = 0
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_profile()
        receipt = json.loads((self.out / "receipt.json").read_text())
        self.assertTrue(receipt["phase"].startswith("failed"))
        self.assertTrue((self.out / "game.save.json").exists())

    def test_failed_game_or_truncated_sampling_retains_partial_evidence(self):
        for failed in [True, False]:
            with self.subTest(failed_game=failed):
                self.runner.failed_game = failed
                self.runner.profile = {"nodes": [], "samples": [], "timeDeltas": []}
                self.out = self.root / str(failed)
                with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                    self.run_profile()
                receipt = json.loads((self.out / "receipt.json").read_text())
                self.assertTrue(receipt["phase"].startswith("failed"))
                self.assertTrue((self.out / "game.log").exists())


if __name__ == "__main__":
    unittest.main()
