#!/usr/bin/env python3
"""The ledger branch round-trips: publish on one clone, pull and list on another."""

from __future__ import annotations

import gzip
import io
import json
import math
import os
import subprocess
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import live_ledger  # noqa: E402

#: A CI runner has no git identity; supplied through the environment so the
#: suite never writes to any repository's config.
PROBE = {
    "GIT_AUTHOR_NAME": "ledger probe",
    "GIT_AUTHOR_EMAIL": "probe@civvis.invalid",
    "GIT_COMMITTER_NAME": "ledger probe",
    "GIT_COMMITTER_EMAIL": "probe@civvis.invalid",
}


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True,
                          text=True, check=True,
                          env={**os.environ, **PROBE}).stdout.strip()


def make_origin_and_clone(root: Path) -> tuple[Path, Path]:
    origin = root / "origin.git"
    subprocess.run(["git", "init", "-q", "--bare", str(origin)], check=True)
    work = root / "work"
    subprocess.run(["git", "init", "-q", str(work)], check=True)
    git(work, "remote", "add", "origin", str(origin))
    return origin, work


def write_run(runs: Path, tag: str, *, score: int, finished: str,
              events: list[dict] | None = None) -> None:
    run = runs / tag
    run.mkdir(parents=True)
    (run / "summary.json").write_text(json.dumps({
        "tag": tag, "finished_utc": finished, "difficulty": "DIFFICULTY_SETTLER",
        "configured": True, "last_turn": 250, "last_score": score,
        "rival_best": score + 100, "orders_seen": 100, "orders_applied": 90,
        "outcome": {"kind": "victory", "team": 4, "local_team": 0, "victory": 0},
        "seat": {"victory_types": [{"index": 0, "type": "VICTORY_SCORE"}]},
        "deals": {"sessions_opened": 3, "sessions_answered": 1,
                  "sessions_unanswered": 2, "closed": 1, "declined": 0,
                  "expired": 1, "peace_accepted": 0, "peace_refused": 2},
    }))
    (run / "events.jsonl").write_text("".join(
        json.dumps(event) + "\n" for event in (events or [{"kind": "seat"}])))


def state(turn, *, science, rival_science, techs, rival_techs, boosted=(), civics=(),
          inspired=(), projects=(), frame=0):
    return {"kind": "state", "turn": turn, "frame": frame, "science": science,
            "techs": [f"TECH_{i}" for i in range(techs)],
            "civics": list(civics), "boosted_techs": list(boosted),
            "boosted_civics": list(inspired), "science_projects": list(projects),
            "rivals": [{"science": rival_science, "techs": rival_techs, "score": 100}]}


def write_game_segment(runs: Path, tag: str, *, finished: str, last_turn: int,
                       events: list[dict], won: bool = False, abandoned: bool = False,
                       cities_at_60=None, combat=None, screen=None, forced=None,
                       withheld=None, difficulty="DIFFICULTY_EMPEROR") -> None:
    run = runs / tag
    run.mkdir(parents=True)
    body = {
        "tag": tag, "finished_utc": finished, "difficulty": difficulty,
        "configured": True, "last_turn": last_turn, "last_score": 500,
        "rival_best": 900, "victory_target": "science",
        "outcome": ({"kind": "victory", "won": True, "victory": 5, "team": 0,
                     "local_team": 0} if won else None),
        "seat": {"victory_types": [{"index": 5, "type": "VICTORY_TECHNOLOGY"}]},
        "abandoned": ({"rule": "below_leader_score", "turn": 150} if abandoned else None),
        "reason": "abandoned" if abandoned else ("stopped" if won else "operator_retired"),
        "cities_at_60": cities_at_60, "combat": combat,
        "forced": forced, "withheld": withheld,
    }
    if screen is not None:
        body["screen_gene"], body["screen_arm"] = screen
    (run / "summary.json").write_text(json.dumps(body))
    (run / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))


