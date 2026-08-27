#!/usr/bin/env python3
"""Regression coverage for the narrow non-Rust pull-request lane."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ci_scope


class CiScopeTests(unittest.TestCase):
    def test_docs_tools_and_top_level_markdown_use_the_fast_lane(self):
        self.assertFalse(ci_scope.requires_rust_gate([
            "docs/FIDELITY.md",
            "docs/eval_manifest.json",
            "tools/civ6_play.py",
            "tools/civ6_control/mod/CivvisControlAgent.lua",
            "GENE_HEURISTIC_RANKING.md",
        ]))

    def test_runtime_and_workflow_inputs_keep_the_full_rust_suite(self):
        for path in (
            "src/ai/advanced.rs",
            "data/units.json",
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            ".github/workflows/tests.yml",
            "beta/app.js",
            "new-unclassified-file.txt",
        ):
            with self.subTest(path=path):
                self.assertTrue(ci_scope.requires_rust_gate([path]))

    def test_empty_and_malformed_paths_fail_closed(self):
        self.assertTrue(ci_scope.requires_rust_gate([]))
        for path in ("", "/src/lib.rs", "../src/lib.rs", "tools/../src/lib.rs"):
            with self.subTest(path=path):
                self.assertTrue(ci_scope.requires_rust_gate([path]))

    def test_nul_delimited_git_paths_round_trip(self):
        self.assertEqual(
            ci_scope.read_paths(b"docs/a.md\0tools/name with spaces.py\0", nul_delimited=True),
            ["docs/a.md", "tools/name with spaces.py"],
        )

    def test_workflow_reports_both_sides_of_a_rename(self):
        """Moving a Rust file into docs must retain the deleted Rust path."""
        workflow = (
            Path(__file__).resolve().parent.parent / ".github/workflows/tests.yml"
        ).read_text(encoding="utf-8")
        self.assertIn(
            'git diff --no-renames --name-only -z "$BASE_SHA..HEAD"', workflow
        )
        self.assertTrue(
            ci_scope.requires_rust_gate(["src/lib.rs", "README.md"]),
        )


if __name__ == "__main__":
    unittest.main()
