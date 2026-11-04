"""Isolated filesystem controls for this trial's start cutoff."""
import importlib.util
import hashlib
import types
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SOURCE = Path(__file__).with_name("2026-10-26-forward-airfield-native.observer.py")
SPEC = importlib.util.spec_from_file_location("observer", SOURCE)
observer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observer)


class ObserverTests(unittest.TestCase):
    def test_exactly_forty_eight_starts_keep_continuations_and_overruns(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            snapshots = home / "civvis-civ6-runs/decider-snapshots"
            snapshots.mkdir(parents=True)
            old = "civvis-20261025T010000Z"
            (snapshots / old).mkdir()

            def arrive(_):
                (snapshots / (old + "-cont1")).mkdir()
                for index in range(1, observer.LIMIT + 2):
                    (snapshots / f"civvis-20261025T0200{index:02d}Z").mkdir()

            out = home / "evidence"
            argv = ["observer", "--repo", tmp, "--repair", "repair",
                    "--qualification-run", old, "--out", str(out)]
            with mock.patch.object(sys, "argv", argv), \
                 mock.patch.object(observer.Path, "home", return_value=home), \
                 mock.patch.object(observer, "qualify", return_value={"fixture": True}), \
                 mock.patch.object(observer, "drivers", return_value=[]), \
                 mock.patch.object(observer.time, "sleep", side_effect=arrive):
                observer.main()
            state = json.loads((out / "state.json").read_text())
            self.assertEqual(len(state["assigned"]), observer.LIMIT)
            self.assertEqual(len(state["overruns"]), 1)
            self.assertNotIn(old, state["assigned"])
            self.assertEqual(state["cutoff"]["action"], "removed_owned_screen")
            self.assertFalse((home / ".civvis-live-screen-gene").exists())

    def test_qualification_retains_baseline_and_rejects_new_or_missing_baseline(self):
        tag = "civvis-20261025T010000Z"
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = root / "control" / tag
            run.mkdir(parents=True)
            binary = root / "binary"
            binary.write_bytes(b"frozen-decider")
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
            runtime = {"status": "start", "binary_revision": "descendant",
                       "binary": str(binary), "binary_sha256": digest}
            capture = {"complete": True, "reads": [{"snapshot_id": "one"}]}
            first = {"binary_on_disk_sha256": digest, "input_capture": capture}
            (run / "runtime_updates.jsonl").write_text(json.dumps(runtime) + "\n")
            (run / "decisions.jsonl").write_text(json.dumps(first) + "\n")
            (run / "events.jsonl").write_text('{"kind":"loaded","version":2}\n')
            installed = root / "installed"
            installed.mkdir()
            (installed / "CivvisControlAgent.lua").write_text("-- observed bytes\n")
            trace = types.ModuleType("civ6_decision_trace")
            trace.captured_input = lambda actual, sid: b"captured bytes"
            ladder = types.ModuleType("civ6_ladder")
            control = types.ModuleType("civ6_control")
            control.install = types.SimpleNamespace(install_dir=lambda: installed)
            driver = {"tag": tag, "argv": ["play", "--civvis-capture-inputs"]}
            for treatments, accepted in [([observer.BASELINE_GENE], True),
                                        ([], False),
                                        ([observer.BASELINE_GENE, observer.GENE], False)]:
                ladder.decider_genome = lambda path, values=treatments: {"treatments": values}
                with mock.patch.dict(sys.modules, {"civ6_decision_trace": trace,
                        "civ6_ladder": ladder, "civ6_control": control}), \
                     mock.patch.object(observer, "drivers", return_value=[driver]), \
                     mock.patch.object(observer, "verify_forceable", return_value={"fixture": True}) as forceable, \
                     mock.patch.object(observer.subprocess, "run") as ancestry:
                    if accepted:
                        result = observer.qualify(root, root, tag, "feature-merge")
                        self.assertEqual(result["played_genome"]["treatments"], treatments)
                        forceable.assert_called_once_with(binary)
                        self.assertEqual(result["reconstructed_reads"]["one"]["bytes"], 14)
                        self.assertFalse(result["installed_mod_on_disk"]["loaded_identity_attested"])
                        self.assertEqual(result["installed_mod_on_disk"]["text"], "-- observed bytes\n")
                        self.assertEqual(ancestry.call_args.args[0][-2:], ["feature-merge", "descendant"])
                    else:
                        with self.assertRaisesRegex(ValueError, "retain the baseline watch with the candidate treatment absent"):
                            observer.qualify(root, root, tag, "feature-merge")

    def test_resume_rejects_a_different_gene_or_cutoff(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            out = home / "evidence"
            out.mkdir()
            state = {"repair": "feature", "gene": "another-gene", "limit": observer.LIMIT}
            (out / "state.json").write_text(json.dumps(state))
            argv = ["observer", "--repo", tmp, "--repair", "feature",
                    "--qualification-run", "unused", "--out", str(out)]
            with mock.patch.object(sys, "argv", argv), \
                 mock.patch.object(observer.Path, "home", return_value=home):
                with self.assertRaisesRegex(ValueError, "different source, gene or cutoff"):
                    observer.main()
            self.assertFalse((home / ".civvis-live-screen-gene").exists())

    def test_foreign_or_replaced_screen_is_never_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            screen = Path(tmp) / "screen"
            screen.write_text(observer.GENE + "\n")
            identity = observer.identity(screen)
            screen.unlink()
            screen.write_text(observer.GENE + "\n")
            self.assertEqual(observer.cut_off(screen, identity),
                             "foreign_or_changed_screen_preserved")
            screen.write_text("another-gene\n")
            self.assertEqual(observer.cut_off(screen, observer.identity(screen)),
                             "foreign_or_changed_screen_preserved")
            self.assertEqual(screen.read_text(), "another-gene\n")

    def test_only_run_tags_join_their_continuations(self):
        original = "civvis-20261025T010000Z"
        self.assertEqual(observer.stem(original + "-cont2"), original)
        self.assertIsNone(observer.stem("unrelated"))


if __name__ == "__main__":
    unittest.main()
