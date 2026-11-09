#!/usr/bin/env python3
"""The live Lua agent has no compiler, so this is the one that reads it.

Lua treats an undeclared name as a nil global and says nothing until something
indexes it, and the agent wraps its work in `pcall`, so even that error
vanishes. `tools/civ6_control/check_scope.py` finds those names; this holds the
shipped mod to zero of them and proves the checker still works.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
MOD = TOOLS / "civ6_control" / "mod"

_spec = importlib.util.spec_from_file_location("check_scope", TOOLS / "civ6_control" / "check_scope.py")
scope = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(scope)


def shipped() -> dict[str, str]:
    return {
        path.stem: path.read_text(encoding="utf-8", errors="replace")
        for path in sorted(MOD.glob("*.lua"))
        if not path.name.endswith("_test.lua")
    }


class TheCheckerReadsLua(unittest.TestCase):
    def test_a_local_used_outside_its_function_is_a_slip(self):
        text = "local function a() local id = 1; end\nlocal function b() return id; end\n"
        self.assertEqual([name for _, name in scope.slip_names(text)], ["id"])

    def test_a_method_definition_does_not_declare_its_table(self):
        text = "local function a() return Board.size; end\nBoard = {};\nfunction Board.grow() end\n"
        self.assertEqual(scope.slip_names(text), [])

    def test_a_number_is_not_a_variable(self):
        self.assertEqual(scope.unexplained_globals("local big = 1e9;\n"), set())

    def test_a_misspelt_global_is_unexplained(self):
        self.assertEqual(scope.unexplained_globals("local x = Gmae.GetTurn();\n"), {"Gmae"})

    def test_a_global_this_file_creates_is_explained(self):
        self.assertEqual(scope.unexplained_globals("Helper = {};\nlocal x = Helper.y;\n"), set())
        self.assertEqual(scope.unexplained_globals("function Go() end\nGo();\n"), set())

    def test_a_probed_global_is_explained(self):
        text = 'if type(OnThing) == "function" then OnThing(); end\n'
        self.assertEqual(scope.unexplained_globals(text), set())

    def test_a_global_from_an_included_sibling_is_explained(self):
        text = 'include("Other");\nlocal x = Thing.y;\n'
        self.assertEqual(scope.unexplained_globals(text), {"Thing"})
        self.assertEqual(scope.unexplained_globals(text, {"Other": "Thing = {};\n"}), set())

    def test_the_self_test_passes(self):
        self.assertEqual(scope.self_test(), 0)


class TheShippedMod(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = shipped()

    def test_the_glob_found_the_mod(self):
        self.assertIn("CivvisControlAgent", self.files)
        self.assertGreaterEqual(len(self.files), 5)

    def test_no_name_is_used_outside_the_scope_that_declares_it(self):
        for stem, text in self.files.items():
            with self.subTest(stem):
                self.assertEqual(scope.slip_names(text), [])

    def test_no_global_goes_unexplained(self):
        for stem, text in self.files.items():
            with self.subTest(stem):
                self.assertEqual(scope.unexplained_globals(text, self.files), set())

    def test_the_scrape_still_sees_the_agent(self):
        _, all_locals, globals_read = scope.analyse(self.files["CivvisControlAgent"])
        self.assertGreater(len(all_locals), 1000)
        self.assertGreater(len(globals_read), 50)


if __name__ == "__main__":
    unittest.main()
