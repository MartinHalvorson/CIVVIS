import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import civ6_artifact_activation_audit as audit


def city(works=None):
    return {"id": 12, "name": "Museum", "buildings": [audit.MUSEUM],
            "great_works": [] if works is None else works, "producing_hash": 0,
            "buildable": [{"t": audit.DIGGER, "p": 8}]}


def work(slot=0, kind=audit.ARTIFACT):
    return {"object": kind, "building": audit.MUSEUM, "slot": slot, "type": "SAME_TYPE"}


def state(turn=9):
    return {"kind": "state", "turn": turn, "frame": 0, "cities": [city()],
            "units": [{"id": 321, "kind": "UNIT_GREAT_SCIENTIST", "great_person": {
                "individual": "GREAT_PERSON_INDIVIDUAL_MARY_LEAKEY",
                "required_great_work": audit.ARTIFACT, "can_activate": False, "charges": 1}}]}


def write_rows(path, rows):
    path.write_bytes(b"".join((json.dumps(row) + "\n").encode() for row in rows))


class InventoryTests(unittest.TestCase):
    def test_zero_requires_complete_valid_work_exports(self):
        self.assertEqual(audit.artifact_count([city()]), 0)
        self.assertIsNone(audit.artifact_count(None))
        self.assertIsNone(audit.artifact_count([]))
        c = city()
        c.pop("great_works")
        self.assertIsNone(audit.artifact_count([city(), c]))
        self.assertIsNone(audit.artifact_count([city([{"object": "NEW_OBJECT"}])]))

    def test_occupied_slots_are_deduplicated_and_conflicts_are_unknown(self):
        self.assertEqual(audit.artifact_count([city([work(), work()])]), 1)
        self.assertEqual(audit.artifact_count([city([work(), work(1)])]), 2)
        self.assertIsNone(audit.artifact_count([city([work(), work(kind="GREATWORKOBJECT_WRITING")])]))
        bad = work()
        bad.pop("slot")
        self.assertIsNone(audit.artifact_count([city([bad])]))

    def test_native_museum_name_legal_quote_and_idle_zero_are_distinct(self):
        reading = audit.snapshot(state())
        self.assertEqual(reading["museum_cities"][0]["legal_digger_quotes"][0]["p"], 8)
        self.assertTrue(reading["museum_cities"][0]["idle"])
        s = state()
        s["cities"][0].pop("producing_hash")
        s["cities"][0].pop("buildable")
        reading = audit.snapshot(s)
        self.assertIsNone(reading["museum_cities"][0]["idle"])
        self.assertIsNone(reading["museum_cities"][0]["legal_digger_quotes"])
        s["cities"][0]["buildings"] = ["BUILDING_ARCHAEOLOGICAL_MUSEUM"]
        self.assertEqual(audit.snapshot(s)["museum_cities"], [])
        s["cities"][0].pop("buildings")
        self.assertFalse(audit.snapshot(s)["museum_inventory_complete"])

    def test_missing_units_and_activation_status_are_not_zero_or_false(self):
        s = state()
        s.pop("units")
        self.assertIsNone(audit.snapshot(s)["owned_diggers"])
        self.assertIsNone(audit.snapshot(s)["artifact_people"])
        s = state()
        s["units"][0]["great_person"].pop("can_activate")
        self.assertNotIn("can_activate", audit.snapshot(s)["artifact_people"][0]["great_person"])


