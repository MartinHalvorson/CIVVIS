#!/usr/bin/env python3
"""Compiler observation guards use synthetic games; the real WASM fixture returns 7."""
from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import site_wasm_compiler_ci as compiler
from test_site_wasm_pair import SOURCE, fixture
from test_site_wasm_profile_ci import RAW_SAVE, RUN, REPO, game_record, run_record, write_game

TINY = bytes([0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 127,
              3, 2, 1, 0, 7, 5, 1, 1, 102, 0, 0, 10, 6, 1, 4, 0, 65, 7, 11])
BASELINE = "Compiled function 0x123#0 using Liftoff, took 0 ms and 22568 bytes; bodysize 4 codesize 116"
OPTIMIZED = ("Compiled function 0x123#0 using TurboFan, took 6 ms and 49320 / 86360 "
             "max/total bytes; bodysize 4 codesize 48 name wasm-function#0")


def qualified_fixture(root: Path) -> dict:
    receipt = fixture(root)
    (root / "qualification/civvis.wasm").write_bytes(TINY)
    receipt["engine"].update(wasm_bytes=len(TINY), wasm_sha256=hashlib.sha256(TINY).hexdigest())
    (root / "wasm-symbols/manifest.json").write_text(json.dumps(receipt["engine"]))
    (root / "qualification-info.json").write_text(json.dumps(receipt))
    return receipt


class CompilerRecords(unittest.TestCase):
    def test_exact_function_body_lengths_are_read_from_module(self):
        self.assertEqual(compiler.function_sizes(TINY), {0: 4})

    def test_invalid_truncated_missing_and_importing_modules_are_rejected(self):
        imported = TINY[:8] + bytes([2, 1, 1]) + TINY[8:]
        for data in [b"wrong", TINY[:-1], TINY[:8], imported,
                     TINY[:8] + bytes([1, 128, 128, 128, 128, 128])]:
            with self.subTest(data=data), self.assertRaises(ValueError):
                compiler.function_sizes(data)

    def events(self, text=BASELINE + "\n" + OPTIMIZED):
        return compiler.compilation_events(text, {0: "example"}, {0: 4})

    def test_baseline_and_optimized_counter_formats_keep_their_meanings(self):
        first, second = self.events()
        self.assertEqual(first["compiler_single_reported_bytes"], 22568)
        self.assertNotIn("compiler_max_bytes", first)
        self.assertEqual(second["compiler_max_bytes"], 49320)
        self.assertEqual(second["compiler_total_bytes"], 86360)
        self.assertEqual(second["generated_code_bytes"], 48)
        self.assertEqual(second["trace_name"], "wasm-function#0")

    def test_float_duration_optional_name_and_turboshaft_label(self):
        text = OPTIMIZED.replace("6 ms", "6.5 ms").replace("TurboFan", "Turboshaft")
        text = text[:text.index(" name ")]
        events = self.events(BASELINE + "\n" + text)
        self.assertEqual(events[-1]["reported_elapsed_ms"], 6.5)
        self.assertIsNone(events[-1]["trace_name"])

    def test_missing_unknown_mismatched_or_multiple_module_records_fail_closed(self):
        variants = ["", "some other output", BASELINE.replace("#0", "#2"),
                    BASELINE.replace("bodysize 4", "bodysize 5"),
                    BASELINE.replace("0 ms", "nan ms"),
                    BASELINE.replace("codesize 116", "codesize 0"),
                    BASELINE.replace("22568 bytes", "unknown bytes"),
                    BASELINE + "\n" + OPTIMIZED.replace("0x123", "0x456"),
                    BASELINE + "\n" + OPTIMIZED.replace("49320 / 86360", "90000 / 86360")]
        for text in variants:
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.events(text)
        with self.assertRaisesRegex(ValueError, "exact module"):
            compiler.compilation_events(BASELINE, {0: "example", 1: "extra"}, {0: 4})

    def test_every_recompilation_is_retained_and_reported_durations_are_not_wall_time(self):
        events = self.events(BASELINE + "\n" + OPTIMIZED + "\n" + OPTIMIZED)
        report = compiler.summarize(events)
        self.assertEqual(report["event_count"], 3)
        self.assertEqual(report["tier_totals"]["TurboFan"]["reported_elapsed_ms_sum"], 12)
        self.assertEqual(report["functions_by_reported_time"][0]["events"], 3)
        self.assertIn("not CPU shares", report["scope"])

    def test_completion_record_can_precede_late_compiler_output(self):
        row = game_record("a" * 64)
        self.assertEqual(compiler.final_game(json.dumps(row) + "\n" + OPTIMIZED), row)
        for text in ["{}", json.dumps(row) + "\n" + json.dumps(row), "{broken"]:
            with self.subTest(text=text), self.assertRaises((ValueError, json.JSONDecodeError)):
                compiler.final_game(text)

    @unittest.skipUnless(shutil.which("node"), "Node is required for the tiny compiler fixture")
    def test_real_node_formats_on_a_tiny_constant_module_without_any_game(self):
        script = ("const b=Uint8Array.from(" + json.dumps(list(TINY)) + ");"
                  "const m=await WebAssembly.instantiate(b);"
                  "for(let n=0;n<100;n++)if(m.instance.exports.f()!==7)throw Error('fixture');"
                  "await new Promise(r=>setTimeout(r,200));")
        result = subprocess.run(["node", compiler.TRACE_FLAG, "--wasm-tiering-budget=1",
                                 "--input-type=module", "-e", script], check=True,
                                text=True, capture_output=True, timeout=10)
        events = self.events(result.stdout + result.stderr)
        self.assertTrue(any(row["tier"] == "Liftoff" for row in events))


