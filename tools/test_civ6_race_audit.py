import gzip
import json
import tempfile
import unittest
from pathlib import Path

import civ6_ladder
import civ6_race_audit as audit


class RaceAuditTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def write_events(self, rows, name="events.jsonl"):
        path = self.root / name
        text = "\n".join(json.dumps(row) for row in rows) + "\n{"
        if name.endswith(".gz"):
            path.write_bytes(gzip.compress(text.encode()))
        else:
            path.write_text(text)
        return path

    def test_milestones_require_completed_unpillaged_districts_and_exact_checkpoints(self):
        pad = {"type": "DISTRICT_SPACEPORT", "complete": False}
        rows = [{"kind": "state", "turn": 99, "cities": [{"id": 7, "districts": [pad]}]},
                {"kind": "state", "turn": 101, "techs": ["TECH_ROCKETRY"],
                 "cities": [{"id": 7, "districts": [{**pad, "complete": True, "pillaged": True}]}]},
                {"kind": "state", "turn": 102, "cities": [
                    {"id": 7, "districts": [{**pad, "complete": True}]}]}]
        result = audit.race_totals(self.write_events(rows, "events.jsonl.gz"))
        self.assertNotIn("100", result["checkpoints"])
        self.assertEqual(result["milestones"]["spaceport"]["observed_turn"], 102)
        self.assertEqual(result["missing_turns"], 1)
        self.assertFalse(result["milestones"]["rocketry"]["present_at_first_frame"])

    def test_boosts_survive_completion_and_midturn_frames_do_not_double_count(self):
        rows = [{"kind": "state", "turn": 1, "techs": [], "cities": []},
                {"kind": "state", "turn": 2, "techs": [], "boosted_techs": ["TECH_WRITING"],
                 "cities": [{"id": 7, "producing": None}]},
                {"kind": "state", "turn": 2, "frame": 1, "techs": ["TECH_WRITING"],
                 "boosted_techs": [], "cities": [{"id": 7, "producing": None}]}]
        result = audit.race_totals(self.write_events(rows))
        self.assertEqual(result["idle_city_turns"], 1)
        self.assertEqual(len(result["boost_audit"]["techs"]), 1)
        self.assertTrue(result["boost_audit"]["techs"][0]["boost_observed"])

    def test_resume_does_not_invent_completion_time_or_earn_a_checkpoint(self):
        result = audit.race_totals(self.write_events([
            {"kind": "state", "turn": 175, "techs": ["TECH_ROCKETRY"], "cities": []}]))
        self.assertTrue(result["milestones"]["rocketry"]["present_at_first_frame"])
        self.assertEqual(result["checkpoints"], {})

    def test_empty_nonstate_stream_is_unknown(self):
        self.assertIsNone(audit.race_totals(self.write_events([{"kind": "orders"}])))

    def test_economy_uses_first_frame_and_preserves_missing_evidence(self):
        result = audit.race_totals(self.write_events([
            {"kind": "state", "turn": 99},
            {"kind": "state", "turn": 100, "gold": 0, "gold_per_turn": -12,
             "military": 50, "cities": []},
            {"kind": "state", "turn": 100, "gold": 500, "gold_per_turn": 2},
            {"kind": "state", "turn": 101, "gold": 20, "gold_per_turn": -12}]))
        self.assertEqual(result["empty_treasury_deficit_turns"], 1)
        self.assertEqual(result["economy_observed_turns"], 2)
        self.assertEqual(result["checkpoints"]["100"]["military"], 50)

    def test_continuations_group_but_capture_free_attempts_do_not(self):
        self.assertEqual(audit.game_key({"tag": "civvis-20260901T000000Z-cont8"}),
                         "civvis-20260901T000000Z")
        self.assertNotEqual(audit.game_key({"tag": "x-capture-free-1"}),
                            audit.game_key({"tag": "x-capture-free-2"}))

    def test_unfinished_or_changed_controller_games_are_excluded(self):
        runs = []
        for index, revision in enumerate(("a", "b")):
            run = self.root / str(index)
            run.mkdir()
            (run / "summary.json").write_text(json.dumps({
                "tag": "game" + ("-cont1" if index else ""),
                "decider_revisions": [revision], "configured": True,
                "outcome": {"kind": "defeat", "ours": False}}))
            runs.append(run)
        result = audit.report(runs)
        self.assertEqual(result["game_count"], 1)
        self.assertEqual(result["eligible_games"], 0)
        self.assertIn("settings_or_controller_changed", result["games"][0]["exclusions"])
        self.assertIn("no_game_outcome", result["games"][0]["exclusions"])

    def test_automatic_record_persists_race_and_genome_identity(self):
        run = self.root / "game"
        run.mkdir()
        summary = {"tag": "game", "genome_treatments": {"treatments": ["science"]},
                   "boosts": {"techs_boosted": 3}}
        path = run / "summary.json"
        path.write_text(json.dumps(summary))
        (run / "events.jsonl").write_text(json.dumps({
            "kind": "state", "turn": 1, "techs": [], "cities": []}) + "\n")
        ledger = self.root / "ladder.json"
        ledger.write_text('{"attempts": [], "wins": {}}')
        self.assertTrue(civ6_ladder.record_summary(path, ledger))
        entry = json.loads(ledger.read_text())["attempts"][0]
        self.assertEqual(entry["race"]["first_turn"], 1)
        self.assertEqual(entry["game_id"], "game")
        self.assertEqual(entry["genome_treatments"], summary["genome_treatments"])
        self.assertEqual(entry["boosts"], summary["boosts"])

    def test_complete_fixed_controller_cohorts_count_games_and_real_winners(self):
        runs = []
        for index, (revision, outcome) in enumerate([
                ("a", {"kind": "victory", "won": True}),
                ("a", {"kind": "victory", "won": False}),
                ("a", None), ("b", {"kind": "victory", "won": False})]):
            run = self.root / str(index)
            run.mkdir()
            summary = {key: "verified" for key in (
                "difficulty", "ruleset", "speed", "map_size", "max_turns",
                "victory_target", "modes", "genome_treatments", "mod_arms")}
            summary.update(tag=str(index), configured=True, outcome=outcome,
                           seat={key: "verified" for key in
                                 ("leader", "map", "players", "victories")},
                           decider_revisions=[revision],
                           decider_binaries=[{"binary_sha256": revision}],
                           race={"first_turn": 1})
            (run / "summary.json").write_text(json.dumps(summary))
            runs.append(run)
        result = audit.report(runs)
        self.assertEqual(result["eligible_games"], 3)
        self.assertEqual(len(result["cohorts"]), 2)
        first = next(c for c in result["cohorts"].values()
                     if c["identity"]["revisions"] == ["a"])
        self.assertEqual((first["wins"], first["losses"], first["excluded_games"]), (1, 1, 1))
        self.assertEqual(first["win_rate"], 0.5)


if __name__ == "__main__":
    unittest.main()
