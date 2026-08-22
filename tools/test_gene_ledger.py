"""The gene ledger: the win-column default rule, the verdict rules, source
precedence, and the two generated files staying together with the recorded
sources."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_ledger  # noqa: E402


PLAYERS = 4


def analysis(regime: str, genes: list[dict], pairs: int = 1000, family: float = 3.0) -> dict:
    """One screen. `wins` is the gene's win column in this screen — wins per
    10,000 games above the 1-in-`PLAYERS` a seat takes by chance — written
    back as the on-rate the analyzer would have printed."""
    return {
        "kind": "gene_screen_analysis",
        "regime": regime,
        "complete_pairs": pairs,
        "family_wise_z": family,
        "profile": {"players": PLAYERS, "victories": "" if regime == "native" else regime},
        "genes": [
            {
                "tag": g["tag"], "pairs": pairs, "n_on": pairs, "n_off": pairs,
                "win_on": 1.0 / PLAYERS + g.get("wins", 0) / 10_000,
                "win_off": 1.0 / PLAYERS,
                "win_delta_pp": g.get("win", 0.0), "win_z": g.get("wz", 0.0),
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
            for i, (regime, data) in enumerate(sources):
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(data))
                paths.append((path, regime))
            return gene_ledger.build_ledger(paths, filter_known=False)

    def test_native_governs_and_war_fills_in_when_native_is_unresolved(self):
        ledger = self.build([
            ("native", analysis("native", [
                {"tag": "a", "wz": 2.4},           # helps natively
                {"tag": "b", "wz": 0.3},           # unresolved natively
                {"tag": "c", "wz": -2.2},          # hurts natively
                {"tag": "d", "wz": 0.1},
            ])),
            ("war", analysis("domination,score", [
                {"tag": "a", "wz": -2.5},          # conflict: native governs
                {"tag": "b", "wz": 3.0},           # war resolves b
                {"tag": "c", "wz": 2.5},           # native governs
                {"tag": "d", "wz": -2.5},          # war resolves d as hurts
            ])),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertEqual(by["a"]["verdict"], "helps")
        self.assertTrue(by["a"]["conflict"])
        self.assertEqual(by["b"]["verdict"], "helps")
        self.assertEqual(by["b"]["deciding_regime"], "war")
        self.assertEqual(by["c"]["verdict"], "hurts")
        self.assertEqual(by["d"]["verdict"], "hurts")
        self.assertEqual(
            [g["default_on"] for g in ledger["genes"]], [False, False, False, False],
            "a verdict no longer turns a gene on: these have no win columns to clear the rule",
        )
        self.assertEqual(
            ledger["counts"],
            {"helps": 2, "hurts": 2, "unresolved": 0, "default_on": 0},
        )

    def test_a_later_source_overrides_an_earlier_one_per_gene_and_regime(self):
        ledger = self.build([
            ("war", analysis("domination,score", [
                {"tag": "repaired", "wz": -4.0}, {"tag": "other", "wz": 2.5},
            ])),
            ("war", analysis("domination,score", [{"tag": "repaired", "wz": 2.5}], pairs=500)),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertEqual(by["repaired"]["verdict"], "helps")
        self.assertEqual(by["repaired"]["war"]["pairs"], 500)
        self.assertEqual(by["other"]["verdict"], "helps", "the earlier screen still stands for the rest")

    def test_family_wise_is_recorded_from_the_deciding_runs_bar(self):
        ledger = self.build([
            ("native", analysis("native", [
                {"tag": "strong", "wz": 3.5}, {"tag": "weak", "wz": 2.2},
            ], family=3.3)),
        ])
        by = {g["tag"]: g for g in ledger["genes"]}
        self.assertTrue(by["strong"]["family_wise"])
        self.assertFalse(by["weak"]["family_wise"])

    def test_chronological_win_tranches_survive_in_the_ledger(self):
        ledger = self.build([("native", analysis("native", [{
            "tag": "repeated-harm",
            "wz": -3.0,
            "tranches": [
                {"position": "latest", "pairs": 10002, "win_delta_pp": -1.2345, "win_se_pp": 0.3444, "win_z": -2.3456},
                {"position": "previous", "pairs": 9996, "win_delta_pp": -0.9876, "win_se_pp": 0.4655, "win_z": -2.1234},
                {"position": "earlier", "pairs": 10002, "win_delta_pp": -1.1111, "win_se_pp": 0.4366, "win_z": -2.5432},
            ],
        }]))])
        measure = ledger["genes"][0]["native"]
        self.assertEqual(measure["win_tranches"], [
            {"position": "latest", "pairs": 10002, "win_delta_pp": -1.234, "win_se_pp": 0.344, "win_z": -2.346},
            {"position": "previous", "pairs": 9996, "win_delta_pp": -0.988, "win_se_pp": 0.466, "win_z": -2.123},
            {"position": "earlier", "pairs": 10002, "win_delta_pp": -1.111, "win_se_pp": 0.437, "win_z": -2.543},
        ])

    def test_a_war_file_recorded_as_native_is_refused(self):
        with self.assertRaises(SystemExit):
            self.build([("native", analysis("domination,score", [{"tag": "a"}]))])


class TheDefaultRule(unittest.TestCase):
    """Operator directive 2026-08-22: the default is read off the ranking's two
    win columns — both positive, or an average above +15 with neither below
    -10 — and nothing else, the verdict included."""

    def on(self, *wins: int) -> bool:
        """Build a gene screened once per given win column, oldest first."""
        with tempfile.TemporaryDirectory() as tmp:
            sources = []
            for i, w in enumerate(wins):
                path = Path(tmp) / f"s{i}.json"
                path.write_text(json.dumps(analysis("native", [{"tag": "g", "wins": w}])))
                sources.append((path, "native"))
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

    def test_one_native_reading_must_clear_twenty(self):
        self.assertTrue(self.on(21), "a single reading above +20 is provisionally on")
        self.assertFalse(self.on(20), "exactly +20 does not clear the strict bar")
        self.assertFalse(self.on(-21))
        self.assertTrue(gene_ledger.default_from_win_columns(None, 21))
        self.assertFalse(gene_ledger.default_from_win_columns(None, 20))
        # A gene no screen has measured has no row at all; the rule still
        # answers for it, and `ledger_default_on` gives the same `false`.
        self.assertFalse(gene_ledger.default_from_win_columns(None, None))

    def test_only_the_last_two_native_readings_decide(self):
        self.assertTrue(self.on(-500, 20, 21), "an old bad screen is history, not a veto")
        self.assertFalse(self.on(20, 21, -500))

    def test_the_war_regime_does_not_supply_a_column(self):
        with tempfile.TemporaryDirectory() as tmp:
            native = Path(tmp) / "n.json"
            native.write_text(json.dumps(analysis("native", [{"tag": "g", "wins": 20}])))
            war = Path(tmp) / "w.json"
            war.write_text(json.dumps(
                analysis("domination,score", [{"tag": "g", "wins": 40, "wz": 3.0}])))
            ledger = gene_ledger.build_ledger(
                [(native, "native"), (war, "war")], filter_known=False)
        gene = ledger["genes"][0]
        self.assertEqual(gene["verdict"], "helps")
        self.assertIsNone(gene["wins_prior_10k"], "the war screen is not a native reading")
        self.assertFalse(gene["default_on"], "the war +40 must not help native +20 clear its bar")


class KnownTags(unittest.TestCase):
    def test_the_registry_is_read_and_a_removed_gene_is_dropped(self):
        known = gene_ledger.known_tags()
        self.assertGreater(len(known), 50, "the treatments registry scrape found too few tags")
        self.assertIn("wide-map-capacity", known)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "s.json"
            path.write_text(json.dumps(analysis("native", [
                {"tag": "wide-map-capacity", "wz": 2.5},
                {"tag": "a-gene-whose-code-was-removed", "wz": 3.0},
            ])))
            ledger = gene_ledger.build_ledger([(path, "native")])
        tags = [g["tag"] for g in ledger["genes"]]
        self.assertIn("wide-map-capacity", tags)
        self.assertNotIn("a-gene-whose-code-was-removed", tags)


class GeneratedFiles(unittest.TestCase):
    """`docs/gene_ledger.json` and `src/ai/advanced/gene_ledger_table.rs` are
    both derived from the sources the JSON records; neither may drift."""

    def test_the_checked_in_ledger_reproduces_from_its_recorded_sources(self):
        current = json.loads(gene_ledger.LEDGER_JSON.read_text())
        ledger = gene_ledger.build_ledger(gene_ledger.sources_from_ledger(current))
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
