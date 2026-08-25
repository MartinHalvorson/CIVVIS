#!/usr/bin/env python3
"""Actuation per kind: the table, the floor check, and the ratchet that only rises."""

from __future__ import annotations

import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import live_actuation  # noqa: E402


def orders_event(turn: int, *, by: dict, seen_by: dict | None = None,
                 refusals: dict | None = None, refused_by: dict | None = None) -> dict:
    # A `produce_next` lease is accepted but deferred: in `by`, out of
    # `seen`/`applied`, exactly as the mod counts it.
    deferred = by.get("produce_next", 0)
    applied = sum(by.values()) - deferred
    refused = sum((refusals or {}).values())
    event = {"kind": "orders", "ctx": "agent", "turn": turn, "source": "civvis",
             "seen": applied + refused, "applied": applied, "refused": refused,
             "deferred": deferred,
             "by": by, "refusals": refusals if refusals else []}
    if seen_by is not None:
        event["seen_by"] = seen_by
        event["refused_by"] = refused_by if refused_by else []
    return event


def write_run(runs: Path, tag: str, finished: str, events: list[dict],
              orders: dict | None = None) -> Path:
    run = runs / tag
    run.mkdir(parents=True)
    body = {"tag": tag, "finished_utc": finished, "difficulty": "DIFFICULTY_SETTLER",
            "configured": True}
    if orders is not None:
        body["orders"] = orders
    (run / "summary.json").write_text(json.dumps(body))
    (run / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))
    return run


NEW_FORMAT = [
    orders_event(1, by={"unit": 8, "produce": 1}, seen_by={"unit": 10, "produce": 1},
                 refusals={"unit_gone": 2}, refused_by={"unit": {"unit_gone": 2}}),
    orders_event(2, by={"unit": 5, "produce": 2, "produce_next": 1},
                 seen_by={"unit": 5, "produce": 3, "produce_next": 1},
                 refusals={"no_params_DISTRICT_CAMPUS": 1},
                 refused_by={"produce": {"no_params_DISTRICT_CAMPUS": 1}}),
    {"kind": "turn", "ctx": "agent", "turn": 2, "orders_seen": 9, "orders_applied": 8},
]
OLD_FORMAT = [
    orders_event(1, by={"unit": 4}, refusals={"UPGRADE": 3}),
]


class OrdersByKind(unittest.TestCase):
    def test_new_format_attributes_seen_and_reasons_per_kind(self):
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), "civvis-n", "2026-08-20T10:00:00Z", NEW_FORMAT)
            block = civ6_ladder.orders_by_kind(run / "events.jsonl")
        self.assertEqual(block["*"], {"seen": 19, "applied": 16, "refused": {
            "unit_gone": 2, "no_params_DISTRICT_CAMPUS": 1}})
        self.assertEqual(block["unit"], {"seen": 15, "applied": 13,
                                         "refused": {"unit_gone": 2}})
        self.assertEqual(block["produce"], {"seen": 4, "applied": 3, "refused": {
            "no_params_DISTRICT_CAMPUS": 1}})
        self.assertEqual(block["produce_next"], {"seen": 1, "applied": 1, "refused": {}})
        self.assertNotIn(civ6_ladder.UNATTRIBUTED, block)
        # `orders_totals` keeps its shape and its source (the turn events).
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), "civvis-n", "2026-08-20T10:00:00Z", NEW_FORMAT)
            self.assertEqual(civ6_ladder.orders_totals(run / "events.jsonl"), (9, 8))

    def test_old_format_keeps_refusals_but_cannot_attribute_them(self):
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), "civvis-o", "2026-08-20T10:00:00Z", OLD_FORMAT)
            block = civ6_ladder.orders_by_kind(run / "events.jsonl")
        self.assertEqual(block["unit"], {"seen": 4, "applied": 4, "refused": {}})
        self.assertEqual(block[civ6_ladder.UNATTRIBUTED],
                         {"seen": 3, "applied": 0, "refused": {"UPGRADE": 3}})
        self.assertEqual(block["*"]["refused"], {"UPGRADE": 3})

    def test_no_orders_event_is_none_and_gzip_reads(self):
        import gzip
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), "civvis-e", "2026-08-20T10:00:00Z",
                            [{"kind": "seat"}])
            self.assertIsNone(civ6_ladder.orders_by_kind(run / "events.jsonl"))
            self.assertIsNone(civ6_ladder.orders_by_kind(run / "missing.jsonl"))
            gz = run / "events.jsonl.gz"
            with gzip.open(gz, "wt") as fh:
                for event in NEW_FORMAT:
                    fh.write(json.dumps(event) + "\n")
            self.assertEqual(civ6_ladder.orders_by_kind(gz)["*"]["seen"], 19)


