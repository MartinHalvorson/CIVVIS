"""The gene ledger: the win-column default rule, the verdict rules, source
precedence, and the two generated files staying together with the recorded
sources."""
from __future__ import annotations

import argparse
import contextlib
import io
import json
import math
import re
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_ledger  # noqa: E402


PLAYERS = gene_ledger.SCREEN["players"]

#: ★★★★ THE DEPLOYMENT GENOME, FROZEN 2026-08-23. Every gene
#: `docs/gene_ledger.json` defaults on, as it stood before the precision-
#: weighted posterior was published beside the rule. This is a tripwire, not a
#: rule: nothing in the repository else pins what the agent actually plays, and
#: a regeneration that quietly moved a default would otherwise be invisible.
#: Moving one is legitimate and routine -- a new screen, a new operator
#: directive -- and the way to do it is to update this tuple in the same change
#: and name the gene and the reason in the pull request.
DEPLOYED_GENOME_20260823 = (
    "amenity-district-path", "barbarian-scouts-are-scouts",
    "blind-objective-strength", "bounded-recovery",
    "builder-worked-tile-priority", "camp-party", "come-ashore",
    "escort-unstick", "founder-temple",
    "great-person-housing", "holy-lane-parity", "housing-research",
    "idle-faith-patronage", "inquisition-on-threat", "loyalty-rate-alarm",
    "one-launch-pad", "opportunistic-war", "peacetime-deterrence",
    "raid-pillage-prizes", "recon-replacement", "relief-targets-the-siege",
    "religion-sues-peace", "settle-sooner", "settler-site-agreement",
    "settler-target-hysteresis", "settler-threat-detour",
    "stranded-settler-discount", "strike-opening",
    "whole-turn-backtrack-guard", "wide-map-capacity",
)


def analysis(genes: list[dict], pairs: int = 1000, family: float = 3.0, **profile) -> dict:
    """One screen. `wins` is the gene's win column in this screen — wins per
    10,000 on-arm seats above the 1-in-`PLAYERS` a seat takes by chance — written
    back as the on-rate the analyzer would have printed. Keyword arguments
    override legs of the screen's profile, which is how a probe is built."""
    return {
        "kind": "gene_screen_analysis",
        "complete_pairs": pairs,
        "family_wise_z": family,
        "profile": {**gene_ledger.SCREEN, **profile},
        "genes": [
            {
                "tag": g["tag"], "pairs": pairs, "n_on": pairs, "n_off": pairs,
                "win_on": 1.0 / PLAYERS + g.get("wins", 0) / 10_000,
                "win_off": 1.0 / PLAYERS,
                "win_delta_pp": g.get("win", 0.0), "win_z": g.get("wz", 0.0),
                # The error on that difference: what the precision-weighted
                # posterior weights each screen by. A fixture without one
                # would silently produce no posterior at all.
                "win_se_pp": g.get("se", 1.0),
                "share_delta_pp": g.get("share", 0.0), "share_z": g.get("sz", 0.0),
                "read": "",
                "win_tranches": g.get("tranches", []),
            }
            for g in genes
        ],
    }


class VerdictRules(unittest.TestCase):
    def test_helps_needs_one_axis_past_the_bar_and_the_other_not_against(self):
        self.assertEqual(gene_ledger.axis_verdict(2.1, 0.0), "helps")
        self.assertEqual(gene_ledger.axis_verdict(0.0, 2.5), "helps")
        self.assertEqual(gene_ledger.axis_verdict(2.1, -1.9), "helps")
        self.assertEqual(gene_ledger.axis_verdict(1.99, 1.99), "unresolved")

    def test_hurts_is_the_mirror_image(self):
        self.assertEqual(gene_ledger.axis_verdict(-2.1, 0.0), "hurts")
        self.assertEqual(gene_ledger.axis_verdict(0.0, -2.5), "hurts")
        self.assertEqual(gene_ledger.axis_verdict(-1.2, -2.6), "hurts")

    def test_two_axes_past_the_bar_in_opposite_directions_is_unresolved(self):
        self.assertEqual(gene_ledger.axis_verdict(2.5, -2.5), "unresolved")
        self.assertTrue(gene_ledger.axes_conflict(2.5, -2.5))
        self.assertFalse(gene_ledger.axes_conflict(2.5, -1.0))


