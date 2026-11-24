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


if __name__ == "__main__":
    unittest.main()
