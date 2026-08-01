#!/usr/bin/env python3
"""The protected-DLC installer path must stay usable without shell access."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import install  # noqa: E402


class ProtectedInstallTest(unittest.TestCase):
    def test_controller_uses_real_project_and_great_person_command_ids(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertNotIn('"PROJECT_CAMPUS_RESEARCH_GRANT"', source)
        self.assertIn('"PROJECT_ENHANCE_DISTRICT_CAMPUS"', source)
        self.assertIn('"UNITCOMMAND_ACTIVATE_GREAT_PERSON"', source)
        self.assertIn("GetActivationHighlightPlots()", source)

    def test_parameterless_unit_commands_match_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        helper = source.split("local function commandUnit", 1)[1].split(
            "-- Spend gold", 1
        )[0]

        self.assertIn("CanStartCommand(unit, hash, false, true)", helper)
        self.assertIn("RequestCommand(unit, hash);", helper)
        self.assertNotIn("params", helper)

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
