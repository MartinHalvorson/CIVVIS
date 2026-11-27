#!/usr/bin/env python3
"""`live_status.py` counts the live ledger the way the site does, on a fixture.

Hermetic: every test reads a ledger written here, never the `ledger` branch.
The one claim this tool exists to make true is that the number an agent reads
from it is the number the Tournaments page shows, so the counting rule is held
to `beta/home_rankings.tournament_series` itself rather than restated.
"""

from __future__ import annotations

import contextlib
import gzip
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "beta"))

import live_status  # noqa: E402

DEITY = "DIFFICULTY_DEITY"
PRINCE = "DIFFICULTY_PRINCE"


def run(tag, outcome, *, utc, difficulty=DEITY, victory="", turns=180, score=500, rival=1000,
        civ="CIVILIZATION_ROME", players=6, map_="Continents.lua", seconds=None, game=None):
    row = {"tag": tag, "game": game or tag, "outcome": outcome, "utc": utc, "difficulty": difficulty,
           "victory": victory, "turns": turns, "score": score, "rival_best": rival, "civ": civ,
           "players": players, "map": map_, "target": "religious", "reason": "stopped"}
    if seconds is not None:
        row["seconds"] = seconds
    return row


#: Eleven launches, eight games on the benchmark setup:
#: - t8 A: an unfinished first run, then a `-cont1` that WON: one game, a win.
#: - t8 B: a rival Science victory at t186.
#: - t8 C: we were eliminated at t90.
#: - t8 D: conceded by the harness (abandoned): a loss.
#: - t8 E: every run unfinished: not a game.
#: - t8 F: a rival Culture victory at t170, on the next day.
#: - t7 G: a Prince win (Religious).
#: - t7 H: two CIVVIS civs at one table: outside the setup.
#: - t7 I: 4P Pangaea: outside the setup.
#: - old J: a Deity loss ten days earlier: outside the default window.
ROWS = [
    run("civvis-t8-20261126T000000Z", "unfinished", utc="2026-11-26T00:10:00Z", seconds=600),
    run("civvis-t8-20261126T000000Z-cont1", "win", utc="2026-11-26T01:00:00Z", victory="VICTORY_RELIGIOUS",
        turns=170, score=600, rival=1000, seconds=3000, game="civvis-t8-20261126T000000Z-cont1"),
    run("civvis-t8-20261126T010000Z", "rival_victory", utc="2026-11-26T02:00:00Z", victory="VICTORY_TECHNOLOGY",
        turns=186, score=400, rival=1000, seconds=3600),
    run("civvis-t8-20261126T020000Z", "defeat", utc="2026-11-26T03:00:00Z", turns=90, score=100, rival=1000,
        seconds=1800),
    run("civvis-t8-20261126T030000Z", "abandoned", utc="2026-11-26T04:00:00Z", turns=150, score=300, rival=1000),
    run("civvis-t8-20261126T040000Z", "unfinished", utc="2026-11-26T04:05:00Z"),
    run("civvis-t8-20261127T000000Z", "rival_victory", utc="2026-11-27T01:00:00Z", victory="VICTORY_CULTURE",
        turns=170, score=200, rival=1000),
    run("civvis-t7-20261126T050000Z", "win", utc="2026-11-26T06:00:00Z", difficulty=PRINCE,
        victory="VICTORY_RELIGIOUS", turns=160),
    run("civvis-t7-20261126T060000Z", "win", utc="2026-11-26T07:00:00Z", civ="CIVILIZATION_ROME+CIVILIZATION_KOREA"),
    run("civvis-t7-20261126T070000Z", "win", utc="2026-11-26T08:00:00Z", players=4, map_="Pangaea.lua"),
    run("civvis-20261116T000000Z", "rival_victory", utc="2026-11-16T03:00:00Z", victory="VICTORY_TECHNOLOGY"),
]


