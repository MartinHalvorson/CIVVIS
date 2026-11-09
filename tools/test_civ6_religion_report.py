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


FAITH = "RELIGION_BUDDHISM"
RIVAL_FAITH = "RELIGION_ISLAM"


def frame(turn, *, frame_index=0, units=(), rival=(), own=(), following=None, majority=None):
    """A state frame as the mod logs it, with only the fields the yield reads."""
    return {
        "kind": "state", "turn": turn, "frame": frame_index, "founded_religion": FAITH,
        "cities_following_religion": following,
        "units": [{"id": uid, "kind": kind, "x": x, "y": y, "spread_charges": charges}
                  for uid, kind, x, y, charges in units],
        "cities": [{"x": x, "y": y, "religion": rel} for x, y, rel in own],
        "minors": [],
        "rivals": [{
            "player": 3, "civ": "CIVILIZATION_ARABIA", "religion": majority,
            "public_stats": {"city_count": 9},
            "cities": [{"x": x, "y": y, "religion_observed": observed, "religion": rel}
                       for x, y, observed, rel in rival]}],
    }


def verified(unit, turn):
    return {"kind": "order_verified", "verb": "SPREAD_RELIGION", "order_kind": "unit",
            "subject": unit, "turn": turn}


CITY = (20, 20)


class SpreadYieldTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def game_yield(self, events, tag="game"):
        directory = self.root / tag
        directory.mkdir()
        (directory / "summary.json").write_text(json.dumps({
            "tag": tag, "victory_target": "religious", "seat": {"civ": "CIVILIZATION_RUSSIA"},
            "finished_utc": "2026-11-09T19:00:00Z", "reason": "abandoned"}))
        (directory / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))
        return religion.report(self.root, with_spread_yield=True)["games"][0]["spread_yield"]

    def test_the_live_root_reports_the_games_under_control(self):
        control = self.root / "control"
        control.mkdir()
        directory = control / "live"
        directory.mkdir()
        (directory / "summary.json").write_text(json.dumps({
            "tag": "live", "victory_target": "religious",
            "seat": {"civ": "CIVILIZATION_RUSSIA"}, "finished_utc": "2026-11-09T19:00:00Z"}))
        report = religion.report(self.root, civ="CIVILIZATION_RUSSIA")
        self.assertEqual([g["tag"] for g in report["games"]], ["live"])
        self.assertEqual(report["summaries_read"], 1)

    def test_a_spread_is_a_charge_that_fell_not_a_log_line(self):
        y = self.game_yield([
            frame(10, units=[(7, "UNIT_MISSIONARY", 21, 20, 3)], rival=[(*CITY, True, RIVAL_FAITH)]),
            frame(10, frame_index=1, units=[(7, "UNIT_MISSIONARY", 21, 20, 2)],
                  rival=[(*CITY, True, RIVAL_FAITH)]),
            frame(11, units=[(7, "UNIT_MISSIONARY", 21, 20, 2)], rival=[(*CITY, True, RIVAL_FAITH)]),
            # The same cast logged three times by re-planned frames.
            verified(7, 10), verified(7, 10), verified(7, 10)])
        self.assertEqual(y["spreads"]["rival"], 1)
        self.assertEqual(y["spread_outcomes"]["no_flip"], 1)
        self.assertEqual(y["rival_spreads_by_player"], {"3": 1})

    def test_a_cast_beside_our_own_city_is_not_a_rival_spread(self):
        y = self.game_yield([
            frame(10, units=[(7, "UNIT_APOSTLE", 10, 11, 2)], own=[(10, 10, FAITH)]),
            frame(11, units=[(7, "UNIT_APOSTLE", 10, 11, 1)], own=[(10, 10, FAITH)])])
        self.assertEqual((y["spreads"]["own"], y["spreads"]["rival"]), (1, 0))

    def test_a_vanished_last_charge_counts_only_with_a_verified_order(self):
        board = dict(rival=[(*CITY, True, RIVAL_FAITH)])
        y = self.game_yield([
            frame(10, units=[(1, "UNIT_MISSIONARY", 21, 20, 1), (2, "UNIT_MISSIONARY", 21, 21, 1),
                             (3, "UNIT_APOSTLE", 21, 19, 3)], **board),
            frame(11, units=[], **board),
            verified(1, 10), verified(3, 10)])
        # Unit 2 had no order; unit 3 held 3 charges, so it died rather than spent one.
        self.assertEqual(y["spreads"]["rival"], 1)

    def test_a_flip_needs_a_reading_either_side_and_the_hold_reads_ten_turns_on(self):
        city_a, city_b, unread = (20, 20), (30, 20), (40, 20)
        def board(turn, a, b):
            return frame(turn, units=[(1, "UNIT_APOSTLE", 21, 20, 2), (2, "UNIT_APOSTLE", 31, 20, 2),
                                      (3, "UNIT_APOSTLE", 41, 20, 2)],
                         rival=[(*city_a, True, a), (*city_b, True, b), (*unread, False, None)])
        spent = frame(10, frame_index=1,
                      units=[(1, "UNIT_APOSTLE", 21, 20, 1), (2, "UNIT_APOSTLE", 31, 20, 1),
                             (3, "UNIT_APOSTLE", 41, 20, 1)],
                      rival=[(*city_a, True, None), (*city_b, True, RIVAL_FAITH), (*unread, False, None)])
        y = self.game_yield([
            board(10, None, RIVAL_FAITH), spent,
            board(11, FAITH, FAITH), board(21, FAITH, RIVAL_FAITH),
            verified(1, 10), verified(2, 10), verified(3, 10)])
        self.assertEqual(y["spreads"]["rival"], 3)
        self.assertEqual(y["spread_outcomes"],
                         {"already_ours": 0, "flipped": 2, "no_flip": 0, "unread": 1})
        self.assertEqual(y["flips_credited"], 2)
        self.assertEqual((y["flips_held"], y["flips_reverted"], y["flips_unread"]), (1, 1, 0))

    def test_a_fogged_city_is_unread_not_a_loss_of_our_majority(self):
        y = self.game_yield([
            frame(10, units=[(1, "UNIT_APOSTLE", 21, 20, 2)], rival=[(*CITY, True, None)]),
            frame(10, frame_index=1, units=[(1, "UNIT_APOSTLE", 21, 20, 1)], rival=[(*CITY, True, None)]),
            frame(11, units=[(1, "UNIT_APOSTLE", 21, 20, 1)], rival=[(*CITY, True, FAITH)]),
            frame(21, units=[(1, "UNIT_APOSTLE", 21, 20, 1)], rival=[(*CITY, False, None)]),
            verified(1, 10)])
        self.assertEqual((y["flips_held"], y["flips_reverted"], y["flips_unread"]), (0, 0, 1))

    def test_the_summary_divides_flips_by_rival_spreads_and_keeps_the_game_count(self):
        game = {"spreads": {"rival": 25, "own": 5, "minor": 0, "unplaced": 0},
                "rival_spreads_by_player": {"1": 20, "2": 5},
                "spread_outcomes": {"already_ours": 1, "flipped": 3, "no_flip": 20, "unread": 1},
                "flips_credited": 5, "flips_uncredited": 1, "flips_held": 2, "flips_reverted": 1,
                "flips_unread": 2,
                "world_followers": {"gained": 9, "lost": 3, "last": 6}}
        summary = religion.yield_summary([game, game])
        self.assertEqual(summary["games"], 2)
        self.assertEqual(summary["spreads"]["rival"], 50)
        self.assertEqual(summary["flips_per_100_rival_spreads"], 20.0)
        self.assertEqual(summary["held_per_100_rival_spreads_confirmed"], 8.0)
        self.assertEqual(summary["held_share_of_read_flips"], round(2 / 3, 3))
        self.assertEqual(summary["mean_rival_share_of_spreads_by_rank"], [0.8, 0.2, 0.0, 0.0, 0.0])


if __name__ == "__main__":
    unittest.main()