class GamesNotRows(unittest.TestCase):
    """`kpis` and `screen` read the ledger as games: a `-contN` row is a
    segment joined back to its stem, every KPI is read off the joined game,
    and the arm comes from the dealt `screen_arm` first."""

    def make_ledger(self, root: Path) -> Path:
        runs = root / "runs"
        combat = {"kills": 10, "losses": 20}
        # Game A: a stem that froze at t120 and a continuation that won.
        write_game_segment(runs, "civvis-a", finished="2026-09-01T10:00:00Z",
                           last_turn=120, cities_at_60=5, combat=combat,
                           screen=("live-move-refusal-break", "on"),
                           events=[
                               state(1, science=5, rival_science=6, techs=1, rival_techs=1),
                               state(100, science=80, rival_science=160, techs=30,
                                     rival_techs=40, boosted=["TECH_1", "TECH_2"],
                                     civics=["C1", "C2"], inspired=["C1"]),
                               # A second frame of t100 must not overwrite the first.
                               state(100, science=999, rival_science=1, techs=30,
                                     rival_techs=40, frame=1),
                               state(120, science=100, rival_science=180, techs=34,
                                     rival_techs=44, civics=["C1", "C2"]),
                           ])
        write_game_segment(runs, "civvis-a-cont1", finished="2026-09-01T12:00:00Z",
                           last_turn=230, won=True, combat=combat,
                           screen=("live-move-refusal-break", "on"),
                           events=[
                               state(150, science=200, rival_science=250, techs=45,
                                     rival_techs=60, boosted=["TECH_40"], civics=["C1", "C2", "C3"]),
                               {"kind": "order_verified", "order_kind": "produce",
                                "turn": 170, "verb": "PROJECT_LAUNCH_EARTH_SATELLITE"},
                               state(200, science=300, rival_science=400, techs=70,
                                     rival_techs=75, civics=["C1", "C2", "C3"],
                                     projects=["PROJECT_LAUNCH_EARTH_SATELLITE"]),
                               state(230, science=320, rival_science=420, techs=77,
                                     rival_techs=77, civics=["C1", "C2", "C3"],
                                     projects=["PROJECT_LAUNCH_EARTH_SATELLITE",
                                               "PROJECT_LAUNCH_MOON_LANDING"]),
                           ])
        # Game B: the off arm, abandoned at t150.
        write_game_segment(runs, "civvis-b", finished="2026-09-01T14:00:00Z",
                           last_turn=150, abandoned=True, cities_at_60=2,
                           combat={"kills": 5, "losses": 25},
                           screen=("live-move-refusal-break", "off"),
                           events=[state(100, science=40, rival_science=160, techs=20,
                                         rival_techs=40),
                                   state(150, science=90, rival_science=250, techs=34,
                                         rival_techs=60)])
        # Game C: a batch-wide forced arm of another gene, no screen: on for
        # that gene, unassigned for the screened one.
        write_game_segment(runs, "civvis-c", finished="2026-09-01T16:00:00Z",
                           last_turn=150, abandoned=True, cities_at_60=3,
                           combat={"kills": 8, "losses": 8}, forced=["raid-pillage-prizes"],
                           events=[state(150, science=100, rival_science=200, techs=30,
                                         rival_techs=50)])
        # Game D: a continuation whose stem never reached the ledger.
        write_game_segment(runs, "civvis-d-cont2", finished="2026-09-01T18:00:00Z",
                           last_turn=210, combat={"kills": 1, "losses": 1},
                           events=[state(205, science=1, rival_science=2, techs=1,
                                         rival_techs=2)])
        return runs

    def test_segments_join_into_one_game_and_every_kpi_reads_off_it(self):
        with TemporaryDirectory() as tmp:
            runs = self.make_ledger(Path(tmp))
            rows = live_ledger.games(runs)
            self.assertEqual([g["tag"] for g in rows], ["civvis-a", "civvis-b", "civvis-c", "civvis-d"])
            a = rows[0]
            self.assertEqual(a["segments"], ["civvis-a", "civvis-a-cont1"])
            self.assertTrue(a["stem_present"])
            self.assertTrue(a["won"])
            self.assertEqual(a["last_turn"], 230)
            self.assertTrue(a["reached_t200"])
            self.assertFalse(a["abandoned_at_150"])
            self.assertEqual(a["cities_at_60"], 5)           # from the stem
            self.assertEqual((a["kills"], a["losses"]), (20, 40))  # summed
            self.assertAlmostEqual(a["kills_per_loss"], 0.5)
            self.assertAlmostEqual(a["losses_per_100_turns"], 40 / 230 * 100)
            self.assertAlmostEqual(a["science_ratio_t100"], 0.5)   # first frame, not 999
            self.assertAlmostEqual(a["science_ratio_t150"], 0.8)   # from the continuation
            self.assertAlmostEqual(a["tech_ratio_t150"], 0.75)
            # Boosts accumulate across segments and are read against the final tree:
            # TECH_1, TECH_2 (stem) and TECH_40 (continuation) of 77 techs.
            self.assertAlmostEqual(a["techs_boosted_share"], 3 / 77)
            self.assertAlmostEqual(a["civics_inspired_share"], 1 / 3)
            self.assertEqual(a["launch_earth"], 200)   # completion, not the order at 170
            self.assertEqual(a["launch_moon"], 230)
            self.assertIsNone(a["launch_mars"])
            d = rows[3]
            self.assertFalse(d["stem_present"])
            self.assertIsNone(d["cities_at_60"])
            self.assertIsNone(d["science_ratio_t100"])
            self.assertEqual(live_ledger.game_stem("civvis-d-cont2"), "civvis-d")
            self.assertEqual(live_ledger.segment_index("civvis-d-cont2"), 2)
            self.assertEqual(live_ledger.segment_index("civvis-d"), 0)
            # Filters.
            self.assertEqual(len(live_ledger.games(runs, last=2)), 2)
            self.assertEqual(len(live_ledger.games(runs, since="2026-09-01T15:00:00Z")), 2)
            self.assertEqual(len(live_ledger.games(runs, difficulty="Emperor")), 4)
            self.assertEqual(len(live_ledger.games(runs, difficulty="DIFFICULTY_KING")), 0)
            self.assertEqual(len(live_ledger.games(runs, lane="culture")), 0)

    def test_the_arm_is_the_dealt_one_then_the_batch_words_then_unassigned(self):
        with TemporaryDirectory() as tmp:
            runs = self.make_ledger(Path(tmp))
            rows = {g["tag"]: g for g in live_ledger.games(runs)}
            arm = live_ledger.arm_of
            self.assertEqual(arm(rows["civvis-a"], "live-move-refusal-break"), "on")
            self.assertEqual(arm(rows["civvis-b"], "live-move-refusal-break"), "off")
            self.assertIsNone(arm(rows["civvis-c"], "live-move-refusal-break"))
            self.assertEqual(arm(rows["civvis-c"], "raid-pillage-prizes"), "on")
            self.assertIsNone(arm(rows["civvis-d"], "live-move-refusal-break"))
            report = live_ledger.screen(list(rows.values()), "live-move-refusal-break")
            self.assertEqual((report["on"], report["off"], report["unassigned"]), (1, 1, 2))
            self.assertEqual(report["segment_only"], 1)
            won = next(k for k in report["kpis"] if k["key"] == "won")
            self.assertEqual((won["a"]["k"], won["a"]["n"], won["b"]["k"], won["b"]["n"]),
                             (1, 1, 0, 1))
            self.assertAlmostEqual(won["diff"], 1.0)
            cities = next(k for k in report["kpis"] if k["key"] == "cities_at_60")
            self.assertAlmostEqual(cities["diff"], 3.0)
            self.assertIsNone(cities["lo"], "one game per arm has no interval")
            text = live_ledger.render_screen(report)
            self.assertIn("live default on, other arm --civvis-without", text)
            self.assertIn("on 1, off 1, unassigned 2", text)
            self.assertIn("cities at t60", text)
            table = live_ledger.kpis_table(list(rows.values()), "live-move-refusal-break")
            self.assertIn("civvis-d*", table)
            self.assertIn("WON", table)
            self.assertIn("aband", table)
            buffer = io.StringIO()
            with redirect_stdout(buffer):
                self.assertEqual(live_ledger.main(
                    ["screen", "live-move-refusal-break", "--runs", str(runs), "--json"]), 0)
            self.assertEqual(json.loads(buffer.getvalue())["on"], 1)
            buffer = io.StringIO()
            with redirect_stdout(buffer):
                self.assertEqual(live_ledger.main(["kpis", "--runs", str(runs), "--last", "2"]), 0)
            self.assertIn("civvis-d*", buffer.getvalue())
            self.assertNotIn("civvis-a ", buffer.getvalue())
            self.assertEqual(live_ledger.main(["screen", "no-such-gene", "--runs", str(runs)]), 2)


