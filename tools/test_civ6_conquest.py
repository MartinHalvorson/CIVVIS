#!/usr/bin/env python3
"""Native ownership accounting must distinguish majors, minors and evidence gaps."""
from __future__ import annotations

import gzip
import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_conquest


def city(owner, x, y=4, *, capital=False, cid=65536):
    return {"id": cid, "x": x, "y": y, "original_owner": owner,
            "original_capital": capital, "capital": capital}


def state(turn, cities, *, rivals=None, minors=None, **extra):
    return {"kind": "state", "turn": turn, "cities": cities,
            "rivals": [{"player": 3, "at_war": False}] if rivals is None else rivals,
            "minors": [{"player": 9, "at_war": False}] if minors is None else minors,
            **extra}


class NativeConquestTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "events.jsonl"

    def totals(self, rows, *, seat=True, compressed=False):
        if seat:
            rows = [{"kind": "seat", "local_player": 2}] + rows
        data = "\n".join(json.dumps(row) for row in rows) + '\n{"kind": "state"'
        path = self.path
        if compressed:
            path = path.with_suffix(".jsonl.gz")
            with gzip.open(path, "wt") as handle:
                handle.write(data)
        else:
            path.write_text(data)
        return civ6_conquest.conquest_totals(path)

    def test_minor_capital_does_not_count_as_a_major_capital(self):
        got = self.totals([state(1, [city(2, 0, capital=True)]),
                           state(20, [city(2, 0, capital=True), city(9, 5, capital=True)])])
        self.assertEqual(got["foreign_major_cities_observed_held"], 0)
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 0)
        self.assertEqual(got["foreign_minor_cities_observed_held"], 1)
        self.assertTrue(got["own_original_capital_held_final"])

    def test_plot_identity_survives_id_change_and_owner_local_id_collision(self):
        got = self.totals([state(1, [city(2, 0, capital=True)]),
                           state(8, [city(2, 0, capital=True), city(3, 4, capital=True), city(3, 5)]),
                           state(9, [city(2, 0, capital=True), city(3, 4, capital=True, cid=99), city(3, 5, cid=100)])])
        self.assertEqual(got["foreign_major_cities_observed_held"], 2)
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 1)
        self.assertEqual(got["first_major_city_held"], {"observed_turn": 8, "present_at_first_frame": False})

    def test_original_capital_is_not_inferred_from_current_capital(self):
        replacement = city(3, 4)
        replacement["capital"] = True
        got = self.totals([state(10, [replacement])])
        self.assertEqual(got["foreign_major_cities_held_final"], 1)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 0)

    def test_capital_taken_then_lost_is_separate_from_final_control(self):
        got = self.totals([state(1, [city(2, 0, capital=True)]),
                           state(8, [city(3, 4, capital=True)]), state(9, [])])
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 1)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 0)
        self.assertFalse(got["own_original_capital_held_final"])

    def test_own_capital_recapture_and_replan_frames_do_not_double_count(self):
        got = self.totals([state(1, [city(2, 0, capital=True)]), state(8, []),
                           state(9, [city(3, 4, capital=True)]),
                           state(9, [city(2, 0, capital=True, cid=99), city(3, 4, capital=True)])])
        self.assertTrue(got["own_original_capital_held_final"])
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 1)
        self.assertEqual(got["observed_turns"], 3)
        self.assertEqual(got["missing_turns"], 6)

    def test_late_continuation_is_explicitly_segment_scoped(self):
        got = self.totals([state(190, [city(3, 4, capital=True)])])
        self.assertEqual(got["scope"], "run_segment")
        self.assertEqual(got["first_major_original_capital_held"],
                         {"observed_turn": 190, "present_at_first_frame": True})
        self.assertIsNone(got["own_original_capital_held_final"])

    def test_eliminated_minor_identity_is_remembered(self):
        got = self.totals([state(1, []), state(20, [city(9, 5, capital=True)], minors=[])])
        self.assertEqual(got["foreign_minor_cities_held_final"], 1)
        self.assertEqual(got["unclassified_cities_held_final"], 0)

    def test_declarations_are_separate_from_defensive_war_exposure(self):
        got = self.totals([
            state(1, [], rivals=[{"player": 3, "at_war": True}]),
            {"kind": "war", "turn": 2, "target": 9, "source": "civvis"},
            {"kind": "war", "turn": 3, "target": 3, "source": "civvis"},
            {"kind": "war", "turn": 3, "target": 77, "source": "civvis"},
            {"kind": "war", "turn": 3, "target": 3, "source": "other"},
        ])
        self.assertEqual(got["major_declaration_requests"], 1)
        self.assertEqual(got["minor_declaration_requests"], 1)
        self.assertEqual(got["unclassified_declaration_requests"], 1)
        self.assertEqual(got["first_major_declaration_request_turn"], 3)
        self.assertEqual(got["first_major_war_observed_turn"], 1)

    def test_late_identity_and_capital_metadata_do_not_backdate_a_capital(self):
        unknown = city(3, 4, capital=True)
        unknown.pop("original_capital")
        got = self.totals([state(10, [unknown], rivals=[]), state(15, [city(3, 4, capital=True)])])
        self.assertEqual(got["first_major_city_held"]["observed_turn"], 10)
        self.assertEqual(got["first_major_original_capital_held"],
                         {"observed_turn": 15, "present_at_first_frame": False})

    def test_missing_owner_is_not_backdated_after_late_readback(self):
        unknown = city(3, 4, capital=True)
        unknown.pop("original_owner")
        got = self.totals([state(10, [unknown]), state(15, [city(3, 4, capital=True)])])
        self.assertEqual(got["first_major_city_held"]["observed_turn"], 15)
        self.assertEqual(got["first_major_original_capital_held"]["observed_turn"], 15)

    def test_capital_flag_without_founder_cannot_be_backfilled_as_a_capital(self):
        unknown = city(3, 4, capital=True)
        unknown.pop("original_owner")
        got = self.totals([state(10, [unknown]), state(15, [city(3, 4)])])
        self.assertEqual(got["foreign_major_cities_observed_held"], 1)
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 0)
        self.assertIsNone(got["first_major_original_capital_held"])

    def test_missing_metadata_stays_unknown_and_bad_rosters_do_not_invent_loss(self):
        unknown = {"id": 44, "x": 4, "y": 4, "capital": True}
        got = self.totals([state(10, [city(2, 0, capital=True)]),
                           state(11, [unknown, city(77, 5), {"id": 45}]),
                           {"kind": "state", "turn": 12}])
        self.assertEqual(got["foreign_major_cities_held_final"], 0)
        self.assertEqual(got["unclassified_cities_held_final"], 3)
        self.assertEqual(got["cities_missing_original_capital_final"], 2)
        self.assertEqual(got["cities_missing_plot_final"], 1)
        self.assertIsNone(got["own_original_capital_held_final"])
        self.assertEqual(got["last_observed_turn"], 11)

    def test_stale_state_cannot_overwrite_last_ownership(self):
        got = self.totals([state(10, [city(3, 4, capital=True)]), state(9, [])])
        self.assertEqual(got["last_observed_turn"], 10)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 1)

    def test_no_seat_or_no_ownership_state_is_unknown(self):
        self.assertIsNone(self.totals([state(10, [])], seat=False))
        self.assertIsNone(self.totals([{"kind": "turn", "turn": 10}]))

    def test_compressed_evidence_and_torn_tail_are_supported(self):
        got = self.totals([state(10, [city(3, 4, capital=True)])], compressed=True)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 1)

    def test_cli_counts_the_host_victory_name_instead_of_the_requested_target(self):
        self.totals([state(10, [city(3, 4, capital=True)])])
        for host_type, expected in (("VICTORY_RELIGIOUS", False), ("VICTORY_CONQUEST", True)):
            summary = {"victory_target": "domination", "outcome": {"kind": "victory", "won": True, "victory": 19},
                       "seat": {"victory_types": [{"index": 19, "type": host_type}]}}
            (self.path.parent / "summary.json").write_text(json.dumps(summary))
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(civ6_conquest.main([str(self.path.parent)]), 0)
            got = json.loads(output.getvalue())
            self.assertEqual(got["victory_type"], host_type)
            self.assertEqual(got["domination_win"], expected)

    def test_cli_does_not_label_an_incomplete_run_as_a_completed_loss(self):
        self.totals([state(10, [])])
        output = io.StringIO()
        with redirect_stdout(output):
            civ6_conquest.main([str(self.path.parent)])
        self.assertIsNone(json.loads(output.getvalue())["domination_win"])


class RecoveredConquestTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.runs = Path(self.tmp.name)

    def run_rows(self, tag, rows, *, seat_changes=None, home=0, compressed=False):
        run = self.runs / tag
        run.mkdir(exist_ok=True)
        seat = {"kind": "seat", "run": tag, "local_player": 2,
                "civ": "CIVILIZATION_GRAN_COLOMBIA", "leader": "LEADER_SIMON_BOLIVAR",
                "difficulty": "DIFFICULTY_KING", "players": 4,
                "map": "Pangaea.lua", "size": "MAPSIZE_TINY", "speed": "GAMESPEED_ONLINE",
                "ruleset": "RULESET_EXPANSION_2", "max_turns": 250, "city_states": 6,
                "modes": [], "victories": {"conquest": True, "science": True}}
        seat.update(seat_changes or {})
        data = [seat]
        for row in rows:
            row = dict(row, run=tag)
            if row.get("kind") == "state" and home is not None:
                row["cities"] = [city(2, home, capital=True), *row["cities"]]
            data.append(row)
        text = "\n".join(json.dumps(row) for row in data) + "\n"
        if compressed:
            with gzip.open(run / "events.jsonl.gz", "wt") as out:
                out.write(text)
        else:
            (run / "events.jsonl").write_text(text)
        return run

    def chain(self, root, *tags):
        # Deliberately unrelated save counters: state exports are the boundary.
        return civ6_conquest.write_recovery_chain(
            self.runs, root, [{"tag": tag, "from_turn": 20,
                               "save": "AutoSave_9999.Civ6Save"} for tag in tags])

    def test_retained_capture_is_counted_once_with_its_original_first_observation(self):
        root = self.run_rows("original", [state(t, [city(3, 4, capital=True)] if t >= 4 else [])
                                           for t in range(1, 9)])
        current = self.run_rows("continued", [state(t, [city(3, 4, capital=True, cid=99)])
                                               for t in range(5, 10)])
        self.chain(root.name, current.name)
        segment = civ6_conquest.conquest_totals(current / "events.jsonl")
        got = civ6_conquest.recovered_conquest(current)
        self.assertEqual(segment["first_major_city_held"],
                         {"observed_turn": 5, "present_at_first_frame": True})
        self.assertEqual(got["first_major_city_held"],
                         {"observed_turn": 4, "present_at_first_frame": False})
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 1)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 1)
        self.assertEqual(got["observed_turns"], 9)
        self.assertEqual(got["missing_turns"], 0)
        self.assertTrue(got["own_original_capital_held_final"])

    def test_capture_and_declaration_on_rolled_back_turns_are_excluded(self):
        rows = []
        for t in range(1, 9):
            if t == 6:
                rows.append({"kind": "war", "turn": t, "source": "civvis", "target": 3})
            rows.append(state(t, [city(3, 4, capital=True)] if t >= 7 else []))
        root = self.run_rows("original", rows)
        current = self.run_rows("continued", [state(t, []) for t in range(5, 10)])
        self.chain(root.name, current.name)
        self.assertEqual(civ6_conquest.conquest_totals(root / "events.jsonl")
                         ["foreign_major_original_capitals_observed_held"], 1)
        got = civ6_conquest.recovered_conquest(current)
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 0)
        self.assertIsNone(got["first_major_original_capital_held"])
        self.assertEqual(got["major_declaration_requests"], 0)

    def test_earlier_second_reload_can_discard_an_entire_intermediate_branch(self):
        self.run_rows("original", [state(t, [city(3, 4)] if t >= 4 else [])
                                    for t in range(1, 13)])
        self.run_rows("first-reload", [state(t, [city(3, 4), city(3, 8, capital=True)])
                                        for t in range(10, 15)])
        current = self.run_rows("second-reload", [state(t, [city(3, 4)]) for t in range(8, 16)])
        self.chain("original", "first-reload", "second-reload")
        got = civ6_conquest.recovered_conquest(current)
        self.assertEqual(got["discarded_segments"], ["first-reload"])
        self.assertEqual(got["segments_used"], [
            {"tag": "original", "first_observed_turn": 1, "last_observed_turn": 7},
            {"tag": "second-reload", "first_observed_turn": 8, "last_observed_turn": 15}])
        self.assertEqual(got["foreign_major_cities_observed_held"], 1)
        self.assertEqual(got["foreign_major_original_capitals_observed_held"], 0)
        self.assertEqual(got["observed_turns"], 15)
        self.assertEqual(got["first_major_city_held"]["observed_turn"], 4)

    def test_identity_survives_elimination_and_minors_remain_separate(self):
        self.run_rows("original", [state(1, []), state(4, [city(3, 4), city(9, 8, capital=True)])])
        current = self.run_rows("continued", [state(5, [city(3, 4), city(9, 8, capital=True)],
                                                        rivals=[], minors=[])])
        self.chain("original", current.name)
        got = civ6_conquest.recovered_conquest(current)
        self.assertEqual(got["foreign_major_cities_held_final"], 1)
        self.assertEqual(got["foreign_minor_cities_held_final"], 1)
        self.assertEqual(got["foreign_major_original_capitals_held_final"], 0)
        self.assertEqual(got["unclassified_cities_held_final"], 0)

    def test_original_home_capital_can_be_verified_after_being_lost(self):
        self.run_rows("original", [state(1, [])])
        current = self.run_rows("continued", [state(2, [], rivals=[
            {"player": 3, "cities": [city(2, 0, capital=True)]}])], home=None)
        self.chain("original", current.name)
        got = civ6_conquest.recovered_conquest(current)
        self.assertTrue(got["available"])
        self.assertFalse(got["own_original_capital_held_final"])

    def test_missing_turns_remain_explicit(self):
        self.run_rows("original", [state(1, []), state(2, [])])
        current = self.run_rows("continued", [state(5, []), state(6, [])])
        self.chain("original", current.name)
        got = civ6_conquest.recovered_conquest(current)
        self.assertEqual(got["observed_turns"], 4)
        self.assertEqual(got["missing_turns"], 2)

    def test_compressed_events_are_supported(self):
        self.run_rows("original", [state(1, [])], compressed=True)
        current = self.run_rows("continued", [state(2, [])], compressed=True)
        self.chain("original", current.name)
        self.assertTrue(civ6_conquest.recovered_conquest(current)["available"])

    def test_names_alone_do_not_infer_ancestry(self):
        self.run_rows("original", [state(1, [])])
        current = self.run_rows("original-cont1", [state(5, [])])
        self.assertIsNone(civ6_conquest.recovered_conquest(current))

    def test_missing_evidence_or_changed_native_identity_is_unavailable(self):
        for mismatch in ("missing-events", "different-seat", "missing-seat-field", "different-home"):
            with self.subTest(mismatch=mismatch):
                root = "root-" + mismatch
                current = "cont-" + mismatch
                self.run_rows(root, [state(1, [])])
                path = self.run_rows(current, [state(5, [])],
                                     seat_changes={"difficulty": "DIFFICULTY_PRINCE"}
                                     if mismatch == "different-seat" else
                                     {"leader": None} if mismatch == "missing-seat-field" else {},
                                     home=9 if mismatch == "different-home" else 0)
                self.chain(root, current)
                if mismatch == "missing-events":
                    (self.runs / root / "events.jsonl").unlink()
                got = civ6_conquest.recovered_conquest(path)
                self.assertFalse(got["available"])
                self.assertNotIn("foreign_major_cities_held_final", got)

    def test_inconsistent_manifest_and_path_traversal_are_unavailable(self):
        self.run_rows("original", [state(1, [])])
        current = self.run_rows("continued", [state(5, [])])
        manifest = self.chain("original", current.name)
        original = json.loads(manifest.read_text())
        for changes in ({"current": "other"}, {"segments": ["original", "original"]},
                        {"segments": ["../original", "continued"]}, {"reloads": []}):
            with self.subTest(changes=changes):
                manifest.write_text(json.dumps(dict(original, **changes)))
                self.assertFalse(civ6_conquest.recovered_conquest(current)["available"])

    def test_cli_accepts_explicit_historical_chain_without_rewriting_the_run(self):
        self.run_rows("original", [state(1, [])])
        current = self.run_rows("continued", [state(2, [])])
        generated = self.chain("original", current.name)
        historical = self.runs / "historical.json"
        generated.rename(historical)
        before = (current / "events.jsonl").read_bytes()
        output = io.StringIO()
        with redirect_stdout(output):
            civ6_conquest.main([str(current), "--chain", str(historical)])
        self.assertTrue(json.loads(output.getvalue())["game_conquest"]["available"])
        self.assertEqual((current / "events.jsonl").read_bytes(), before)
        self.assertFalse(generated.exists())


if __name__ == "__main__":
    unittest.main()
