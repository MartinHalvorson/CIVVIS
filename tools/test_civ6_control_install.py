#!/usr/bin/env python3
"""The protected-DLC installer path must stay usable without shell access."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import install  # noqa: E402


class ProtectedInstallTest(unittest.TestCase):
    def test_city_export_preserves_great_works_citizens_yields_and_progress(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local function exportState", 1)[1].split(
            "local units = {};", 1
        )[0]

        self.assertIn("citizens:IsPlotWorked(px, py)", exporter)
        self.assertIn("plot:GetWorkerCount()", exporter)
        self.assertIn("blds:GetGreatWorkInSlot", exporter)
        self.assertIn("Game.GetGreatWorkDataFromIndex", exporter)
        self.assertIn("city:GetYield(YieldTypes.SCIENCE)", exporter)
        self.assertIn(
            "local prodProgress, prodCost = productionProgress(city, queue)", exporter
        )
        self.assertIn("production_progress = prodProgress", exporter)
        self.assertIn("production_cost = prodCost", exporter)

    def test_purchase_actuator_preserves_faith_formation_and_district_placement(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn('kind == "purchase_faith"', source)
        self.assertIn('kind == "purchase_faith" and "YIELD_FAITH" or "YIELD_GOLD"', source)
        self.assertIn("MilitaryFormationTypes.CORPS_MILITARY_FORMATION", source)
        self.assertIn("MilitaryFormationTypes.ARMY_MILITARY_FORMATION", source)
        self.assertIn(
            "formationForCost = MilitaryFormationTypes.STANDARD_MILITARY_FORMATION",
            source,
        )
        self.assertNotIn(
            "params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] =\n"
            "\t\t\t\t\tMilitaryFormationTypes.STANDARD_MILITARY_FORMATION",
            source,
        )
        self.assertIn("params[CityOperationTypes.PARAM_X] = x", source)
        self.assertIn("params[CityOperationTypes.PARAM_Y] = y", source)
        self.assertIn("results[CityCommandResults.FAILURE_REASONS]", source)
        self.assertIn("reasons = reasons", source)

    def test_military_emergency_popup_uses_firaxis_pass_path(self) -> None:
        modinfo = (install.MOD_SOURCE / "CivvisControl.modinfo").read_text()
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn(
            '<LuaContext>WorldCongressPopup</LuaContext>',
            modinfo,
        )
        self.assertIn('NAME == "WorldCongressPopup"', closer)
        self.assertIn('type(OnPass) == "function"', closer)
        self.assertIn("OnPass();", closer)

    def test_between_turns_congress_uses_the_complete_firaxis_hide_path(self) -> None:
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()
        congress = closer.split('NAME == "WorldCongressBetweenTurns"', 1)[1].split(
            "return true;", 1
        )[0]

        self.assertIn('type(OnHide) == "function"', congress)
        self.assertIn("OnHide();", congress)
        self.assertNotIn("ReleaseEventLock();", congress)

    def test_diplomacy_popups_escalate_on_the_quick_clock(self) -> None:
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn('NAME == "DiplomacyActionView"', closer)
        self.assertIn('NAME == "DiplomacyDealView"', closer)
        self.assertIn("tonumber(cfg.DialogueSeconds) or 0.25", closer)
        self.assertIn("DESKTOP_AFTER = 4", closer)
        self.assertIn('report("autoclose_desktop"', closer)

    def test_governors_export_exact_state_and_use_stock_operation_indices(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("-- Governor Titles", 1)[1].split('emit("state"', 1)[0]
        actuator = source.split('if kind == "governor_appoint"', 1)[1].split(
            'if kind == "war"', 1
        )[0]

        self.assertIn("governors:GetGovernorList()", exporter)
        self.assertIn("governors:GetGovernorPoints()", exporter)
        self.assertIn("governors:GetGovernorPointsSpent()", exporter)
        self.assertIn("governor:GetAssignedCity()", exporter)
        self.assertIn("governor:HasPromotion(promotion.Hash)", exporter)
        self.assertIn("governor:IsEstablished()", exporter)
        self.assertIn("governor:GetNeutralizedTurns()", exporter)
        self.assertIn(
            "params[PlayerOperations.PARAM_GOVERNOR_TYPE] = governor.Index", actuator
        )
        self.assertIn(
            "params[PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE] = promotion.Index",
            actuator,
        )
        self.assertIn("params[PlayerOperations.PARAM_PLAYER_ONE] = cityOwner", source)
        self.assertIn("GovernorAppointed = onGovernorAppointed", source)
        self.assertNotIn("params[govParam] = row.Hash", source)
        self.assertIn("params[govParam] = row.Index", source)
        self.assertIn("params[playerParam] = pid", source)

    def test_district_export_distinguishes_foundations_from_completed_districts(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("-- ★★★★★ AND WHAT IT HAS DISTRICTED", 1)[1].split(
            "-- ★★★★★ WHOSE RELIGION", 1
        )[0]

        self.assertIn("district:GetX()", exporter)
        self.assertIn("district:GetY()", exporter)
        self.assertIn("district:IsComplete()", exporter)
        self.assertIn("complete = districtComplete[", exporter)

    def test_district_pillage_uses_firaxis_city_district_api(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("cityDistricts:IsPillaged(dtype, plotIndex)", source)
        self.assertNotIn("plot:IsDistrictPillaged()", source)

    def test_production_resumes_placed_district_without_replacing_its_plot(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        builder = source.split("local function buildParams", 1)[1].split(
            "-- What each city was last told to build", 1
        )[0]

        self.assertIn("city:GetBuildQueue():HasBeenPlaced(row.Hash)", builder)
        self.assertIn("if not alreadyPlaced then", builder)
        self.assertIn("if not alreadyPlaced and where == nil then", builder)

    def test_controller_uses_real_project_and_great_person_command_ids(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertNotIn('"PROJECT_CAMPUS_RESEARCH_GRANT"', source)
        self.assertIn('"PROJECT_ENHANCE_DISTRICT_CAMPUS"', source)
        self.assertIn('"UNITCOMMAND_ACTIVATE_GREAT_PERSON"', source)
        self.assertIn("GetActivationHighlightPlots()", source)

    def test_controller_maps_every_civvis_project_to_its_firaxis_type(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        aliases = {
            "PROJECT_CAMPUS_RESEARCH_GRANTS": "PROJECT_ENHANCE_DISTRICT_CAMPUS",
            "PROJECT_HOLY_SITE_PRAYERS": "PROJECT_ENHANCE_DISTRICT_HOLY_SITE",
            "PROJECT_COMMERCIAL_HUB_INVESTMENT": (
                "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB"
            ),
            "PROJECT_HARBOR_SHIPPING": "PROJECT_ENHANCE_DISTRICT_HARBOR",
            "PROJECT_ENCAMPMENT_TRAINING": "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT",
            "PROJECT_INDUSTRIAL_ZONE_LOGISTICS": (
                "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE"
            ),
            "PROJECT_THEATER_SQUARE_FESTIVAL": "PROJECT_ENHANCE_DISTRICT_THEATER",
        }
        for civvis_type, firaxis_type in aliases.items():
            self.assertIn(f'{civvis_type} = "{firaxis_type}"', source)

        produce = source.split('if kind == "produce" then', 1)[1].split(
            'if kind == "purchase" or kind == "purchase_faith" then', 1
        )[0]
        alias_at = produce.index("CIVVIS_PROJECT_TYPES[verb]")
        resolve_at = produce.index("resolveType(GameInfo.Types, verb)")
        self.assertLess(alias_at, resolve_at)

    def test_controller_maps_government_building_names_to_firaxis_types(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        aliases = {
            "BUILDING_AUDIENCE_CHAMBER": "BUILDING_GOV_TALL",
            "BUILDING_ANCESTRAL_HALL": "BUILDING_GOV_WIDE",
            "BUILDING_WARLORDS_THRONE": "BUILDING_GOV_CONQUEST",
            "BUILDING_FOREIGN_MINISTRY": "BUILDING_GOV_CITYSTATES",
            "BUILDING_INTELLIGENCE_AGENCY": "BUILDING_GOV_SPIES",
            "BUILDING_GRAND_MASTERS_CHAPEL": "BUILDING_GOV_FAITH",
            "BUILDING_WAR_DEPARTMENT": "BUILDING_GOV_MILITARY",
            "BUILDING_NATIONAL_HISTORY_MUSEUM": "BUILDING_GOV_CULTURE",
            "BUILDING_ROYAL_SOCIETY": "BUILDING_GOV_SCIENCE",
        }
        for civvis_type, firaxis_type in aliases.items():
            self.assertIn(f'{civvis_type} = "{firaxis_type}"', source)

        produce = source.split('if kind == "produce" then', 1)[1].split(
            'if kind == "purchase" or kind == "purchase_faith" then', 1
        )[0]
        alias_at = produce.index("CIVVIS_GOVERNMENT_BUILDING_TYPES[verb]")
        resolve_at = produce.index("resolveType(GameInfo.Types, verb)")
        self.assertLess(alias_at, resolve_at)

    def test_parameterless_unit_commands_match_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        helper = source.split("local function commandUnit", 1)[1].split(
            "-- Spend gold", 1
        )[0]

        self.assertIn("CanStartCommand(unit, hash, false, true)", helper)
        self.assertIn("RequestCommand(unit, hash);", helper)
        self.assertNotIn("params", helper)

    def test_unit_formation_bridge_uses_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local units = {};", 1)[1].split("-- Rivals:", 1)[0]
        handler = source.split('if verb == "ENTER_FORMATION" then', 1)[1].split(
            'if verb == "EXIT_FORMATION" then', 1
        )[0]

        self.assertIn("unit:GetFormationUnitCount()", exporter)
        self.assertIn('CMD["UNITCOMMAND_ENTER_FORMATION"]', source)
        self.assertIn('CMD["UNITCOMMAND_EXIT_FORMATION"]', source)
        self.assertIn("params[UnitCommandTypes.PARAM_UNIT_PLAYER] = x", handler)
        self.assertIn("params[UnitCommandTypes.PARAM_UNIT_ID] = y", handler)
        self.assertIn("UnitManager.CanStartCommand(unit, hash, params)", handler)
        self.assertIn("UnitManager.RequestCommand(unit, hash, params)", handler)

    def test_religion_bridge_uses_firaxis_player_operations_and_exports_its_gate(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split('if kind == "religion" then', 1)[1].split(
            'if kind == "research" or kind == "civic" then', 1
        )[0]

        self.assertIn("HasReligiousFoundingUnit()", source)
        self.assertIn("founded_religion = founded_religion", source)
        self.assertIn("founded_religions = founded_religions", source)
        self.assertIn("religion_beliefs = religion_beliefs", source)
        self.assertIn("taken_religion_beliefs = taken_religion_beliefs", source)
        self.assertIn("prophet_pending = prophet_pending", source)
        self.assertIn("PlayerOperations.FOUND_RELIGION", handler)
        self.assertIn("PlayerOperations.ADD_BELIEF", handler)
        self.assertIn("PlayerOperations.PARAM_RELIGION_TYPE", handler)
        self.assertIn("PlayerOperations.PARAM_BELIEF_TYPE", handler)
        self.assertIn("gameReligion:IsInSomeReligion(follower.Index)", handler)
        self.assertNotIn("IsBeliefInSomeReligion", handler)
        self.assertIn('GameInfo.UnitOperations["UNITOPERATION_FOUND_RELIGION"]', handler)
        self.assertIn('"UNITOPERATION_FOUND_RELIGION",', source)
        self.assertIn(
            "CanStartOperation(prophet, foundOperation.Hash, nil, false,", handler
        )
        self.assertIn("OperationResultsTypes.NO_TARGETS", handler)
        self.assertIn("RequestOperation(prophet, foundOperation.Hash);", handler)

    def test_religious_units_export_progress_and_actuate_promote_and_spread(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        progress = source.split("local function unitProgress", 1)[1].split(
            "-- ⚠⚠ THE pcall GOES INSIDE THE LOOP", 1
        )[0]
        handler = source.split('if kind == "unit" then', 1)[1].split(
            'return false, "unknown_kind_"', 1
        )[0]

        self.assertIn("experience:GetExperiencePoints()", progress)
        self.assertIn("experience:GetLevel()", progress)
        self.assertIn("experience:GetPromotions()", progress)
        self.assertIn("unit:GetBuildCharges()", progress)
        self.assertIn("unit:GetSpreadCharges()", progress)
        self.assertIn("unit:GetReligionType()", progress)
        self.assertIn('"UNITOPERATION_SPREAD_RELIGION"', source)
        self.assertIn('"^PROMOTE:(.+)$"', handler)
        self.assertIn("results[UnitCommandResults.PROMOTIONS]", handler)
        self.assertIn(
            "params[UnitCommandTypes.PARAM_PROMOTION_TYPE] = promotion.Index",
            handler,
        )
        self.assertIn("UnitManager.RequestCommand(unit, hash, params)", handler)

    def test_civvis_soft_blockers_do_not_invoke_legacy_unit_ai(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split("local blocker = currentBlocker(pid);", 1)[1].split(
            "-- Only if the same blocker", 1
        )[0]
        civvis_branch = handler.split("if cfg.CivvisDecides then", 1)[1].split(
            "else", 1
        )[0]
        legacy_branch = handler.split("if cfg.CivvisDecides then", 1)[1].split(
            "else", 1
        )[1]

        self.assertIn('answered = "civvis_complete"', civvis_branch)
        self.assertNotIn("orderUnits(", civvis_branch)
        self.assertIn("orderUnits(player, pid, turn);", legacy_branch)

    def test_completed_civvis_pass_does_not_invoke_owned_blocker_fallbacks(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split("local function answerBlocker", 1)[1].split(
            "local function dismissBlocker", 1
        )[0]
        completed = handler.index('return "civvis_complete";')
        residual = handler.index("residualAnswers[name]")

        self.assertLess(completed, residual)
        self.assertIn("CIVVIS_OWNED_BLOCKERS[name]", handler[:completed])
        self.assertIn('awaiting.source == "civvis"', handler[:completed])
        self.assertIn("driveProduction(player, turn, true)", handler[residual:])

    def test_empty_civvis_order_batch_completes_without_legacy_fallback(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        settle = source.split("local function settleTurn", 1)[1].split(
            "-- Past the wait", 1
        )[0]

        self.assertIn("ready ~= nil and ready >= 0", settle)
        self.assertIn("if #rows == ready then", settle)
        self.assertNotIn("ready > 0", settle)
        self.assertNotIn("#rows > 0", settle)

    def test_live_rehost_assigns_and_reads_back_the_requested_leader(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        rehost = source.split("local function applyConfiguration()", 1)[1].split(
            "local function rehost()", 1
        )[0]
        self.assertIn("PlayerConfigurations[id]:SetLeaderTypeName(cfg.Leader)", rehost)
        self.assertIn("GetLeaderTypeName()", source.split("local function rehost()", 1)[1])

    def test_permission_denied_deploys_a_staged_copy_through_finder(self) -> None:
        target = Path("/protected/DLC/CivvisControl")
        staging = Path(tempfile.mkdtemp(prefix="civvis-install-test-"))
        config = {"RunTag": "live-test"}
        try:
            with patch.object(install, "install_dir", return_value=target), \
                 patch.object(
                     install, "_write_mod", side_effect=[PermissionError("TCC"), None],
                 ) as write_mod, \
                 patch.object(install, "_finder_replace") as replace, \
                 patch.object(install, "_drop_mod_index") as drop, \
                 patch.object(install.tempfile, "mkdtemp", return_value=str(staging)), \
                 patch.object(install.shutil, "rmtree") as cleanup:
                self.assertEqual(install.install(config), target)

            self.assertEqual(write_mod.call_args_list, [call(target, config), call(staging, config)])
            replace.assert_called_once_with(staging, target)
            drop.assert_called_once_with()
            cleanup.assert_called_once_with(staging, ignore_errors=True)
        finally:
            staging.rmdir()


class SealAttributionTest(unittest.TestCase):
    """Issue #1342: installing the mod unsigns `Civ6.app`, and nothing said so.

    The literal `codesign -v --verbose=2` output recorded in the issue, and
    reproduced on this host on 2026-08-08. The point of parsing it is the
    attribution: our own files are expected and `--uninstall` restores the
    seal, while anyone else's are a problem teardown cannot fix.
    """

    MOD = Path(
        "/Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl"
    )
    BROKEN = (
        "/Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app: a sealed resource is missing or invalid\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/config.json\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/"
        "CivvisControlSetup.lua\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/"
        "CivvisControlAgent.lua\n"
    )

    def test_the_issues_own_codesign_output_is_entirely_our_doing(self) -> None:
        ours, foreign = install.seal_breakers(self.BROKEN, self.MOD)

        self.assertEqual(len(ours), 3)
        self.assertEqual(foreign, [])
        self.assertTrue(all(p.endswith((".json", ".lua")) for p in ours))

    def test_the_summary_line_is_not_read_as_an_offending_file(self) -> None:
        """`<bundle>: <sentence>` and `file added: <path>` are the same shape
        reversed, so the parser keys on which half is the path."""
        ours, foreign = install.seal_breakers(self.BROKEN, self.MOD)

        self.assertNotIn(
            "a sealed resource is missing or invalid", "".join(ours + foreign)
        )

    def test_somebody_elses_modification_is_reported_separately(self) -> None:
        """The half that `--uninstall` will NOT fix."""
        text = self.BROKEN + (
            "file modified: /Users/x/Library/Application Support/Steam/steamapps/"
            "common/Sid Meier's Civilization VI/Civ6.app/Contents/Resources/other\n"
        )

        ours, foreign = install.seal_breakers(text, self.MOD)

        self.assertEqual(len(ours), 3)
        self.assertEqual([p.rsplit("/", 1)[-1] for p in foreign], ["other"])

    def test_a_sibling_directory_sharing_our_prefix_is_not_ours(self) -> None:
        """`CivvisControlOld/` starts with the mod path as a string and is a
        different directory; claiming it would promise an --uninstall that
        never removes it."""
        text = (
            "file added: " + str(self.MOD) + "Old/leftover.lua\n"
            "file added: " + str(self.MOD) + "/config.json\n"
        )

        ours, foreign = install.seal_breakers(text, self.MOD)

        self.assertEqual([p.rsplit("/", 1)[-1] for p in ours], ["config.json"])
        self.assertEqual([p.rsplit("/", 1)[-1] for p in foreign], ["leftover.lua"])

    def test_bundle_dir_is_the_enclosing_app_not_a_second_hardcoded_path(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD):
            self.assertEqual(install.bundle_dir(), Path(str(self.MOD).split("/Contents/")[0]))

    def test_an_install_outside_a_bundle_breaks_no_signature(self) -> None:
        outside = Path("/Users/x/Library/Application Support/Sid Meier's Civ VI/Mods/CivvisControl")
        with patch.object(install, "install_dir", return_value=outside):
            self.assertIsNone(install.bundle_dir())
            self.assertEqual(install.signature_report()["state"], "no-bundle")

    def test_a_valid_bundle_reports_valid_and_names_nobody(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run",
                          return_value=Mock(returncode=0, stdout="", stderr="")):
            seal = install.signature_report()

        self.assertEqual(seal["state"], "valid")
        self.assertEqual((seal["ours"], seal["foreign"]), ([], []))

    def test_a_broken_bundle_reports_the_verdict_and_attributes_the_files(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run",
                          return_value=Mock(returncode=1, stdout="", stderr=self.BROKEN)):
            seal = install.signature_report()

        self.assertEqual(seal["state"], "broken")
        self.assertEqual(seal["detail"], "a sealed resource is missing or invalid")
        self.assertEqual(len(seal["ours"]), 3)

    def test_an_unrunnable_codesign_is_unknown_rather_than_valid(self) -> None:
        """Absence of a verdict must never be reported as a good one."""
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run", side_effect=OSError("no codesign")):
            self.assertEqual(install.signature_report()["state"], "unknown")
