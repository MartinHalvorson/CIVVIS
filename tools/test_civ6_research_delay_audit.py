import json
from pathlib import Path
import tempfile
import unittest

from civ6_research_delay_audit import audit, native_milestones, parse_choices, prerequisite_closure, summarize_attempt


class ResearchDelayAuditTests(unittest.TestCase):
    def test_military_label_does_not_mean_unrelated_prerequisite(self):
        graph = {"satellites": {"requires": ["advanced_flight", "rocketry"]},
                 "advanced_flight": {}, "rocketry": {}, "ballistics": {}}
        path = prerequisite_closure(graph, "satellites")
        self.assertIn("advanced_flight", path)
        self.assertNotIn("ballistics", path)

    def test_native_state_is_distinct_from_selection_and_uses_first_turn(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "events.jsonl"
            path.write_text("\n".join(json.dumps(row) for row in [
                {"kind": "state", "turn": 171, "techs": ["TECH_ROCKETRY"]},
                {"kind": "state", "turn": 172, "techs": ["TECH_ROCKETRY"]},
                {"kind": "state", "turn": 196, "techs": ["TECH_ROCKETRY", "TECH_SATELLITES"]},
            ]) + "\n")
            self.assertEqual(native_milestones(path)["first_observed_owned_turns"],
                             {"rocketry": 171, "satellites": 196})

    def test_duplicate_frames_are_not_extra_detours(self):
        text = "\n".join([
            "[why] t163 Research/Decision Researching rocketry | science needs it",
            "[why] t171 Research/Decision Researching ballistics | catch a nearby barbarian army",
            "[why] t171 Research/Decision Researching ballistics | catch a nearby barbarian army",
            "[why] t192 Research/Decision Researching satellites | science needs it",
            "[why] t193 Research/Detail Researching printing | not a decision",
        ])
        result = summarize_attempt({}, parse_choices(text))
        self.assertEqual(result["rocket_to_satellite_selection_turns"], 29)
        self.assertEqual(len(result["military_choices_between_rocket_and_satellite"]), 1)
        self.assertFalse(result["satellite_unobserved_after_rocketry"])

    def test_unobserved_satellite_is_censored_not_zero_delay(self):
        result = summarize_attempt({}, [{"turn": 180, "tech": "rocketry", "reason": "science"}])
        self.assertIsNone(result["rocket_to_satellite_selection_turns"])
        self.assertTrue(result["satellite_unobserved_after_rocketry"])

    def test_missing_logs_and_abandonments_are_visible(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            base = {"utc": "2026-11-02", "configured": True, "difficulty": "emperor", "victory_target": "science", "won": False}
            attempts = [dict(base, tag="absent"), dict(base, tag="abandoned", reason="abandoned")]
            (root / "ladder.json").write_text(json.dumps({"attempts": attempts}))
            (root / "abandoned").mkdir()
            (root / "abandoned" / "why.log").write_text("[why] t180 Research/Decision Researching rocketry | science\n")
            result = audit(root, "2026-11-01")
            self.assertEqual(result["excluded"], [{"tag": "absent", "reason": "missing why.log"}])
            self.assertEqual(result["summary"]["emperor/science/abandoned"]["attempts"], 1)
            self.assertEqual(len(result["attempts"][0]["why_sha256"]), 64)


if __name__ == "__main__":
    unittest.main()