class CountingIsTheSites(unittest.TestCase):
    def test_a_continuation_is_the_same_game_and_its_best_run_counts(self):
        counted = {game["key"]: game["row"]["outcome"] for game in live_status.games(ROWS)}
        self.assertEqual(counted["civvis-t8-20261126T000000Z"], "win")
        self.assertNotIn("civvis-t8-20261126T000000Z-cont1", counted)

    def test_a_launch_that_never_finished_is_not_a_game(self):
        keys = {game["key"] for game in live_status.games(ROWS)}
        self.assertNotIn("civvis-t8-20261126T040000Z", keys)

    def test_the_outcome_order_is_home_rankings(self):
        import home_rankings
        self.assertEqual(live_status.OUTCOMES, home_rankings.OUTCOMES)
        self.assertEqual(live_status.RUNGS, home_rankings.CIV6_RUNGS)
        for key, name in live_status.VICTORIES.items():
            self.assertEqual(home_rankings.VICTORIES[key], name)

    def test_win_and_loss_counts_equal_the_tournament_series(self):
        """The tool and the Tournaments page count one tournament the same."""
        import home_rankings
        spec = {"name": "t8", "start": "civvis-t8-", "end": "civvis-t9-", "series_size": 100,
                "match": {"difficulty": DEITY, "players": 6, "map": "Continents.lua"}}
        series = home_rankings.tournament_series(ROWS, [spec])
        state = live_status.status(ROWS, since=None, tags=["civvis-t8-"], genes_section=None)
        deity = next(rec for rec in state["levels"] if rec["level"] == 8)
        self.assertEqual((deity["wins"], deity["games"]),
                         (series[0]["wins"], series[0]["wins"] + series[0]["losses"]))
        self.assertEqual((deity["wins"], deity["games"]), (1, 5))


class Window(unittest.TestCase):
    def test_the_default_window_is_the_ledgers_last_seven_days(self):
        state = live_status.status(ROWS, genes_section=None)
        self.assertEqual(state["window"]["since"], "2026-11-21")
        self.assertNotIn("civvis-", state["focus"]["by_family"])

    def test_since_all_counts_every_game(self):
        state = live_status.status(ROWS, since=None, genes_section=None)
        self.assertEqual(state["focus"]["record"]["games"], 6)

    def test_only_the_benchmark_setup_counts_unless_asked(self):
        state = live_status.status(ROWS, since="2026-11-26", genes_section=None)
        self.assertEqual(state["counting"]["outside_setup"], 2)
        self.assertEqual(state["counting"]["launches_never_finished"], 1)
        wide = live_status.status(ROWS, since="2026-11-26", any_setup=True, genes_section=None)
        self.assertEqual(sum(rec["games"] for rec in wide["levels"]),
                         sum(rec["games"] for rec in state["levels"]) + 2)


class TheDeityDetail(unittest.TestCase):
    def setUp(self):
        self.focus = live_status.status(ROWS, since="2026-11-26", genes_section=None)["focus"]

    def test_record_par_and_interval(self):
        record = self.focus["record"]
        self.assertEqual((record["wins"], record["games"]), (1, 5))
        self.assertAlmostEqual(record["par"], 1 / 6, places=4)
        low, high = record["interval"]
        self.assertLess(low, 0.2)
        self.assertGreater(high, 0.2)
        self.assertEqual(record["verdict"], "par within interval")

    def test_losses_by_cause(self):
        self.assertEqual(dict(map(tuple, self.focus["losses"])),
                         {"rival Science": 1, "rival Culture": 1, "eliminated": 1, "conceded": 1})

    def test_by_day_uses_the_day_the_game_ended(self):
        self.assertEqual([(day["day"], day["wins"], day["games"]) for day in self.focus["by_day"]],
                         [("2026-11-26", 1, 4), ("2026-11-27", 0, 1)])

    def test_turns_and_score_ratios(self):
        self.assertEqual(self.focus["turns"]["rival Science wins"]["p50"], 186)
        self.assertEqual(self.focus["turns"]["our eliminations"]["p50"], 90)
        self.assertEqual(self.focus["turns"]["our Religious wins"]["n"], 1)
        self.assertAlmostEqual(self.focus["score_ratio"]["wins"]["p50"], 0.6)
        self.assertAlmostEqual(self.focus["score_ratio"]["losses"]["p50"], 0.25)

    def test_diplomatic_split_when_the_settings_are_read(self):
        settings = {"civvis-t8-20261126T000000Z": False, "civvis-t8-20261126T010000Z": False,
                    "civvis-t8-20261126T020000Z": True}
        focus = live_status.status(ROWS, since="2026-11-26", victory_settings=settings,
                                   genes_section=None)["focus"]
        self.assertEqual((focus["diplomatic"]["off"]["wins"], focus["diplomatic"]["off"]["games"]), (1, 2))
        self.assertEqual(focus["diplomatic"]["on"]["games"], 1)
        self.assertEqual(focus["diplomatic"]["unknown"], 2)


