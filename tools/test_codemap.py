#!/usr/bin/env python3
"""The code map finds real things, and every shipping module describes itself.

A search tool that quietly stops matching would answer "nothing here" to the
agent relying on it, so each scrape has a floor on the real tree as well as a
test on a planted file.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import codemap


def repo(files: dict[str, str]) -> tempfile.TemporaryDirectory:
    holder = tempfile.TemporaryDirectory()
    for name, text in files.items():
        path = Path(holder.name) / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return holder


class Undocumented(unittest.TestCase):
    def missing(self, files):
        with repo(files) as root:
            return codemap.undocumented(Path(root))

    def test_a_module_with_no_header_is_named(self):
        self.assertEqual(self.missing({"src/a.rs": "fn x() {}\n"}), ["src/a.rs"])

    def test_a_header_is_enough(self):
        self.assertEqual(self.missing({"src/a.rs": "//! What a does.\n\nfn x() {}\n"}), [])

    def test_a_test_module_needs_none(self):
        files = {
            "src/lib.rs": "//! Root.\n#[cfg(test)]\nmod a;\n",
            "src/a.rs": "fn t() {}\n",
            "src/b/tests.rs": "fn t() {}\n",
            "src/b/thing_tests.rs": "fn t() {}\n",
        }
        self.assertEqual(self.missing(files), [])

    def test_a_test_module_behind_a_path_attribute_needs_none(self):
        files = {
            "src/lib.rs": '//! Root.\n#[cfg(test)]\n#[path = "x/a.rs"]\nmod a;\n',
            "src/x/a.rs": "fn t() {}\n",
        }
        self.assertEqual(self.missing(files), [])

    def test_an_included_file_cannot_carry_one(self):
        files = {
            "src/lib.rs": '//! Root.\ninclude!("plans.rs");\n',
            "src/plans.rs": "// plain comment\nconst X: u8 = 1;\n",
        }
        self.assertEqual(self.missing(files), [])


class Search(unittest.TestCase):
    ENTRIES = [
        codemap.Entry("src/a/settler.rs", 10, "Where a city goes.", [("step", 3)]),
        codemap.Entry("src/a/other.rs", 10, "Nothing relevant.", [("settler_step", 7)]),
        codemap.Entry("docs/SETTLING.md", 10, "Settling: notes."),
    ]

    def paths(self, *words):
        return [row[1].path for row in codemap.search(list(words), self.ENTRIES)]

    def test_a_file_name_outranks_a_function_name(self):
        self.assertEqual(self.paths("settler")[0], "src/a/settler.rs")

    def test_every_word_must_match(self):
        self.assertEqual(self.paths("settler", "nothing"), ["src/a/other.rs"])

    def test_no_match_is_empty(self):
        self.assertEqual(self.paths("zzz"), [])


class Summaries(unittest.TestCase):
    def test_rust_takes_the_first_sentence_of_the_header(self):
        text = "//! Does one thing. And more.\n//!\n//! Detail.\n\nfn x() {}\n"
        self.assertEqual(codemap._rust_summary(text), "Does one thing.")

    def test_rust_header_after_a_comment(self):
        self.assertEqual(codemap._rust_summary("// note\n//! Real.\n"), "Real.")

    def test_python_takes_the_docstring(self):
        self.assertEqual(codemap._python_summary('#!/x\n"""Does it.\n\nMore.\n"""\n'), "Does it.")

    def test_a_line_number_is_the_name_not_a_blank_before_it(self):
        self.assertEqual(codemap._functions(codemap._RUST_FN, "\n\nfn a() {}\n"), [("a", 3)])


class TheRealTree(unittest.TestCase):
    def test_every_shipping_module_says_what_it_is_for(self):
        self.assertEqual(codemap.undocumented(), [])

    def test_the_module_scrape_still_sees_the_crate(self):
        self.assertGreater(len(codemap.shipping_rust()), 400)

    def test_test_only_modules_are_not_counted(self):
        self.assertNotIn("src/game/victory_conditions.rs", codemap.shipping_rust())

    def test_the_gene_scrape_still_sees_the_registry(self):
        rows = codemap.gene_rows()
        self.assertGreater(len(rows), 500)
        self.assertIn("anvil", rows)

    def test_a_turn_has_its_known_steps_in_order(self):
        names = [name for _, name, _ in codemap.turn_order()]
        self.assertGreater(len(names), 50)
        self.assertLess(names.index("maintain_victory_portfolio"), names.index("advanced_research"))

    def test_a_search_finds_the_turn_loop(self):
        ranked = codemap.search(["take_turn_inner"], codemap.index())
        self.assertIn("src/ai/advanced.rs", [row[1].path for row in ranked])


if __name__ == "__main__":
    unittest.main()
