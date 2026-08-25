#!/usr/bin/env python3
"""`civ6_scripts.py` searches the directories it says it does, in the form a
PR body can quote."""

from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_scripts  # noqa: E402


def fake_install(root: Path) -> Path:
    files = {
        "Base/Assets/UI/DiplomacyActionView.lua": "local a = 1\nfunction MakeDeal_ApplyStatement(handler)\n",
        "Base/Assets/Text/en_US/DiplomacyModifiers_Text.xml": '<Row Tag="LOC_DIPLO_MODIFIER_GIFT_NOT_HERE"/>\n',
        "Base/Assets/Gameplay/Data/Barbarians.xml": "<BarbarianAttackForces>\n",
        "DLC/Expansion2/UI/Replacements/DiplomacyActionView_Expansion2.lua": "MakeDeal_ApplyStatement = nil\n",
        "DLC/Expansion1/Data/Expansion1_Loyalty.xml": "<LoyaltyLevels/>\n",
        "DLC/CivvisControl/UI/Civvis.lua": "MakeDeal_ApplyStatement -- ours, not the game's\n",
        "Base/Assets/UI/notes.txt": "MakeDeal_ApplyStatement in a file the game does not run\n",
    }
    for relative, text in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return root


class TheSearchCoversWhatItSays(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.assets = fake_install(self.tmp)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_ui_base_finds_the_shipped_line_in_quotable_form(self):
        lines = civ6_scripts.grep("MakeDeal_ApplyStatement", self.assets, "ui", "base", use_ripgrep=False)
        self.assertEqual(lines, [
            "Base/Assets/UI/DiplomacyActionView.lua:2: function MakeDeal_ApplyStatement(handler)"])

    def test_an_expansion_search_reads_only_that_expansion(self):
        lines = civ6_scripts.grep("MakeDeal_ApplyStatement", self.assets, "ui", "2", use_ripgrep=False)
        self.assertEqual([line.split(":")[0] for line in lines],
                         ["DLC/Expansion2/UI/Replacements/DiplomacyActionView_Expansion2.lua"])

    def test_all_reads_base_then_every_dlc_but_never_our_own_mod(self):
        lines = civ6_scripts.grep("MakeDeal_ApplyStatement", self.assets, "all", "all", use_ripgrep=False)
        files = [line.split(":")[0] for line in lines]
        self.assertEqual(files, [
            "Base/Assets/UI/DiplomacyActionView.lua",
            "DLC/Expansion2/UI/Replacements/DiplomacyActionView_Expansion2.lua",
        ], "CivvisControl is installed into the same tree and is not the shipped game")

    def test_where_selects_the_kind_of_source(self):
        text = civ6_scripts.grep("LOC_DIPLO_MODIFIER", self.assets, "text", "all", use_ripgrep=False)
        gameplay = civ6_scripts.grep("LOC_DIPLO_MODIFIER", self.assets, "gameplay", "all", use_ripgrep=False)
        self.assertEqual(len(text), 1)
        self.assertEqual(gameplay, [])
        loyalty = civ6_scripts.grep("LoyaltyLevels", self.assets, "gameplay", "1", use_ripgrep=False)
        self.assertEqual(loyalty, ["DLC/Expansion1/Data/Expansion1_Loyalty.xml:1: <LoyaltyLevels/>"])

    @unittest.skipIf(shutil.which("rg") is None, "ripgrep not on this PATH")
    def test_ripgrep_and_the_python_walk_print_the_same_lines(self):
        fast = civ6_scripts.grep("MakeDeal_ApplyStatement", self.assets, "all", "all", use_ripgrep=True)
        slow = civ6_scripts.grep("MakeDeal_ApplyStatement", self.assets, "all", "all", use_ripgrep=False)
        self.assertEqual(sorted(fast), sorted(slow))

    def test_locate_names_every_root_it_will_search(self):
        lines = civ6_scripts.locate(self.assets)
        joined = "\n".join(lines)
        self.assertIn("Base/Assets/UI", joined)
        self.assertIn("DLC/Expansion2/UI", joined)
        self.assertIn("database:", joined)
        self.assertEqual(civ6_scripts.locate(None)[0][:35], "no Civilization VI install found (s")

    def test_the_command_line_exits_by_match_count(self):
        import contextlib
        import io
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            hit = civ6_scripts.main(["--civ6", str(self.assets), "grep", "Barbarian", "--where", "gameplay", "--no-rg"])
            miss = civ6_scripts.main(["--civ6", str(self.assets), "grep", "NothingNamedThis", "--no-rg"])
        self.assertEqual((hit, miss), (0, 1))
        self.assertIn("Base/Assets/Gameplay/Data/Barbarians.xml:1:", out.getvalue())


if __name__ == "__main__":
    unittest.main()
