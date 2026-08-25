"""The operator-pinned gene ledger, its screen evidence, source precedence,
and the generated files staying together with the recorded sources."""
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
import genes  # noqa: E402

# Both of the tools this file used to test are one module now; the old names
# stay as aliases so every test reads as it did.
gene_ledger = genes
ranking = genes


PLAYERS = gene_ledger.SCREEN["players"]

#: ★★★★ THE OPERATOR-PINNED DEPLOYMENT GENOME, 2026-08-24.
#: This is the exact list the agent plays. It retains the 36 selections that
#: were already deployed, then adds the sixteen explicit operator promotions:
#: `unit-cost-efficiency`, `unit-objective-memory`, `camp-party`,
#: `slot-kind-tiebreak`, `promote-when-wounded`, `religion-sues-peace`, and
#: `lane-great-people`, `one-launch-pad`, `civilian-rescue`,
#: `missionary-evades-raiders`, `district-planning`,
#: `missionary-last-charge-explores`, `settlement-gap-target`,
#: `religious-defence-scales`, `lane-policy-deck`, and
#: `science-multiplier-payoff`. Screen results remain evidence; they cannot
#: move this tuple automatically.
#: Changing it requires an intentional edit and PR note.
DEPLOYED_GENOME_20260824 = (
    "air-surge", "amenity-district-path", "apostle-promotion-by-role",
    "barbarian-bargain", "barbarian-scouts-are-scouts", "bounded-recovery",
    "buildings-before-projects", "camp-party", "civilian-rescue", "competition-victory-points",
    "culture-building-debt", "district-planning", "early-contact-window", "engine-faith-price",
    "escort-unstick", "founder-temple", "great-person-housing",
    "holy-lane-parity", "idle-faith-patronage", "inquisition-on-threat",
    "lane-culture-spending", "lane-great-people", "lane-policy-deck", "loyalty-rate-alarm", "maintenance-aware-deck",
    "missionary-evades-raiders", "missionary-last-charge-explores",
    "one-launch-pad",
    "opportunistic-war", "peacetime-deterrence", "price-the-suzerainty",
    "promote-when-wounded", "raid-pillage-prizes", "recon-replacement",
    "recorded-tactical-step", "relief-targets-the-siege", "religion-sues-peace",
    "religious-defence-scales", "religious-units-heal-first", "science-multiplier-payoff",
    "score-horizon", "settle-sooner", "settlement-gap-target", "settler-threat-detour", "slot-kind-tiebreak", "strike-opening",
    "theology-for-founders", "unit-cost-efficiency", "unit-objective-memory",
    "war-economy", "war-reinforcement",
    "wide-map-capacity",
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
            # These stand-in tags model evidence only; they deliberately do
            # not inherit the repository's real pinned deployment selection.
            return gene_ledger.build_ledger(paths, filter_known=False,
                                            deployment_genome=())

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
        self.assertEqual(ledger["rules"]["deployment_policy"], "operator-pinned")
        self.assertEqual(ledger["rules"]["deployment_genome"], [])

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
        # A legacy tranche of N matched pairs is 2N seats; the pair count is
        # kept beside it as the currency the ranking still speaks.
        self.assertEqual(measure["win_tranches"], [
            {"position": "latest", "seats": 20004, "pairs": 10002, "win_delta_pp": -1.234, "win_se_pp": 0.344, "win_z": -2.346},
            {"position": "previous", "seats": 19992, "pairs": 9996, "win_delta_pp": -0.988, "win_se_pp": 0.466, "win_z": -2.123},
            {"position": "earlier", "seats": 20004, "pairs": 10002, "win_delta_pp": -1.111, "win_se_pp": 0.437, "win_z": -2.543},
        ])

    def test_a_seat_screen_records_seats_and_their_pair_equivalent(self):
        """A screen since 2026-08-23 counts seats — every seat its own random
        genome — and the ledger carries the seat count beside the matched-pair
        currency the ranking's bands are still stated in."""
        data = analysis([{"tag": "a", "wz": 2.5}], pairs=1000)
        del data["complete_pairs"]
        data["seats"] = 1800
        data["games"] = 300
        data["profile"]["design"] = "independent"
        gene = data["genes"][0]
        del gene["pairs"]
        gene["seats"] = 1800
        gene["n_on"] = 1300
        gene["n_off"] = 500
        ledger = self.build([data])
        source = ledger["sources"][0]
        self.assertEqual((source["seats"], source["games"], source["complete_pairs"]),
                         (1800, 300, 900))
        self.assertEqual(source["shape"], "standard", "the draw design is not a leg of the shape")
        measure = ledger["genes"][0]["screen"]
        self.assertEqual((measure["seats"], measure["pairs"], measure["n_on"], measure["n_off"]),
                         (1800, 900, 1300, 500))

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
            args = argparse.Namespace(sources=[str(path)], legacy_shape=legacy_shape)
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

    def test_a_contested_field_batch_is_refused_as_a_source(self):
        """⚠⚠ The one refusal the map legs cannot make.

        `gene_screen --contested` pins rival seats to pursue a victory lane and
        seats native scored competitions; it changes NO map leg, so without
        `FIELDLESS` a contested batch reads `standard` and pools with every
        recorded column. A gene priced against a board racing for a diplomatic
        victory is not the same number as the same gene priced fieldless -- that
        is the entire reason the mode exists -- and the ledger must never hold
        both under one name."""
        contested = analysis([{"tag": "a"}], contested_field="diplomatic,culture",
                             native_competitions=True)
        self.assertEqual(gene_ledger.shape_of(gene_ledger.profile_of(contested)), "legacy")
        with self.assertRaises(SystemExit):
            self.sources(contested)
        # Each leg refuses on its own: a fieldless batch that still seats
        # competitions is not the world the ledger holds either.
        for leg in ({"contested_field": "diplomatic"}, {"native_competitions": True}):
            with self.subTest(leg=leg):
                probe = analysis([{"tag": "a"}], **leg)
                self.assertEqual(gene_ledger.shape_of(gene_ledger.profile_of(probe)), "legacy")
                self.assertIn(next(iter(leg)), gene_ledger.shape_gap(gene_ledger.profile_of(probe)))

    def test_the_retired_paired_field_is_not_a_contested_board(self):
        """⚠ `field` was already taken. Every header the paired designs wrote
        carries `field: "advanced"` -- the agent the treated seat played
        against -- and nine of them are recorded sources. Naming the new leg
        `field` reclassified all nine and `check` reported drift on the
        ledger's own history; this holds the two names apart."""
        legacy = analysis([{"tag": "a"}], field="advanced")
        profile = gene_ledger.profile_of(legacy)
        self.assertEqual(profile.get("contested_field", ""), "")
        self.assertEqual(gene_ledger.shape_of(profile), "standard")

    def test_a_source_written_before_the_leg_existed_records_no_leg(self):
        """The legs are recorded only when set, which is what keeps every
        record written before them byte-stable through `check`."""
        profile = gene_ledger.profile_of(analysis([{"tag": "a"}]))
        self.assertNotIn("contested_field", profile)
        self.assertNotIn("native_competitions", profile)

    def test_a_rotating_victory_mask_is_recorded_and_stays_the_standard_shape(self):
        """⭐ `--victory-mask rotate:N` closes N real conditions per game from
        the seed; `victories` stays the batch-level set and every lane is live
        across the batch, so the batch pools with the ledger, and the mask is
        written onto the source as provenance. An unmasked source records
        nothing, so every older record stays byte-stable."""
        masked = analysis([{"tag": "a"}], victory_mask="rotate:2")
        profile = gene_ledger.profile_of(masked)
        self.assertEqual(profile["victory_mask"], "rotate:2")
        self.assertEqual(profile["victories"], gene_ledger.SCREEN["victories"])
        self.assertEqual(gene_ledger.shape_of(profile), "standard")
        self.assertEqual(gene_ledger.shape_gap(profile), "")
        self.assertEqual(len(self.sources(masked)), 1, "accepted as a source")
        self.assertNotIn("victory_mask", gene_ledger.profile_of(analysis([{"tag": "a"}])))
        # A restricted batch-level set is still a probe, mask or no mask.
        probe = analysis([{"tag": "a"}], victories="domination,score", victory_mask="rotate:1")
        self.assertEqual(gene_ledger.shape_of(gene_ledger.profile_of(probe)), "legacy")

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
        # And the two legs that are NOT map legs: the binary writes them into
        # every header and this tool refuses on them, so a rename on one side
        # would silently stop refusing rather than fail anything.
        for field in gene_ledger.FIELDLESS:
            self.assertIn(f"{field}: ", rs, field)
            self.assertIn(f"header.{field}", rs, field)


class TheOperatorPinnedDefaults(unittest.TestCase):
    """Screen statistics remain recorded, but selection comes from an explicit
    list rather than a threshold, posterior, or pooled-difference rule."""

    def build(self, wins: list[int], deployment_genome=()):
        with tempfile.TemporaryDirectory() as tmp:
            sources = []
            for i, win in enumerate(wins):
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(analysis([{"tag": "g", "wins": win}])))
                sources.append(path)
            return gene_ledger.build_ledger(
                sources, filter_known=False, deployment_genome=deployment_genome)

    def test_measurements_do_not_automatically_change_a_pinned_gene(self):
        on = self.build([-500, 20, 21], deployment_genome=("g",))["genes"][0]
        off = self.build([500, 200, 100], deployment_genome=())["genes"][0]
        self.assertTrue(on["default_on"])
        self.assertFalse(off["default_on"])
        self.assertEqual(on["wins_last_10k"], 21)
        self.assertEqual(off["wins_last_10k"], 100)

    def test_the_pinned_list_rejects_unknown_tags_and_multiple_versions(self):
        with self.assertRaises(SystemExit):
            self.build([1], deployment_genome=("not-a-screenable-gene",))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "versions.json"
            path.write_text(json.dumps(analysis([{"tag": "g"}, {"tag": "g-2"}])))
            with self.assertRaises(SystemExit):
                gene_ledger.build_ledger(
                    [path], filter_known=False, deployment_genome=("g", "g-2"))