class CompleteObservations(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.qualified = self.root / "qualified"
        self.receipt = qualified_fixture(self.qualified)
        self.out = self.root / "out"
        self.calls = []
        self.mutate = None
        self.runtime = compiler.guard.NODE_VERSION
        self.provenance = run_record()

    def fake_run(self, args, **kwargs):
        self.calls.append(args)
        if args[0] == "gh":
            return subprocess.CompletedProcess(args, 0, stdout=json.dumps(self.provenance))
        if args[1] == "-p":
            return subprocess.CompletedProcess(args, 0, stdout=json.dumps({"node": self.runtime}))
        if args[1] == "--input-type=module":
            return subprocess.CompletedProcess(args, 0, stdout="")
        traced = compiler.TRACE_FLAG in args
        save = Path(args[-1])
        write_game(save)
        row = game_record(self.receipt["engine"]["wasm_sha256"])
        log = BASELINE + "\n" + OPTIMIZED + "\n" if traced else ""
        if self.mutate:
            row, log = self.mutate(traced, row, log, save)
        kwargs["stdout"].write(log + json.dumps(row) + "\n")
        return subprocess.CompletedProcess(args, 0)

    def observe(self, **overrides):
        values = dict(qualified=self.qualified, source=SOURCE, out=self.out,
                      case="six-7311003", run_id=RUN, repo=REPO, run=self.fake_run)
        values.update(overrides)
        return compiler.observe(**values)

    def test_two_normal_games_differ_only_by_logging_flag_and_preserve_all_evidence(self):
        result = self.observe()
        self.assertEqual(result["phase"], "complete")
        self.assertTrue(result["original_save_and_entire_trace_bytes_identical"])
        control, traced = result["commands"]["control"], result["commands"]["traced"]
        self.assertEqual(control[1:-1], traced[2:-1])
        self.assertEqual(traced[1], compiler.TRACE_FLAG)
        self.assertEqual(control[0], traced[0])
        self.assertNotIn("--cpu-prof", " ".join(traced))
        self.assertNotIn("tiering-budget", " ".join(traced))
        self.assertEqual(len([args for args in self.calls if str(compiler.pair.DRIVER.resolve()) in args]), 2)
        self.assertEqual((self.out / "control.save.json").read_bytes(), RAW_SAVE)
        self.assertEqual((self.out / "traced.save.json").read_bytes(), RAW_SAVE)
        self.assertEqual(result["qualification"], self.receipt)
        self.assertTrue((self.out / "compilation-events.json").exists())

    def test_invalid_case_repo_or_run_cannot_start_a_process(self):
        for override in [dict(case="tiny-7"), dict(repo="bad;repo"), dict(run_id=0)]:
            with self.subTest(override=override), self.assertRaises(ValueError):
                self.observe(**override)
        self.assertEqual(self.calls, [])

    def test_existing_evidence_is_not_overwritten(self):
        self.out.mkdir()
        marker = self.out / "keep"
        marker.write_text("original")
        with self.assertRaises(FileExistsError):
            self.observe()
        self.assertEqual(marker.read_text(), "original")
        self.assertEqual(self.calls, [])

    def test_wrong_qualification_source_or_runtime_prevents_games(self):
        with self.assertRaises(ValueError):
            self.observe(source="c" * 40)
        self.provenance["conclusion"] = "failure"
        with self.assertRaises(ValueError):
            self.observe()
        self.provenance = run_record()
        self.runtime = "wrong"
        with self.assertRaises(ValueError):
            self.observe()
        self.assertFalse(self.out.exists())

    def failure(self, mutate):
        self.mutate = mutate
        with self.assertRaises(ValueError):
            self.observe()
        result = json.loads((self.out / "receipt.json").read_text())
        self.assertEqual(result["phase"], "failed; original partial evidence retained")
        self.assertTrue((self.out / "traced.log").exists())

    def test_missing_compilation_log_retains_original_games_and_fails(self):
        self.failure(lambda traced, row, log, save: (row, ""))
        self.assertTrue((self.out / "control.save.json").exists())

    def test_trace_binding_failure_does_not_claim_a_complete_diagnostic(self):
        self.failure(lambda traced, row, log, save: (row, log.replace("bodysize 4", "bodysize 5")))

    def test_recomputed_hash_cannot_hide_a_changed_saved_game(self):
        def mutate(traced, row, log, save):
            if traced:
                save.write_bytes(RAW_SAVE.replace(b"18446744073709551615", b"18446744073709552000"))
                row["saveHash"] = hashlib.sha256(save.read_bytes()).hexdigest()
            return row, log
        self.failure(mutate)

    def test_unfinished_trace_game_is_rejected_and_preserved(self):
        def mutate(traced, row, log, save):
            if traced:
                row["finished"] = False
            return row, log
        self.failure(mutate)

    def test_trace_byte_changes_with_identical_frame_hashes_are_rejected(self):
        def mutate(traced, row, log, save):
            if traced:
                path = save.with_suffix(save.suffix + ".trace.jsonl")
                path.write_text("\n".join(line + " " for line in path.read_text().splitlines()) + "\n")
            return row, log
        self.failure(mutate)

    def test_workflow_selects_only_the_compiler_game_pair_and_preserves_partial_output(self):
        text = (Path(__file__).resolve().parents[1] / ".github/workflows/wasm-profile.yml").read_text()
        steps = re.split(r"\n      - ", text)[1:]
        selected = [step for step in steps if "tools/site_wasm_compiler_ci.py " in step]
        self.assertEqual(len(selected), 1)
        self.assertIn("if: ${{ inputs.mode == 'compiler' }}", selected[0])
        for step in steps:
            if "tools/site_wasm_profile_ci.py " in step or "name: civvis-wasm-cpu-profile" in step:
                self.assertIn("inputs.mode != 'compiler'", step)
        upload = next(step for step in steps if "name: civvis-wasm-compiler-trace" in step)
        self.assertIn("always() && inputs.mode == 'compiler'", upload)
        self.assertIn("test_site_wasm_compiler_ci.py -v", text)


if __name__ == "__main__":
    unittest.main()
