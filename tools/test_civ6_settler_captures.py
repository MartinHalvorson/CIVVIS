#!/usr/bin/env python3
"""The settler-capture detector: founds are not captures, captures are named.

Every fixture is a synthetic run directory in a temporary sandbox. Nothing
here reads this machine's real run directories, ledger, or halt files.
"""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_settler_captures as captures  # noqa: E402

SETTLER = 524291
GUARD = 655366
SCOUT = 262147
RAIDER = 393220


def unit(uid, kind, x, y, moves=2, combat=0, hp=100, activity="awake"):
    return {"id": uid, "kind": kind, "x": x, "y": y, "moves": moves, "combat": combat,
            "hp": hp, "activity": activity, "embarked": False}


def hostile(uid, kind, x, y, combat=10, moves=3, hp=100, player=63):
    return {"id": uid, "type": kind, "player": player, "x": x, "y": y,
            "combat": combat, "moves": moves, "hp": hp}


def state(turn, units, hostiles=()):
    return {"kind": "state", "turn": turn, "frame": 0, "units": list(units),
            "hostiles": list(hostiles)}


def make_run(root: Path, name: str, events: list[dict], why: list[str] | None = None) -> Path:
    run = root / name
    run.mkdir(parents=True)
    (run / "events.jsonl").write_text(
        "".join(json.dumps(e) + "\n" for e in events))
    if why is not None:
        (run / "why.log").write_text("".join(line + "\n" for line in why))
    return run


def settler_walk(turns, x, y, hostiles_by_turn=None, friends_by_turn=None, moves=2):
    """Three frames of a settler standing at (x, y) with whatever is around it."""
    out = []
    for t in turns:
        units = [unit(SETTLER, "UNIT_SETTLER", x, y, moves=moves)]
        units += (friends_by_turn or {}).get(t, [])
        out.append(state(t, units, (hostiles_by_turn or {}).get(t, [])))
    return out


class GeometryTest(unittest.TestCase):
    def test_odd_r_neighbours_are_one_apart(self):
        # Odd row (y=5): its six neighbours in odd-r offset coordinates.
        centre = (5, 5)
        for neighbour in ((4, 5), (6, 5), (5, 4), (6, 4), (5, 6), (6, 6)):
            self.assertEqual(captures.hex_distance(centre, neighbour), 1, neighbour)
        # Even row (y=4): shifted the other way.
        centre = (5, 4)
        for neighbour in ((4, 4), (6, 4), (4, 3), (5, 3), (4, 5), (5, 5)):
            self.assertEqual(captures.hex_distance(centre, neighbour), 1, neighbour)
        self.assertEqual(captures.hex_distance((5, 5), (5, 5)), 0)
        self.assertEqual(captures.hex_distance((0, 0), (3, 0)), 3)


class DetectionTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_settler_that_founds_a_city_is_not_a_capture(self):
        events = settler_walk((3, 4, 5), 10, 10) + [
            {"kind": "found", "turn": 5, "unit": SETTLER, "x": 10, "y": 10},
            {"kind": "unit_lost", "turn": 5, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "gold": 40},
        ]
        run = make_run(self.root, "civvis-found", events)
        self.assertEqual(captures.detect_captures(run), [])
        row = captures.census_row(run)
        self.assertEqual((row["settlers_lost"], row["founds"], row["captures"]), (1, 1, 0))

    def test_a_settler_lost_beside_a_scout_is_a_barbarian_scout_capture(self):
        # The scout closes from two tiles to one; the settler keeps full moves and
        # does not step away. No journal, so no site: the scout is the verdict.
        scouts = {8: [hostile(SCOUT, "UNIT_SCOUT", 7, 5)],
                  9: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)],
                  10: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)]}
        events = settler_walk((8, 9, 10), 5, 5, scouts) + [
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "gold": 12},
        ]
        run = make_run(self.root, "civvis-scout", events)
        found = captures.detect_captures(run)
        self.assertEqual(len(found), 1)
        capture = found[0]
        self.assertEqual(capture["method"], "unit_lost_without_found")
        self.assertEqual(capture["mechanism"], "barbarian-scout")
        self.assertIn("held-beside-raider", capture["mechanisms"])
        self.assertEqual(capture["nearest_hostile"]["type"], "UNIT_SCOUT")
        self.assertEqual(capture["nearest_hostile"]["distance"], 1)
        self.assertEqual(capture["pos"], [5, 5])
        self.assertEqual([f["turn"] for f in capture["frames"]], [8, 9, 10])
        self.assertIsNone(capture["guard"])
        self.assertIsNone(capture["captor"])
        markdown = captures.render_markdown(run.name, found)
        self.assertIn("`barbarian-scout`", markdown)
        self.assertIn("UNIT_SCOUT", markdown)

    def test_a_settler_that_was_never_told_to_move_is_named_not_unclassified(self):
        # ⚠⚠⚠ The capture where nothing went wrong tactically: the settler simply
        # was not asked to move. Every other mechanism describes a settler doing
        # something — walking into a nest, holding beside a raider, fleeing into
        # reach — so this one used to land as `unclassified`, and the operator's
        # rule that every capture gets a forensic got no answer.
        #
        # Measured 2026-08-29, run civvis-20260829T120711Z, settler 1441803 at t86:
        # three turns holding at full movement, a Warrior closing from d=2 to d=1,
        # and NO order for the settler in the whole window, while the journal still
        # said "Settler marching to (-4,19)". Shaped here the same way.
        #
        # ⚠ It is deliberately none of the others: a hostile is in view at t-1 so
        # it is not `alone-in-fog`, that hostile is at d=2 so it is not
        # `held-beside-raider`, nothing is stacked so it is not `weak-guard`, and
        # no journal means no site so it is not `site-in-barbarian-nest`.
        approach = {8: [], 9: [hostile(RAIDER, "UNIT_WARRIOR", 7, 5)],
                    10: [hostile(RAIDER, "UNIT_WARRIOR", 6, 5)]}
        events = settler_walk((8, 9, 10), 5, 5, approach) + [
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "gold": 12},
        ]
        run = make_run(self.root, "civvis-stranded", events)
        found = captures.detect_captures(run)
        self.assertEqual(len(found), 1)
        capture = found[0]
        self.assertEqual(capture["mechanism"], "stranded-without-orders")
        self.assertEqual(capture["orders"], [])
        markdown = captures.render_markdown(run.name, found)
        self.assertIn("`stranded-without-orders`", markdown)

    def test_a_settler_that_was_ordered_and_refused_keeps_its_own_name(self):
        # Non-vacuity for the mechanism above: the same three frames, but the
        # settler WAS told to move and did not. That is a different fault and must
        # not be swallowed by the stranded name.
        approach = {8: [], 9: [hostile(RAIDER, "UNIT_WARRIOR", 7, 5)],
                    10: [hostile(RAIDER, "UNIT_WARRIOR", 6, 5)]}
        events = settler_walk((8, 9, 10), 5, 5, approach) + [
            {"kind": "order_failed", "turn": 9, "subject": SETTLER, "order_kind": "unit",
             "verb": "MOVE_TO", "reason": "did_not_move"},
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "gold": 12},
        ]
        run = make_run(self.root, "civvis-ordered", events)
        found = captures.detect_captures(run)
        self.assertEqual(len(found), 1)
        self.assertNotIn("stranded-without-orders", found[0]["mechanisms"])

    def test_the_mods_unit_captured_event_is_the_precise_path(self):
        events = settler_walk((8, 9, 10), 5, 5) + [
            {"kind": "unit_captured", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "owner": 0, "captor": 63, "captor_is_barbarian": True},
            # The same removal still reaches `unit_lost`; it must not count twice.
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER",
             "gold": 12},
        ]
        run = make_run(self.root, "civvis-precise", events)
        found = captures.detect_captures(run)
        self.assertEqual(len(found), 1)
        self.assertEqual(found[0]["method"], "unit_captured")
        self.assertEqual(found[0]["captor"], 63)
        self.assertTrue(found[0]["captor_is_barbarian"])
        self.assertEqual(found[0]["mechanism"], "alone-in-fog")
        self.assertEqual(captures.census_row(run)["precise"], 1)
        self.assertIn("captor player 63 (barbarian)", captures.render_markdown(run.name, found))

    def test_a_captured_non_settler_is_not_a_settler_capture(self):
        events = [
            state(10, [unit(77, "UNIT_BUILDER", 5, 5)]),
            {"kind": "unit_captured", "turn": 10, "unit": 77, "unit_kind": "UNIT_BUILDER",
             "owner": 0, "captor": 63, "captor_is_barbarian": True},
            {"kind": "unit_lost", "turn": 10, "unit": 77, "unit_kind": "UNIT_BUILDER"},
        ]
        run = make_run(self.root, "civvis-builder", events)
        self.assertEqual(captures.detect_captures(run), [])

    def test_a_rivals_defeat_does_not_hide_a_capture_but_our_own_end_does(self):
        base = settler_walk((8, 9, 10), 5, 5) + [
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
        ]
        rival = make_run(self.root, "civvis-rival-defeat", [
            {"kind": "defeat", "turn": 9, "player": 12, "local_player": 0, "ours": False},
        ] + base)
        self.assertEqual(len(captures.detect_captures(rival)), 1)
        ours = make_run(self.root, "civvis-our-defeat", [
            {"kind": "defeat", "turn": 10, "player": 0, "local_player": 0, "ours": True},
        ] + base)
        self.assertEqual(captures.detect_captures(ours), [])

    def test_the_named_escort_is_the_guard_and_a_wounded_stacked_guard_is_weak(self):
        warrior = unit(GUARD, "UNIT_WARRIOR", 5, 5, moves=0, combat=20, hp=30)
        raider = hostile(9001, "UNIT_SPEARMAN", 6, 5, combat=25, moves=2)
        events = settler_walk((8, 9, 10), 5, 5, {9: [raider], 10: [raider]},
                              {8: [warrior], 9: [warrior]}) + [
            {"kind": "escort_cap_synced", "turn": 8, "settler": SETTLER, "guard": GUARD,
             "sent": [5, 5], "want": [5, 5]},
            {"kind": "order_verified", "turn": 9, "subject": GUARD, "verb": "FORTIFY"},
            {"kind": "order_verified", "turn": 9, "subject": SETTLER, "verb": "MOVE_TO"},
            {"kind": "order_verified", "turn": 9, "subject": 4242, "verb": "MOVE_TO"},
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
        ]
        run = make_run(self.root, "civvis-guard", events)
        capture = captures.detect_captures(run)[0]
        self.assertEqual(capture["guard"]["id"], GUARD)
        self.assertEqual(capture["guard"]["named_by"], "escort_cap_synced")
        self.assertEqual(capture["guard"]["hp"], 30)
        self.assertEqual(capture["guard"]["distance"], 0)
        self.assertEqual(capture["mechanism"], "weak-guard")
        subjects = {o.get("subject") or o.get("settler") for o in capture["orders"]}
        self.assertEqual(subjects, {GUARD, SETTLER})

    def test_a_nearby_guard_replaces_a_stale_named_escort(self):
        nearby_guard = unit(9003, "UNIT_WARRIOR", 6, 5, moves=0, combat=20, hp=100)
        stale_guard = unit(GUARD, "UNIT_ARCHER", 10, 5, moves=0, combat=15, hp=81)
        events = [
            state(8, [unit(SETTLER, "UNIT_SETTLER", 5, 5), stale_guard]),
            state(9, [unit(SETTLER, "UNIT_SETTLER", 5, 5), stale_guard]),
            state(10, [unit(SETTLER, "UNIT_SETTLER", 5, 5), stale_guard, nearby_guard]),
            {"kind": "escort_cap_synced", "turn": 8, "settler": SETTLER, "guard": GUARD},
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
        ]
        run = make_run(self.root, "civvis-replaced-guard", events)
        capture = captures.detect_captures(run)[0]
        self.assertEqual(capture["guard"]["id"], 9003)
        self.assertEqual(capture["guard"]["named_by"], "proximity")
        self.assertEqual(capture["guard"]["distance"], 1)
        self.assertEqual(capture["guard"]["assigned_guard"]["id"], GUARD)

    def test_a_site_beside_a_hostile_seen_this_week_is_a_nest(self):
        # The journal names the target; a spearman stood two tiles from it
        # five turns ago and has not been seen since.
        events = settler_walk((3, 4, 5), 10, 10) + [
            state(0, [unit(SETTLER, "UNIT_SETTLER", 8, 10)],
                  [hostile(9002, "UNIT_SPEARMAN", 14, 10, combat=25)]),
            {"kind": "unit_lost", "turn": 5, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
        ]
        why = [
            "[why] t4 Expansion/Detail Settler marching to (7, 10) | 2 tiles away, "
            "the site is worth 120.0  [civ6 (12,10) = axial (7,10)]",
            "[why] t4 Expansion/Detail settler flees a barbarian's reach | "
            "(12,10) is out of reach of the raider",
            "[why] t4 Economy/Decision Rome starts warrior | nothing to do with it",
        ]
        run = make_run(self.root, "civvis-nest", events, why)
        capture = captures.detect_captures(run)[0]
        self.assertEqual(capture["site"], [12, 10])
        self.assertTrue(capture["site_hostile_seen"])
        self.assertEqual(capture["mechanism"], "site-in-barbarian-nest")
        self.assertIn("fled-into-reach", capture["mechanisms"])
        self.assertEqual(len(capture["why"]), 2, "only lines about the settler are quoted")

    def test_a_barbarian_camp_beside_the_site_is_a_nest_too(self):
        events = settler_walk((3, 4, 5), 10, 10) + [
            {"kind": "tiles", "turn": 1, "plots": [
                {"x": 13, "y": 10, "im": "IMPROVEMENT_BARBARIAN_CAMP", "t": "TERRAIN_PLAINS"}]},
            {"kind": "tiles_done", "turn": 1, "plots": 1},   # a count, not a list
            {"kind": "unit_lost", "turn": 5, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
        ]
        why = ["[why] t5 Expansion/Detail Settler marching to (7, 10) | 2 tiles away  "
               "[civ6 (12,10) = axial (7,10)]"]
        run = make_run(self.root, "civvis-camp", events, why)
        capture = captures.detect_captures(run)[0]
        self.assertTrue(capture["camp_near_site"])
        self.assertEqual(capture["mechanism"], "site-in-barbarian-nest")

    def test_a_run_without_events_is_not_a_run(self):
        (self.root / "civvis-empty").mkdir()
        self.assertEqual(captures.detect_captures(self.root / "civvis-empty"), [])
        self.assertIsNone(captures.census_row(self.root / "civvis-empty"))


class LedgerAndCliTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        scouts = {9: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)], 10: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)]}
        other = 99
        self.run = make_run(self.root, "civvis-two", settler_walk((8, 9, 10), 5, 5, scouts) + [
            state(20, [unit(other, "UNIT_SETTLER", 20, 20)]),
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
            {"kind": "unit_lost", "turn": 20, "unit": other, "unit_kind": "UNIT_SETTLER"},
            {"kind": "turn", "turn": 25},
        ])
        make_run(self.root, "civvis-clean", settler_walk((3, 4, 5), 1, 1) + [
            {"kind": "found", "turn": 5, "unit": SETTLER, "x": 1, "y": 1},
            {"kind": "unit_lost", "turn": 5, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
            {"kind": "turn", "turn": 30},
        ])
        (self.root / "not-a-run").mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def test_the_ledger_is_appended_once_per_run_and_unit(self):
        ledger = self.root / "ledger" / "captures.jsonl"
        found = captures.detect_captures(self.run)
        self.assertEqual(captures.append_ledger(ledger, found), 2)
        self.assertEqual(captures.append_ledger(ledger, found), 0)
        rows = [json.loads(l) for l in ledger.read_text().splitlines()]
        self.assertEqual(len(rows), 2)
        self.assertEqual({(r["run"], r["unit"]) for r in rows}, {("civvis-two", SETTLER), ("civvis-two", 99)})
        for key in ("turn", "pos", "mechanism", "method", "nearest_hostile", "guard"):
            self.assertIn(key, rows[0])
        # A second run of the CLI with the same ledger writes nothing new.
        err = io.StringIO()
        with redirect_stdout(io.StringIO()):
            saved, sys.stderr = sys.stderr, err
            try:
                code = captures.main([str(self.run), "--ledger", str(ledger)])
            finally:
                sys.stderr = saved
        self.assertEqual(code, 0)
        self.assertIn("0 new row(s)", err.getvalue())
        self.assertEqual(len(ledger.read_text().splitlines()), 2)

    def test_the_census_covers_every_run_directory_and_skips_the_rest(self):
        rows = captures.census(self.root)
        self.assertEqual([r["run"] for r in rows], ["civvis-clean", "civvis-two"])
        by_run = {r["run"]: r for r in rows}
        self.assertEqual(by_run["civvis-two"]["captures"], 2)
        self.assertEqual(by_run["civvis-two"]["last_turn"], 25)
        self.assertEqual(by_run["civvis-two"]["settlers_lost"], 2)
        self.assertEqual(by_run["civvis-two"]["mechanisms"][0], "barbarian-scout")
        self.assertEqual(by_run["civvis-clean"]["captures"], 0)
        self.assertEqual(by_run["civvis-clean"]["founds"], 1)
        table = captures.format_census(rows)
        self.assertTrue(table.startswith("| run | last_turn | settlers_lost | founds | captures |"))
        self.assertIn("| civvis-two | 25 | 2 | 0 | 2 |", table)
        self.assertIn("**2**", table.splitlines()[-1])

    def test_the_cli_all_mode_prints_the_table_or_json(self):
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(captures.main(["--all", str(self.root)]), 0)
        self.assertIn("| civvis-two |", out.getvalue())
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(captures.main(["--all", str(self.root), "--json",
                                            "--match", "civvis-t*"]), 0)
        rows = json.loads(out.getvalue())
        self.assertEqual([r["run"] for r in rows], ["civvis-two"])

    def test_the_cli_single_run_prints_json_or_markdown(self):
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(captures.main([str(self.run), "--json"]), 0)
        self.assertEqual(len(json.loads(out.getvalue())), 2)
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(captures.main([str(self.run), "--markdown"]), 0)
        self.assertIn("# Settler captures — civvis-two", out.getvalue())
        self.assertIn("2 capture(s)", out.getvalue())
        with redirect_stdout(io.StringIO()):
            saved, sys.stderr = sys.stderr, io.StringIO()
            try:
                self.assertEqual(captures.main([str(self.root / "not-a-run")]), 2)
            finally:
                sys.stderr = saved


class LadderRowTest(unittest.TestCase):
    """The climb's row and the run finalizer count the same captures the CLI does."""

    def setUp(self):
        import civ6_civvis_climb as climb  # noqa: PLC0415
        self.climb = climb
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.saved_root = climb.RUN_ROOT
        climb.RUN_ROOT = self.root

    def tearDown(self):
        self.climb.RUN_ROOT = self.saved_root
        self.tmp.cleanup()

    def _captured_run(self, name="civvis-x"):
        scouts = {9: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)], 10: [hostile(SCOUT, "UNIT_SCOUT", 6, 5)]}
        return make_run(self.root, name, settler_walk((8, 9, 10), 5, 5, scouts) + [
            {"kind": "unit_lost", "turn": 10, "unit": SETTLER, "unit_kind": "UNIT_SETTLER"},
            {"kind": "turn", "turn": 12, "score": 40},
        ])

    def test_the_row_counts_and_names_the_captures(self):
        self._captured_run()
        with redirect_stdout(io.StringIO()):
            record = self.climb.outcome_of("civvis-x")
        self.assertEqual(record["settlers_captured"], 1)
        self.assertEqual(record["settler_captures"], [{"turn": 10, "mechanism": "barbarian-scout"}])
        self.assertEqual(record["last_turn"], 12)

    def test_a_clean_run_reads_zero_and_a_missing_stream_reads_none(self):
        make_run(self.root, "civvis-clean", [{"kind": "turn", "turn": 3}])
        with redirect_stdout(io.StringIO()):
            record = self.climb.outcome_of("civvis-clean")
        self.assertEqual(record["settlers_captured"], 0)
        self.assertEqual(record["settler_captures"], [])
        (self.root / "civvis-none").mkdir()
        with redirect_stdout(io.StringIO()):
            record = self.climb.outcome_of("civvis-none")
        self.assertIsNone(record["settlers_captured"])

    def test_the_finalizer_writes_the_dossier_only_when_a_settler_was_taken(self):
        self._captured_run()
        make_run(self.root, "civvis-clean", [{"kind": "turn", "turn": 3}])
        with redirect_stdout(io.StringIO()):
            path = self.climb.write_settler_capture_dossiers("civvis-x")
            self.assertIsNone(self.climb.write_settler_capture_dossiers("civvis-clean"))
            self.assertIsNone(self.climb.write_settler_capture_dossiers("civvis-absent"))
        self.assertEqual(path, self.root / "civvis-x" / "settler_captures.md")
        text = path.read_text()
        self.assertIn("# Settler captures — civvis-x", text)
        self.assertIn("`barbarian-scout`", text)
        self.assertFalse((self.root / "civvis-clean" / "settler_captures.md").exists())

    def test_a_detector_failure_costs_neither_the_row_nor_the_loop(self):
        self._captured_run()
        saved = captures.detect_captures

        def boom(run_dir):
            raise RuntimeError("synthetic detector failure")

        captures.detect_captures = boom
        try:
            out = io.StringIO()
            with redirect_stdout(out):
                record = self.climb.outcome_of("civvis-x")
                self.assertIsNone(self.climb.write_settler_capture_dossiers("civvis-x"))
        finally:
            captures.detect_captures = saved
        self.assertEqual(record["last_turn"], 12)
        self.assertIsNone(record["settlers_captured"])
        self.assertIn("synthetic detector failure", out.getvalue())


if __name__ == "__main__":
    unittest.main()