class Merging(unittest.TestCase):
    def build(self, sources):
        with tempfile.TemporaryDirectory() as tmp:
            paths = []
            for i, data in enumerate(sources):
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(data))
                paths.append(path)
            return gene_ledger.build_ledger(paths, filter_known=False)

    def test_the_newest_screen_that_priced_a_gene_supplies_its_verdict(self):
        ledger = self.build([
            analysis([
                {"tag": "a", "wz": 2.4},           # helps, and not re-screened
                {"tag": "b", "wz": 0.3},           # unresolved, then resolved below
                {"tag": "c", "wz": -2.2},          # hurts, and stays hurt
            ]),
            analysis([
                {"tag": "b", "wz": 3.0},
                {"tag": "c", "wz": -2.4},
                {"tag": "d", "wz": -2.5},          # first priced by the newer screen
            ]),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertEqual(by["a"]["verdict"], "helps", "the older screen still stands where nothing re-priced it")
        self.assertEqual(by["b"]["verdict"], "helps")
        self.assertEqual(by["c"]["verdict"], "hurts")
        self.assertEqual(by["d"]["verdict"], "hurts")
        self.assertEqual(
            [g["default_on"] for g in ledger["genes"]], [False, False, False, False],
            "a verdict no longer turns a gene on: these have no win columns to clear the rule",
        )
        self.assertEqual(
            {key: ledger["counts"][key]
             for key in ("helps", "hurts", "unresolved", "default_on")},
            {"helps": 2, "hurts": 2, "unresolved": 0, "default_on": 0},
        )
        # Every authority is priced beside the one in force, so the delta is
        # in the ledger and not only in the ranking.
        for candidate in gene_ledger.AUTHORITIES:
            self.assertIn(f"default_on_under_{candidate}", ledger["counts"])
            self.assertIn(f"moved_by_{candidate}", ledger["counts"])

    def test_a_later_source_overrides_an_earlier_one_per_gene(self):
        ledger = self.build([
            analysis([{"tag": "repaired", "wz": -4.0}, {"tag": "other", "wz": 2.5}]),
            analysis([{"tag": "repaired", "wz": 2.5}], pairs=500),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertEqual(by["repaired"]["verdict"], "helps")
        self.assertEqual(by["repaired"]["screen"]["pairs"], 500)
        self.assertEqual(by["other"]["verdict"], "helps", "the earlier screen still stands for the rest")

    def test_family_wise_is_recorded_from_the_deciding_runs_bar(self):
        ledger = self.build([
            analysis([
                {"tag": "strong", "wz": 3.5}, {"tag": "weak", "wz": 2.2},
            ], family=3.3),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertTrue(by["strong"]["family_wise"])
        self.assertFalse(by["weak"]["family_wise"])

    def test_chronological_win_tranches_survive_in_the_ledger(self):
        ledger = self.build([analysis([{
            "tag": "repeated-harm",
            "wz": -3.0,
            "tranches": [
                {"position": "latest", "pairs": 10002, "win_delta_pp": -1.2345, "win_se_pp": 0.3444, "win_z": -2.3456},
                {"position": "previous", "pairs": 9996, "win_delta_pp": -0.9876, "win_se_pp": 0.4655, "win_z": -2.1234},
                {"position": "earlier", "pairs": 10002, "win_delta_pp": -1.1111, "win_se_pp": 0.4366, "win_z": -2.5432},
            ],
        }])])
        measure = ledger["genes"][0]["screen"]
        self.assertEqual(measure["win_tranches"], [
            {"position": "latest", "pairs": 10002, "win_delta_pp": -1.234, "win_se_pp": 0.344, "win_z": -2.346},
            {"position": "previous", "pairs": 9996, "win_delta_pp": -0.988, "win_se_pp": 0.466, "win_z": -2.123},
            {"position": "earlier", "pairs": 10002, "win_delta_pp": -1.111, "win_se_pp": 0.437, "win_z": -2.543},
        ])

    def test_every_source_records_the_shape_it_was_played_at(self):
        ledger = self.build([
            analysis([{"tag": "a"}]),
            analysis([{"tag": "a"}], map="pangaea", width=60, height=38, city_states=6),
        ])
        self.assertEqual([s["shape"] for s in ledger["sources"]], ["standard", "legacy"])


class OneShape(unittest.TestCase):
    """⭐ There is one screen (operator, 2026-08-22). A batch played at another
    profile answers a different question, so it is refused as a source rather
    than pooled into a column beside the screen's."""

    def sources(self, data, legacy_shape=False):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "probe.json"
            path.write_text(json.dumps(data))
            args = argparse.Namespace(source=[str(path)], legacy_shape=legacy_shape)
            return gene_ledger.sources_from_args(args)

    def test_the_screens_own_shape_is_accepted(self):
        self.assertEqual(len(self.sources(analysis([{"tag": "a"}]))), 1)

    def test_a_probe_at_another_map_is_refused(self):
        with self.assertRaises(SystemExit) as refusal:
            self.sources(analysis([{"tag": "a"}], map="pangaea"))
        self.assertIn("map='pangaea'", str(refusal.exception))

    def test_a_restricted_lane_set_is_a_probe_not_a_screen(self):
        """The war regime, as it would arrive today: refused at the door."""
        with self.assertRaises(SystemExit):
            self.sources(analysis([{"tag": "a"}], players=4, victories="domination,score"))

    def test_legacy_shape_records_a_probe_deliberately(self):
        self.assertEqual(len(self.sources(analysis([{"tag": "a"}], map="pangaea"),
                                          legacy_shape=True)), 1)

    def test_the_tool_and_the_binary_name_the_same_screen(self):
        """`gene_screen`'s bare defaults ARE this shape; if one side moves, the
        ledger would silently accept a batch the binary no longer plays."""
        rs = (gene_ledger.ROOT / "src" / "bin" / "gene_screen.rs").read_text()
        for constant, value in (
            ("SCREEN_PLAYERS: usize", gene_ledger.SCREEN["players"]),
            ("SCREEN_WIDTH: i32", gene_ledger.SCREEN["width"]),
            ("SCREEN_HEIGHT: i32", gene_ledger.SCREEN["height"]),
            ("SCREEN_CITY_STATES: usize", gene_ledger.SCREEN["city_states"]),
        ):
            self.assertIn(f"const {constant} = {value};", rs, constant)
        self.assertIn("const SCREEN_MAP: MapScript = MapScript::Continents;", rs)


class TheDefaultRule(unittest.TestCase):
    """Operator directive 2026-08-22: the default is read off the ranking's two
    win columns — both positive, or an average above +15 with neither below
    -10 — vetoed by a negative pooled on-off difference, and nothing else, the
    verdict included. The fixture's screens are equal-sized and its off arm is
    exactly chance, so a gene's pooled difference is the mean of its columns in
    hundredths of a point: every case here clears the veto unless it is named."""

    def on(self, *wins: int) -> bool:
        """Build a gene screened once per given win column, oldest first."""
        with tempfile.TemporaryDirectory() as tmp:
            sources = []
            for i, w in enumerate(wins):
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(analysis([{"tag": "g", "wins": w}])))
                sources.append(path)
            ledger = gene_ledger.build_ledger(sources, filter_known=False)
        gene = ledger["genes"][0]
        self.assertEqual(gene["wins_last_10k"], wins[-1] if wins else None)
        self.assertEqual(gene["wins_prior_10k"], wins[-2] if len(wins) > 1 else None)
        return gene["default_on"]

    def test_two_positive_columns_turn_a_gene_on(self):
        self.assertTrue(self.on(29, 25))
        self.assertTrue(self.on(1, 1))

    def test_a_zero_column_is_not_a_positive_one(self):
        self.assertFalse(self.on(1, 0))
        self.assertTrue(self.on(0, 39), "an average of 19.5 carries it instead")

    def test_a_strong_average_carries_one_negative_column_down_to_the_floor(self):
        self.assertTrue(self.on(-10, 48), "average 19, floor exactly met")
        self.assertFalse(self.on(-11, 50), "a column below -10 is off however good the average")
        self.assertFalse(self.on(0, 30), "an average of exactly 15 does not clear +15")

    def test_one_bad_reading_sinks_a_gene_the_average_does_not_carry(self):
        self.assertFalse(self.on(-26, 39), "housing-research: average 6.5")
        self.assertFalse(self.on(-192, 8), "war-economy: a helps verdict does not save it")

    def test_one_reading_must_clear_twenty(self):
        self.assertTrue(self.on(21), "a single reading above +20 is provisionally on")
        self.assertFalse(self.on(20), "exactly +20 does not clear the strict bar")
        self.assertFalse(self.on(-21))
        self.assertTrue(gene_ledger.default_from_win_columns(None, 21))
        self.assertFalse(gene_ledger.default_from_win_columns(None, 20))
        # A gene no screen has measured has no row at all; the rule still
        # answers for it, and `ledger_default_on` gives the same `false`.
        self.assertFalse(gene_ledger.default_from_win_columns(None, None))

    def test_only_the_last_two_readings_supply_the_columns(self):
        self.assertTrue(
            gene_ledger.default_from_win_columns(21, 20),
            "the column clause never sees a third screen",
        )
        self.assertFalse(gene_ledger.default_from_win_columns(-500, 21))
        self.assertFalse(self.on(20, 21, -500))

    def test_an_old_bad_screen_is_a_veto_through_the_difference(self):
        """The one clause that lets a screen older than the last two speak.

        Until 2026-08-22 this case shipped: the columns read the newest two
        readings and an old collapse was history. `war-economy` is the live
        example — +38/+8 over a -3.84 pp screen it has not made back."""
        self.assertFalse(self.on(-500, 20, 21), "the record is -1.53 pp")
        self.assertTrue(self.on(-40, 20, 21), "a record of +0.003 pp is not negative")

    def test_a_legacy_shape_still_supplies_a_column(self):
        """The Pangaea history is what the deployment genome stands on: it is
        kept, and the standard screen overwrites it gene by gene."""
        with tempfile.TemporaryDirectory() as tmp:
            old = Path(tmp) / "legacy.json"
            old.write_text(json.dumps(
                analysis([{"tag": "g", "wins": 30}], map="pangaea", width=60, height=38)))
            new = Path(tmp) / "screen.json"
            new.write_text(json.dumps(analysis([{"tag": "g", "wins": 25}])))
            ledger = gene_ledger.build_ledger([old, new], filter_known=False)
        gene = ledger["genes"][0]
        self.assertEqual((gene["wins_prior_10k"], gene["wins_last_10k"]), (30, 25))
        self.assertTrue(gene["default_on"], "two positive columns, whatever shape they came from")


class TheDifferenceVeto(unittest.TestCase):
    """Operator directive 2026-08-22: a gene whose pooled on-off difference is
    negative defaults off whatever its win columns say.

    The figure is `HEURISTIC_GENE_RANKING.md`'s *Diff* — the pooled on rate
    minus the pooled off rate over EVERY screen that priced the gene, each
    weighted by its games, in percentage points."""

    def gene(self, screens: list[tuple[float, float, int]]) -> dict:
        """One gene measured across `(win_on, win_off, pairs)` screens."""
        with tempfile.TemporaryDirectory() as tmp:
            sources = []
            for i, (on, off, pairs) in enumerate(screens):
                data = analysis([{"tag": "g"}], pairs=pairs)
                data["genes"][0].update(win_on=on, win_off=off)
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(data))
                sources.append(path)
            return gene_ledger.build_ledger(sources, filter_known=False)["genes"][0]

    def test_the_difference_is_weighted_by_each_screens_games(self):
        gene = self.gene([(0.10, 0.20, 3000), (0.20, 0.19, 1000)])
        # (-10 * 3000 + 1 * 1000) / 4000 pp.
        self.assertAlmostEqual(gene["win_diff_pp"], -7.25)
        self.assertFalse(gene["default_on"])

    def test_the_veto_beats_every_column_clause(self):
        for columns in ((78, None), (38, 8), (48, -10)):
            self.assertTrue(gene_ledger.default_from_win_columns(*columns), columns)
            self.assertFalse(
                gene_ledger.default_from_columns(*columns, -0.01),
                f"{columns} must not survive a negative record",
            )

    def test_the_veto_is_one_way(self):
        """A positive record promotes nothing: the columns still have to clear
        their bars, so this cannot turn a gene on behind the operator's rule."""
        self.assertFalse(gene_ledger.default_from_columns(-5, -11, 1.20))
        self.assertFalse(gene_ledger.default_from_columns(-26, 39, 0.65))

    def test_a_record_of_exactly_zero_is_not_negative(self):
        gene = self.gene([(0.20, 0.19, 1000), (0.18, 0.19, 1000)])
        self.assertEqual(gene["win_diff_pp"], 0.0)
        self.assertTrue(gene_ledger.default_from_columns(38, 8, gene["win_diff_pp"]))

    def test_the_decision_is_taken_on_the_figure_the_ledger_publishes(self):
        """Rounded first, then decided — so the generated Rust table re-derives
        the same answer from the same number and the two cannot drift at a
        boundary."""
        tiny = 1.0 / PLAYERS - 1e-12
        gene = self.gene([(tiny, 1.0 / PLAYERS, 1000)])
        self.assertEqual(gene["win_diff_pp"], 0.0)
        self.assertEqual(
            gene["default_on"],
            gene_ledger.default_from_columns(
                gene["wins_last_10k"], gene["wins_prior_10k"], gene["win_diff_pp"]
            ),
        )

    def test_an_unmeasured_gene_has_no_record_to_read(self):
        self.assertFalse(gene_ledger.default_from_columns(None, None, None))
        self.assertTrue(gene_ledger.default_from_columns(21, None, None))

    def test_the_ledger_records_the_difference_beside_the_columns(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertIn("win_diff", ledger["rules"])
        for gene in ledger["genes"]:
            self.assertIsInstance(gene["win_diff_pp"], float, gene["tag"])
            self.assertEqual(
                gene["default_on"],
                gene_ledger.default_from_columns(
                    gene["wins_last_10k"], gene["wins_prior_10k"], gene["win_diff_pp"]
                ),
                gene["tag"],
            )


class ThePrecisionWeightedPosterior(unittest.TestCase):
    """Random-effects (DerSimonian-Laird) inverse-variance pooling: the
    arithmetic, worked by hand, and the heterogeneity case it exists for."""

    def test_the_pool_is_inverse_variance_weighted_worked_by_hand(self):
        """Two screens, chosen so every step lands on a whole number.

        On the win column's scale (`x pp` becomes `x * 50` wins/10k):

            y1 = +50, s1 = 20 -> w1 = 1/400   = 0.0025
            y2 = +25, s2 = 15 -> w2 = 1/225   = 0.004444...
            sum w                             = 0.006944...
            fixed effect = (0.0025*50 + 0.004444*25) / 0.006944 = 34
            Q  = 0.0025*(50-34)^2 + 0.004444*(25-34)^2 = 0.64 + 0.36 = 1.0
            k-1 = 1, so tau^2 = max(0, 1.0 - 1) / C   = 0
            pooled = the fixed effect                 = 34
            se     = sqrt(1 / 0.006944...)            = 12
        """
        pooled = gene_ledger.pooled_posterior([
            {"win_delta_pp": 1.0, "win_se_pp": 0.4},
            {"win_delta_pp": 0.5, "win_se_pp": 0.3},
        ])
        self.assertEqual(pooled["screens"], 2)
        self.assertEqual(pooled["fixed_effect"], 34.0)
        self.assertEqual(pooled["q"], 1.0)
        self.assertEqual(pooled["tau"], 0.0)
        self.assertEqual(pooled["effect"], 34.0)
        self.assertEqual(pooled["se"], 12.0)
        self.assertAlmostEqual(pooled["lo"], 34.0 - 1.959963984540054 * 12.0, places=5)
        self.assertAlmostEqual(pooled["hi"], 34.0 + 1.959963984540054 * 12.0, places=5)
        # The tighter screen pulls the pool toward itself: 34 sits nearer 25
        # than 50 because 15 < 20. That is the whole point of the weighting.
        self.assertLess(pooled["effect"] - 25.0, 50.0 - pooled["effect"])

    def test_heterogeneity_widens_the_interval_instead_of_averaging_it_away(self):
        """Two screens that disagree far past their errors.

            y1 = +100, y2 = -100, both s = 10
            fixed effect = 0, se would be sqrt(1/0.02) = 7.07
            Q = 0.01*100^2 + 0.01*100^2 = 200
            C = 0.02 - (0.0001 + 0.0001)/0.02 = 0.01
            tau^2 = (200 - 1) / 0.01 = 19,900 -> tau = 141.07
            w* = 1/(100 + 19,900) = 1/20,000 each
            pooled = 0, se = sqrt(1/0.0001) = 100

        A fixed-effect pool would report 0 +/- 14 and call it settled. The
        random-effects pool reports 0 +/- 196 and calls it what it is."""
        pooled = gene_ledger.pooled_posterior([
            {"win_delta_pp": 2.0, "win_se_pp": 0.2},
            {"win_delta_pp": -2.0, "win_se_pp": 0.2},
        ])
        self.assertEqual(pooled["q"], 200.0)
        self.assertAlmostEqual(pooled["tau"], 141.06736, places=4)
        self.assertEqual(pooled["effect"], 0.0)
        self.assertEqual(pooled["se"], 100.0)
        self.assertEqual(pooled["fixed_effect"], 0.0)
        self.assertEqual(pooled["p_positive"], 0.5)
        self.assertEqual(gene_ledger.posterior_call(pooled["effect"], pooled["se"]),
                         "unresolved")

    def test_one_screen_pools_to_itself_and_all_the_work_is_in_the_interval(self):
        """The shrinkage the operator asked for is not in the point estimate.

        +30 from a screen resolving +/-64 and +30 from one resolving +/-29
        print the same number and are not the same evidence."""
        wide = gene_ledger.pooled_posterior(
            [{"win_delta_pp": 0.6, "win_se_pp": 0.4576}])
        tight = gene_ledger.pooled_posterior(
            [{"win_delta_pp": 0.6, "win_se_pp": 0.2052}])
        self.assertEqual(wide["effect"], tight["effect"])
        self.assertEqual((wide["screens"], wide["tau"], wide["q"]), (1, 0.0, 0.0))
        self.assertAlmostEqual(2.8 * wide["se"], 64.06, places=1)
        self.assertAlmostEqual(2.8 * tight["se"], 28.73, places=1)
        self.assertLess(wide["p_positive"], 0.91)
        self.assertGreater(tight["p_positive"], 0.99)

    def test_the_column_and_its_error_are_one_scale(self):
        """`column_estimate / column_se` must reproduce the screen's own
        `win_z`, which it can only do if both are the halved column scale."""
        for delta, se in ((0.91, 0.37), (-4.73, 0.3077), (0.2, 0.7214)):
            self.assertAlmostEqual(
                gene_ledger.column_estimate(delta) / gene_ledger.column_se(se),
                delta / se, places=9)

    def test_a_screen_with_no_error_is_skipped_not_given_one(self):
        self.assertIsNone(gene_ledger.pooled_posterior(
            [{"win_delta_pp": 1.0, "win_se_pp": None}]))
        self.assertIsNone(gene_ledger.pooled_posterior(
            [{"win_delta_pp": 1.0, "win_se_pp": 0.0}]))
        self.assertIsNone(gene_ledger.pooled_posterior([]))
        one = gene_ledger.pooled_posterior([
            {"win_delta_pp": 1.0, "win_se_pp": None},
            {"win_delta_pp": 1.0, "win_se_pp": 0.4},
        ])
        self.assertEqual((one["screens"], one["effect"]), (1, 50.0))

    def test_the_pool_can_be_taken_per_shape(self):
        """Pooling a `standard` reading with a `legacy` one is exactly the case
        `tau` exists to expose, so the pool must be able to read them apart."""
        history = [
            {"win_delta_pp": 1.0, "win_se_pp": 0.4, "shape": "legacy"},
            {"win_delta_pp": 0.5, "win_se_pp": 0.3, "shape": "standard"},
        ]
        self.assertEqual(
            gene_ledger.pooled_posterior(history, ("legacy",))["effect"], 50.0)
        self.assertEqual(
            gene_ledger.pooled_posterior(history, ("standard",))["effect"], 25.0)
        self.assertEqual(
            gene_ledger.pooled_posterior(history, ("standard", "legacy"))["effect"],
            34.0)
        self.assertIsNone(gene_ledger.pooled_posterior(history, ()))

    def test_what_a_direct_arm_buys_and_how_big_it_has_to_be(self):
        """The two planning numbers `--boundary` ranks on."""
        # A posterior of +30 +/- 20 straddles zero (30 < 1.96*20). An arm whose
        # per-column error is `constant / sqrt(N)` resolves it once
        #   N > constant^2 * ((1.96/30)^2 - 1/400)
        #     = 4,000,000 * (0.00426753 - 0.0025) = 7,073.1  ->  7,074
        constant = 2000.0
        needs = gene_ledger.arm_pairs_to_resolve(30.0, 20.0, constant)
        self.assertEqual(needs, 7074)
        self.assertAlmostEqual(constant / math.sqrt(needs), 23.78, places=2)
        # At exactly that size the combined interval just clears zero, and one
        # pair short of it does not.
        for pairs, clears in ((needs, True), (needs - 200, False)):
            arm_se = constant / math.sqrt(pairs)
            combined = 1.0 / math.sqrt(1.0 / 400.0 + 1.0 / (arm_se * arm_se))
            self.assertEqual(30.0 - 1.959963984540054 * combined > 0.0, clears, pairs)
        # A posterior that already resolves needs nothing, and one at exactly
        # zero is never resolved by any finite arm.
        self.assertEqual(gene_ledger.arm_pairs_to_resolve(40.0, 10.0, constant), 0)
        self.assertIsNone(gene_ledger.arm_pairs_to_resolve(0.0, 10.0, constant))

    def test_an_arm_buys_most_where_the_genome_and_the_evidence_disagree(self):
        """EVSI is read against the SHIPPED state, which is what makes it
        answer the operator's question rather than the estimator's."""
        held_off = gene_ledger.arm_information_value(26.0, 21.5, 24.3, deployed=False)
        already_on = gene_ledger.arm_information_value(26.0, 21.5, 24.3, deployed=True)
        self.assertGreater(held_off, 26.0)
        self.assertLess(already_on, 1.0)
        self.assertAlmostEqual(held_off - already_on, 26.0, places=6)
        # Nothing to learn (a useless arm) buys nothing beyond the incumbent.
        self.assertAlmostEqual(
            gene_ledger.arm_information_value(26.0, 21.5, 1e12, deployed=True),
            0.0, places=3)
        # And it is never negative: information cannot hurt.
        for effect in (-80.0, -5.0, 0.0, 5.0, 80.0):
            for deployed in (True, False):
                self.assertGreaterEqual(
                    gene_ledger.arm_information_value(effect, 20.0, 24.3, deployed),
                    -1e-9, (effect, deployed))

    def test_the_direct_arm_constant_is_discovered_from_the_widest_arm_run(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        sources = [
            {"name": Path(s["path"]).name,
             "analysis": json.loads((gene_ledger.ROOT / s["path"]).read_text())}
            for s in ledger["sources"]
        ]
        constant, name = gene_ledger.direct_arm_constant(sources)
        # g1 replaced h1 as the widest arm on 2026-08-23. It flips
        # `governor-victory-lanes`, which reaches nearly every decision the
        # governor makes, so its two arms play very different games and the
        # foldover cancels almost nothing: 39.09 per column against h1's
        # 24.34. Widest is deliberately the conservative end, so the constant
        # rising is the estimator getting safer, not worse.
        self.assertIn("g1-governor-victory-lanes-direct", name)
        # g1: 3,600 seat pairs at a 39.09 per-column standard error.
        self.assertAlmostEqual(constant / math.sqrt(3600), 39.09, places=2)
        # The four-player single-gene probe is not eligible: a 1-in-4 chance
        # base is not the screen's instrument.
        self.assertNotIn("s2-step-and-reassess", name)


class TheDeploymentAuthority(unittest.TestCase):
    """⭐ THE SWITCH. `AUTHORITY` decides which rule writes `default_on`, the
    ledger records it, and the Rust mirror re-derives under the recorded one.
    This PR publishes the posterior and does NOT throw the switch."""

    def test_the_shipped_genome_is_unchanged_and_the_threshold_rule_wrote_it(self):
        """★★★★ THE GENOME THIS PR DID NOT MOVE.

        The 31 tags below are `docs/gene_ledger.json`'s `default_on` set as it
        stood on 2026-08-23, before the posterior existed. A change here is a
        change to what the agent plays in every verification game, and it is
        the operator's call: if you are moving a default deliberately -- a new
        screen, a new directive -- update this tuple in the same change and
        say which gene moved and why in the pull request. If you did not mean
        to move one, this test has just caught a regeneration that did."""
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertEqual(gene_ledger.authority_of(ledger), "columns")
        self.assertEqual(gene_ledger.AUTHORITY, "columns")
        self.assertEqual(
            tuple(sorted(g["tag"] for g in ledger["genes"] if g["default_on"])),
            DEPLOYED_GENOME_20260823,
        )
        self.assertEqual(ledger["counts"]["default_on"], len(DEPLOYED_GENOME_20260823))
        # And every one of them is the threshold rule's own call, so the
        # posterior beside it has touched nothing.
        for gene in ledger["genes"]:
            self.assertEqual(
                gene["default_on"],
                gene_ledger.default_from_columns(
                    gene["wins_last_10k"], gene["wins_prior_10k"], gene["win_diff_pp"]),
                gene["tag"])

    def test_the_ledger_records_a_posterior_for_every_priced_gene(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertIn("posterior", ledger["rules"])
        self.assertIn("authority", ledger["rules"])
        for gene in ledger["genes"]:
            self.assertIsInstance(gene["posterior_pp"], float, gene["tag"])
            self.assertIsInstance(gene["posterior_se_pp"], float, gene["tag"])
            self.assertGreater(gene["posterior_se_pp"], 0.0, gene["tag"])
            self.assertEqual(
                gene["default_on"],
                gene_ledger.deployment_default_on(
                    "columns", gene["wins_last_10k"], gene["wins_prior_10k"],
                    gene["win_diff_pp"], gene["posterior_pp"],
                    gene["posterior_se_pp"]),
                gene["tag"])

    def test_the_authorities_form_a_chain_and_only_the_first_ships(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        counts = ledger["counts"]
        self.assertEqual(counts["default_on_under_columns"], counts["default_on"])
        self.assertEqual(counts["moved_by_columns"], 0)
        # The published delta the operator takes the call on. Both settings
        # re-admit exactly the three genes the sign-of-Diff veto removed --
        # none of which has a resolved negative record.
        # 33, not 34, since 2026-08-23: the base moved 31 -> 30 when g1's
        # direct arm resolved `governor-victory-lanes` off. The posterior
        # re-admits the same three genes and agrees with the threshold rule
        # about that gene, so the delta itself is unchanged at 3.
        self.assertEqual(counts["default_on_under_posterior-veto"], 33)
        self.assertEqual(counts["default_on_under_posterior"], 33)
        self.assertEqual(counts["moved_by_posterior-veto"], 3)
        self.assertEqual(counts["moved_by_posterior"], 3)

    def test_the_posterior_decides_only_where_its_interval_excludes_zero(self):
        # 20 +/- 1.96*10 = [0.4, 39.6]: wholly above zero.
        self.assertEqual(gene_ledger.posterior_call(20.0, 10.0), "on")
        self.assertEqual(gene_ledger.posterior_call(-20.0, 10.0), "off")
        # 19 straddles, and the incumbent call stands either way.
        self.assertEqual(gene_ledger.posterior_call(19.0, 10.0), "unresolved")
        self.assertTrue(gene_ledger.default_from_posterior(19.0, 10.0, True))
        self.assertFalse(gene_ledger.default_from_posterior(19.0, 10.0, False))
        self.assertTrue(gene_ledger.default_from_posterior(20.0, 10.0, False))
        self.assertFalse(gene_ledger.default_from_posterior(-20.0, 10.0, True))
        self.assertEqual(gene_ledger.posterior_call(None, None), "unresolved")

    def test_the_veto_with_an_error_bar_is_strictly_weaker_than_the_sign_veto(self):
        """`war-economy`: two positive columns removed by a record of -0.78 pp
        whose interval is [-185, +88]."""
        self.assertFalse(gene_ledger.default_from_columns(38, 8, -0.78))
        self.assertTrue(gene_ledger.default_from_resolved_veto(38, 8, -48.4, 69.7))
        # A record that IS resolved negative still vetoes.
        self.assertFalse(gene_ledger.default_from_resolved_veto(38, 8, -86.5, 18.6))
        # It can only re-admit what the columns already like.
        self.assertFalse(gene_ledger.default_from_resolved_veto(-5, -11, 200.0, 1.0))

    def test_the_dispatcher_refuses_an_authority_it_does_not_know(self):
        with self.assertRaises(SystemExit):
            gene_ledger.deployment_default_on("wishful", 21, None, 1.0, 50.0, 1.0)
        # A ledger written before the key existed was written under the rule
        # that shipped.
        self.assertEqual(gene_ledger.authority_of({"rules": {}}), "columns")

    def test_flipping_the_switch_regenerates_a_different_genome(self):
        """The switch is real: build the same sources under each authority and
        the `default_on` column actually differs. Nothing is written."""
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        sources = gene_ledger.sources_from_ledger(current)
        shipped = gene_ledger.build_ledger(sources, authority="columns")
        posterior = gene_ledger.build_ledger(sources, authority="posterior")
        self.assertEqual(posterior["rules"]["authority"], "posterior")
        moved = {g["tag"] for g, h in zip(shipped["genes"], posterior["genes"])
                 if g["default_on"] != h["default_on"]}
        self.assertEqual(moved, {"war-economy", "siege-commitment",
                                 "apostle-promotion-by-role"})
        # And the generated Rust carries the authority it was written under,
        # so the mirror re-derives under the same rule.
        self.assertIn('pub(super) const AUTHORITY: &str = "posterior";',
                      gene_ledger.render_rust(posterior))
        self.assertIn('pub(super) const AUTHORITY: &str = "columns";',
                      gene_ledger.LEDGER_RS.read_text())


class KnownTags(unittest.TestCase):
    def test_the_registry_is_read_and_a_removed_gene_is_dropped(self):
        known = gene_ledger.known_tags()
        self.assertGreater(len(known), 50, "the treatments registry scrape found too few tags")
        self.assertIn("wide-map-capacity", known)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "s.json"
            path.write_text(json.dumps(analysis([
                {"tag": "wide-map-capacity", "wz": 2.5},
                {"tag": "a-gene-whose-code-was-removed", "wz": 3.0},
            ])))
            ledger = gene_ledger.build_ledger([path])
        tags = [g["tag"] for g in ledger["genes"]]
        self.assertIn("wide-map-capacity", tags)
        self.assertNotIn("a-gene-whose-code-was-removed", tags)


def stamped(data: dict, tags: list[str], commit: str = "c" * 40, dirty: bool = False,
            fingerprint: str | None = None) -> dict:
    """One screen, stamped by the binary that played it: the gene set it
    compiled in, and the commit it says that set came from."""
    data["profile"]["genes"] = list(tags)
    data["profile"]["screened"] = list(tags)
    data["profile"]["build"] = {
        "commit": commit,
        "commit_source": "env",
        "dirty": dirty,
        "genes_sha256": fingerprint or gene_ledger.gene_set_fingerprint(tags),
        "binary_sha256": "b" * 64,
    }
    return data


def preregistered(data: dict, target: int) -> dict:
    """The same screen, declaring the size it was launched to play."""
    data["batch"] = {
        "target_pairs": target,
        "target_comparisons": target,
        "complete_comparisons": data["complete_pairs"],
        "completion": data["complete_pairs"] / target,
        "partial": data["complete_pairs"] < target,
    }
    return data


class TheGeneSetDerivation(unittest.TestCase):
    """⭐ The ledger re-derives a binary's gene set from the source tables at
    the commit a screen claims. If that derivation were wrong the guard would
    refuse honest screens, so it is checked against a real binary's own output
    rather than against itself."""

    def test_the_derivation_reproduces_a_real_binarys_gene_list(self):
        """P10's header was written by a binary built at `d23f92d9`, and lists
        the 75 genes that binary compiled in. The derivation must reproduce it
        exactly, in order — no fixture, a genuine artefact.

        ⚠ Needs the commit. A shallow CI checkout has one commit, so this
        corroboration skips there; `gene_screen.rs`'s
        `the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables`
        holds the same rule against the compiled table on every run."""
        p10 = json.loads((gene_ledger.ROOT / "docs" / "gene_screens" /
                          "2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json"
                          ).read_text())
        commit = p10["batch"]["source_commit"]
        derived = gene_ledger.gene_tags_at(commit)
        if derived is None:
            self.skipTest(f"this clone cannot read {commit[:12]}")
        self.assertEqual(derived, p10["profile"]["genes"])
        self.assertEqual(len(derived), 75)

    def test_a_commit_this_clone_cannot_read_derives_nothing(self):
        self.assertIsNone(gene_ledger.gene_tags_at("0" * 40))

    def test_comments_and_nested_brackets_do_not_invent_genes(self):
        """The tables are heavily commented and the comments quote tag names."""
        elo = '''
            pub const OTHER: &[&str] = &["not-a-gene"];
            /// A doc comment naming "decoy-one".
            pub const ENGINE_REPAIR_TREATMENTS: &[&str] = &[
                "war-reinforcement",
                // "decoy-two" was culled; see #2266.
                "come-ashore", /* "decoy-three" */
            ];
        '''
        treatments = '''
            pub const PRODUCTION_TREATMENTS: &[LiveTreatment] = &[
                ("strategic_wonders", "strategic-wonders", AdvancedAi::disable),
            ];
            pub const PRODUCTION_OPT_INS: &[LiveTreatment] = &[
                // The joint search: "decoy-four".
                ("joint_tactics", "joint-tactics", AdvancedAi::enable),
            ];
        '''
        read = {"src/elo.rs": elo, "src/ai/advanced/treatments.rs": treatments}
        self.assertEqual(
            gene_ledger.gene_tags_from_sources(read.__getitem__),
            ["war-reinforcement", "come-ashore", "strategic-wonders", "joint-tactics"],
        )

    def test_the_fingerprint_is_the_tags_newline_terminated(self):
        """`gene_screen.rs` pins the same two-tag constant, so the Rust binary
        and this tool cannot compute different fingerprints for one gene set."""
        self.assertEqual(
            gene_ledger.gene_set_fingerprint(["a", "b"]),
            "911169ddaaf146aff539f58c26c489af3b892dff0fe283c1c264c65ae5aa59a2",
        )

    def test_the_working_tree_derives_the_gene_set_the_repository_registers(self):
        tags = gene_ledger.gene_tags_now()
        self.assertGreater(len(tags), 50, "the tables scrape found too few genes")
        self.assertEqual(len(tags), len(set(tags)), "a tag is listed twice")
        self.assertLessEqual(set(tags), gene_ledger.known_tags(),
                             "every gene the screen varies is a registered treatment")
        # One tag from each of the three tables, so a parse that silently
        # lost a whole table fails here. Deliberately structural rather than
        # topical: naming a gene under review would make this test a hostage
        # to the next cull.
        self.assertIn("war-reinforcement", tags, "ENGINE_REPAIR_TREATMENTS")
        self.assertIn("strategic-wonders", tags, "PRODUCTION_TREATMENTS")
        self.assertIn("joint-tactics", tags, "PRODUCTION_OPT_INS")


class TheBuildGuard(unittest.TestCase):
    """⚠⚠ A SCREEN MUST NOT PRICE CODE IT DID NOT PLAY. This has happened three
    times: P10 published a `holy-lane-parity` column after #2266 deleted the
    gene (#2299, #2307 restored it at +99); #2307 had to state its build in
    prose; and on 2026-08-23 a sibling change was minutes from deleting
    `barbarian-hunt` while the first standard-shape screen re-priced it."""

    TAGS = ["alpha", "beta", "gamma"]

    def sources(self, data, legacy_shape=False, unverified_build=None, notes=None,
                at=None, now=None):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "screen.json"
            path.write_text(json.dumps(data))
            args = argparse.Namespace(source=[str(path)], legacy_shape=legacy_shape,
                                      unverified_build=unverified_build)
            with unittest.mock.patch.object(
                gene_ledger, "gene_tags_at",
                lambda commit: None if at is None else at.get(commit)
            ), unittest.mock.patch.object(
                gene_ledger, "gene_tags_now",
                lambda: list(self.TAGS if now is None else now)
            ):
                return gene_ledger.sources_from_args(args, notes)

    def screen(self, tags=None, **kwargs):
        tags = self.TAGS if tags is None else tags
        return stamped(analysis([{"tag": tag} for tag in tags]), tags, **kwargs)

    def refusal(self, data, **kwargs) -> str:
        with self.assertRaises(SystemExit) as refused:
            self.sources(data, **kwargs)
        return str(refused.exception)

    def test_a_screen_played_by_the_code_it_names_is_accepted(self):
        self.assertEqual(
            len(self.sources(self.screen(), at={"c" * 40: self.TAGS})), 1)

    def test_a_source_pricing_a_gene_its_commit_does_not_have_is_refused(self):
        """P10's shape exactly: the binary had `holy-lane-parity`, the commit
        it is recorded against does not."""
        said = self.refusal(self.screen(), at={"c" * 40: ["alpha", "beta"]})
        self.assertIn("NOT played by the code at the commit it names", said)
        self.assertIn("priced here but absent", said)
        self.assertIn("gamma", said)

    def test_a_source_missing_a_gene_its_commit_has_is_refused(self):
        """The other direction, which is what an unmeasured gene quietly looks
        like: the code has a gene this screen never compiled in."""
        said = self.refusal(self.screen(),
                            at={"c" * 40: self.TAGS + ["barbarian-hunt"]})
        self.assertIn("never compiled in", said)
        self.assertIn("barbarian-hunt", said)

    def test_a_reordered_gene_set_is_still_a_different_gene_set(self):
        said = self.refusal(self.screen(), at={"c" * 40: list(reversed(self.TAGS))})
        self.assertIn("different order", said)

    def test_an_unstamped_build_is_refused(self):
        said = self.refusal(self.screen(commit=""), at={})
        self.assertIn("names no commit", said)
        self.assertIn("CIVVIS_COMMIT", said)

    def test_a_dirty_build_is_refused(self):
        said = self.refusal(self.screen(dirty=True), at={"c" * 40: self.TAGS})
        self.assertIn("DIRTY tree", said)

    def test_a_commit_this_clone_cannot_read_is_refused_not_shrugged_at(self):
        said = self.refusal(self.screen(), at={})
        self.assertIn("cannot read", said)
        self.assertIn("git fetch", said)

    def test_an_edited_artefact_is_refused(self):
        """A stamp copied from a screen that did pass, onto a header it does
        not describe."""
        said = self.refusal(self.screen(fingerprint="f" * 64),
                            at={"c" * 40: self.TAGS})
        self.assertIn("does not describe its own header", said)
        self.assertIn("edited", said)

    def test_a_source_pricing_a_gene_the_repository_removed_is_refused(self):
        """2026-08-23's near miss: the screen and its commit agree, and the
        gene was deleted from the trunk in between."""
        said = self.refusal(self.screen(), at={"c" * 40: self.TAGS},
                            now=["alpha", "beta"])
        self.assertIn("no longer registers", said)
        self.assertIn("gamma", said)

    def test_the_shape_escape_does_not_waive_the_build_check(self):
        probe = stamped(analysis([{"tag": "alpha"}], map="pangaea"), self.TAGS, dirty=True)
        self.assertIn("DIRTY tree",
                      self.refusal(probe, legacy_shape=True, at={"c" * 40: self.TAGS}))

    def test_the_build_escape_does_not_waive_the_shape_check(self):
        probe = stamped(analysis([{"tag": "alpha"}], map="pangaea"), self.TAGS)
        said = self.refusal(probe, unverified_build="whatever",
                            at={"c" * 40: self.TAGS})
        self.assertIn("not played at the screen's shape", said)

    def test_the_escape_records_its_reason_beside_the_source(self):
        notes = {}
        self.sources(self.screen(dirty=True), unverified_build="rebuilt from a lost worktree",
                     notes=notes, at={"c" * 40: self.TAGS})
        self.assertEqual(notes, {"screen.json": "rebuilt from a lost worktree"})
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "screen.json"
            path.write_text(json.dumps(self.screen(dirty=True)))
            ledger = gene_ledger.build_ledger([path], filter_known=False,
                                              build_notes={"screen.json": "a reason"})
        source = ledger["sources"][0]
        self.assertEqual(source["unverified"], "a reason")
        self.assertEqual(source["build"]["commit"], "c" * 40)
        self.assertTrue(source["build"]["dirty"])
        # And a re-derivation carries the reason back, so `--check` reproduces
        # the file rather than reporting drift on its own record.
        self.assertEqual(gene_ledger.notes_from_ledger(ledger), {"screen.json": "a reason"})

    def test_an_escape_with_no_reason_is_not_an_escape(self):
        """`--unverified-build` takes a REASON, so an operator cannot wave a
        source through without saying why: an empty string is falsy and the
        refusal stands."""
        self.assertIn("DIRTY tree",
                      self.refusal(self.screen(dirty=True), unverified_build="",
                                   at={"c" * 40: self.TAGS}))


class PreFingerprintSources(unittest.TestCase):
    """The twenty sources recorded before 2026-08-23 carry no build block. They
    are grandfathered — the games are played, the artefacts are history — but
    they are NAMED, because a grandfather clause nobody can see is the same as
    no guard at all."""

    def test_a_source_with_no_build_block_is_grandfathered_and_named(self):
        data = analysis([{"tag": "a"}])
        self.assertEqual(gene_ledger.build_state(data), "pre-fingerprint")
        self.assertEqual(gene_ledger.build_gap(data, "old.json"), "")

    def test_a_grandfathered_source_records_no_build_and_prints_as_such(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "old.json"
            path.write_text(json.dumps(analysis([{"tag": "a"}])))
            ledger = gene_ledger.build_ledger([path], filter_known=False)
        self.assertNotIn("build", ledger["sources"][0])
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            gene_ledger.print_table(ledger)
        self.assertIn("pre-fingerprint", printed.getvalue())
        self.assertIn("predate the build stamp", printed.getvalue())

    def test_every_source_the_ledger_records_today_is_pre_fingerprint(self):
        """⚠ This is the boundary. Everything already in the ledger predates
        the stamp; the moment a stamped source is recorded it is checked, and
        it cannot become history by having its block removed without that
        removal showing up here."""
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        for source in current["sources"]:
            data = gene_ledger.load_source(gene_ledger.ROOT / source["path"])
            self.assertEqual(gene_ledger.build_state(data), "pre-fingerprint",
                             source["path"])
            self.assertNotIn("build", source, source["path"])

    def test_a_stamped_source_cannot_be_grandfathered_by_a_blank_stamp(self):
        data = analysis([{"tag": "a"}])
        data["profile"]["build"] = {"commit": "", "commit_source": "unstamped",
                                    "dirty": False, "genes_sha256": "",
                                    "binary_sha256": ""}
        self.assertEqual(gene_ledger.build_state(data), "stamped")
        self.assertIn("no gene-set fingerprint", gene_ledger.build_gap(data, "new.json"))


class Preregistration(unittest.TestCase):
    """⚠ THE ANALYSIS MUST NOT PRESENT A TRUNCATED RUN AS A COMPLETED ONE. P10
    stopped at 5,858 of a planned 10,000 games at the operator's request; the
    stop was legitimate, the unmarked artefact was not."""

    def test_a_partial_source_is_recorded_and_printed_as_partial(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "partial.json"
            path.write_text(json.dumps(preregistered(analysis([{"tag": "a"}], pairs=1000),
                                                     target=6000)))
            ledger = gene_ledger.build_ledger([path], filter_known=False)
        self.assertEqual(ledger["sources"][0]["batch"],
                         {"target_comparisons": 6000, "complete_comparisons": 1000,
                          "partial": True})
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            gene_ledger.print_table(ledger)
        self.assertIn("PARTIAL 1000/6000", printed.getvalue())

    def test_a_source_that_played_what_it_declared_reads_complete(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "whole.json"
            path.write_text(json.dumps(preregistered(analysis([{"tag": "a"}], pairs=6000),
                                                     target=6000)))
            ledger = gene_ledger.build_ledger([path], filter_known=False)
        self.assertFalse(ledger["sources"][0]["batch"]["partial"])

    def test_a_source_with_no_target_is_never_called_complete(self):
        data = analysis([{"tag": "a"}], pairs=1000)
        self.assertIsNone(gene_ledger.batch_of(data)["partial"])
        # A hand-written `batch` block — P10's, the prose ancestor of the
        # generated one — still declares nothing this tool can read against.
        data["batch"] = {"requested_target_games": 10000, "stopped_at_operator_request": True}
        self.assertIsNone(gene_ledger.batch_of(data)["partial"])


class TheHeaderFieldsMatch(unittest.TestCase):
    """⭐ The screen's shape is pinned on both sides; so are the provenance
    fields. A field added to `Build` or `Batch` in Rust and forgotten in this
    tool — or the reverse — fails here rather than reaching the ledger."""

    def rust_struct_fields(self, name: str) -> list[str]:
        text = (gene_ledger.ROOT / "src" / "bin" / "gene_screen.rs").read_text()
        start = text.index(f"struct {name} {{")
        body = text[start:text.index("\n}\n", start)]
        return re.findall(r"^    ([a-z0-9_]+): ", body, re.M)

    def test_the_build_stamp_names_the_same_fields_on_both_sides(self):
        self.assertEqual(self.rust_struct_fields("Build"), list(gene_ledger.BUILD_KEYS))

    def test_the_pre_registration_names_the_same_fields_on_both_sides(self):
        self.assertEqual(self.rust_struct_fields("Batch"), list(gene_ledger.BATCH_KEYS))

    def test_both_sides_read_the_same_source_tables(self):
        """The Rust side proves the parse against its compiled table; this side
        runs it over a commit. They must be looking at the same tables."""
        text = (gene_ledger.ROOT / "src" / "bin" / "gene_screen.rs").read_text()
        for path, table, _ in gene_ledger.GENE_TABLES:
            self.assertIn(table, text, table)
            self.assertIn(path, text, path)


class GeneratedFiles(unittest.TestCase):
    """`docs/gene_ledger.json` and `src/ai/advanced/gene_ledger_table.rs` are
    both derived from the sources the JSON records; neither may drift."""

    def test_the_checked_in_ledger_reproduces_from_its_recorded_sources(self):
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        ledger = gene_ledger.rebuild_from_ledger(current)
        self.assertEqual(gene_ledger.render_json(ledger), gene_ledger.LEDGER_JSON.read_text(),
                         "docs/gene_ledger.json is stale: run tools/gene_ledger.py --write")
        self.assertEqual(gene_ledger.render_rust(ledger), gene_ledger.LEDGER_RS.read_text(),
                         "gene_ledger_table.rs is stale: run tools/gene_ledger.py --write")

    def test_the_rust_table_is_valid_looking_and_names_every_gene_once(self):
        text = gene_ledger.LEDGER_RS.read_text()
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        for gene in current["genes"]:
            self.assertEqual(text.count(f'tag: "{gene["tag"]}",'), 1, gene["tag"])
        self.assertIn("GENERATED by tools/gene_ledger.py", text)


if __name__ == "__main__":
    unittest.main()