class TheDifferenceEvidence(unittest.TestCase):
    """The ranking still reports its pooled on-off difference, but it no
    longer acts as a deployment veto."""

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

    def test_the_ledger_records_the_difference_beside_the_columns(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertIn("win_diff", ledger["rules"])
        for gene in ledger["genes"]:
            self.assertIsInstance(gene["win_diff_pp"], float, gene["tag"])


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


class TheOperatorPinnedDeploymentGenome(unittest.TestCase):
    """The deployment selection is a deliberate list, not an alternate
    estimator or threshold configuration."""

    def test_the_shipped_genome_is_the_recorded_operator_selection(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertEqual(ledger["rules"]["deployment_policy"], "operator-pinned")
        self.assertEqual(tuple(ledger["rules"]["deployment_genome"]),
                         DEPLOYED_GENOME_20260824)
        selected = set(DEPLOYED_GENOME_20260824)
        measured = {g["tag"] for g in ledger["genes"]}
        self.assertEqual(
            {g["tag"] for g in ledger["genes"] if g["default_on"]},
            selected & measured,
        )
        self.assertEqual(ledger["counts"]["default_on"], 52)
        self.assertEqual(tuple(ledger["rules"]["operator_promotions"]),
                         gene_ledger.OPERATOR_PROMOTIONS_20260824)

    def test_the_ledger_records_evidence_without_redeciding_the_list(self):
        ledger = json.loads(gene_ledger.LEDGER_JSON.read_text())
        self.assertIn("posterior", ledger["rules"])
        self.assertNotIn("authority", ledger["rules"])
        selected = set(ledger["rules"]["deployment_genome"])
        for gene in ledger["genes"]:
            self.assertIsInstance(gene["posterior_pp"], float, gene["tag"])
            self.assertIsInstance(gene["posterior_se_pp"], float, gene["tag"])
            self.assertGreater(gene["posterior_se_pp"], 0.0, gene["tag"])
            self.assertEqual(gene["default_on"], gene["tag"] in selected, gene["tag"])

    def test_rebuild_preserves_the_recorded_pinned_selection(self):
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        rebuilt = gene_ledger.rebuild_from_ledger(current)
        self.assertEqual(rebuilt["rules"]["deployment_genome"],
                         current["rules"]["deployment_genome"])
        self.assertIn('pub(super) const DEPLOYMENT_POLICY: &str = "operator-pinned";',
                      gene_ledger.render_rust(rebuilt))


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

        def reader(path):
            # A commit older than the registry has no genes.rs.
            if path not in read:
                raise LookupError(path)
            return read[path]
        self.assertEqual(
            gene_ledger.gene_tags_from_sources(reader),
            ["war-reinforcement", "come-ashore", "strategic-wonders", "joint-tactics"],
        )

    def test_the_registry_is_read_in_order_and_host_only_rows_stay_out(self):
        """The one registry: every screenable row in order, a plain host-only
        row excluded, comments and the enable/disable paths ignored."""
        registry = '''
            pub const GENES: &[Gene] = &[
                // "decoy-one" in a comment
                Gene { tag: "war-reinforcement", field: "war_reinforcement", kind: Kind::Repair(Axis::War), enable: AdvancedAi::enable_war_reinforcement, disable: AdvancedAi::disable_war_reinforcement },
                Gene { tag: "land-grab", field: "land_grab", kind: Kind::HostOnly, enable: AdvancedAi::enable_land_grab, disable: AdvancedAi::disable_land_grab },
                Gene { tag: "strategic-wonders", field: "strategic_wonders", kind: Kind::Production, enable: AdvancedAi::enable_strategic_wonders, disable: AdvancedAi::disable_strategic_wonders },
                Gene { tag: "joint-tactics", field: "joint_tactics", kind: Kind::HostOnlyOptIn, enable: AdvancedAi::enable_joint_tactics, disable: AdvancedAi::disable_joint_tactics },
                Gene { tag: "war-economy-2", field: "war_economy_2", kind: Kind::OptIn, enable: AdvancedAi::enable_war_economy_2, disable: AdvancedAi::disable_war_economy_2 },
            ];
        '''
        read = {genes.REGISTRY: registry}

        def reader(path):
            if path not in read:
                raise LookupError(path)
            return read[path]
        self.assertEqual(
            gene_ledger.gene_tags_from_sources(reader),
            ["war-reinforcement", "strategic-wonders", "joint-tactics", "war-economy-2"],
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
                             "every gene the screen varies is a registered gene")
        # One tag of each screenable kind, so a parse that silently lost a
        # kind fails here. Deliberately structural rather than topical: naming
        # a gene under review would make this test a hostage to the next cull.
        self.assertIn("war-reinforcement", tags, "Kind::Repair")
        self.assertIn("strategic-wonders", tags, "Kind::Production")
        self.assertIn("joint-tactics", tags, "Kind::HostOnlyOptIn")
        self.assertNotIn("land-grab", tags, "a plain host-only gene is never screened")


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
            args = argparse.Namespace(sources=[str(path)], legacy_shape=legacy_shape,
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

    def test_a_reporting_build_exception_is_explicit_and_recorded(self):
        """Report-only data gets no silent provenance bypass."""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "screen.json"
            path.write_text(json.dumps(self.screen()))
            with self.assertRaises(SystemExit):
                gene_ledger.reporting_batch_records([path])
            records = gene_ledger.reporting_batch_records(
                [path], {path.name: "the remote build claim is unavailable"})
        self.assertEqual(records[0]["unverified"],
                         "the remote build claim is unavailable")

    def test_new_reporting_batch_evicts_the_oldest_fixed_slot(self):
        existing = [Path("last.json"), Path("prior.json"), Path("third.json")]
        self.assertEqual(
            gene_ledger.latest_reporting_batches([Path("new.json")], existing),
            [Path("new.json"), Path("last.json"), Path("prior.json")],
        )


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

    def test_recorded_sources_keep_their_build_provenance(self):
        """⚠ This is the boundary. The 38,160-seat cutoff is the first stamped
        source; older history stays visibly pre-fingerprint. A stamped source
        cannot become history by having its block removed without that removal
        showing up here."""
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        stamped = []
        for source in current["sources"]:
            data = gene_ledger.load_source(gene_ledger.ROOT / source["path"])
            if gene_ledger.build_state(data) == "stamped":
                stamped.append(source["path"])
                self.assertIn("build", source, source["path"])
            else:
                self.assertEqual(gene_ledger.build_state(data), "pre-fingerprint",
                                 source["path"])
                self.assertNotIn("build", source, source["path"])
        self.assertEqual(stamped, [
            "docs/gene_screens/2026-08-24-standard-continuous-38160-total-seats.json"
        ])

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
        # A legacy file pre-registered 6,000 matched comparisons and played
        # 1,000: in seats, 12,000 intended and 2,000 played.
        self.assertEqual(ledger["sources"][0]["batch"],
                         {"target_seats": 12000, "complete_seats": 2000,
                          "partial": True})
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            gene_ledger.print_table(ledger)
        self.assertIn("PARTIAL 2000/12000 seats", printed.getvalue())

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

    def test_both_sides_read_the_same_registry(self):
        """The Rust side proves the parse against its compiled table; this side
        runs it over a commit. They must be looking at the same registry."""
        text = (gene_ledger.ROOT / "src" / "bin" / "gene_screen.rs").read_text()
        path, table, _ = gene_ledger.GENE_TABLES[0]
        self.assertIn(table, text, table)
        self.assertIn(path, text, path)


class GeneratedFiles(unittest.TestCase):
    """`docs/gene_ledger.json`, the verdict block at the end of
    `src/ai/advanced/genes.rs` and `HEURISTIC_GENE_RANKING.md` are all derived
    from the sources the JSON records; none may drift."""

    def test_the_checked_in_ledger_reproduces_from_its_recorded_sources(self):
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        ledger = gene_ledger.rebuild_from_ledger(current)
        self.assertEqual(gene_ledger.render_json(ledger), gene_ledger.LEDGER_JSON.read_text(),
                         "docs/gene_ledger.json is stale: run tools/genes.py write")
        self.assertEqual(gene_ledger.render_rust(ledger),
                         genes.rust_block_of(genes.REGISTRY_PATH.read_text()),
                         "the verdict block in genes.rs is stale: run tools/genes.py write")
        self.assertEqual(genes.render(ledger), genes.RANKING_MD.read_text(),
                         "HEURISTIC_GENE_RANKING.md is stale: run tools/genes.py write")

    def test_display_batches_do_not_change_the_pinned_deployment_selection(self):
        """A display-only screen refreshes evidence but never rewrites defaults."""
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        deployment_only = gene_ledger.build_ledger(
            gene_ledger.sources_from_ledger(current),
            build_notes=gene_ledger.notes_from_ledger(current),
            deployment_genome=gene_ledger.deployment_genome_of(current),
        )
        with_reporting = gene_ledger.rebuild_from_ledger(current)
        self.assertEqual(
            [(g["tag"], g["default_on"]) for g in deployment_only["genes"]],
            [(g["tag"], g["default_on"]) for g in with_reporting["genes"]],
        )
        self.assertEqual(gene_ledger.render_rust(deployment_only),
                         gene_ledger.render_rust(with_reporting))

        reporting = ranking.load_reporting_batches(current)
        self.assertEqual(len(reporting), len(ranking.REPORTING_BATCH_LABELS))
        for batch in reporting:
            newest = batch["meta"]
            artifact = ranking.load_source(ranking.ROOT / newest["path"])
            self.assertEqual(newest["seats"], artifact["seats"])
            self.assertEqual(newest["games"], artifact["games"])
            self.assertEqual(newest["batch"], ranking.batch_of(artifact))
            self.assertTrue(newest["build"]["commit"])
            self.assertFalse(newest["build"]["dirty"])
            if newest.get("unverified"):
                self.assertGreater(len(newest["unverified"]), 40)
            self.assertNotIn(newest["path"], {s["path"] for s in current["sources"]})
        authoritative, _ = ranking.load_sources(current)
        displayed, _ = ranking.load_display_sources(current)
        self.assertIn("engine-faith-price", authoritative)
        self.assertIn("engine-faith-price", displayed)

    def test_the_verdict_block_sits_under_the_rows_and_names_every_gene_once(self):
        text = genes.REGISTRY_PATH.read_text()
        rows_end = text.index("];\n")  # the end of `GENES`
        self.assertIn(genes.GENERATED_BEGIN, text)
        self.assertGreater(text.index(genes.GENERATED_BEGIN), rows_end,
                           "the generated block comes after the hand-written rows")
        block = genes.rust_block_of(text)
        self.assertTrue(block.rstrip().endswith(genes.GENERATED_END))
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        for gene in current["genes"]:
            self.assertEqual(block.count(f'tag: "{gene["tag"]}",'), 1, gene["tag"])
        # The writer never touches the rows above the marker.
        head = text[: text.index(genes.GENERATED_BEGIN)]
        self.assertEqual(genes.registry_with_block(block), head + block)


class VersionedGenes(unittest.TestCase):
    """⭐ An improvement to a gene is a new gene, `<base>-<n>`, screened beside
    the original; the deployment genome carries at most one version of a
    family. The pinned selection, rather than a statistic, chooses that
    version."""

    def test_families_are_read_off_the_tags(self):
        self.assertEqual(
            gene_ledger.families_of(["war-economy", "war-economy-2", "war-economy-3",
                                     "one-launch-pad", "war-economy-1", "b-2"]),
            [["war-economy", "war-economy-2", "war-economy-3"]])
        self.assertEqual(gene_ledger.families_of(["a", "a-10", "a-9"]), [["a", "a-9", "a-10"]])

    def test_family_annotation_never_rewrites_the_pinned_choice(self):
        genes = [
            {"tag": "war-economy", "default_on": True, "wins_last_10k": 40, "win_diff_pp": 0.4},
            {"tag": "war-economy-2", "default_on": False, "wins_last_10k": 70, "win_diff_pp": 0.7},
            {"tag": "war-economy-3", "default_on": False, "wins_last_10k": -5, "win_diff_pp": -0.05},
            {"tag": "other", "default_on": True, "wins_last_10k": 10, "win_diff_pp": 0.1},
        ]
        gene_ledger.annotate_families(genes)
        by = {g["tag"]: g for g in genes}
        self.assertTrue(by["war-economy"]["default_on"])
        self.assertFalse(by["war-economy-2"]["default_on"])
        self.assertFalse(by["war-economy-3"]["default_on"])
        self.assertEqual((by["war-economy"]["family"], by["war-economy"]["version"]), ("war-economy", 1))
        self.assertEqual((by["war-economy-3"]["family"], by["war-economy-3"]["version"]), ("war-economy", 3))
        self.assertNotIn("family", by["other"], "a gene with no versions is not a family")
        self.assertTrue(by["other"]["default_on"])

    def test_the_pinned_list_rejects_multiple_versions_of_a_family(self):
        with self.assertRaises(SystemExit):
            gene_ledger.normalize_deployment_genome(
                ("g", "g-2"), allowed_tags={"g", "g-2"})
        self.assertEqual(
            gene_ledger.normalize_deployment_genome(("g-2",), allowed_tags={"g", "g-2"}),
            ("g-2",),
        )
        self.assertEqual(gene_ledger.tracked_wins({"wins_last_10k": 25}), 0.25,
                         "ranking order still falls back to the newest column")

    def test_the_ranking_names_the_best_version_and_shows_the_best_two_rates(self):
        """Operator, 2026-08-23: a *Best version* column after *Description*,
        and a versioned row's on/off cells list the best two versions — each
        version's on is only that version on; anything else is its off."""
        tags = ["plain", "g", "g-2", "g-3"]
        measured = {
            "plain": [{"win_on": 0.20, "win_off": 0.16, "n_on": 100, "n_off": 300}],
            "g": [{"win_on": 0.18, "win_off": 0.16, "n_on": 1000, "n_off": 3000}],
            "g-2": [{"win_on": 0.21, "win_off": 0.16, "n_on": 500, "n_off": 3500}],
            "g-3": [{"win_on": 0.19, "win_off": 0.16, "n_on": 400, "n_off": 3600}],
        }
        verdict = {
            "g": {"default_on": False, "win_diff_pp": 2.0},
            "g-2": {"default_on": True, "win_diff_pp": 5.0},
            "g-3": {"default_on": False, "win_diff_pp": 3.0},
        }
        self.assertEqual(gene_ledger.family_of("g-3", tags), ["g", "g-2", "g-3"])
        self.assertEqual(gene_ledger.family_of("plain", tags), [])
        self.assertEqual(gene_ledger.best_versions(["g", "g-2", "g-3"], verdict, measured),
                         ["g-2", "g-3", "g"], "the shipping version leads, then tracked wins")
        for tag in ("g", "g-2", "g-3"):
            self.assertEqual(gene_ledger.best_version_cell(tag, tags, verdict, measured), "2", tag)
        self.assertEqual(gene_ledger.best_version_cell("plain", tags, verdict, measured), "—")
        self.assertEqual(
            gene_ledger.family_rate_cells("g", tags, verdict, measured),
            ("v2 21.00% (n=500) · v3 19.00% (n=400)", "v2 16.00% (n=3,500) · v3 16.00% (n=3,600)"))
        self.assertIsNone(gene_ledger.family_rate_cells("plain", tags, verdict, measured))
        # When no version is pinned, the display's ordering is still useful;
        # a version the ledger has not recorded is read off its display record.
        loose = {"g": {"win_diff_pp": 3.0}, "g-2": {"win_diff_pp": 4.0}}
        self.assertEqual(gene_ledger.best_versions(["g", "g-2", "g-3"], loose, measured),
                         ["g-2", "g-3", "g"],
                         "g-3 reads 3.0 off its display record and ties g; the higher version leads")
        self.assertEqual(gene_ledger.best_version_cell("g", tags, loose, measured), "2")
        # An unpriced version that ships still leads; an unpriced family with
        # nothing shipping has no best version yet.
        fresh = {"g": {"default_on": True}}
        self.assertEqual(gene_ledger.best_version_cell("g-2", tags, fresh, {}), "1")
        self.assertIsNone(gene_ledger.family_rate_cells("g-2", tags, fresh, {}))
        self.assertEqual(gene_ledger.best_version_cell("g-2", tags, {}, {}), "—")

    def test_the_explicit_version_is_emitted_in_the_generated_table(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "s.json"
            path.write_text(json.dumps(analysis([
                {"tag": "g", "wz": 3.0, "wins": 40},
                {"tag": "g-2", "wz": 3.5, "wins": 70},
            ])))
            ledger = gene_ledger.build_ledger(
                [path], filter_known=False, deployment_genome=("g-2",))
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertTrue(by["g-2"]["default_on"])
        self.assertFalse(by["g"]["default_on"])
        rust = gene_ledger.render_rust(ledger)
        self.assertIn('tag: "g", verdict: Verdict::Helps, default_on: false,', rust)
        self.assertIn('"g-2",', rust)
        self.assertEqual(ledger["counts"]["default_on"], 1)
        self.assertEqual(ledger["rules"]["deployment_genome"], ["g-2"])


# ═════════════════════════════════════════════════════════════════════════════
# THE RANKING (formerly tools/test_heuristic_gene_ranking.py)
# ═════════════════════════════════════════════════════════════════════════════



#: The main table's columns, in order. The batch sample sizes are derived from
#: their immutable reporting artefacts so an automated completed-batch publish
#: does not require a hand edit to a stale golden number. `rebuild_from_ledger`
#: above independently re-reads those files and makes the generated ledger and
#: ranking byte-for-byte current; this helper only gives every cell assertion a
#: stable, named column to read.
def expected_columns() -> str:
    ledger = json.loads(ranking.LEDGER_JSON.read_text())
    batches = ranking.load_reporting_batches(ledger)
    slots = batches + [None] * (len(ranking.REPORTING_BATCH_LABELS) - len(batches))
    reporting = " | ".join(
        ranking.reporting_batch_header(label, batch)
        for label, batch in zip(ranking.REPORTING_BATCH_LABELS, slots)
    )
    return (
        "| Rank | Gene | Description | Best version | Default | "
        + reporting
        + " | Total (on) Win rate | Total (off) Win rate | Diff | "
        "Posterior (95% CI) | P(>0) | Share Δpp (z) | "
        "cost (compute) | cost (time) |"
    )


EXPECTED_COLUMNS = expected_columns()

#: Every column by name, so an assertion says which cell it reads instead of
#: counting to it.
#:
#: ⚠ THE INDICES USED TO BE WRITTEN OUT, and inserting the third batch
#: column between `prior` and the win rates moved six of them along by one —
#: every positional assertion in this file began reading its neighbour. Named
#: lookup makes the next inserted column one loud header mismatch instead of
#: six assertions quietly checking the wrong cell.
COLUMN = {
    name: index
    for index, name in enumerate(
        c.strip() for c in EXPECTED_COLUMNS.strip().strip("|").split(" | ")
    )
}


def cell(cells, name):
    """One named cell of a split table row."""
    return cells[COLUMN[name]]


class TheTableIsDerived(unittest.TestCase):
    #: Kept as a class attribute too: other classes read
    #: `TheTableIsDerived.EXPECTED_COLUMNS` to find the table in the file.
    EXPECTED_COLUMNS = EXPECTED_COLUMNS

    def test_the_checked_in_table_matches_the_ledgers_sources(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.assertEqual(
            ranking.render(ledger),
            ranking.RANKING_MD.read_text(),
            "HEURISTIC_GENE_RANKING.md is stale: run tools/genes.py write",
        )

    def test_every_screenable_gene_is_visible(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_display_sources(ledger)
        text = ranking.RANKING_MD.read_text()
        for tag in ranking.screenable_tags():
            if tag in measured:
                self.assertIn(f"`{tag}`", text, tag)
            else:
                self.assertIn("## Awaiting measurement", text)
                self.assertIn(f"| `{tag}` | off (unmeasured) |", text, tag)
        self.assertNotIn("`step-and-reassess` | ", text, "a host-only flag is not ranked")

    def test_descriptions_come_from_the_toggle_docs(self):
        desc = ranking.descriptions()
        self.assertGreater(len(desc), 50)
        self.assertTrue(desc["recon-replacement"].startswith("Rebuild the recon arm"))
        self.assertTrue(desc["loyalty-rate-alarm"].startswith("Rank loyalty emergencies"))

    def test_operator_columns_are_in_the_requested_order(self):
        header = next(
            line for line in ranking.RANKING_MD.read_text().splitlines()
            if line.startswith("| Rank |")
        )
        self.assertEqual(header, self.EXPECTED_COLUMNS)

    def _ranked_rows(self):
        """Every ranked row of the main table, split into its cells."""
        lines = ranking.RANKING_MD.read_text().splitlines()
        start = lines.index(self.EXPECTED_COLUMNS) + 2
        rows = []
        for line in lines[start:]:
            if not line.startswith("| "):
                break
            rows.append([c.strip() for c in line.strip().strip("|").split(" | ")])
        return rows

    def test_diff_is_the_on_rate_minus_the_off_rate(self):
        """The column that replaced the pooled seat count (operator, 2026-08-22).

        It is the WHOLE on−off difference, so it sits at roughly twice the
        scale of the win columns beside it and must be judged against a
        screen's difference band, not the halved column band the table prints.
        """
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_display_sources(ledger)
        rows = self._ranked_rows()
        self.assertGreater(len(rows), 50)
        for cells in rows:
            history = measured[cell(cells, "Gene").strip("`")]
            on_seats = sum(m["n_on"] for m in history)
            off_seats = sum(m["n_off"] for m in history)
            on = sum(m["win_on"] * m["n_on"] for m in history) / on_seats
            off = sum(m["win_off"] * m["n_off"] for m in history) / off_seats
            self.assertEqual(cell(cells, "Diff"), ranking.diff_cell(history),
                             cell(cells, "Gene"))
            self.assertRegex(cell(cells, "Diff"), r"^-?\d+\.\d\d%$",
                             cell(cells, "Gene"))
            # Taken off the unrounded rates, so it can land a hundredth away
            # from subtracting the two printed cells by eye — 0.01% against a
            # band of half a point. Never further: that would be a real slip.
            # A versioned row leads with `v<n> `; the first rate is the row's
            # own only when the gene is the family's best, so read plain rows.
            if cell(cells, "Best version") != "—":
                continue
            shown = (float(cell(cells, "Total (on) Win rate").split("%")[0])
                     - float(cell(cells, "Total (off) Win rate").split("%")[0]))
            self.assertAlmostEqual(100 * (on - off), shown, delta=0.011,
                                   msg=cell(cells, "Gene"))
        self.assertNotIn("Total seats (on+off)", ranking.RANKING_MD.read_text())

    def test_diff_cell_is_a_percent_and_keeps_a_negative_sign(self):
        def arms(on, off):
            return [{"win_on": on, "win_off": off, "n_on": 1000, "n_off": 1000}]

        self.assertEqual(ranking.diff_cell(arms(0.17, 0.15)), "2.00%")
        self.assertEqual(ranking.diff_cell(arms(0.15, 0.17)), "-2.00%")

    def test_rows_rank_by_descending_pooled_diff(self):
        """The rendered Rank column follows its displayed pooled Diff column."""
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_display_sources(ledger)
        shown = []
        for cells in self._ranked_rows():
            tag = cell(cells, "Gene").strip("`")
            shown.append((tag, ranking.pooled_win_diff_pp(measured[tag])))
        self.assertEqual(shown, sorted(shown, key=lambda row: (-row[1], row[0])))

    def test_the_printed_diff_includes_the_display_batch_but_not_the_default(self):
        """The completed 10k report refreshes display statistics only.

        Its shown *Diff* must be calculated from the displayed rows, while
        deployment still follows the unmodified ledger record. This prevents a
        reporting refresh from silently changing game rules.
        """
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_display_sources(ledger)
        recorded = {g["tag"]: g for g in ledger["genes"]}
        self.assertGreater(len(recorded), 50)
        for cells in self._ranked_rows():
            tag = cell(cells, "Gene").strip("`")
            self.assertEqual(cell(cells, "Diff"), ranking.diff_cell(measured[tag]), tag)
            if tag in recorded:
                self.assertEqual(
                    cell(cells, "Default"),
                    "**on**" if recorded[tag]["default_on"] else "off",
                    tag,
                )

    def test_each_win_rate_cell_carries_its_own_sample_size(self):
        """`n` is per arm, not one pooled figure: the arms are equal only while
        every screen that measured a gene split them evenly, and the row reads
        them from `n_on`/`n_off` separately so an uneven screen shows up."""
        one = r"\d+\.\d\d% \(n=[\d,]+\)"
        for cells in self._ranked_rows():
            versioned = cell(cells, "Best version") != "—"
            for rate in (cell(cells, "Total (on) Win rate"),
                         cell(cells, "Total (off) Win rate")):
                self.assertRegex(
                    rate,
                    rf"^v\d+ {one}( · v\d+ {one})?$" if versioned else rf"^{one}$",
                    cell(cells, "Gene"))

    def test_each_batch_cell_is_scaled_to_10k_with_n_in_the_header_only(self):
        """Each fixed batch has one total-seat `n` header, never row clutter."""
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        batches = ranking.load_reporting_batches(ledger)
        self.assertEqual(len(batches), 3)
        slots = batches + [None] * (len(ranking.REPORTING_BATCH_LABELS) - len(batches))
        columns = tuple(
            (index, ranking.reporting_batch_header(label, batch))
            for index, (label, batch) in enumerate(
                zip(ranking.REPORTING_BATCH_LABELS, slots))
        )
        for cells in self._ranked_rows():
            tag = cell(cells, "Gene").strip("`")
            for back, column in columns:
                expected = ranking.reporting_batch_cell(batches[back], tag)
                self.assertEqual(cell(cells, column), expected, f"{tag}: {column}")
                self.assertNotIn("n=", cell(cells, column), f"{tag}: {column}")

    def test_batch_win_cell_does_not_repeat_its_sample_size(self):
        history = [{
            "win_on": 1 / 6 + 0.01,
            "players": 6,
            "n_on": 1300,
            "n_off": 500,
        }]
        self.assertEqual(ranking.batch_win_cell(history), "+100")

    def test_load_sources_preserves_explicit_arm_sizes_without_legacy_pairs(self):
        source = {
            "profile": {"players": 6},
            "genes": [
                {
                    "tag": "explicit-arms",
                    "win_on": 0.18, "win_off": 0.16,
                    "n_on": 1300, "n_off": 500,
                    "win_z": 1.0, "share_z": 0.0,
                    "win_delta_pp": 2.0, "win_se_pp": 1.0,
                    "share_delta_pp": 0.0,
                },
                {
                    "tag": "legacy-seats",
                    "win_on": 0.18, "win_off": 0.16, "seats": 1800,
                    "win_z": 1.0, "share_z": 0.0,
                    "win_delta_pp": 2.0, "win_se_pp": 1.0,
                    "share_delta_pp": 0.0,
                },
            ],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "batch.json"
            path.write_text(json.dumps(source))
            measured, _ = ranking.load_sources({"sources": [{
                "path": str(path), "shape": "standard",
            }]})
        self.assertEqual((measured["explicit-arms"][0]["n_on"],
                          measured["explicit-arms"][0]["n_off"]), (1300, 500))
        self.assertEqual((measured["legacy-seats"][0]["n_on"],
                          measured["legacy-seats"][0]["n_off"]), (900, 900))

    def test_descriptions_print_whole(self):
        """The Description column was widened 160 → 480 characters, which is
        past the longest first sentence in the registry — so nothing clips."""
        longest = max(len(d) for d in ranking.descriptions().values())
        self.assertLess(longest, ranking.DESCRIPTION_CHARS)
        for cells in self._ranked_rows():
            self.assertNotIn("\u2026", cell(cells, "Description"),
                             cell(cells, "Gene"))

    def test_cost_uses_the_newest_real_measurement_and_never_invents_zero(self):
        history = [
            {"compute_cost_pct": 90.0, "compute_cost_se_pct": 4.0},
            {"compute_cost_pct": None, "compute_cost_se_pct": None},
            {"compute_cost_pct": 1.234, "compute_cost_se_pct": 0.456},
        ]
        self.assertEqual(
            ranking.cost_cell(history, "compute_cost_pct", "compute_cost_se_pct"),
            "+1.23% ±0.46%",
        )
        self.assertEqual(
            ranking.cost_cell([{}], "compute_cost_pct", "compute_cost_se_pct"),
            "–",
        )

    def test_the_band_is_the_columns_own_scale_not_the_differences(self):
        """A column is half the on−off difference, so its band is half too.

        The regression this guards: the header quoted ±110/10k — correct for
        `win_delta_pp`, twice too wide for the column beside it — and #2266
        removed eight genes calling readings up to that "inside the noise".
        """
        self.assertAlmostEqual(ranking.column_se(1.0), ranking.PER / 200.0)
        # Proved from a screen's own numbers rather than asserted: the
        # resolution column is half the on-off difference, so `column /
        # column_se` must reproduce `win_z` exactly. An independent screen's
        # arms need not be symmetric about chance, which is why this uses the
        # measured difference instead of `win_on - chance`.
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        source = ledger["sources"][-1]
        data = json.loads((ranking.ROOT / source["path"]).read_text())
        for gene in data["genes"]:
            column = float(gene["win_delta_pp"]) * ranking.PER / 200.0
            se = ranking.column_se(float(gene["win_se_pp"]))
            self.assertAlmostEqual(column / se, float(gene["win_z"]), places=6, msg=gene["tag"])

    def test_every_screen_prints_its_own_band_and_its_shape(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        rows = ranking.resolutions(ledger)
        self.assertEqual(len(rows), len(ledger["sources"]))
        text = ranking.RANKING_MD.read_text()
        for row in rows:
            # ⭐ The shape is printed beside the band because a `legacy` row is
            # a reading from the retired Pangaea instrument, not from the
            # screen the ledger now accepts.
            self.assertIn(f"`{row['name']}` | {row['shape']} | {row['genes']} |", text, row["name"])
        # Screens are not interchangeable, so no single number serves — and
        # none of them is the retired ±110.
        self.assertGreater(len({round(r["band"]) for r in rows}), 1)
        self.assertTrue(all(round(r["band"]) != 110 for r in rows))
        self.assertIn("twice too wide", text)

    def test_the_band_is_not_explained_by_gene_count(self):
        """The table's own rows make gene count look causal; this holds the
        falsifier the prose cites, so neither can rot silently.

        `h1` carries ONE gene over 7,200 pairs and still resolves wider, at a
        lower pairing gain, than four-gene `s6` over 6,000 — because its gene
        changes nearly every game and `s7`'s rarely fires. A foldover cancels
        only what the arms play in common.
        """
        def read(name):
            data = json.loads((ranking.ROOT / "docs" / "gene_screens" / name).read_text())
            errors = sorted(ranking.column_se(float(g["win_se_pp"])) for g in data["genes"])
            median = errors[len(errors) // 2]
            pairs = int(data["complete_pairs"])
            players = int(data["profile"]["players"])
            gain = ranking.unpaired_constant(players) / (median * (pairs ** 0.5))
            return len(errors), pairs, ranking.POWER_80 * median, gain

        h1 = read("2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json")
        s6 = read("2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json")
        self.assertLess(h1[0], s6[0], "h1 must carry fewer genes")
        self.assertGreater(h1[1], s6[1], "h1 must carry more pairs")
        self.assertGreater(h1[2], s6[2], "yet h1 must resolve WIDER — the whole point")
        self.assertLess(h1[3], s6[3], "and at a lower pairing gain")
        self.assertIn("Pairing gain", ranking.RANKING_MD.read_text())


    def test_the_pinned_default_summary_is_the_only_text_ahead_of_the_table(self):
        """The title gets one concise, generated policy before the table.

        The long reference remains below the tables; this is the operator's
        requested at-a-glance explanation of the *Default* column.
        """
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        lines = ranking.RANKING_MD.read_text().splitlines()
        self.assertEqual(lines[0], "# The heuristic gene ranking")
        self.assertEqual(lines[1], "")
        self.assertEqual(
            lines[2], ranking.default_on_summary(ledger),
            "the heading summary must derive from the pinned deployment genome",
        )
        self.assertEqual(lines[3], "")
        self.assertTrue(lines[4].startswith("| Rank | Gene |"), lines[4])
        self.assertTrue(lines[5].startswith("|---:|"), lines[5])
        self.assertTrue(lines[6].startswith("| 1 | `"), lines[6])

    def test_the_reference_is_carried_under_the_tables_not_deleted(self):
        """Moving the preamble must not become dropping it.

        Every derived paragraph the header used to open with is load-bearing —
        the band correction in particular is why a culled gene came back — so
        each is asserted present, and after the last table rather than before
        the first.
        """
        text = ranking.RANKING_MD.read_text()
        self.assertIn("## How to read this", text)
        for phrase in (
            "Reading the table",
            "What each screen resolves",
            "Pairing gain",
            "twice too wide",
            "**Cost.**",
            "Regenerate with",
        ):
            self.assertIn(phrase, text, phrase)
        self.assertLess(
            text.index("| Rank | Gene |"),
            text.index("## How to read this"),
            "the reference must sit under the table, not over it",
        )
        for heading in ("## Awaiting measurement", "## Removed from the code"):
            if heading in text:
                self.assertLess(text.index(heading), text.index("## How to read this"), heading)


class ThePosteriorIsPublishedAsEvidence(unittest.TestCase):
    """The precision-weighted posterior: printed beside the win columns,
    deciding nothing, with the delta it would make published under it."""

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.authoritative, _ = ranking.load_sources(self.ledger)
        self.measured, _ = ranking.load_display_sources(self.ledger)
        self.text = ranking.RANKING_MD.read_text()

    def _rows(self):
        lines = self.text.splitlines()
        start = lines.index(TheTableIsDerived.EXPECTED_COLUMNS) + 2
        rows = []
        for line in lines[start:]:
            if not line.startswith("| "):
                break
            rows.append([c.strip() for c in line.strip().strip("|").split(" | ")])
        return rows

    def test_the_printed_posterior_uses_the_displayed_observations(self):
        """The table's posterior moves with its report-only display batch."""
        seen = 0
        for cells in self._rows():
            tag = cell(cells, "Gene").strip("`")
            posterior = ranking.posterior_of(self.measured[tag])
            self.assertEqual(cell(cells, "Posterior (95% CI)"),
                             ranking.posterior_cell(posterior), tag)
            self.assertEqual(cell(cells, "P(>0)"),
                             ranking.probability_cell(posterior), tag)
            seen += 1
        self.assertGreater(seen, 50)

    def test_the_shrinkage_is_visible_in_the_probability_not_the_point(self):
        """The operator's own framing: a +30 from a ±64 screen and a +30 from
        a ±29 screen must not read the same. They print the same point and
        different `P(>0)`, making the difference in precision visible."""
        wide = ranking.posterior_of([{"win_delta_pp": 0.6, "win_se_pp": 0.4576,
                                      "shape": "legacy"}])
        tight = ranking.posterior_of([{"win_delta_pp": 0.6, "win_se_pp": 0.2052,
                                       "shape": "legacy"}])
        self.assertEqual(ranking.posterior_cell(wide).split(" ")[0],
                         ranking.posterior_cell(tight).split(" ")[0])
        self.assertNotEqual(ranking.probability_cell(wide),
                            ranking.probability_cell(tight))

    def test_nothing_the_posterior_publishes_decides_a_default(self):
        """★ The hard constraint of this change. The ranking's *Default*
        column is the ledger's explicit operator selection, gene for gene."""
        recorded = {g["tag"]: g for g in self.ledger["genes"]}
        self.assertEqual(self.ledger["rules"]["deployment_policy"], "operator-pinned")
        selected = set(self.ledger["rules"]["deployment_genome"])
        for cells in self._rows():
            tag = cell(cells, "Gene").strip("`")
            self.assertEqual(
                cell(cells, "Default"),
                "**on**" if tag in selected else "off",
                cell(cells, "Gene"))
            if tag in recorded:
                self.assertEqual(recorded[tag]["default_on"], tag in selected,
                                 cell(cells, "Gene"))

    def test_unmeasured_pinned_genes_stay_on_in_the_rendered_ranking_and_list(self):
        """An authoritative row is optional; the explicit selection is not."""
        recorded = {g["tag"] for g in self.ledger["genes"]}
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(ranking.main(["list"]), 0)
        list_rows = output.getvalue()
        displayed = {
            cell(cells, "Gene").strip("`"): cell(cells, "Default")
            for cells in self._rows()
        }
        for tag in ("missionary-evades-raiders", "missionary-last-charge-explores"):
            self.assertNotIn(tag, recorded, tag)
            self.assertEqual(displayed[tag], "**on**", tag)
            self.assertRegex(list_rows, rf"(?m)^{re.escape(tag)}\s+.+\s+on\s+unmeasured$")

    def test_the_evidence_section_marks_pinned_state_without_a_counterfactual_rule(self):
        self.assertIn("## Evidence for future operator selections", self.text)
        self.assertNotIn("## What the posterior would change", self.text)
        rows = ranking.evidence_table(self.ledger, self.authoritative)
        selected = set(self.ledger["rules"]["deployment_genome"])
        self.assertTrue(rows)
        for row in rows:
            self.assertEqual(row["pinned"], row["tag"] in selected, row["tag"])
            self.assertIn(row["call"], {"on", "off", "unresolved"})

    def test_the_evidence_call_covers_every_priced_gene(self):
        rows = ranking.evidence_table(self.ledger, self.authoritative)
        calls = [r["call"] for r in rows]
        self.assertEqual(len(rows), len(calls))
        self.assertGreater(calls.count("on"), 0)
        self.assertGreater(calls.count("unresolved"), calls.count("on"))
        self.assertIn("### What the posterior resolves", self.text)
        for row in rows:
            if row["call"] != "unresolved":
                self.assertIn(f"| `{row['tag']}` |", self.text, row["tag"])
        # A host-only flag carries a ledger row from a retired native
        # stand-in and the ledger never governs it, so it is in no table here.
        self.assertNotIn("step-and-reassess", {r["tag"] for r in rows})

    def test_the_shapes_are_published_apart(self):
        self.assertIn("## The two shapes, apart", self.text)
        self.assertIn("| standard |", self.text)
        self.assertIn("| legacy |", self.text)
        # Today every source is legacy; the file must say so rather than let a
        # reader take a Pangaea column for the deployment shape.
        shapes = {s["shape"] for s in self.ledger["sources"]}
        if shapes == {"legacy"}:
            self.assertIn("No `standard` source is in the ledger yet", self.text)


class TheBoundarySet(unittest.TestCase):
    """`--boundary`: the genes whose interval straddles the decision line,
    ranked by what one single-gene direct arm would buy, printed as an
    argument list."""

    maxDiff = None

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.measured, _ = ranking.load_sources(self.ledger)

    def test_the_boundary_is_exactly_the_straddling_intervals(self):
        rows, arm = ranking.boundary_table(self.ledger, self.measured)
        self.assertIsNotNone(arm)
        every = ranking.evidence_table(self.ledger, self.measured)
        self.assertEqual({r["tag"] for r in rows},
                         {r["tag"] for r in every if r["call"] == "unresolved"})
        for row in rows:
            self.assertLessEqual(row["posterior"]["lo"], 0.0, row["tag"])
            self.assertGreaterEqual(row["posterior"]["hi"], 0.0, row["tag"])

    def test_it_is_ranked_by_what_an_arm_buys_and_the_top_is_a_disagreement(self):
        rows, _ = ranking.boundary_table(self.ledger, self.measured)
        self.assertEqual([r["buys"] for r in rows],
                         sorted((r["buys"] for r in rows), reverse=True))
        # An arm buys most where the genome and the pooled evidence disagree,
        # and the disagreement runs BOTH ways: a gene held off whose posterior
        # leans positive, or one that ships while its posterior leans negative.
        #
        # ⚠ THIS USED TO PIN THE OFF-AND-POSITIVE DIRECTION ONLY, and the
        # standard screen put the other one on top: `war-economy` ships on two
        # positive columns while its whole record pools to -6.6 across an
        # interval 250 wide. That is the most valuable arm in the table and the
        # test called it a defect. What the ranking actually claims is the
        # disagreement, not its sign.
        top = rows[0]
        self.assertNotEqual(top["pinned"], top["posterior"]["effect"] > 0.0,
                            top["tag"])

    def test_a_bigger_arm_buys_more_and_resolves_more(self):
        small, _ = ranking.boundary_table(self.ledger, self.measured, arm_pairs=2000)
        large, _ = ranking.boundary_table(self.ledger, self.measured, arm_pairs=40000)
        by_small = {r["tag"]: r["buys"] for r in small}
        for row in large:
            self.assertGreaterEqual(row["buys"] + 1e-9, by_small[row["tag"]], row["tag"])

    def test_the_output_is_a_genes_argument_list(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ranking.print_boundary(self.ledger, ranking.ARM_PAIRS,
                                   ranking.FEASIBLE_ARM_PAIRS)
        text = out.getvalue()
        self.assertIn("boundary genes ·", text)
        line = next(l for l in text.splitlines() if l.startswith("--genes "))
        tags = line.removeprefix("--genes ").split(",")
        self.assertLessEqual(len(tags), ranking.BOUNDARY_SUGGESTIONS)
        known = set(ranking.screenable_tags())
        for tag in tags:
            self.assertIn(tag, known, tag)
        # Only genes one batch could actually resolve are proposed.
        needs = {r["tag"]: r["needs"]
                 for r in ranking.boundary_table(self.ledger, self.measured)[0]}
        for tag in tags:
            self.assertLessEqual(needs[tag], ranking.FEASIBLE_ARM_PAIRS, tag)
        self.assertIn("gene_screen --genes ", ranking.RANKING_MD.read_text())

    def test_the_two_stage_arithmetic_is_recorded_where_it_will_be_read(self):
        """⚠ The efficient plan is two stage and NOT a partial foldover. The
        ranking says so; `docs/GENE_SCREEN.md` carries the arithmetic."""
        self.assertIn("two stage", ranking.RANKING_MD.read_text())
        screen = (ranking.ROOT / "docs" / "GENE_SCREEN.md").read_text()
        for phrase in ("two-stage", "±145", "partial", "blocked", "8× the games"):
            self.assertIn(phrase, screen, phrase)

    def test_the_eight_times_figure_is_the_screens_own_arithmetic(self):
        """±146 is not quoted, it is derived — and this recomputes it from the
        ledger's own screens so the paragraph cannot rot.

        Split p10's budget into one screen per gene: 17,574 / 75 = 234 pairs
        each. Even at the best single-gene pairing gain the repository has
        measured (`s7`'s 3.32× against p10's 1.09×) that resolves ±146, which
        is 2.9× wider than p10's ±51 — 8× the games for the same band.
        """
        rows = {r["name"].split("-native")[0].split("-holy")[0].split("-idle")[0]:
                r for r in ranking.resolutions(self.ledger)}
        p10 = next(r for k, r in rows.items() if "p10" in k)
        s7 = next(r for k, r in rows.items() if "s7" in k)
        self.assertEqual(round(p10["band"]), 51)
        self.assertEqual(round(s7["band"]), 29)
        per_gene_pairs = p10["pairs"] / p10["genes"]
        self.assertAlmostEqual(per_gene_pairs, 234.32, places=1)
        # p10's own error per pair, improved by the single-gene gain ratio.
        constant = p10["se"] * math.sqrt(p10["pairs"]) * p10["gain"] / s7["gain"]
        band = ranking.POWER_80 * constant / math.sqrt(per_gene_pairs)
        self.assertAlmostEqual(band, 145, delta=1)
        self.assertAlmostEqual(band / p10["band"], 2.84, places=2)
        # Error falls with the square root of the games, so 2.84x the width is
        # 8.1x the games for the same band.
        self.assertAlmostEqual((band / p10["band"]) ** 2, 8.1, places=1)


class TheLaneGenes(unittest.TestCase):
    """A lane gene is discovered from the code, and its share reading is
    published beside its win columns."""

    def test_the_lane_set_is_read_off_victory_lane_rs(self):
        tags = ranking.lane_tags()
        self.assertGreaterEqual(len(tags), 6)
        for tag in ("lane-congress-ballot", "lane-great-people", "lane-policy-deck",
                    "lane-space-race", "lane-culture-spending", "lane-congress-favor"):
            self.assertIn(tag, tags, tag)
        # Discovered, not listed: a gene joins by being read in the module.
        reg = ranking.registry()
        read = ranking.LANE_MODULES[0].read_text()
        for tag in tags:
            self.assertIn(f"self.{reg[tag][0]}", read, tag)
        self.assertNotIn("wide-map-capacity", tags)

    def test_every_lane_gene_appears_with_its_axis(self):
        text = ranking.RANKING_MD.read_text()
        self.assertIn("## Lane genes and the share axis", text)
        for tag in ranking.lane_tags():
            self.assertIn(f"| `{tag}` |", text, tag)
        self.assertIn("science 0/8", text)
        self.assertIn("t283", text)

    def test_the_share_cell_carries_the_reading_and_its_verdict(self):
        self.assertEqual(ranking.share_verdict(2.0), "helps *")
        self.assertEqual(ranking.share_verdict(-2.0), "hurts *")
        self.assertEqual(ranking.share_verdict(1.99), "~")
        history = [{"share_delta_pp": -1.024, "share_z": -15.92}]
        self.assertEqual(ranking.share_cell(history), "-1.02 (z -15.92) hurts *")

    def test_the_pinned_lane_selection_is_written_down_before_the_screen(self):
        screen = (ranking.ROOT / "docs" / "GENE_SCREEN.md").read_text()
        self.assertIn("Pre-registered", screen)
        self.assertIn("lane gene", screen)
        self.assertIn("deployment choice is explicit", screen)


class TheStandardScreen(unittest.TestCase):
    """⭐ THE SCREEN THE NOTE PREVIEWED IS NOW A LEDGER SOURCE, and this class
    is what holds the two together.

    `docs/gene_ranking_notes.md` was written while the first standard-shape
    screen existed only as a published table: every figure in it was read by
    hand out of `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`
    (PR #2323), because no `gene_screen --analyze --json` file for it had been
    recorded. That file is now
    `docs/gene_screens/2026-08-22-standard-10k-6p-allseats-23622-pairs.json`
    and the ledger reads it, so the hand table below stops being the only
    record of the screen and becomes a check ON the recorded one: if the two
    ever disagree, either the note misread the document or the source is not
    the screen it claims to be.

    The screen: 3,937 complete map pairs, 23,622 matched seat comparisons per
    gene, 74x46 Continents / 9 CS / Online-250, all six lanes, best-genome
    baseline, all-seats foldover, source commit `b3ad9f00`.
    """

    #: tag -> (on−off win Δpp, win z), read off that document's table BY HAND.
    #: Deliberately not derived from the source JSON — that is the whole point
    #: of `test_the_hand_read_note_matches_the_recorded_source`.
    STANDARD = {
        "governor-victory-lanes": (-4.73, -15.37),
        "governor-every-lane": (-4.68, -15.12),
        "governor-expansion-lane": (-0.55, -1.76),
        "war-economy": (2.35, 7.50),
        "apostle-promotion-by-role": (0.32, 1.02),
        "theology-for-founders": (0.45, 1.43),
        "settler-site-agreement": (-0.46, -1.47),
        "settler-target-hysteresis": (-0.36, -1.16),
        "housing-research": (-0.35, -1.10),
        "religion-sues-peace": (-0.36, -1.14),
    }
    #: The eight historical pooled-*Diff* candidates: two were clear signals
    #: and six were noise. They now demonstrate why evidence cannot decide a
    #: deployment selection automatically.
    FORMER_VETO_CANDIDATES = ("governor-victory-lanes", "war-economy",
                              "settler-site-agreement", "settler-target-hysteresis",
                              "housing-research", "religion-sues-peace",
                              "apostle-promotion-by-role", "theology-for-founders")

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.measured, _ = ranking.load_sources(self.ledger)
        self.notes = ranking.NOTES_MD.read_text()

    def reading(self, tag):
        """That screen's row as a history entry, from the hand-read table: a
        foldover holds the arms symmetric about chance, so
        `win_on = 1/6 + Δ/200`."""
        delta, z = self.STANDARD[tag]
        chance = 1.0 / gene_ledger.SCREEN["players"]
        return {"win_on": chance + delta / 200.0, "win_off": chance - delta / 200.0,
                "n_on": 23622, "n_off": 23622, "win_delta_pp": delta,
                "win_se_pp": abs(delta / z), "shape": "standard"}

    def pools(self, tag):
        """The gene's record read three ways: legacy shapes only, standard
        shapes only, and everything pooled.

        ⚠ THESE ARE SLICES OF THE REAL RECORD NOW. They used to append
        `reading(tag)` to the history, because the standard screen was not a
        source; doing that today would count it twice and quietly widen every
        pooled interval this class asserts."""
        history = self.measured[tag]
        return (gene_ledger.pooled_posterior(history, ("legacy",)),
                gene_ledger.pooled_posterior(history, ("standard",)),
                gene_ledger.pooled_posterior(history, gene_ledger.POSTERIOR_SHAPES))

    def screen_row(self, filename, tag):
        source = json.loads(
            (gene_ledger.ROOT / "docs" / "gene_screens" / filename).read_text())
        return next(g for g in source["genes"] if g["tag"] == tag)

    def test_the_hand_read_note_matches_the_recorded_source(self):
        """★ The join. Every figure the note read out of the published
        document is the figure the recorded analysis JSON carries, to the
        precision the document printed."""
        source = json.loads(
            (gene_ledger.ROOT / "docs" / "gene_screens"
             / "2026-08-22-standard-10k-6p-allseats-23622-pairs.json").read_text())
        self.assertEqual(source["complete_pairs"], 23622)
        self.assertEqual(gene_ledger.shape_of(gene_ledger.profile_of(source)), "standard")
        self.assertEqual(source["profile"]["start_seed"], 141000000)
        by_tag = {g["tag"]: g for g in source["genes"]}
        for tag, (delta, z) in self.STANDARD.items():
            self.assertAlmostEqual(by_tag[tag]["win_delta_pp"], delta, places=2, msg=tag)
            self.assertAlmostEqual(by_tag[tag]["win_z"], z, places=2, msg=tag)

    def test_governor_victory_lanes_historical_evidence_justifies_the_cull(self):
        """Before the policy was pinned, P10's single +46 column had placed
        it on. The 23,622-seat deployment shape read it at −237, and the
        pre-registered direct arm `g1` confirmed −4.78 pp at win z −6.11.
        The newer 38,160-seat cutoff independently reads −2.21 pp at z −5.81.
        The explicit 2026-08-24 cull then removed its implementation.

        ⭐ THE THREE DEPLOYMENT WINDOWS NOW TELL THE HISTORICAL STORY IN ONE ROW:
        the cutoff at −110, g1 at −239, and the whole-genome standard screen
        at −237. The legacy +46 remains in the pooled record as the original
        promotion, even though it has now fallen beyond the printed third
        window."""
        live_tags = {g["tag"] for g in self.ledger["genes"]}
        self.assertNotIn("governor-victory-lanes", live_tags)
        ranked = ranking.RANKING_MD.read_text()
        self.assertIn("## Removed from the code", ranked)
        self.assertIn("| `governor-victory-lanes` |", ranked)
        cutoff = self.screen_row(
            "2026-08-24-standard-continuous-38160-total-seats.json",
            "governor-victory-lanes")
        self.assertAlmostEqual(cutoff["win_delta_pp"], -2.21, places=2)
        self.assertAlmostEqual(cutoff["win_z"], -5.81, places=2)
        legacy, standard, pooled = self.pools("governor-victory-lanes")
        self.assertEqual((round(legacy["effect"]), round(legacy["lo"]),
                          round(legacy["hi"])), (46, 9, 82))
        # The third standard window is still strongly harmful but smaller, so
        # the random-effects pool carries its between-window disagreement.
        self.assertEqual((round(standard["effect"]), round(standard["lo"]),
                          round(standard["hi"])), (-193, -287, -100))
        self.assertEqual(round(standard["tau"]), 78)
        self.assertEqual(standard["screens"], 3)
        # The two instruments do not merely disagree, they do not come close.
        self.assertGreater(legacy["lo"] - standard["hi"], 100)
        # So the pool across shapes is a warning, not an answer, even though
        # the historical evidence decisively met the explicit cull threshold.
        self.assertEqual(pooled["screens"], 4)
        self.assertEqual(gene_ledger.posterior_call(pooled["effect"], pooled["se"]),
                         "unresolved")
        self.assertEqual(gene_ledger.posterior_call(standard["effect"],
                                                    standard["se"]), "off")
        for phrase in ("-237", "[-267, -206]", "-15.37"):
            self.assertIn(phrase, self.notes, phrase)

    def test_research_cull_keeps_its_historical_rows(self):
        """The four 38,160-seat research candidates left source, not history."""
        tags = (
            "chain-tech-lookahead",
            "research-floor-holds",
            "research-grants-first",
            "science-payback-horizon",
        )
        live_tags = {gene["tag"] for gene in self.ledger["genes"]}
        ranked = ranking.RANKING_MD.read_text()
        cutoff = json.loads(
            (gene_ledger.ROOT / "docs" / "gene_screens"
             / "2026-08-24-standard-continuous-38160-total-seats.json").read_text())
        cutoff_tags = {gene["tag"] for gene in cutoff["genes"]}
        for tag in tags:
            self.assertNotIn(tag, live_tags)
            self.assertIn(f"| `{tag}` |", ranked)
            self.assertIn(tag, cutoff_tags)
        reporting_notes = "\n".join(
            str(batch.get("unverified", ""))
            for batch in self.ledger["reporting_batches"]
        )
        self.assertIn("research-planning genes were deliberately removed",
                      reporting_notes)

    def test_the_legacy_share_axis_already_said_it(self):
        """P10 read this gene win z +2.46 / share z −15.92 — a recorded
        `conflict`; the former rule looked only at its win axis. The share axis
        was right a day before the win axis caught up, and g1's own arm now
        agrees on BOTH axes, so the conflict is gone."""
        cutoff = self.screen_row(
            "2026-08-24-standard-continuous-38160-total-seats.json",
            "governor-victory-lanes")
        # The cutoff has both axes negative, so no conflict remains in the
        # historical evidence that justified removal.
        self.assertAlmostEqual(cutoff["win_z"], -5.81, places=2)
        self.assertAlmostEqual(cutoff["share_z"], -16.41, places=2)
        self.assertNotIn("governor-victory-lanes",
                         {g["tag"] for g in self.ledger["genes"]})
        # P10's share axis (−15.92) landed within half a sigma of what the
        # deployment shape's WIN axis said a day later (−15.37).
        self.assertLess(abs(15.92 - abs(self.STANDARD["governor-victory-lanes"][1])),
                        0.6)
        self.assertIn("-15.92", self.notes)

    def test_the_composite_harm_is_carried_by_one_named_half(self):
        composite = self.STANDARD["governor-every-lane"][0]
        victory = self.STANDARD["governor-victory-lanes"][0]
        expansion = self.STANDARD["governor-expansion-lane"][0]
        self.assertLess(abs(composite - victory), 0.1, "the half is the composite")
        self.assertLess(abs(expansion), abs(victory) / 5, "the other half is cheap")
        self.assertLess(abs(victory + expansion - composite), 0.7, "roughly additive")
        # The harmful half was the only one that shipped, until g1 resolved
        # it off on 2026-08-23. The 2026-08-24 cull removes all three genes.
        live_tags = {g["tag"] for g in self.ledger["genes"]}
        ranked = ranking.RANKING_MD.read_text()
        for tag in ("governor-victory-lanes", "governor-every-lane",
                    "governor-expansion-lane"):
            self.assertNotIn(tag, live_tags)
            self.assertIn(f"| `{tag}` |", ranked)

    def test_six_of_the_eight_former_candidates_are_at_z_about_one(self):
        """Why the former automatic rule was retired: six of its candidates
        were |z| ≈ 1 on the next screen."""
        signal = [tag for tag in self.FORMER_VETO_CANDIDATES
                  if abs(self.STANDARD[tag][1]) >= 3.0]
        noise = [tag for tag in self.FORMER_VETO_CANDIDATES
                 if abs(self.STANDARD[tag][1]) < 2.0]
        self.assertEqual(sorted(signal), ["governor-victory-lanes", "war-economy"])
        self.assertEqual(len(noise), 6)
        for tag in noise:
            self.assertLess(abs(self.STANDARD[tag][1]), 1.5, tag)

    def test_standard_only_posterior_resolves_exactly_the_two_as_evidence(self):
        """The pooled reading resolves none because tau swamps both. Neither
        reading changes the pinned deployment selection automatically."""
        resolved_standard, resolved_pooled = [], []
        for tag in self.FORMER_VETO_CANDIDATES:
            _, standard, pooled = self.pools(tag)
            if gene_ledger.posterior_call(standard["effect"], standard["se"]) != "unresolved":
                resolved_standard.append(tag)
            if gene_ledger.posterior_call(pooled["effect"], pooled["se"]) != "unresolved":
                resolved_pooled.append(tag)
        self.assertEqual(sorted(resolved_standard),
                         ["governor-victory-lanes", "war-economy"])
        self.assertEqual(resolved_pooled, [])
        self.assertIn("two strong standard-shape signals", self.notes)
        self.assertIn("not a fallback policy", self.notes)

    def test_the_note_is_carried_into_the_published_ranking(self):
        text = ranking.RANKING_MD.read_text()
        self.assertIn("23,622", text)
        self.assertIn("governor-victory-lanes", text)
        self.assertIn("## Removed from the code", text)
        # ⚠ The note used to say the screen was "not a ledger source". It is
        # one now, and a paragraph that still said otherwise would be the most
        # misleading line in the file.
        self.assertNotIn("not a ledger source", text)
        self.assertIn("2026-08-22-standard-10k-6p-allseats-23622-pairs.json", text)


class EveryTestInThisFileIsCollected(unittest.TestCase):
    """⚠⚠ A TEST THAT FALLS OUT OF ITS CLASS STILL PASSES, BY NOT RUNNING.

    Adding the third window to this file, a helper was pasted at module
    indentation between the class header and its first method. Everything
    below became the body of that helper: fifteen `def test_...` still parsed,
    still read as tests to any human scrolling past, and were never collected.
    The suite went green with 98 of 113 tests, and nothing said so.

    So this counts them. `def test_` at method indentation is a test method,
    and unittest must have loaded every one of them."""

    def test_every_method_named_test_is_loaded(self):
        source = Path(__file__).read_text()
        written = set(re.findall(r"^    def (test_\w+)", source, re.M))
        loaded = set()
        for suite in unittest.defaultTestLoader.loadTestsFromName(__name__):
            for case in suite:
                loaded.add(case.id().rsplit(".", 1)[-1])
        self.assertGreater(len(written), 30)
        self.assertEqual(
            sorted(written - loaded), [],
            "these are indented as test methods but unittest never loaded them: "
            "check that each sits directly inside a TestCase class",
        )


if __name__ == "__main__":
    unittest.main()
