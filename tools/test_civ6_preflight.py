#!/usr/bin/env python3
"""Regression tests for live CIV VI preflight checks."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_preflight  # noqa: E402


class InstalledSourceMatchesTest(unittest.TestCase):
    def test_exact_worktree_source_matches(self) -> None:
        self.assertTrue(civ6_preflight.installed_source_matches(b"print('ok')\n", b"print('ok')\n"))

    def test_configured_install_prelude_matches_its_source_suffix(self) -> None:
        source = b"print('ok')\n"
        installed = b"CivvisControlConfig = { RunTag = 'live' }\n\n" + source

        self.assertTrue(civ6_preflight.installed_source_matches(installed, source))

    def test_different_installed_source_does_not_match(self) -> None:
        self.assertFalse(civ6_preflight.installed_source_matches(b"print('old')\n", b"print('new')\n"))


if __name__ == "__main__":
    unittest.main()
