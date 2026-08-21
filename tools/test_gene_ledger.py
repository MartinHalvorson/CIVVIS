"""The gene ledger: verdict rules, source precedence, and the two generated
files staying together with the recorded sources."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_ledger  # noqa: E402


def analysis(regime: str, genes: list[dict], pairs: int = 1000, family: float = 3.0) -> dict:
    return {
        "kind": "gene_screen_analysis",
        "regime": regime,
        "complete_pairs": pairs,
        "family_wise_z": family,
        "profile": {"players": 4, "victories": "" if regime == "native" else regime},
        "genes": [
            {
                "tag": g["tag"], "pairs": pairs, "n_on": pairs, "n_off": pairs,
                "win_delta_pp": g.get("win", 0.0), "win_z": g.get("wz", 0.0),
                "share_delta_pp": g.get("share", 0.0), "share_z": g.get("sz", 0.0),
                "read": "",
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
            return gene_ledger.build_ledger(paths)

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
        self.assertEqual([g["default_on"] for g in ledger["genes"]], [True, True, False, False])
        self.assertEqual(ledger["counts"], {"helps": 2, "hurts": 2, "unresolved": 0})

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

    def test_a_war_file_recorded_as_native_is_refused(self):
        with self.assertRaises(SystemExit):
            self.build([("native", analysis("domination,score", [{"tag": "a"}]))])


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
