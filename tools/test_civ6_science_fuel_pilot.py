import json
from pathlib import Path
import tempfile
import unittest

from civ6_science_fuel_pilot import GENE, audit_run, census, records


class FuelPilotTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def write(self, directory, name, rows):
        (directory / name).write_text("".join(json.dumps(r) + "\n" for r in rows))

    def run_dir(self, name, start, lane="science", configured=True):
        directory = self.root / name
        directory.mkdir()
        self.write(directory, "runtime_updates.jsonl", [
            {"status": "start", "utc": start, "binary_revision": "rev"}])
        (directory / "summary.json").write_text(json.dumps({
            "configured": configured, "victory_target": lane, "operator_retire": {"turn": 122}}))
        return directory

    def test_first_eight_families_join_continuations_and_keep_retirement(self):
        self.run_dir("civvis-20261102T120000Z", "2026-11-02T12:00:00Z")
        self.run_dir("civvis-20261102T120000Z-cont1", "2026-11-02T13:00:00Z")
        self.run_dir("civvis-20261102T130100Z", "2026-11-02T13:01:00Z")
        self.run_dir("civvis-20261102T130100Z-cont1", "2026-11-02T13:02:00Z")
        self.run_dir("civvis-20261102T130300Z", "2026-11-02T13:03:00Z", "culture")
        self.run_dir("civvis-20261102T130400Z", "2026-11-02T13:04:00Z", configured=False)
        self.run_dir("civvis-20261102T130500Z", "2026-11-02T13:05:00Z")
        r = census(self.root, "2026-11-02T12:30:00Z", 1)
        self.assertEqual(r["primary_observed"], 2)
        self.assertEqual(len(r["primary"][0]["runs"]), 2)
        self.assertEqual(r["primary"][0]["runs"][0]["summary"]["operator_retire"], {"turn": 122})
        self.assertEqual(len(r["primary_after_stop_boundary"]), 1)
        self.assertEqual(len(r["other_lanes"]), 1)
        self.assertEqual(len(r["pending_or_setup_attrition"]), 1)

    def test_actual_genome_normalizes_only_candidate_and_keeps_profile_changes(self):
        d = self.run_dir("civvis-20261102T130100Z", "2026-11-02T13:01:00Z")
        off = {"kind": "genome", "treatments": ["other"], "forced": ["other"]}
        on = {**off, "treatments": [GENE, "other"], "forced": [GENE, "other"]}
        changed = {**on, "treatments": [GENE, "other", "new"]}
        self.write(d, "why.log", [off, on, changed])
        with (d / "why.log").open("a") as f:
            f.write("ordinary controller explanation\n")
        g = audit_run(d)["genomes"]
        self.assertEqual([r["enabled"] for r in g], [False, True, True])
        self.assertEqual(g[0]["normalized_sha256"], g[1]["normalized_sha256"])
        self.assertNotEqual(g[1]["normalized_sha256"], g[2]["normalized_sha256"])

    def test_earliest_frame_false_and_zero_power_and_unknown_stocks(self):
        d = self.run_dir("civvis-20261102T130100Z", "2026-11-02T13:01:00Z")
        state = {"kind": "state", "turn": 150, "frame": 0, "science": 10,
                 "strategic_resource_income": {"RESOURCE_COAL": 0}, "cities": [
                     {"id": 1, "buildings": ["BUILDING_RESEARCH_LAB"],
                      "power": {"free": 0, "fully_powered": False}}]}
        self.write(d, "events.jsonl", [{**state, "frame": 1, "science": 20}, state])
        r = audit_run(d)["readings"][0]
        self.assertEqual(r["science"], 10)
        self.assertEqual(r["cities"][0]["power"], {"free": 0, "fully_powered": False})
        self.assertIsNone(r["fuel_stocks"]["RESOURCE_COAL"])
        self.assertEqual(r["fuel_income"]["RESOURCE_COAL"], 0)

    def test_numeric_deal_reply_and_sent_acceptance_are_raw_evidence(self):
        d = self.run_dir("civvis-20261102T130100Z", "2026-11-02T13:01:00Z")
        rows = [{"kind": "deal_offer", "want": "RESOURCE_COAL=20", "turn": 152},
                {"kind": "deal_response", "direction": "buy", "theirs": "RESOURCES:44=20x0",
                 "closable": True, "ours": "GOLD:0=5x30", "pay": 125},
                {"kind": "deal_closed", "direction": "buy", "sent": True}]
        self.write(d, "events.jsonl", rows)
        self.assertEqual([r["raw"] for r in audit_run(d)["buy_deal_events"]], rows)

    def test_partial_live_tail_stays_visible_without_inventing_a_row(self):
        p = self.root / "events.jsonl"
        p.write_bytes(b'{"kind":"state","turn":1}\n{"kind":')
        sources = {}
        rows = list(records(p, sources))
        self.assertEqual(len(rows), 1)
        self.assertTrue(sources[p.name]["partial_tail"])


if __name__ == "__main__":
    unittest.main()