class Tooling(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        self.runs = Path(self.tmp.name) / "runs"
        # Two runs with the summary block, one older run without it (events only).
        for i in range(2):
            run = write_run(self.runs, f"civvis-{i}", f"2026-08-2{i}T10:00:00Z", NEW_FORMAT)
            block = civ6_ladder.orders_by_kind(run / "events.jsonl")
            (run / "summary.json").write_text(json.dumps({
                "tag": f"civvis-{i}", "finished_utc": f"2026-08-2{i}T10:00:00Z",
                "orders": block}))
        write_run(self.runs, "civvis-x", "2026-08-19T10:00:00Z", NEW_FORMAT)

    def tearDown(self):
        self.tmp.cleanup()

    def aggregate(self, last: int = 5) -> dict:
        import live_ledger
        return live_actuation.aggregate(live_ledger.summaries(self.runs, last))

    def test_aggregate_sums_summary_blocks_and_events_alike(self):
        agg = self.aggregate()
        self.assertEqual(agg["*"], {"seen": 57, "applied": 48, "refused": {
            "unit_gone": 6, "no_params_DISTRICT_CAMPUS": 3}})
        self.assertEqual(agg["unit"], {"seen": 45, "applied": 39, "refused": {"unit_gone": 6}})
        self.assertEqual(self.aggregate(last=2)["*"]["seen"], 19 * 2)

    def test_table_lists_kind_rate_and_reasons(self):
        out = live_actuation.table(self.aggregate())
        self.assertIn("unit", out)
        self.assertIn("86.7%", out)
        self.assertIn("unit_gone 6", out)
        self.assertNotIn("unattributed", out)

    def test_check_fails_under_the_floor_and_skips_thin_kinds(self):
        agg = self.aggregate()
        floors = {"unit": 90.0, "produce": 70.0, "*": 80.0}
        problems = live_actuation.check_floors(agg, floors, min_seen=10)
        self.assertEqual(len(problems), 1)
        self.assertIn("unit: applied 86.7% of 45 < floor 90.0%", problems[0])
        self.assertIn("unit_gone 6", problems[0])
        # `produce` has 12 seen: under a higher sample floor it is not judged.
        self.assertEqual(live_actuation.check_floors(agg, {"produce": 100.0}, min_seen=20), [])
        self.assertEqual(live_actuation.check_floors(agg, {"unit": 86.7}, min_seen=10), [])
        # An unknown kind on the floors file is not a failure.
        self.assertEqual(live_actuation.check_floors(agg, {"levy": 50.0}, min_seen=1), [])

    def test_ratchet_only_rises(self):
        agg = self.aggregate()
        floors = live_actuation.ratchet({}, agg, min_seen=10)
        self.assertEqual(floors["unit"], 86.7)
        self.assertEqual(floors["produce"], 75.0)
        self.assertEqual(floors["*"], 84.2)
        self.assertNotIn("produce_next", floors)     # 3 seen < min_seen
        # A better window raises; a worse window is kept where it was.
        raised = live_actuation.ratchet(floors, {"unit": {"seen": 100, "applied": 95,
                                                          "refused": {}}}, min_seen=10)
        self.assertEqual(raised["unit"], 95.0)
        kept = live_actuation.ratchet(raised, {"unit": {"seen": 100, "applied": 50,
                                                        "refused": {}}}, min_seen=10)
        self.assertEqual(kept["unit"], 95.0)
        self.assertEqual(kept["*"], 84.2)

    def test_ratchet_writes_no_per_kind_floor_from_unattributed_runs(self):
        write_run(self.runs, "civvis-old", "2026-08-22T10:00:00Z", OLD_FORMAT)
        agg = self.aggregate()
        self.assertIn(civ6_ladder.UNATTRIBUTED, agg)
        floors = live_actuation.ratchet({"unit": 80.0}, agg, min_seen=1)
        self.assertEqual(floors, {"unit": 80.0, "*": round(100 * 52 / 64, 1)})

    def test_cli_round_trip(self):
        floors = Path(self.tmp.name) / "floors.json"
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(live_actuation.main(
                ["--runs", str(self.runs), "--min-seen", "10", "floors",
                 "--floors", str(floors), "--write"]), 0)
            self.assertEqual(live_actuation.main(
                ["--runs", str(self.runs), "--min-seen", "10", "check",
                 "--floors", str(floors)]), 0)
            self.assertEqual(live_actuation.main(
                ["--runs", str(self.runs), "table"]), 0)
        body = json.loads(floors.read_text())
        self.assertEqual(body["floors"]["unit"], 86.7)
        self.assertEqual(body["window"], 5)
        self.assertIn("civvis-1", body["runs"])
        self.assertIn("floor(s) held", out.getvalue())
        # Lower the data, keep the floors: the check fails.
        body["floors"]["unit"] = 99.0
        floors.write_text(json.dumps(body))
        with redirect_stdout(out):
            self.assertEqual(live_actuation.main(
                ["--runs", str(self.runs), "--min-seen", "10", "check",
                 "--floors", str(floors)]), 1)
        self.assertIn("ACTUATION: unit", out.getvalue())


if __name__ == "__main__":
    unittest.main()
