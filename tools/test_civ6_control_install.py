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