class Helpers(unittest.TestCase):
    def test_percentiles_interpolate(self):
        self.assertEqual(live_status.percentiles([1, 2, 3, 4, 5])["p50"], 3)
        self.assertEqual(live_status.percentiles([10, 20])["p25"], 12.5)
        self.assertIsNone(live_status.percentiles([]))

    def test_wilson_matches_the_tournaments_page(self):
        import tournaments_page
        for wins, games in ((0, 10), (46, 245), (10, 10)):
            low, high = live_status.wilson(wins, games)
            self.assertEqual([round(100 * low), round(100 * high)], tournaments_page.wilson(wins, games))

    def test_seat_victories_reads_only_the_seat_event(self):
        lines = [{"kind": "loaded"}, {"kind": "seat", "victories": {"diplomatic": False, "technology": True}}]
        blob = gzip.compress("\n".join(json.dumps(line) for line in lines).encode() + b"\n")
        self.assertEqual(live_status.seat_victories(blob), {"diplomatic": False, "technology": True})
        self.assertIsNone(live_status.seat_victories(gzip.compress(b'{"kind": "turn"}\n')))


class Throughput(unittest.TestCase):
    def test_idle_share_is_the_gap_between_timed_launches(self):
        rows = [
            run("civvis-20261126T000000Z", "win", utc="2026-11-26T01:00:00Z", seconds=3600),
            run("civvis-20261126T020000Z", "rival_victory", utc="2026-11-26T03:00:00Z", seconds=3600),
            run("civvis-20261126T030000Z", "unfinished", utc="2026-11-26T03:30:00Z", seconds=1800),
        ]
        out = live_status.throughput(rows, None, None)
        # Busy 00-01, 02-03:30 of a 00:00-03:30 span: idle 1 h of 3.5 h.
        self.assertAlmostEqual(out["idle_share"], 1 / 3.5, places=3)
        self.assertEqual(out["finished_per_day"], [["2026-11-26", 2]])


REGISTRY = """
pub const GENES: &[Gene] = &[
    Gene { tag: "alpha", field: "alpha", kind: Kind::OptIn, enable: AdvancedAi::enable_alpha, disable: AdvancedAi::disable_alpha },
    Gene { tag: "beta", field: "beta", kind: Kind::OptIn, enable: AdvancedAi::enable_beta, disable: AdvancedAi::disable_beta },
    Gene { tag: "gamma", field: "gamma", kind: Kind::OptIn, enable: AdvancedAi::enable_gamma, disable: AdvancedAi::disable_gamma },
    Gene { tag: "delta", field: "delta", kind: Kind::Repair(Axis::Economy), enable: AdvancedAi::enable_delta, disable: AdvancedAi::disable_delta },
    Gene { tag: "host", field: "host", kind: Kind::HostOnly, enable: AdvancedAi::enable_host, disable: AdvancedAi::disable_host },
];
"""


class GeneBacklog(unittest.TestCase):
    def backlog(self, force="beta,delta,alpha\n", played=None):
        ledger = {"rules": {"deployment_genome": ["alpha"]}}
        return live_status.gene_backlog(REGISTRY, ledger, force, played)

    def test_counts(self):
        out = self.backlog()
        self.assertEqual(out["registry_rows"], 5)
        self.assertEqual(out["kinds"], {"OptIn": 3, "Repair": 1, "HostOnly": 1})
        self.assertEqual(out["default_on"], 1)
        self.assertEqual(out["force_file"]["tags"], 3)
        self.assertEqual(out["force_file"]["opt_ins_it_adds"], 1)
        self.assertEqual(out["force_file"]["already_default_on"], 1)
        self.assertEqual(out["force_file"]["held_live_rows_it_restores"], 1)
        # gamma is neither on by default nor forced.
        self.assertEqual(out["opt_in_not_armed"], 1)
        # alpha (ledger), host (host-only), but not delta (a repair the ledger holds off).
        self.assertEqual(out["live_seat_default"], 2)

    def test_what_a_game_played_wins_over_the_file(self):
        out = self.backlog(played={"tag": "civvis-x", "utc": "", "forced": ["gamma", "delta"]})
        self.assertEqual(out["played"]["vs_force_file"], {"only_in_game": ["gamma"], "only_in_file": ["alpha", "beta"]})
        self.assertEqual(out["opt_in_not_armed"], 1)  # beta is not armed once the game says so

    def test_played_genome_skips_summaries_without_the_lists(self):
        with tempfile.TemporaryDirectory() as root:
            runs = Path(root)
            for tag, body in (("civvis-new", {"partial": True}), ("civvis-old", {"forced": ["beta"]})):
                (runs / tag).mkdir()
                (runs / tag / "summary.json").write_text(json.dumps(body))
            rows = [run("civvis-new", "win", utc="2026-11-26T02:00:00Z"),
                    run("civvis-old", "win", utc="2026-11-26T01:00:00Z")]
            played = live_status.played_genome(runs, None, runs, rows)
        self.assertEqual((played["tag"], played["forced"]), ("civvis-old", ["beta"]))