class Intervals(unittest.TestCase):
    def test_the_t_quantile_matches_the_tables(self):
        for df, expected in ((1, 12.706), (5, 2.571), (10, 2.228), (30, 2.042), (100, 1.984)):
            self.assertAlmostEqual(live_ledger.t_quantile(df), expected, places=2, msg=df)
        self.assertAlmostEqual(live_ledger.t_quantile(1000), 1.96, places=2)
        self.assertAlmostEqual(live_ledger.student_t_cdf(0.0, 7), 0.5)

    def test_welch_and_wilson(self):
        w = live_ledger.welch([10.0, 12.0, 11.0, 13.0], [8.0, 9.0, 7.0, 8.0])
        self.assertAlmostEqual(w["diff"], 3.5)
        self.assertLess(w["lo"], 3.5)
        self.assertGreater(w["hi"], 3.5)
        self.assertGreater(w["lo"], 0.0, "four clean games a side separate at 95 %")
        self.assertIsInstance(w["n80"], int)
        self.assertGreaterEqual(w["n80"], 2)
        same = live_ledger.welch([1.0, 1.0, 1.0], [1.0, 1.0, 1.0])
        self.assertEqual(same["diff"], 0.0)
        self.assertIsNone(same["n80"])
        self.assertIsNone(live_ledger.welch([], [1.0])["diff"])
        wi = live_ledger.wilson(5, 10)
        self.assertAlmostEqual(wi["rate"], 0.5)
        self.assertAlmostEqual(wi["lo"], 0.2366, places=3)
        self.assertAlmostEqual(wi["hi"], 0.7634, places=3)
        self.assertIsNone(live_ledger.wilson(0, 0)["rate"])
        rd = live_ledger.rate_difference(8, 10, 2, 10)
        self.assertAlmostEqual(rd["diff"], 0.6)
        self.assertGreater(rd["lo"], 0.0)
        self.assertIsInstance(rd["n80"], int)
        mi = live_ledger.mean_interval([2.0, 4.0])
        self.assertAlmostEqual(mi["mean"], 3.0)
        self.assertAlmostEqual(mi["hi"] - mi["mean"], 12.706 * math.sqrt(2) / math.sqrt(2), places=1)


