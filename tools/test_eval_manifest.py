#!/usr/bin/env python3
"""Tests for the generated evaluation inventory and status snapshot."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

try:
    from tools import eval_manifest
except ImportError:  # unittest discovery adds tools/ directly to sys.path
    import eval_manifest


REPO = Path(__file__).resolve().parent.parent


class EvalManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = eval_manifest.build_manifest(REPO)

    def test_every_registry_declaration_is_counted_without_duplicates(self):
        for name, row in self.manifest["registry"].items():
            self.assertEqual(row["declared_count"], len(row["items"]), name)
            self.assertEqual(len(row["items"]), len(set(row["items"])), name)

    def test_derived_counts_are_from_the_registry(self):
        registry = self.manifest["registry"]
        derived = self.manifest["derived"]
        live = set(registry["LIVE_BRIDGE_TREATMENTS"]["items"])
        firaxis = set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
        self.assertEqual(derived["eval_only_count"], len(registry["EVAL_ONLY_AIS"]["items"]))
        self.assertEqual(derived["live_bridge_count"], len(live))
        self.assertEqual(derived["firaxis_only_count"], len(firaxis))
        self.assertEqual(derived["withholdable_live_count"], len(live - firaxis))

    def test_committed_json_and_status_are_current(self):
        self.assertEqual(
            json.loads((REPO / "docs" / "eval_manifest.json").read_text()),
            self.manifest,
        )
        self.assertEqual(
            (REPO / "docs" / "EVAL_STATUS.md").read_text(),
            eval_manifest.render_status(self.manifest),
        )

    def test_evidence_and_roadmap_link_to_the_current_snapshot(self):
        for path in (REPO / "docs" / "EVAL.md", REPO / "docs" / "ROADMAP.md"):
            self.assertIn("EVAL_STATUS.md", path.read_text(encoding="utf-8"), path.name)


if __name__ == "__main__":
    unittest.main()