class FromALedgerTree(unittest.TestCase):
    """The ref path `beta/publish.sh` uses: results, run summaries and seat
    events all read out of one commit, as the ledger branch lays them out."""

    def test_results_settings_and_the_played_list_come_from_the_ref(self):
        import subprocess
        with tempfile.TemporaryDirectory() as root:
            repo = Path(root)
            git = ["git", "-C", root, "-c", "user.name=t", "-c", "user.email=noreply@anthropic.com"]
            subprocess.run(git[:3] + ["init", "-q"], check=True)
            (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in ROWS))
            for key, diplomatic in (("civvis-t8-20261126T010000Z", False), ("civvis-t8-20261126T020000Z", True)):
                (repo / "runs" / key).mkdir(parents=True)
                events = [{"kind": "loaded"}, {"kind": "seat", "victories": {"diplomatic": diplomatic}}]
                (repo / "runs" / key / "events.jsonl.gz").write_bytes(
                    gzip.compress("".join(json.dumps(e) + "\n" for e in events).encode()))
            (repo / "runs" / "civvis-t8-20261127T000000Z").mkdir(parents=True)
            (repo / "runs" / "civvis-t8-20261127T000000Z" / "summary.json").write_text(
                json.dumps({"forced": ["beta", "gamma"], "genome_treatments": {"forced": ["gamma"],
                                                                                "treatments": ["a", "b"]}}))
            subprocess.run(git + ["add", "-A"], check=True)
            subprocess.run(git + ["commit", "-q", "-m", "ledger"], check=True)
            args = live_status.argparse.Namespace(ledger=None, ref="HEAD", repo=root, fetch_timeout=5)
            rows, origin = live_status.load(args)
            self.assertEqual(len(rows), len(ROWS))
            keys = [game["key"] for game in live_status.games(rows) if live_status.level(game["row"]["difficulty"]) == 8]
            settings = live_status.diplomatic_settings(repo, origin["ref"], None, keys)
            self.assertEqual(settings, {"civvis-t8-20261126T010000Z": False, "civvis-t8-20261126T020000Z": True})
            played = live_status.played_genome(repo, origin["ref"], None, rows)
            self.assertEqual((played["tag"], played["forced"], played["decider_treatments"]),
                             ("civvis-t8-20261127T000000Z", ["beta", "gamma"], 2))


class Outputs(unittest.TestCase):
    def test_every_output_renders_from_a_file(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "results.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in ROWS) + "not json\n")
            for flag, check in (("--json", lambda out: json.loads(out)["focus"]["record"]["games"] == 6),
                                ("--markdown", lambda out: "| Lv 8 Deity |" in out),
                                (None, lambda out: "Lv 8 Deity: 1/6" in out)):
                buffer = io.StringIO()
                with contextlib.redirect_stdout(buffer):
                    live_status.main(["--ledger", str(path), "--since", "all", "--no-genes"]
                                     + ([flag] if flag else []))
                self.assertTrue(check(buffer.getvalue()), (flag, buffer.getvalue()[:400]))

    def test_the_real_registry_reads(self):
        """The backlog's scrape of the real registry finds rows, so an empty
        answer cannot pass for a backlog that was cleared."""
        out = live_status.gene_backlog(live_status.GENES_RS.read_text(encoding="utf-8"),
                                       json.loads(live_status.GENE_LEDGER.read_text(encoding="utf-8")),
                                       live_status.FORCE_FILE.read_text(encoding="utf-8"))
        self.assertGreater(out["registry_rows"], 100)
        self.assertGreater(out["default_on"], 0)
        self.assertEqual(out["force_file"]["unknown"], [])


if __name__ == "__main__":
    unittest.main()
