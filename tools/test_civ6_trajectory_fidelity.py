#!/usr/bin/env python3
"""Tests for the trajectory fidelity ledger.

The tool's whole value is that it refuses to pool two corpora that played
different games, so most of these are about what it declines to compare.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_trajectory_fidelity as fidelity  # noqa: E402


def live_row(difficulty="DIFFICULTY_EMPEROR", **fields):
    row = {
        "difficulty": difficulty,
        "speed": "GAMESPEED_ONLINE",
        "map_size": "MAPSIZE_SMALL",
        "turns": 200,
    }
    row.update(fields)
    return row


def write_live(rows, directory: Path) -> Path:
    path = directory / "ladder.json"
    path.write_text(json.dumps({"attempts": rows}), encoding="utf-8")
    return path


def write_sim(games, directory: Path, difficulty="emperor", handicap=None) -> Path:
    path = directory / "screen.jsonl"
    header = {
        "difficulty": difficulty,
        "speed": "online",
        "width": 74,
        "height": 46,
    }
    if handicap is not None:
        header["handicap"] = handicap
    lines = [json.dumps(header)]
    for index, seats in enumerate(games):
        for seat in seats:
            kind = seat.pop("kind", "game")
            lines.append(json.dumps({"kind": kind, "game": index, **seat}))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


class TheMapSizeTableIsDiscovered(unittest.TestCase):
    def test_the_shipped_sizes_are_read_out_of_setup_rs(self):
        sizes = fidelity.map_sizes()
        self.assertEqual(sizes.get((74, 46)), "small")
        self.assertGreater(len(sizes), 3, "the size table parsed to almost nothing")


class ItRefusesToPoolDifferentGames(unittest.TestCase):
    def test_a_configuration_only_one_corpus_played_is_named_not_dropped(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row("DIFFICULTY_SETTLER", score=100, rival_best=200)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp, "emperor"),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(report["matched_cells"], [], "settler is not emperor")
        self.assertEqual(
            [entry["cell"] for entry in report["live_only"]],
            [["settler", "online", "small", "rivals"]],
        )
        self.assertEqual(
            [entry["cell"] for entry in report["sim_only"]],
            [["emperor", "online", "small", "all"]],
        )

    def test_a_matched_configuration_is_compared(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=200)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp, handicap="rivals"),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(len(report["matched_cells"]), 1)
        standing = report["matched_cells"][0]["subsystems"]["standing"]
        self.assertTrue(standing["available"])
        # The live row is OUR seat against the best rival: 100 of 200.
        self.assertAlmostEqual(standing["live"], 0.5)
        # Every screen seat is a CIVVIS seat, so each one contributes its own
        # standing against the best OTHER seat: 100/200 and 200/100. The median
        # of the two is the typical seat, which is the live row's counterpart.
        self.assertAlmostEqual(standing["sim"], 1.25)
        self.assertAlmostEqual(standing["divergence"], 2.5)

    def test_a_shallow_run_is_not_a_trajectory(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            rows = [live_row(turns=fidelity.MIN_TURNS - 1, score=1, rival_best=1)]
            self.assertEqual(fidelity.live_records(write_live(rows, tmp)), [])


class AMissingFieldIsNamed(unittest.TestCase):
    def test_a_subsystem_one_side_cannot_report_says_which_side(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live(
                    [
                        live_row(
                            score=100,
                            rival_best=200,
                            combat={"cities_taken": 3, "cities_lost": 1},
                        )
                    ],
                    tmp,
                )
            )
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp, handicap="rivals"),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        taken = report["matched_cells"][0]["subsystems"]["cities_taken"]
        self.assertFalse(taken["available"])
        self.assertEqual(taken["why"], "sim", "the live side had the field")


class TheRatchetOnlyTightens(unittest.TestCase):
    def test_divergence_does_not_care_which_side_is_larger(self):
        self.assertAlmostEqual(fidelity.divergence(2.0, 1.0), 2.0)
        self.assertAlmostEqual(fidelity.divergence(1.0, 2.0), 2.0)
        self.assertAlmostEqual(fidelity.divergence(3.0, 3.0), 1.0)

    def test_a_tolerance_belongs_to_one_cell_not_to_every_rung(self):
        """Prince reproduces at 1.01x and Emperor at 1.26x. One number for both
        means the tighter cell sets a bar the looser one never clears."""
        report = {
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "subsystems": {"standing": {"available": True, "divergence": 1.01}},
                },
                {
                    "cell": ["emperor", "online", "small", "rivals"],
                    "subsystems": {"standing": {"available": True, "divergence": 1.26}},
                },
            ]
        }
        worst = fidelity.worst_divergences(report)
        self.assertEqual(len(worst), 2, f"one entry per cell: {worst}")
        self.assertIn("prince/online/small/n/a|standing", worst)
        self.assertIn("emperor/online/small/rivals|standing", worst)
        # Each is judged against its own recorded value, so neither drags the
        # other: both pass here, and the loose one alone fails when it moves.
        status, _ = fidelity.check(
            report,
            {
                "prince/online/small/n/a|standing": 1.01,
                "emperor/online/small/rivals|standing": 1.26,
            },
            0,
        )
        self.assertEqual(status, 0)
        status, _ = fidelity.check(
            report,
            {
                "prince/online/small/n/a|standing": 1.01,
                "emperor/online/small/rivals|standing": 1.00,
            },
            0,
        )
        self.assertEqual(status, 1, "the emperor cell alone is past its own bar")

    def test_a_subsystem_past_its_tolerance_fails_the_check(self):
        report = {
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "subsystems": {"standing": {"available": True, "divergence": 4.0}},
                }
            ]
        }
        status, notes = fidelity.check(
            report, {"prince/online/small/n/a|standing": 2.0}, 0
        )
        self.assertEqual(status, 1)
        self.assertTrue(any("exceeds" in note for note in notes))

    def test_a_small_wobble_inside_the_regression_margin_does_not_fail(self):
        allowed = 1.01
        report = {
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "subsystems": {
                        "standing": {
                            "available": True,
                            "divergence": allowed * (1 + fidelity.FIDELITY_SLACK / 2),
                        }
                    },
                }
            ]
        }
        status, _ = fidelity.check(
            report, {"prince/online/small/n/a|standing": allowed}, 0
        )
        self.assertEqual(status, 0, "the ratchet is a regression alarm, not a caliper")

    def test_a_subsystem_inside_its_tolerance_passes(self):
        report = {
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "subsystems": {"standing": {"available": True, "divergence": 1.5}},
                }
            ]
        }
        status, _ = fidelity.check(
            report, {"prince/online/small/n/a|standing": 2.0}, 0
        )
        self.assertEqual(status, 0)

    def test_a_subsystem_with_no_tolerance_yet_does_not_fail(self):
        report = {
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "subsystems": {"standing": {"available": True, "divergence": 9.0}},
                }
            ]
        }
        status, notes = fidelity.check(report, {}, 0)
        self.assertEqual(status, 0)
        self.assertTrue(any("no tolerance recorded" in note for note in notes))


class TheFieldIsTheRivalSeats(unittest.TestCase):
    """⚠⚠ A rival-mix run seats its opponents as `kind: "rival"`, and they ARE
    the field. Gathering only the measured seats left one seat per game, so
    "the best other seat" fell back to the seat itself and every standing read
    exactly 1.00 — on the very run this ledger exists to interpret."""

    def test_a_rival_mix_run_measures_the_seat_against_its_rivals(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            sim = fidelity.sim_records(
                write_sim(
                    [
                        [
                            {"kind": "game", "score": 100},
                            {"kind": "rival", "score": 400},
                            {"kind": "rival", "score": 200},
                        ]
                    ],
                    tmp,
                    handicap="rivals",
                ),
                fidelity.map_sizes(),
            )
        self.assertEqual(len(sim), 1, "only the measured seat is a reading")
        self.assertAlmostEqual(
            sim[0]["_score_ratio"], 0.25, msg="100 against the best rival's 400"
        )


class AMismatchedMarkIsNeverCompared(unittest.TestCase):
    """The most dangerous shape a ledger can have is two numbers that are
    readings of different things, because it looks like an answer."""

    def test_no_subsystem_is_left_comparing_different_moments(self):
        """The two known mismatches are fixed at the source: the screen now
        reads both marks at the RAW game turn the live ladder uses. Any
        subsystem still declared incomparable must say why in its own words."""
        for subsystem in fidelity.SUBSYSTEMS:
            with self.subTest(subsystem=subsystem.name):
                if subsystem.incomparable:
                    self.assertGreater(len(subsystem.incomparable), 20)

    def test_the_mechanism_still_works_for_the_next_mismatch(self):
        probe = fidelity.Subsystem(
            "probe", "_a", "_b", "a probe", incomparable="different moments"
        )
        self.assertTrue(probe.incomparable)

    def test_an_incomparable_subsystem_produces_no_divergence(self):
        report = {
            "live_runs": 1,
            "sim_seats": 1,
            "host_only_genes": [],
            "live_only": [],
            "sim_only": [],
            "matched_cells": [
                {
                    "cell": ["prince", "online", "small", "n/a"],
                    "live_runs": 1,
                    "sim_seats": 1,
                    "subsystems": {
                        "probe": {
                            "available": False,
                            "why": "incomparable",
                            "incomparable": "different moments",
                        }
                    },
                }
            ],
        }
        self.assertEqual(fidelity.worst_divergences(report), {})
        # `render` walks SUBSYSTEMS, so a synthetic name is not printed; what
        # matters is that an incomparable body never becomes a divergence.
        self.assertNotIn("probe", fidelity.worst_divergences(report))


class TheLedgerDisclosesItsOwnBound(unittest.TestCase):
    """`Kind::HostOnly` genes ship on the live seat and are inert headless, so
    the two sides of this ledger are never quite the same agent. A reader who
    does not know that will read an opening-band gap as an engine defect."""

    def test_the_host_only_genes_are_discovered_from_the_registry(self):
        found = fidelity.host_only_genes()
        self.assertGreater(len(found), 10, f"the registry scan went thin: {found}")
        self.assertEqual(found, sorted(set(found)), "sorted and deduplicated")
        # Several shape the opening; that is why the notice exists.
        for opening in ("parallel-settlers", "land-grab", "host-settler-pop"):
            self.assertIn(opening, found)
        # A screenable opt-in is NOT host-only and must not be swept in.
        self.assertNotIn("chop-for-expansion", found)

    def test_the_report_names_the_bound_before_any_number(self):
        live = [{"_cell": ("prince", "online", "small", "n/a"), "_score_ratio": 0.5}]
        report = fidelity.ledger(live, [])
        self.assertTrue(report["host_only_genes"])
        text = fidelity.render(report)
        head = text.split("##")[0]
        self.assertIn("not quite the same agent", head)
        self.assertIn("inert here", head)


class TheOpeningBandIsComparedAtTheLiveMark(unittest.TestCase):
    """Turn 60 is where this corpus's strongest result lives. Both sides now
    read it at the same RAW game turn — the mark `civ6_play.OPENING_TEMPO_TURN`
    uses — so it can finally be compared."""

    def test_both_sides_are_read_at_the_same_game_turn_and_compared(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=200, cities_at_60=5)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim(
                    [
                        [
                            {"score": 100, "cities_at_game_turn_60": 4},
                            {"score": 200, "cities_at_game_turn_60": 6},
                        ]
                    ],
                    tmp,
                    handicap="rivals",
                ),
                fidelity.map_sizes(),
            )
        self.assertAlmostEqual(live[0]["cities_at_60"], 5.0, msg="live side read")
        self.assertAlmostEqual(
            sim[0]["cities_at_game_turn_60"], 4.0, msg="sim side, same raw turn"
        )
        band = next(s for s in fidelity.SUBSYSTEMS if s.name == "opening_band")
        self.assertIsNone(
            band.incomparable, "both sides read raw game turn 60, so it compares"
        )
        report = fidelity.ledger(live, sim)
        cell = report["matched_cells"][0]["subsystems"]["opening_band"]
        self.assertTrue(cell["available"])
        self.assertAlmostEqual(cell["live"], 5.0)
        self.assertAlmostEqual(cell["sim"], 5.0, msg="median of 4 and 6")


class TheCityLedgerIsComparable(unittest.TestCase):
    """Over 96 deep live Emperor runs the seat took 2 cities and lost 65.
    Nothing on the simulator side could be set beside that number."""

    def test_both_halves_are_read_from_each_corpus(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live(
                    [
                        live_row(
                            score=100,
                            rival_best=200,
                            combat={"cities_taken": 2, "cities_lost": 65},
                        )
                    ],
                    tmp,
                )
            )
            sim = fidelity.sim_records(
                write_sim(
                    [
                        [
                            {"score": 100, "cities_taken": 2, "cities_lost": 1},
                            {"score": 200, "cities_taken": 4, "cities_lost": 3},
                        ]
                    ],
                    tmp,
                    handicap="rivals",
                ),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        subsystems = report["matched_cells"][0]["subsystems"]
        taken = subsystems["cities_taken"]
        self.assertTrue(taken["available"])
        self.assertAlmostEqual(taken["live"], 2.0)
        self.assertAlmostEqual(taken["sim"], 3.0, msg="median of 2 and 4")
        lost = subsystems["cities_lost"]
        self.assertTrue(lost["available"])
        self.assertAlmostEqual(lost["live"], 65.0)
        self.assertAlmostEqual(lost["sim"], 2.0, msg="median of 1 and 3")
        self.assertGreater(
            lost["divergence"], 10.0, "a 65-against-2 gap must show as a large one"
        )

    def test_a_live_run_with_no_combat_block_says_which_side_is_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=200)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim(
                    [[{"score": 100, "cities_lost": 1}, {"score": 200, "cities_lost": 3}]],
                    tmp,
                    handicap="rivals",
                ),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        lost = report["matched_cells"][0]["subsystems"]["cities_lost"]
        self.assertFalse(lost["available"])
        self.assertEqual(lost["why"], "live")


class TheHandicapIsPartOfTheConfiguration(unittest.TestCase):
    """The rung says how big the bonus is; the handicap mode says who gets it.

    A screen run with `gene_screen`'s default hands the rung's bonus to EVERY
    seat, our measured ones included, so it cancels. A live seat never carries
    it. Comparing those two is comparing different games, which is the one
    thing this ledger exists not to do.
    """

    def test_prince_confers_nothing_so_the_mode_cannot_matter(self):
        neutral = fidelity.neutral_rungs()
        self.assertIn("prince", neutral)
        self.assertEqual(
            fidelity.handicap_for("prince", "all", neutral), fidelity.NEUTRAL_HANDICAP
        )
        self.assertEqual(
            fidelity.handicap_for("prince", "rivals", neutral),
            fidelity.NEUTRAL_HANDICAP,
        )

    def test_every_other_shipped_rung_tilts_one_way_or_the_other(self):
        neutral = fidelity.neutral_rungs()
        for rung in ("settler", "chieftain", "warlord", "king", "emperor", "deity"):
            with self.subTest(rung=rung):
                self.assertNotIn(
                    rung, neutral, "a rung with a bonus is not neutral"
                )
                self.assertEqual(fidelity.handicap_for(rung, "all", neutral), "all")

    def test_a_symmetric_emperor_screen_does_not_match_the_live_seat(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=300)], tmp)
            )
            # No `handicap` key at all: `gene_screen` omits the whole rival
            # block without `--rivals`, and its default is every seat.
            sim = fidelity.sim_records(
                write_sim([[{"score": 100}, {"score": 200}]], tmp),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(
            report["matched_cells"],
            [],
            "an all-seats handicap is not the field a live seat meets",
        )
        self.assertEqual(report["live_only"][0]["cell"][3], "rivals")
        self.assertEqual(report["sim_only"][0]["cell"][3], "all")

    def test_an_asymmetric_emperor_screen_does_match(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            live = fidelity.live_records(
                write_live([live_row(score=100, rival_best=300)], tmp)
            )
            sim = fidelity.sim_records(
                write_sim(
                    [[{"score": 100}, {"score": 200}]], tmp, handicap="rivals"
                ),
                fidelity.map_sizes(),
            )
            report = fidelity.ledger(live, sim)
        self.assertEqual(len(report["matched_cells"]), 1)
        self.assertEqual(report["matched_cells"][0]["cell"][3], "rivals")

    def test_the_committed_corpora_still_share_the_neutral_rung(self):
        """Prince is where the simulator is actually shown to be sound, so the
        ratchet must still have a cell to measure."""
        live = fidelity.live_records(fidelity.LIVE_DEFAULT)
        sim = fidelity.sim_records(fidelity.SIM_DEFAULT, fidelity.map_sizes())
        report = fidelity.ledger(live, sim)
        cells = [c["cell"] for c in report["matched_cells"]]
        self.assertTrue(cells, "no configuration is shared any more")
        self.assertTrue(
            all(cell[3] == fidelity.NEUTRAL_HANDICAP for cell in cells),
            f"only neutral-rung cells should match today, got {cells}",
        )


class ItSkipsLoudlyWithNoCorpus(unittest.TestCase):
    def test_check_passes_with_a_named_skip_when_the_live_corpus_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "nothing.json"
            self.assertEqual(
                fidelity.main(["--check", "--live", str(missing)]),
                0,
            )


class TheCommittedCorpusIsTheRealOne(unittest.TestCase):
    """The evidence ships with the repository, so the ratchet is a real number
    on a hosted runner and not a permanent skip."""

    def test_the_default_live_corpus_is_committed_and_deep(self):
        self.assertTrue(fidelity.LIVE_DEFAULT.exists())
        records = fidelity.live_records(fidelity.LIVE_DEFAULT)
        self.assertGreater(len(records), 100, "the committed corpus went thin")

    def test_the_default_run_finds_a_configuration_both_corpora_played(self):
        live = fidelity.live_records(fidelity.LIVE_DEFAULT)
        sim = fidelity.sim_records(fidelity.SIM_DEFAULT, fidelity.map_sizes())
        report = fidelity.ledger(live, sim)
        self.assertTrue(
            report["matched_cells"],
            "no configuration is shared, so the ratchet measures nothing",
        )


if __name__ == "__main__":
    unittest.main()
