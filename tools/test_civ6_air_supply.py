#!/usr/bin/env python3
"""Native supply regressions: proposals, missing data, upgrades and reloads."""
import contextlib
import gzip
import io
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import civ6_air_supply as supply


def seat(pid=0):
    return {"kind": "seat", "local_player": pid}


def state(turn, **extra):
    return {"kind": "state", "turn": turn, "units": [], "cities": [], **extra}


def tiles(turn, *plots):
    return {"kind": "tiles", "turn": turn, "plots": list(plots)}


def deposit(x, y, owner=-1, **extra):
    return {"x": x, "y": y, "r": supply.ALUMINUM, "o": owner, **extra}


class SupplyTests(unittest.TestCase):
    def test_no_state_has_no_measurement(self):
        self.assertIsNone(supply._air_supply_totals([seat(), tiles(80, deposit(2, 3))]))

    def test_queue_and_research_selection_do_not_prove_completion(self):
        result = supply._air_supply_totals([seat(), state(120,
            research="TECH_ADVANCED_FLIGHT", techs=[], cities=[
                {"producing": "DISTRICT_AERODROME", "districts": []},
                {"producing": "UNIT_BOMBER", "districts": []}]),
            {"kind": "orders", "turn": 120, "orders": [{"kind": "unit", "verb": "UNIT_BOMBER"}]}])
        self.assertEqual(set(result["milestones"]), {"aerodrome_queued", "bomber_queued"})
        self.assertEqual(result["peak_bombers_observed"], 0)

    def test_jet_upgrades_and_exported_promotion_class_preserve_the_wing(self):
        result = supply._air_supply_totals([seat(), state(100, units=[
            {"kind": "UNIT_BOMBER"}, {"kind": "UNIT_JET_BOMBER"}]), state(101, units=[
            {"kind": "UNIT_JET_BOMBER"}, {"kind": "UNIT_CUSTOM_BOMBER", "class": "PROMOTION_CLASS_AIR_BOMBER"}])])
        self.assertEqual(result["peak_bombers_observed"], 2)
        self.assertEqual(result["last_snapshot"]["bombers"], 2)
        self.assertEqual(result["milestones"]["two_bombers"]["observed_turn"], 100)

    def test_foreign_bombers_are_not_ours(self):
        result = supply._air_supply_totals([seat(), state(90,
            rivals=[{"player": 9, "units": [{"kind": "UNIT_BOMBER"}]}],
            minors=[{"player": 1, "units": [{"kind": "UNIT_JET_BOMBER"}]}])])
        self.assertEqual(result["peak_bombers_observed"], 0)

    def test_missing_measurements_are_unknown_and_explicit_zero_is_observed(self):
        result = supply._air_supply_totals([seat(), {"kind": "state", "turn": 90},
            state(92, strategic_resources={"RESOURCE_IRON": 4},
                  strategic_resource_income={supply.ALUMINUM: 0})])
        self.assertEqual(result["coverage"]["income"], {"observed_frames": 1, "unknown_frames": 1})
        self.assertEqual(result["coverage"]["stock"], {"observed_frames": 1, "unknown_frames": 1})
        self.assertEqual(result["last_snapshot"]["aluminum_stock"], 0)
        self.assertEqual(result["last_snapshot"]["aluminum_gross_income"], 0)
        self.assertEqual(result["missing_turns"], 1)
        self.assertNotIn("aluminum_positive_income", result["milestones"])

    def test_missing_resource_in_income_table_is_unknown(self):
        result = supply._air_supply_totals([state(90,
            strategic_resource_income={"RESOURCE_IRON": 2})])
        self.assertIsNone(result["last_snapshot"]["aluminum_gross_income"])

    def test_invalid_resource_values_cannot_prove_supply(self):
        for value in (True, -1, "2", float("nan"), float("inf")):
            with self.subTest(value=value):
                result = supply._air_supply_totals([state(90,
                    strategic_resource_income={supply.ALUMINUM: value},
                    strategic_resources={supply.ALUMINUM: value})])
                self.assertIsNone(result["last_snapshot"]["aluminum_stock"])
                self.assertIsNone(result["last_snapshot"]["aluminum_gross_income"])

    def test_incomplete_or_pillaged_base_is_not_usable(self):
        for complete, pillaged, expected in ((False, False, 0), (True, True, 0), (True, False, 1)):
            with self.subTest(complete=complete, pillaged=pillaged):
                result = supply._air_supply_totals([state(100, cities=[{"districts": [
                    {"type": "DISTRICT_AERODROME", "complete": complete, "pillaged": pillaged}]}])])
                self.assertEqual(result["last_snapshot"]["usable_aerodromes"], expected)

    def test_missing_district_or_unit_metadata_is_not_zero(self):
        for extra in ({"cities": [{}]}, {"cities": [{"districts": [{"type": "DISTRICT_AERODROME", "complete": True}]}]},
                      {"units": [None]}, {"units": [{}]}):
            with self.subTest(extra=extra):
                result = supply._air_supply_totals([state(100, **extra)])
                field = "usable_aerodromes" if "cities" in extra else "bombers"
                self.assertIsNone(result["last_snapshot"][field])

    def test_income_and_completed_techs_have_separate_first_observations(self):
        result = supply._air_supply_totals([state(100, techs=["TECH_RADIO"]),
            state(101, techs=["TECH_RADIO", "TECH_ADVANCED_FLIGHT"],
                  strategic_resource_income={supply.ALUMINUM: 2}, strategic_resources={supply.ALUMINUM: 1})])
        self.assertEqual(result["milestones"]["radio"]["observed_turn"], 100)
        self.assertEqual(result["milestones"]["advanced_flight"]["observed_turn"], 101)
        self.assertEqual(result["milestones"]["aluminum_positive_income"]["observed_turn"], 101)

    def test_native_owner_ids_and_actual_local_seat_classify_deposits(self):
        result = supply._air_supply_totals([seat(9), tiles(100,
            deposit(1, 2, 9), deposit(2, 2, 1), deposit(3, 2, 0), deposit(4, 2, -1), deposit(5, 2, 99)),
            state(100, rivals=[{"player": 1}], minors=[{"player": 0}])])
        self.assertEqual([d["observations"][0]["owner_kind"] for d in result["aluminum_deposits"]],
                         ["ours", "major", "minor", "unowned", "unknown"])

    def test_unknown_seat_never_assumes_player_zero(self):
        result = supply._air_supply_totals([tiles(100, deposit(1, 2, 0)), state(100)])
        self.assertIsNone(result["local_player"])
        self.assertNotIn("aluminum_owned_plot", result["milestones"])

    def test_delta_acquisition_repair_and_resource_loss_are_retained(self):
        result = supply._air_supply_totals([seat(), state(100), tiles(100, deposit(3, 2)),
            tiles(101, deposit(3, 2, 0, im="IMPROVEMENT_MINE", p=True)), state(101),
            tiles(102, deposit(3, 2, 0, im="IMPROVEMENT_MINE", p=False)), state(102),
            tiles(103, {"x": 3, "y": 2, "o": 0}), state(103)])
        self.assertEqual(result["milestones"]["aluminum_owned_plot"]["observed_turn"], 101)
        self.assertEqual(result["milestones"]["aluminum_owned_unpillaged_mine"]["observed_turn"], 102)
        self.assertFalse(result["aluminum_deposits"][0]["observations"][-1]["resource_present"])

    def test_deposit_milestones_are_earliest_across_plot_iteration_order(self):
        result = supply._air_supply_totals([seat(), state(100), tiles(100, deposit(9, 2, 0)),
            state(101), tiles(101, deposit(1, 2, 0))])
        self.assertEqual(result["milestones"]["aluminum_owned_plot"]["observed_turn"], 100)

    def test_sight_changes_do_not_invent_resource_acquisitions(self):
        result = supply._air_supply_totals([seat(), state(100), tiles(100, deposit(1, 2, vis=True)),
            tiles(100, deposit(1, 2, vis=False)), state(100)])
        self.assertEqual(len(result["aluminum_deposits"][0]["observations"]), 1)
        self.assertEqual(result["observed_turns"], 1)
        self.assertEqual(result["observed_state_frames"], 2)

    def test_stale_frames_cannot_add_a_wing_or_retroactive_supply(self):
        result = supply._air_supply_totals([seat(), state(140),
            state(139, units=[{"kind": "UNIT_BOMBER"}]), tiles(139, deposit(1, 2, 0))])
        self.assertEqual(result["observed_state_frames"], 1)
        self.assertEqual(result["peak_bombers_observed"], 0)
        self.assertEqual(result["aluminum_deposits"], [])

    def test_resumed_first_frame_is_not_a_completion_date(self):
        result = supply._air_supply_totals([seat(), state(160, units=[{"kind": "UNIT_JET_BOMBER"}])])
        self.assertEqual(result["scope"], "run_segment")
        self.assertTrue(result["milestones"]["first_bomber"]["present_at_first_frame"])

    def test_gzip_cli_and_partial_live_line_do_not_mutate_evidence(self):
        with TemporaryDirectory() as tmp:
            run = Path(tmp) / "native"; run.mkdir()
            content = json.dumps(state(140, units=[{"kind": "UNIT_JET_BOMBER"}])) + '\n{"kind":'
            path = run / "events.jsonl.gz"
            path.write_bytes(gzip.compress(content.encode()))
            summary = run / "summary.json"; summary.write_text(json.dumps({"seat": {"players": 4}}))
            before = (path.read_bytes(), summary.read_bytes())
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(supply.main([str(run)]), 0)
            result = json.loads(output.getvalue())
            self.assertEqual(result["air_supply"]["peak_bombers_observed"], 1)
            self.assertEqual((path.read_bytes(), summary.read_bytes()), before)


if __name__ == "__main__":
    unittest.main()
