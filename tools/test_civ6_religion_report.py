"""Religious progress distinguishes native endings, missing data and segments."""

import gzip
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import civ6_religion_report as religion


class ReligionReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def segment(self, tag, events=(), *, outcome=None, compressed=False, **fields):
        directory = self.root / tag
        directory.mkdir()
        summary = {"tag": tag, "victory_target": "religious",
                   "finished_utc": "2026-11-09T19:00:00Z",
                   "seat": {"civ": "CIVILIZATION_RUSSIA"},
                   "reason": "abandoned", "outcome": outcome, **fields}
        (directory / "summary.json").write_text(json.dumps(summary))
        text = "".join(json.dumps(e) + "\n" for e in events)
        if compressed:
            (directory / "events.jsonl.gz").write_bytes(gzip.compress(text.encode()))
        else:
            (directory / "events.jsonl").write_text(text)
        return directory

    def test_world_cities_and_founder_do_not_count_as_converted_majorities(self):
        progress = religion.final_progress({
            "turn": 166, "founded_religion": "RELIGION_BUDDHISM",
            "cities_following_religion": 15,
            "rivals": [
                {"player": 1, "religion": None},
                {"player": 2, "religion": "RELIGION_CONFUCIANISM"},
                {"player": 3, "religion": "RELIGION_BUDDHISM"},
                {"player": 4, "religion": "RELIGION_BUDDHISM", "alive": False},
                {"player": 5, "religion": "RELIGION_BUDDHISM", "is_minor": True}],
            "religions": [{"founder": 2, "type": "RELIGION_BUDDHISM"}]})
        self.assertEqual(progress["world_cities_following_religion"], 15)
        self.assertEqual(progress["reported_rival_majorities_matching_ours"], 1)
        self.assertEqual(progress["reported_rivals_without_resolved_majority"], 1)
        self.assertEqual(len(progress["reported_rivals"]), 3)
        self.assertIsNone(religion.final_progress({"rivals": [{"religion": None}]})[
            "reported_rival_majorities_matching_ours"])

    def test_missing_rival_and_unit_exports_are_unknown_rather_than_zero(self):
        progress = religion.final_progress({"founded_religion": "RELIGION_BUDDHISM"})
        for key in ("reported_rivals", "reported_rival_majorities_matching_ours",
                    "reported_rivals_without_resolved_majority", "religious_units"):
            self.assertIsNone(progress[key])
        empty = religion.final_progress({"founded_religion": "RELIGION_BUDDHISM",
                                         "rivals": [], "units": []})
        self.assertEqual(empty["reported_rival_majorities_matching_ours"], 0)
        self.assertEqual(empty["religious_units"]["UNIT_APOSTLE"], 0)

    def test_only_our_explicit_religious_win_counts(self):
        for won, kind, expected in (
                (True, "VICTORY_RELIGIOUS", "religious_win"),
                (True, "VICTORY_TECHNOLOGY", "other_victory"),
                (False, "VICTORY_RELIGIOUS", "loss"),
                (None, "VICTORY_RELIGIOUS", "terminal_unresolved"),
                (True, None, "win_type_unknown")):
            with self.subTest(won=won, kind=kind):
                summary = {"outcome": {"kind": "victory", "won": won,
                                       "victory_type": kind}}
                self.assertEqual(religion.outcome_of(summary)[0], expected)
        summary = {"outcome": {"kind": "victory", "won": True, "victory": 17},
                   "seat": {"victory_types": [{"index": 17,
                                                "type": "VICTORY_RELIGIOUS"}]}}
        self.assertEqual(religion.outcome_of(summary)[0], "religious_win")

    def test_retirements_are_censored_and_only_our_elimination_is_a_loss(self):
        for reason in ("abandoned", "operator_retired", "stopped"):
            self.assertEqual(religion.outcome_of({"reason": reason})[0], "censored")
        self.assertEqual(religion.outcome_of({"outcome": {
            "kind": "defeat", "ours": True}})[0], "loss")
        self.assertEqual(religion.outcome_of({"outcome": {
            "kind": "defeat", "ours": False}})[0], "censored")

    def test_continuation_keeps_milestones_and_terminal_outcome_once(self):
        self.segment("game", [
            {"kind": "state", "turn": 22, "founded_religion": "RELIGION_BUDDHISM"},
            {"kind": "state", "turn": 47, "civics": ["CIVIC_THEOLOGY"]},
            {"kind": "state", "turn": 51,
             "cities": [{"buildings": ["BUILDING_TEMPLE"]}],
             "units": [{"kind": "UNIT_APOSTLE"}]}],
            genome_treatments={"treatments": []})
        self.segment("game-cont1", [
            {"kind": "order_verified", "turn": 51, "checked_on": 52,
             "order_kind": "purchase_faith", "verb": "UNIT_APOSTLE"},
            {"kind": "state", "turn": 90, "civics": ["CIVIC_REFORMED_CHURCH"],
             "government": "GOVERNMENT_THEOCRACY",
             "policies": ["POLICY_RELIGIOUS_ORDERS"]}], compressed=True,
            outcome={"kind": "victory", "won": True, "victory_type": "VICTORY_RELIGIOUS"},
            genome_treatments={"treatments": [religion.REFORMED_GENE]})
        report = religion.report(self.root)
        self.assertEqual(report["counts"], {"religious_win": 1})
        game = report["games"][0]
        self.assertTrue(game["stem_present"])
        self.assertEqual(game["first_observed_turn"], {
            "founded_religion": 22, "theology": 47, "reformed_church": 90,
            "temple": 51, "apostle": 51, "verified_apostle_purchase": 51,
            "theocracy": 90, "religious_orders": 90})
        self.assertEqual([s["reformed_church_reported_active"] for s in game["segments"]],
                         [False, True])

    def test_missing_final_segment_state_is_not_replaced_with_old_progress(self):
        self.segment("game", [{"kind": "state", "turn": 10, "faith": 100}])
        self.segment("game-cont1")
        self.assertIsNone(religion.report(self.root)["games"][0]["final"])
        self.segment("orphan-cont2", [{"kind": "state", "turn": 150}])
        orphan = next(g for g in religion.report(self.root)["games"] if g["tag"] == "orphan")
        self.assertFalse(orphan["stem_present"])
        self.assertIsNone(orphan["first_observed_turn"]["theology"])

    def test_final_frame_and_partial_tail_are_explicit(self):
        directory = self.segment("game", [
            {"kind": "state", "turn": 10, "frame": 1, "faith": 90},
            {"kind": "state", "turn": 10, "frame": 0, "faith": 100},
            {"kind": "state", "turn": 9, "faith": 200}])
        with (directory / "events.jsonl").open("a") as stream:
            stream.write('{"kind":')
        game = religion.report(directory)["games"][0]
        self.assertEqual(game["final"]["faith"], 90)
        self.assertEqual(game["invalid_event_lines"], 1)

    def test_filters_keep_religious_lane_and_requested_cohort(self):
        self.segment("russia")
        self.segment("old", finished_utc="2026-11-08T23:59:00Z")
        self.segment("khmer", seat={"civ": "CIVILIZATION_KHMER"})
        self.segment("science", victory_target="science")
        report = religion.report(self.root, since="2026-11-09", civ="CIVILIZATION_RUSSIA")
        self.assertEqual([g["tag"] for g in report["games"]], ["russia"])


if __name__ == "__main__":
    unittest.main()
