"""Terminal video HUD uses confirmed results without changing historical frames."""
import importlib.util
import json
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock


class RecordingVictoryTest(unittest.TestCase):
    def test_missing_final_state_gets_the_confirmed_winning_minimum(self):
        with tempfile.TemporaryDirectory() as root:
            control = Path(root)
            run = control / "game"
            run.mkdir()
            frame = dict(kind="state", turn=172, local_player=0, dvp=16, cities=[], rivals=[])
            outcome = dict(kind="victory", turn=173, local_player=0, won=True,
                           victory_type="VICTORY_DIPLOMATIC")
            (run / "events.jsonl").write_text(json.dumps(frame) + "\n" + json.dumps(outcome) + "\n")
            keeper = types.ModuleType("civvis_clip_keeper")
            keeper.CONTROL = control
            spec = importlib.util.spec_from_file_location("recording_story", Path(__file__).parent / "recording/civvis_video_story.py")
            module = importlib.util.module_from_spec(spec)
            with mock.patch.dict(sys.modules, civvis_clip_keeper=keeper):
                spec.loader.exec_module(module)
                story = module.build("game", "Diplomatic", "Greece", {"172": 0, "173": 5}, 173, 12)
            self.assertIn("16 / 20", story["items"][0]["race"])
            final = story["items"][-1]
            self.assertTrue(final["confirmed_result"])
            self.assertIn("≥20 / 20", final["race"])
            self.assertEqual(final["progress"], 1)
            self.assertTrue(any("at least 20" in line for line in story["story"]))
            for lane in ("Science", "Culture", "Religious", "Domination", "Score"):
                _, progress = module._race(lane, {"confirmed_victory": lane.lower()}, 0, 6)
                self.assertEqual(progress, 1.0, lane)


@unittest.skipIf(sys.platform == "win32", "macOS recorder uses fcntl")
class TerminalGateTest(unittest.TestCase):
    def setUp(self):
        keeper = types.ModuleType("civvis_clip_keeper")
        keeper.HOME = keeper.CLIPS = Path("/unused-test-root")
        spec = importlib.util.spec_from_file_location("recording_gate", Path(__file__).parent / "recording/civvis_video_goal.py")
        self.gate = importlib.util.module_from_spec(spec)
        with mock.patch.dict(sys.modules, civvis_clip_keeper=keeper):
            spec.loader.exec_module(self.gate)
        self.evidence = dict(final_turn=2, expected_seconds=16,
            turn_offsets_seconds={"1": 0, "2": 10}, terminal_presentation=dict(
                kind="confirmed_victory_card", start_seconds=10, outcome=dict(
                    kind="victory", victory_type="VICTORY_TECHNOLOGY", turn=2,
                    won=True, local_player=0)))

    def test_only_the_confirmed_final_turn_can_replace_gameplay(self):
        self.assertEqual(self.gate.terminal_presentation(self.evidence), (10, 1))
        for changes in (dict(start_seconds=1), dict(kind="arbitrary_card"),
                        dict(outcome=dict(kind="defeat", turn=2))):
            invalid = dict(self.evidence, terminal_presentation=dict(self.evidence["terminal_presentation"], **changes))
            with self.assertRaises(self.gate.QualityError):
                self.gate.terminal_presentation(invalid)

    def test_gameplay_gate_checks_the_entire_prefix_and_result_has_separate_proof(self):
        proof = dict(schema=self.gate.PANEL_SCHEMA, both_visible=True, samples=10,
                     turn_samples=2, left_visible_share=1, right_visible_share=1,
                     max_blackout_seconds=0, missing_turns=[])
        evidence = dict(self.evidence, schema=self.gate.QUALITY_SCHEMA, all_turns=True,
                        first_turn=1, covered_turns=2, playback_rate=1,
                        export_recipe="unscaled_time_range", dual_panels=dict(source=proof))
        with mock.patch.object(self.gate, "media_info", return_value=dict(duration_seconds=16, width=3840, height=2160, fps=60)), \
             mock.patch.object(self.gate, "file_fingerprint", return_value={"size_bytes": 1}), \
             mock.patch.object(self.gate, "panel_evidence", return_value=dict(proof, turn_samples=1)) as scan:
            self.gate.validate_master("candidate.mp4", evidence)
        self.assertEqual(scan.call_args.args, ("candidate.mp4", 0, 10.0, {"1": 0}))


if __name__ == "__main__":
    unittest.main()