class AuditTests(unittest.TestCase):
    def test_proposal_and_excavation_verdict_do_not_become_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            s = state()
            write_rows(d / "events.jsonl", [s, {"kind": "order_verified", "verb": "EXCAVATE", "unit": 72}, s])
            write_rows(d / "decisions.jsonl", [{"orders": [{"kind": "produce", "subject": 12, "verb": audit.DIGGER}],
                                                "decision": {"turn": 9, "frame": 0}, "execution_status": "submitted"}])
            write_rows(d / "why.log", [{"kind": "genome", "treatments": [audit.GENE]}])
            before = {p.name: p.read_bytes() for p in d.iterdir()}
            result = audit.audit_run(d)
            self.assertTrue(result["genomes"][0]["enabled"])
            self.assertEqual(len(result["production_proposals"]), 1)
            self.assertEqual(result["production_proposals"][0]["execution_status"], "submitted")
            self.assertEqual(result["state_rows"], 2)
            self.assertEqual(len(result["observations"]), 2)
            self.assertEqual([r["artifact_count"] for r in result["observations"]], [0, 0])
            self.assertTrue(all(r["owned_diggers"] == [] for r in result["observations"]))
            self.assertEqual(len(result["event_evidence"]), 1)
            self.assertEqual(before, {p.name: p.read_bytes() for p in d.iterdir()})
            line = (d / "events.jsonl").read_bytes().splitlines(keepends=True)[0]
            self.assertEqual(result["observations"][0]["raw_line_sha256"], hashlib.sha256(line).hexdigest())

    def test_owned_units_work_gains_and_person_events_remain_separate(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            first = state()
            second = copy.deepcopy(first)
            second["turn"] = 10
            second["units"].append({"id": 77, "kind": audit.DIGGER})
            third = copy.deepcopy(second)
            third["turn"] = 11
            third["cities"][0]["great_works"] = [work()]
            write_rows(d / "events.jsonl", [first, {"kind": "order_verified", "unit": 321, "verb": "ACTIVATE"}, second, third])
            result = audit.audit_run(d)
            self.assertEqual([r["artifact_count"] for r in result["observations"]], [0, 0, 1])
            self.assertEqual(result["observations"][1]["owned_diggers"][0]["id"], 77)
            self.assertEqual(result["event_evidence"][0]["raw"]["verb"], "ACTIVATE")
            self.assertEqual(result["production_proposals"], [])

    def test_partial_tails_and_incomplete_summary_do_not_invent_configuration(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            write_rows(d / "events.jsonl", [state()])
            with (d / "events.jsonl").open("ab") as f:
                f.write(b'{"kind":"state"')
            (d / "summary.json").write_text('{"configured":')
            write_rows(d / "why.log", [{"kind": "genome", "treatments": "false"}])
            result = audit.audit_run(d)
            self.assertEqual(result["state_rows"], 1)
            self.assertTrue(result["sources"]["events.jsonl"]["partial_tail"])
            self.assertFalse(result["sources"]["summary.json"]["complete"])
            self.assertIsNone(result["summary"]["configured"])
            self.assertIsNone(result["genomes"][0]["enabled"])

    def test_cli_changes_only_the_requested_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            run = d / "run"
            run.mkdir()
            write_rows(run / "events.jsonl", [state()])
            before = {p.name: p.read_bytes() for p in run.iterdir()}
            output = d / "audit.json"
            p = subprocess.run([sys.executable, str(Path(audit.__file__)), "--run-dir", str(run), "--out", str(output)], capture_output=True)
            self.assertEqual(p.returncode, 0, p.stderr)
            self.assertEqual(json.loads(output.read_text())["state_rows"], 1)
            self.assertEqual(before, {p.name: p.read_bytes() for p in run.iterdir()})

    def test_missing_state_exports_are_counted_even_without_interesting_rows(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            write_rows(d / "events.jsonl", [{"kind": "state", "turn": 1}])
            result = audit.audit_run(d)
            self.assertEqual(result["observations"], [])
            self.assertEqual(result["unknown_state_fields"], {
                "units": 1, "artifact_inventory": 1, "museum_inventory": 1})

    def test_cli_refuses_to_overwrite_input_run_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            d = Path(temporary)
            source = d / "events.jsonl"
            write_rows(source, [state()])
            before = source.read_bytes()
            p = subprocess.run([sys.executable, str(Path(audit.__file__)), "--run-dir", str(d), "--out", str(source)], capture_output=True)
            self.assertNotEqual(p.returncode, 0)
            self.assertIn(b"outside its input", p.stderr)
            self.assertEqual(source.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
