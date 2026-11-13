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
                "lib.rs": 'mod shipped;\n#[cfg(test)]\nmod declared;\n#[cfg(test)]\n#[path = "elsewhere/pathed.rs"]\nmod pathed;\n',
                "elsewhere/pathed.rs": 'name!("g");\n',
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


class InternedEffectArguments(unittest.TestCase):
    DECLARED = "fn policy_effect_named(&self, pid: usize, effect: Name) -> f64 {}\n"

    def reads(self, text):
        return inert.effect_reads({"src/x.rs": self.DECLARED + text})

    def names(self, text):
        return inert.name_literals({"src/x.rs": self.DECLARED + text})

    def test_an_interned_effect_argument_is_read(self):
        self.assertEqual(sorted(self.reads('g.policy_effect_named(0, crate::name!("gold_pct"))')), ["gold_pct"])

    def test_a_new_accessor_is_discovered_from_its_parameter(self):
        text = 'fn new_lookup(&self, effect_key: Name) {} g.new_lookup(name!("food"));'
        self.assertEqual(sorted(self.reads(text)), ["food"])
        self.assertEqual(self.names(text), {})

    def test_effect_and_ordinary_uses_of_the_same_name_keep_separate_audits(self):
        text = 'g.policy_effect_named(0, crate::name!("waived_effect")); let id = name!("waived_effect");'
        self.assertEqual(len(self.names(text)["waived_effect"]), 1)
        self.assertEqual(inert.audit_names("effects", self.reads(text), set(), {"waived_effect": "deliberate extension"}), [])
        self.assertEqual(inert.audit_names("names", self.names(text), set(), {}), ["waived_effect"])

    def test_a_named_effect_typo_is_still_rejected(self):
        text = 'g.policy_effect_named(0, crate::name!("foood"));'
        self.assertEqual(self.names(text), {})
        self.assertEqual(inert.audit_names("effects", self.reads(text), {"food"}, {}), ["foood"])

    def test_an_interned_unit_selector_does_not_replace_the_text_effect(self):
        text = 'fn unit_effect_for_kind(&self, pid: usize, kind: Name, effect: &str) {} g.unit_effect_for_kind(0, name!("warrior"), "gold_pct");'
        self.assertEqual(sorted(self.reads(text)), ["gold_pct"])
        self.assertEqual(sorted(self.names(text)), ["warrior"])

    def test_an_interned_unit_and_interned_effect_have_distinct_roles(self):
        text = 'fn kind_effect(&self, kind: Name, effect: Name) {} g.kind_effect(name!("warrior"), name!("gold_pct"));'
        self.assertEqual(sorted(self.reads(text)), ["gold_pct"])
        self.assertEqual(sorted(self.names(text)), ["warrior"])

    def test_a_nested_name_is_not_the_effect_key(self):
        text = 'g.policy_effect_named(lookup(crate::name!("warrior")), dynamic_key);'
        self.assertEqual(self.reads(text), {})
        self.assertEqual(sorted(self.names(text)), ["warrior"])

    def test_a_non_effect_parameter_keeps_ordinary_name_validation(self):
        text = 'fn apply_effect(&self, project: Name) {} g.apply_effect(name!("waived_effect"));'
        self.assertEqual(self.reads(text), {})
        self.assertEqual(sorted(self.names(text)), ["waived_effect"])

    def test_an_invalid_effect_key_is_not_dropped_from_both_audits(self):
        text = 'g.policy_effect_named(0, name!("bad-key"));'
        self.assertEqual(self.reads(text), {})
        self.assertEqual(sorted(self.names(text)), ["bad-key"])

    def test_ambiguous_overloads_do_not_borrow_an_effect_waiver(self):
        text = 'fn project_effect(&self, effect: Name) {} fn project_effect(&self, project: Name) {} g.project_effect(name!("waived_effect"));'
        self.assertEqual(self.reads(text), {})
        self.assertEqual(sorted(self.names(text)), ["waived_effect"])

    def test_non_key_text_arguments_are_not_recorded_as_effects(self):
        text = 'fn labeled_effect(&self, label: &str, effect: Name) {} g.labeled_effect("selector", name!("gold_pct"));'
        self.assertEqual(sorted(self.reads(text)), ["gold_pct"])
        self.assertEqual(self.names(text), {})

    def test_mutable_and_typed_receivers_do_not_shift_key_positions(self):
        for receiver in ["mut self", "self: Box<Self>", "&'a mut self"]:
            text = f'fn receiver_effect({receiver}, effect: Name) {{}} g.receiver_effect(name!("gold_pct"));'
            self.assertEqual(sorted(self.reads(text)), ["gold_pct"])
            self.assertEqual(self.names(text), {})


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
