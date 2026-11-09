#!/usr/bin/env python3
"""The audits in civvis_inert.py find what they say they find.

Each of these is a scrape, and a scrape that quietly stops matching reports a
clean repository. So each is tested on a planted defect, and the real tree is
checked for the floor a working scrape must clear.
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import civvis_inert as inert


def tree(files: dict[str, str]) -> tempfile.TemporaryDirectory:
    holder = tempfile.TemporaryDirectory()
    for name, text in files.items():
        path = Path(holder.name) / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return holder


class DuplicateKeys(unittest.TestCase):
    def test_a_key_written_twice_is_named_with_its_file(self):
        with tree({"a.json": '{"moai": {"x": 1}, "moai": {"x": 2}}'}) as root:
            self.assertEqual(inert.duplicate_keys(Path(root)), ["a.json: moai"])

    def test_a_nested_duplicate_counts(self):
        with tree({"a.json": '{"moai": {"x": 1, "x": 2}}'}) as root:
            self.assertEqual(inert.duplicate_keys(Path(root)), ["a.json: x"])

    def test_the_same_key_in_two_objects_is_fine(self):
        with tree({"a.json": '{"a": {"x": 1}, "b": {"x": 2}}'}) as root:
            self.assertEqual(inert.duplicate_keys(Path(root)), [])

    def test_no_files_is_a_broken_check_not_a_clean_one(self):
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaises(SystemExit):
                inert.duplicate_keys(Path(root))


class ProductionSources(unittest.TestCase):
    def sources(self, files):
        with tree({f"src/{name}": text for name, text in files.items()}) as root:
            return inert.production_sources(Path(root) / "src")

    def test_every_shape_of_test_code_is_dropped(self):
        found = self.sources(
            {
                "lib.rs": 'mod shipped;\n#[cfg(test)]\nmod declared;\n',
                "shipped.rs": 'name!("a");\n#[cfg(test)]\nmod inner {\n    name!("b");\n}\n',
                "declared.rs": 'name!("c");\n',
                "game/tests.rs": 'name!("d");\n',
                "game/thing_tests.rs": 'name!("e");\n',
                "game/tests/deep.rs": 'name!("f");\n',
            }
        )
        self.assertEqual(sorted(inert.name_literals(found)), ["a"])

    def test_a_comment_is_not_a_read(self):
        found = self.sources({"lib.rs": '// name!("gone");\nname!("kept");\n'})
        self.assertEqual(sorted(inert.name_literals(found)), ["kept"])


class EffectReads(unittest.TestCase):
    def reads(self, text):
        return sorted(inert.effect_reads({"src/x.rs": text}))

    def test_a_map_lookup_across_lines(self):
        self.assertEqual(self.reads('spec.effects\n    .get("gold")\n'), ["gold"])

    def test_contains_key(self):
        self.assertEqual(self.reads('spec.effects.contains_key("gold")'), ["gold"])

    def test_an_accessor_with_earlier_arguments(self):
        self.assertEqual(self.reads('self.policy_effect(city.owner, "gold_pct")'), ["gold_pct"])

    def test_an_accessor_nobody_listed(self):
        self.assertEqual(self.reads('g.brand_new_effect_lookup(1, "food")'), ["food"])

    def test_a_definition_is_not_a_read(self):
        self.assertEqual(self.reads('fn policy_effect(pid: usize, key: &str) {}'), [])

    def test_a_text_argument_of_a_nested_call_is_not_the_key(self):
        self.assertEqual(self.reads('self.policy_effect(lookup("nope"), other)'), [])

    def test_a_resolution_id_is_not_an_effect(self):
        self.assertEqual(self.reads('g.congress_effect_active("world_ideology")'), [])

    def test_a_format_string_is_skipped_not_misread(self):
        self.assertEqual(self.reads('g.policy_effect(0, "{} bonus")'), [])


class AuditNames(unittest.TestCase):
    def test_an_unknown_name_is_a_problem(self):
        self.assertEqual(
            inert.audit_names("x", {"theater": ["a:1"]}, {"theater_square"}, {}), ["theater"]
        )

    def test_a_waived_name_is_not(self):
        self.assertEqual(inert.audit_names("x", {"unknown": ["a:1"]}, set(), {"unknown": "y"}), [])

    def test_a_waiver_for_a_name_nothing_reads_is_a_problem(self):
        self.assertEqual(inert.audit_names("x", {}, set(), {"gone": "y"}), ["gone"])


class TheRealTree(unittest.TestCase):
    """Floors: a scrape that broke would otherwise look like a clean tree."""

    @classmethod
    def setUpClass(cls):
        cls.sources = inert.production_sources()

    def test_no_data_file_writes_a_key_twice(self):
        self.assertEqual(inert.duplicate_keys(), [])

    def test_the_name_scrape_still_sees_the_engine(self):
        self.assertGreater(len(inert.name_literals(self.sources)), 100)

    def test_the_effect_scrape_still_sees_the_engine(self):
        self.assertGreater(len(inert.effect_reads(self.sources)), 500)

    def test_test_only_modules_are_not_scanned(self):
        self.assertNotIn("src/game/victory_conditions.rs", self.sources)

    def test_every_waiver_gives_a_reason(self):
        sections = json.loads(inert.WAIVERS.read_text(encoding="utf-8"))
        for section in ("waivers", "names", "effect_reads"):
            for name, reason in sections[section].items():
                self.assertTrue(reason.strip(), f"{section}/{name} has no reason")


if __name__ == "__main__":
    unittest.main()
