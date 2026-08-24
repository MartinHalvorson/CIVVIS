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


class CatalogImport(unittest.TestCase):
    """The refusals, because a wrongly imported row is worse than a missing one.

    These run on hermetic fixtures rather than the game database, so they gate
    the translation rules on a CI runner that has no Civilization VI install.
    """

    def build(self, xml: str):
        modifiers = civ6_modifiers.Modifiers()
        apply_xml(modifiers, xml)
        return civ6_modifiers.build_catalog(modifiers, civ6_modifiers.REPO / "data")

    def test_a_declared_effect_on_a_modelled_owner_is_translated(self):
        catalog, wiring, skipped = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="SIGHT_TYPE" CollectionType="COLLECTION_OWNER"
         EffectType="EFFECT_ADJUST_UNIT_SIGHT" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_sight" ModifierType="SIGHT_TYPE" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="fixture_sight" Name="Amount" Value="2" />
  </ModifierArguments>
  <UnitPromotionModifiers>
    <Row UnitPromotionType="PROMOTION_SPYGLASS" ModifierId="fixture_sight" />
  </UnitPromotionModifiers>
</GameInfo>
""")
        self.assertEqual(catalog, {"fixture_sight": {"effects": {"sight": 2}}})
        self.assertEqual(wiring, {"promotions": {"spyglass": ["fixture_sight"]}})
        self.assertEqual(skipped, [])

    def test_a_row_with_a_requirement_set_is_refused(self):
        catalog, _, skipped = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="SIGHT_TYPE" CollectionType="COLLECTION_OWNER"
         EffectType="EFFECT_ADJUST_UNIT_SIGHT" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_sight" ModifierType="SIGHT_TYPE"
         SubjectRequirementSetId="WHEN_EMBARKED" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="fixture_sight" Name="Amount" Value="2" />
  </ModifierArguments>
  <RequirementSets>
    <Row RequirementSetId="WHEN_EMBARKED" RequirementSetType="REQUIREMENTSET_TEST_ALL" />
  </RequirementSets>
  <RequirementSetRequirements>
    <Row RequirementSetId="WHEN_EMBARKED" RequirementId="UNIT_EMBARKED" />
  </RequirementSetRequirements>
  <Requirements>
    <Row RequirementId="UNIT_EMBARKED" RequirementType="REQUIREMENT_UNIT_EMBARKED" />
  </Requirements>
  <UnitPromotionModifiers>
    <Row UnitPromotionType="PROMOTION_SPYGLASS" ModifierId="fixture_sight" />
  </UnitPromotionModifiers>
</GameInfo>
""")
        self.assertEqual(catalog, {})
        self.assertEqual(len(skipped), 1)
        self.assertIn("requirement set cannot express", skipped[0])

    def test_a_collection_the_static_fold_cannot_scope_is_refused(self):
        catalog, _, skipped = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="SIGHT_TYPE" CollectionType="COLLECTION_PLAYER_UNITS"
         EffectType="EFFECT_ADJUST_UNIT_SIGHT" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_sight" ModifierType="SIGHT_TYPE" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="fixture_sight" Name="Amount" Value="2" />
  </ModifierArguments>
  <UnitPromotionModifiers>
    <Row UnitPromotionType="PROMOTION_SPYGLASS" ModifierId="fixture_sight" />
  </UnitPromotionModifiers>
</GameInfo>
""")
        self.assertEqual(catalog, {})
        self.assertIn("not a flattenable collection", skipped[0])

    def test_an_undeclared_effect_is_left_in_the_backlog(self):
        catalog, wiring, skipped = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="MYSTERY_TYPE" CollectionType="COLLECTION_OWNER"
         EffectType="EFFECT_SOMETHING_CIVVIS_DOES_NOT_MODEL" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_mystery" ModifierType="MYSTERY_TYPE" />
  </Modifiers>
  <UnitPromotionModifiers>
    <Row UnitPromotionType="PROMOTION_SPYGLASS" ModifierId="fixture_mystery" />
  </UnitPromotionModifiers>
</GameInfo>
""")
        self.assertEqual((catalog, wiring, skipped), ({}, {}, []))

    def test_an_owner_civvis_does_not_model_emits_nothing(self):
        catalog, wiring, _ = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="SIGHT_TYPE" CollectionType="COLLECTION_OWNER"
         EffectType="EFFECT_ADJUST_UNIT_SIGHT" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_sight" ModifierType="SIGHT_TYPE" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="fixture_sight" Name="Amount" Value="2" />
  </ModifierArguments>
  <UnitPromotionModifiers>
    <Row UnitPromotionType="PROMOTION_NOT_IN_CIVVIS" ModifierId="fixture_sight" />
  </UnitPromotionModifiers>
</GameInfo>
""")
        self.assertEqual((catalog, wiring), ({}, {}))

    def test_one_row_two_consumer_keys_becomes_two_bundles(self):
        # GRANT_INFLUENCE_TOKEN's single-Envoy award is attached to five civics
        # and to the Greek Acropolis. A tree node reads `free_envoys` and a
        # district reads `envoys`, so one bundle carrying both would put a key
        # on each owner that the other's consumer reads.
        catalog, wiring, _ = self.build("""\
<GameInfo>
  <DynamicModifiers>
    <Row ModifierType="ENVOY_TYPE" CollectionType="COLLECTION_OWNER"
         EffectType="EFFECT_GRANT_INFLUENCE_TOKEN" />
  </DynamicModifiers>
  <Modifiers>
    <Row ModifierId="fixture_envoy" ModifierType="ENVOY_TYPE" />
  </Modifiers>
  <ModifierArguments>
    <Row ModifierId="fixture_envoy" Name="Amount" Value="1" />
  </ModifierArguments>
  <CivicModifiers>
    <Row CivicType="CIVIC_MYSTICISM" ModifierId="fixture_envoy" />
  </CivicModifiers>
  <DistrictModifiers>
    <Row DistrictType="DISTRICT_ACROPOLIS" ModifierId="fixture_envoy" />
  </DistrictModifiers>
</GameInfo>
""")
        self.assertEqual(
            catalog,
            {
                "fixture_envoy__envoys": {"effects": {"envoys": 1}},
                "fixture_envoy__free_envoys": {"effects": {"free_envoys": 1}},
            },
        )
        self.assertEqual(
            wiring,
            {
                "civics": {"mysticism": ["fixture_envoy__free_envoys"]},
                "districts": {"acropolis": ["fixture_envoy__envoys"]},
            },
        )

    def test_the_catalog_text_is_byte_stable_and_integral(self):
        text = civ6_modifiers.catalog_json(
            {"b": {"effects": {"z": 2.0, "a": 1.0}}, "a": {"effects": {"m": -3.0}}}
        )
        self.assertEqual(
            text,
            '{\n  "a": {\n    "effects": {\n      "m": -3\n    }\n  },\n'
            '  "b": {\n    "effects": {\n      "a": 1,\n      "z": 2\n    }\n  }\n}\n',
        )


if __name__ == "__main__":
    unittest.main()
