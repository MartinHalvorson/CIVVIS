#!/usr/bin/env python3
"""Regression coverage for the interactive Civ VI popup keeper runtime."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
KEEPER = TOOLS / "ops" / "civvis-popup-keeper.sh"
CLEARER = TOOLS / "civ6_control" / "popup_clear.py"


@unittest.skipUnless(shutil.which("zsh"), "zsh is not installed here")
class PopupKeeperPythonResolutionTest(unittest.TestCase):
    def resolve(self, **overrides: str) -> Path:
        env = os.environ.copy()
        env.update(overrides)
        result = subprocess.run(
            ["zsh", str(KEEPER), "--print-python"],
            env=env,
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        )
        return Path(result.stdout.strip())

    def test_default_resolves_an_executable_that_loads_the_real_clearer(self) -> None:
        python = self.resolve()
        self.assertTrue(python.is_file(), python)
        self.assertTrue(os.access(python, os.X_OK), python)
        subprocess.run(
            [str(python), str(CLEARER), "--help"],
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        )

    def test_explicit_python_override_is_used(self) -> None:
        explicit = Path(sys.executable).resolve()
        self.assertEqual(self.resolve(CIVVIS_POPUP_PYTHON=str(explicit)).resolve(), explicit)

    def test_bad_explicit_override_refuses_before_entering_the_keeper_loop(self) -> None:
        result = subprocess.run(
            ["zsh", str(KEEPER), "--print-python"],
            env={**os.environ, "CIVVIS_POPUP_PYTHON": "/definitely/not/python"},
            capture_output=True,
            text=True,
            timeout=20,
            check=False,
        )
        self.assertEqual(result.returncode, 70)
        self.assertIn("no usable Python", result.stderr)


if __name__ == "__main__":
    unittest.main()