class RoundTrip(unittest.TestCase):
    def test_publish_pull_and_list(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            origin, work = make_origin_and_clone(root)
            reader = root / "reader"
            subprocess.run(["git", "init", "-q", str(reader)], check=True)
            git(reader, "remote", "add", "origin", str(origin))
            runs = root / "runs"
            write_run(runs, "civvis-a", score=500, finished="2026-08-20T10:00:00Z")
            write_run(runs, "civvis-b", score=700, finished="2026-08-21T10:00:00Z")
            for tag in ("civvis-a", "civvis-b"):
                self.assertEqual(civ6_ladder.publish_run(
                    tag, runs, repo=work, env=PROBE), "published")

            cache = root / "cache"
            fresh = live_ledger.pull(cache, repo=reader, env=PROBE)
            self.assertEqual(sorted(fresh), ["civvis-a", "civvis-b"])
            self.assertFalse((reader / "runs").exists(), "pull must not check out")
            with gzip.open(cache / "runs" / "civvis-b" / "events.jsonl.gz", "rt") as fh:
                self.assertEqual(json.loads(fh.readline())["kind"], "seat")
            # A second pull copies nothing and leaves the cache intact.
            self.assertEqual(live_ledger.pull(cache, repo=reader, env=PROBE), [])
            self.assertEqual((cache / "TIP").read_text().strip(),
                             git(origin, "rev-parse", "refs/heads/ledger"))

            out = live_ledger.runs_table(cache, last=1)
            self.assertIn("civvis-b", out)
            self.assertNotIn("civvis-a", out)
            self.assertIn("2026-08-21T10:00:00Z", out)
            self.assertIn("Settler", out)
            self.assertIn("700", out)
            self.assertIn("800", out)          # rival_best
            self.assertIn("VICTORY_SCORE", out)
            self.assertIn("90.0%", out)
            self.assertIn("s3/a1/u2 c1 d0 e1 p+0/-2", out)

            buffer = io.StringIO()
            with redirect_stdout(buffer):
                self.assertEqual(live_ledger.main(
                    ["--cache", str(cache), "runs", "--last", "5"]), 0)
            self.assertIn("civvis-a", buffer.getvalue())
            # A live runs directory reads the same way as the cache.
            self.assertIn("civvis-a", live_ledger.runs_table(runs, last=5))

    def test_pull_without_a_ledger_says_so(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            _, work = make_origin_and_clone(root)
            with self.assertRaises(RuntimeError):
                live_ledger.pull(root / "cache", repo=work, env=PROBE)


if __name__ == "__main__":
    unittest.main()
