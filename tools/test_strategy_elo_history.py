#!/usr/bin/env python3
"""The Elo history keeps every published fit, and its newest is the one shipped."""

import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import strategy_elo_history as history  # noqa: E402


def fit(jack: float, tables: int = 100) -> dict:
    return {
        "tables": tables,
        "runs": [{"run": "2026-11-22-ca8754f", "tables": tables}],
        "ratings": {
            "jack": {"elo": jack, "sd": 14.4, "margin95": 28.0, "bound": ""},
            "civ6:deity": {"elo": 1953.6, "sd": 24.6, "bound": ""},
        },
        "ladder": {"prince": 1000.0, "deity": 1953.6},
    }


class HistoryTest(unittest.TestCase):
    def tree(self, published: dict) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp())
        (root / "data").mkdir()
        (root / history.PUBLISHED).write_text(json.dumps(published), encoding="utf-8")
        return root

    def test_the_repository_history_ends_at_the_published_fit(self):
        self.assertIsNone(history.check(history.ROOT))

    def test_the_repository_diplo_off_history_ends_at_its_published_fit(self):
        self.assertIsNone(history.check_all(history.ROOT))

    def test_the_diplo_off_fit_keeps_a_history_of_its_own(self):
        root = self.tree(fit(1141.3))
        history.append(root, "2026-11-24T05:00:00Z")
        # A tree from before the second fit is no fault...
        self.assertIsNone(history.check_all(root))
        self.assertFalse(history.append(root, "2026-11-24T05:00:00Z", history.NO_DIPLO_PUBLISHED,
                                        history.NO_DIPLO_HISTORY))
        # ...but once it is published, it wants its point.
        (root / history.NO_DIPLO_PUBLISHED).write_text(json.dumps(fit(1210.0)), encoding="utf-8")
        self.assertIn("no_diplo_history", history.check_all(root))
        self.assertTrue(history.append(root, "2026-11-24T05:00:00Z", history.NO_DIPLO_PUBLISHED,
                                       history.NO_DIPLO_HISTORY))
        self.assertIsNone(history.check_all(root))
        on = json.loads((root / history.HISTORY).read_text())["points"]
        off = json.loads((root / history.NO_DIPLO_HISTORY).read_text())["points"]
        self.assertEqual([p["ratings"]["jack"][0] for p in on], [1141.3])
        self.assertEqual([p["ratings"]["jack"][0] for p in off], [1210.0])

    def test_a_point_keeps_elo_margin_and_bound_and_derives_a_missing_margin(self):
        p = history.point(fit(1141.26), "2026-11-22T10:28:41Z")
        self.assertEqual(p["ratings"]["jack"], [1141.3, 28, ""])
        self.assertEqual(p["ratings"]["civ6:deity"], [1953.6, 48, ""])  # 1.96 x 24.6
        self.assertEqual(p["runs"], ["2026-11-22-ca8754f"])

    def test_append_adds_a_new_fit_once_and_check_then_passes(self):
        root = self.tree(fit(1141.3))
        self.assertTrue(history.append(root, "2026-11-22T10:00:00Z"))
        self.assertFalse(history.append(root, "2026-11-22T11:00:00Z"))
        self.assertIsNone(history.check(root))
        (root / history.PUBLISHED).write_text(json.dumps(fit(1150.0, 200)), encoding="utf-8")
        self.assertIn("newest point", history.check(root))
        self.assertTrue(history.append(root, "2026-11-23T10:00:00Z"))
        self.assertIsNone(history.check(root))
        points = json.loads((root / history.HISTORY).read_text())["points"]
        self.assertEqual([p["ratings"]["jack"][0] for p in points], [1141.3, 1150.0])

    def test_the_file_is_a_point_to_a_line(self):
        root = self.tree(fit(1141.3))
        history.append(root, "2026-11-22T10:00:00Z")
        text = (root / history.HISTORY).read_text()
        self.assertEqual(sum(line.lstrip().startswith('{"at"') for line in text.splitlines()), 1)
        json.loads(text)

    def test_each_named_setup_has_a_fit_and_the_tool_names_the_same_setups(self):
        # Operator 2026-11-27: "6p Continents Online (Diplo off, Diplo on,
        # science, culture, religion, diplomacy, domination, score) , 2P
        # Pangaea (religion off, religion on). we can elo for each". Each of
        # strategy_elo's setups publishes a fit this keeps a history of.
        files = {published.name for published, _ in history.FITS}
        for name in history.SETUPS:
            self.assertIn(f"strategy_elo_{name}.json", files)
        tool = (history.ROOT / "src" / "bin" / "strategy_elo.rs").read_text(encoding="utf-8")
        named = {name.replace("-", "_") for name in __import__("re").findall(r'Setup\s*\{\s*name:\s*"([a-z0-9-]+)"', tool)}
        self.assertEqual(named, set(history.SETUPS))
        self.assertEqual(len(history.SETUPS), 10)

    def test_a_named_setup_fit_once_published_must_have_its_history(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "data").mkdir()
            published = {"tables": 12, "runs": [{"run": "r1", "tables": 12}], "ratings": {"stock": {"elo": 1000.0, "sd": 0.0}}, "ladder": {}}
            for path in (history.PUBLISHED, pathlib.Path("data/strategy_elo_continents6_science.json")):
                (root / path).write_text(json.dumps(published), encoding="utf-8")
            history.append(root, "2026-11-27T05:00:00Z")
            # `append` (main's loop) adds a point for every published fit; this
            # one only did the stock fit, so the science fit has none yet.
            self.assertIn("strategy_elo_continents6_science_history.json", history.check_all(root) or "")
            history.append(root, "2026-11-27T05:00:00Z", pathlib.Path("data/strategy_elo_continents6_science.json"),
                           pathlib.Path("data/strategy_elo_continents6_science_history.json"))
            self.assertIsNone(history.check_all(root))


if __name__ == "__main__":
    unittest.main()
