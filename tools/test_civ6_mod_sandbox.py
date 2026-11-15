#!/usr/bin/env python3
"""The shipped mod scripts call nothing the live host's Lua does not have.

Civilization VI runs the mod in a restricted Lua. A name it lacks is nil, so
calling it raises only when that line is reached, and plain Lua 5.1 (the
`control-mod` job, and every `*_test.lua`) has the whole base library and
cannot see the difference. Each entry in ``ABSENT`` was raised by a live game.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MOD_DIRS = (REPO / "tools" / "civ6_control" / "mod", REPO / "tools" / "civ6_mod")

# name -> the live game that raised on it.
ABSENT = {
    # civvis-20261115T075055Z, turn 54: "CivvisControlAgent.lua:8199: function
    # expected instead of nil" on `tonumber(rawget(results, i))`, the read of
    # `CombatManager.IsAttackChangeWarState`'s answer. It had raised on every
    # answer that named a player since the war-starter guard shipped.
    "rawget": "civvis-20261115T075055Z turn 54",
}


def shipped_scripts() -> list[Path]:
    """Every Lua script the game loads: discovered, so a new one is scanned."""
    return sorted(
        path
        for root in MOD_DIRS
        for path in root.rglob("*.lua")
        if not path.name.endswith("_test.lua")
    )


def calls(line: str, name: str) -> bool:
    """Whether a line of Lua calls the bare global `name` outside a comment."""
    code = line.split("--", 1)[0]
    return re.search(rf"(?<![\w.:]){re.escape(name)}\s*\(", code) is not None


class Sandbox(unittest.TestCase):
    def test_the_scan_covers_the_mod(self):
        names = {path.name for path in shipped_scripts()}
        self.assertIn("CivvisControlAgent.lua", names)
        self.assertGreaterEqual(len(names), 5, "the scan would cover almost nothing")

    def test_the_scan_tells_a_call_from_a_mention(self):
        self.assertTrue(calls("local id = tonumber(rawget(results, i));", "rawget"))
        self.assertTrue(calls("rawget (t, k)", "rawget"))
        self.assertFalse(calls("-- `rawget(_G, ...)` raises at load", "rawget"))
        self.assertFalse(calls("local id = tonumber(results[i]); -- not rawget(results, i)", "rawget"))
        self.assertFalse(calls("strawget(1)", "rawget"))
        self.assertFalse(calls("helpers.rawget(t, k)", "rawget"))

    def test_no_shipped_script_calls_a_function_the_host_lacks(self):
        hits = []
        for path in shipped_scripts():
            for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
                for name, evidence in ABSENT.items():
                    if calls(line, name):
                        hits.append(f"{path.relative_to(REPO)}:{number}: {name} (absent on the live host: {evidence})")
        self.assertEqual(hits, [])


if __name__ == "__main__":
    unittest.main()
