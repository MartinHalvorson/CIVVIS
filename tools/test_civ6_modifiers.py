#!/usr/bin/env python3
"""Regression tests for the live Gathering Storm modifier census."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_modifiers  # noqa: E402


def apply_xml(modifiers: civ6_modifiers.Modifiers, text: str) -> None:
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "fixture.xml"
        path.write_text(text, encoding="utf-8")
        modifiers.apply_file(path)


class ActiveModifiers(unittest.TestCase):
    def test_expansion_remove_data_runs_before_its_replacements(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            base = root / "Base/Assets/Gameplay/Data"
            expansion = root / "DLC/Expansion1/Data"
            base.mkdir(parents=True)
            expansion.mkdir(parents=True)
            (base / "Base.xml").write_text("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="TYPE" EffectType="EFFECT_LIVE" CollectionType="COLLECTION_OWNER" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="retired" ModifierType="TYPE" />
  </Modifiers>
  <PolicyModifiers>
    <Row PolicyType="POLICY_RETIRED" ModifierId="retired" />
  </PolicyModifiers>
</GameInfo>
""", encoding="utf-8")
            (expansion / "Expansion1_RemoveData.xml").write_text("""\
<GameInfo>
  <PolicyModifiers>
    <Delete ModifierId="retired" />
  </PolicyModifiers>
</GameInfo>
""", encoding="utf-8")
            (expansion / "Expansion1_Policies.xml").write_text("""\
<GameInfo>
  <Modifiers>
    <Row ModifierId="replacement" ModifierType="TYPE" />
  </Modifiers>
  <PolicyModifiers>
    <Row PolicyType="POLICY_LIVE" ModifierId="replacement" />
  </PolicyModifiers>
</GameInfo>
""", encoding="utf-8")
            previous = civ6_modifiers.LOAD_ORDER
            civ6_modifiers.LOAD_ORDER = [
                "Base/Assets/Gameplay/Data",
                "DLC/Expansion1/Data",
            ]
            try:
                modifiers = civ6_modifiers.load(root)
            finally:
                civ6_modifiers.LOAD_ORDER = previous

        self.assertEqual(modifiers.active_modifier_ids(), {"replacement"})

    def test_deleted_owner_binding_cannot_resurrect_a_retired_modifier(self):
        modifiers = civ6_modifiers.Modifiers()
        apply_xml(modifiers, """\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="LIVE_TYPE" EffectType="EFFECT_LIVE" CollectionType="COLLECTION_OWNER" />
    <Row ModifierType="RETIRED_TYPE" EffectType="EFFECT_RETIRED" CollectionType="COLLECTION_OWNER" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="live" ModifierType="LIVE_TYPE" />
    <Row ModifierId="retired" ModifierType="RETIRED_TYPE" />
  </Modifiers>
  <PolicyModifiers>
    <Row PolicyType="POLICY_LIVE" ModifierId="live" />
    <Row PolicyType="POLICY_RETIRED" ModifierId="retired" />
  </PolicyModifiers>
</GameInfo>
""")
        apply_xml(modifiers, """\
<GameInfo>
  <PolicyModifiers>
    <Delete ModifierId="retired" />
  </PolicyModifiers>
</GameInfo>
""")

        self.assertEqual(modifiers.active_modifier_ids(), {"live"})
        self.assertNotIn("retired", modifiers.attachments)
        self.assertEqual(
            [entry["effect"] for entry in civ6_modifiers.census(modifiers)], ["LIVE"]
        )

    def test_nested_modifier_stays_live_through_its_parent_attachment(self):
        modifiers = civ6_modifiers.Modifiers()
        apply_xml(modifiers, """\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="ATTACH_TYPE" EffectType="EFFECT_ATTACH_MODIFIER" CollectionType="COLLECTION_OWNER" />
    <Row ModifierType="CHILD_TYPE" EffectType="EFFECT_CHILD" CollectionType="COLLECTION_OWNER" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="parent" ModifierType="ATTACH_TYPE" />
    <Row ModifierId="child" ModifierType="CHILD_TYPE" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="parent" Name="ModifierId" Value="child" />
  </ModifierArguments>
  <TraitModifiers>
    <Row TraitType="TRAIT_LIVE" ModifierId="parent" />
  </TraitModifiers>
</GameInfo>
""")

        self.assertEqual(modifiers.active_modifier_ids(), {"parent", "child"})
        entries = {entry["effect"]: entry for entry in civ6_modifiers.census(modifiers)}
        self.assertEqual(entries["CHILD"]["rows"], 1)
        self.assertEqual(entries["CHILD"]["owners"], {"(nested modifier)": 1})

    def test_delete_only_detaches_the_named_owner_table(self):
        modifiers = civ6_modifiers.Modifiers()
        apply_xml(modifiers, """\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="LIVE_TYPE" EffectType="EFFECT_LIVE" CollectionType="COLLECTION_OWNER" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="shared" ModifierType="LIVE_TYPE" />
  </Modifiers>
  <PolicyModifiers>
    <Row PolicyType="POLICY_LIVE" ModifierId="shared" />
    <Row PolicyType="POLICY_LIVE" ModifierId="shared" />
  </PolicyModifiers>
  <TraitModifiers>
    <Row TraitType="TRAIT_LIVE" ModifierId="shared" />
  </TraitModifiers>
</GameInfo>
""")
        apply_xml(modifiers, """\
<GameInfo>
  <PolicyModifiers>
    <Delete ModifierId="shared" />
  </PolicyModifiers>
</GameInfo>
""")

        self.assertEqual(modifiers.attachments["shared"], ["TraitModifiers"])
        self.assertEqual(modifiers.owners["shared"], ["TRAIT_LIVE"])
        self.assertEqual(modifiers.active_modifier_ids(), {"shared"})

    def test_deleted_modifier_row_does_not_leave_an_active_owner_reference(self):
        modifiers = civ6_modifiers.Modifiers()
        apply_xml(modifiers, """\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="LIVE_TYPE" EffectType="EFFECT_LIVE" CollectionType="COLLECTION_OWNER" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="removed" ModifierType="LIVE_TYPE" />
  </Modifiers>
  <ProjectCompletionModifiers>
    <Row ProjectType="PROJECT_LIVE" ModifierId="removed" />
  </ProjectCompletionModifiers>
</GameInfo>
""")
        apply_xml(modifiers, """\
<GameInfo>
  <Modifiers>
    <Delete ModifierId="removed" />
  </Modifiers>
</GameInfo>
""")

        self.assertEqual(modifiers.active_modifier_ids(), set())


if __name__ == "__main__":
    unittest.main()
