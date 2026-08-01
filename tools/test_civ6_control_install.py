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
