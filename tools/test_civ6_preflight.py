#!/usr/bin/env python3
"""Regression tests for live CIV VI preflight checks."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_preflight  # noqa: E402


def _helpers(text: str):
    return mock.patch.object(
        civ6_preflight.subprocess,
        "run",
        return_value=mock.Mock(stdout=text),
    )


class InstalledSourceMatchesTest(unittest.TestCase):
    def test_exact_worktree_source_matches(self) -> None:
        self.assertTrue(civ6_preflight.installed_source_matches(b"print('ok')\n", b"print('ok')\n"))

    def test_configured_install_prelude_matches_its_source_suffix(self) -> None:
        source = b"print('ok')\n"
        installed = b"CivvisControlConfig = { RunTag = 'live' }\n\n" + source

        self.assertTrue(civ6_preflight.installed_source_matches(installed, source))

    def test_different_installed_source_does_not_match(self) -> None:
        self.assertFalse(civ6_preflight.installed_source_matches(b"print('old')\n", b"print('new')\n"))


class SteamSignInTest(unittest.TestCase):
    def test_signed_out_client_reads_zero(self) -> None:
        """The state that blocked every run on 2026-08-01.

        `steamid=0` is what a running-but-unauthenticated client carries, and it
        must be distinguishable from "no Steam at all" — one is fixable by the
        launcher, the other needs a human.
        """
        with _helpers("69141 /Steam Helper -steampid=69126 -steamid=0 -uimode=7\n"):
            self.assertEqual(civ6_preflight.steam_account_id(), 0)

    def test_signed_in_client_reads_its_account(self) -> None:
        with _helpers("69141 /Steam Helper -steamid=76561198766826522 -uimode=7\n"):
            self.assertEqual(
                civ6_preflight.steam_account_id(), 76561198766826522
            )

    def test_a_lagging_helper_does_not_mask_a_signed_in_client(self) -> None:
        """⚠ Steam runs several helpers and a freshly spawned one can still read 0
        while the client is authenticated. Taking any single line would report a
        signed-in host as signed out and refuse every batch — a false FAIL here is
        worse than the bug, because it stops work that would have succeeded."""
        with _helpers(
            "69141 /Steam Helper -steamid=76561198766826522\n"
            "69146 /Steam Helper --type=gpu-process -steamid=0\n"
        ):
            self.assertEqual(
                civ6_preflight.steam_account_id(), 76561198766826522
            )

    def test_no_steam_at_all_is_unknown_not_signed_out(self) -> None:
        """None, not 0. "Steam is absent" must not be reported as "signed out",
        which would send the operator to a login screen that is not the problem."""
        with _helpers(""):
            self.assertIsNone(civ6_preflight.steam_account_id())


class BundleSignatureTest(unittest.TestCase):
    """Issue #1342: the harness unsigns `Civ6.app` and preflight never looked.

    Every one of these is a WARN. A broken seal does not refuse a launch — a
    host with a healthy trust record plays with the mod installed, and a host
    with a poisoned one is refused with the signature valid — so failing here
    would block runs that would have worked.
    """

    def _seal(self, **fields):
        seal = {"bundle": "/Games/Civ6.app", "state": "broken",
                "ours": [], "foreign": [], "detail": ""}
        seal.update(fields)
        return mock.patch(
            "civ6_control.install.signature_report", return_value=seal
        )

    def test_a_valid_signature_passes_without_a_warning(self) -> None:
        report = civ6_preflight.Report()
        with self._seal(state="valid", detail="valid on disk"):
            civ6_preflight.check_bundle(report)

        self.assertEqual((report.failures, report.warnings), ([], []))

    def test_our_own_install_warns_and_names_the_reversal(self) -> None:
        report = civ6_preflight.Report()
        with self._seal(detail="a sealed resource is missing or invalid",
                        ours=["/Games/Civ6.app/Contents/Assets/DLC/CivvisControl/a.lua"]):
            civ6_preflight.check_bundle(report)

        self.assertEqual(report.failures, [])
        self.assertIn("--uninstall", report.warnings[0])

    def test_a_foreign_file_is_called_out_as_not_ours_to_fix(self) -> None:
        """The one state teardown cannot resolve, so it must not read like the
        routine one."""
        report = civ6_preflight.Report()
        with self._seal(detail="a sealed resource is missing or invalid",
                        ours=["/Games/Civ6.app/.../CivvisControl/a.lua"],
                        foreign=["/Games/Civ6.app/Contents/Resources/other"]):
            civ6_preflight.check_bundle(report)

        self.assertEqual(report.failures, [])
        self.assertIn("NOT", report.warnings[0])
        self.assertIn("/Contents/Resources/other", report.warnings[0])

    def test_no_game_installed_warns_instead_of_exiting_preflight(self) -> None:
        """⚠ `civ6_env.install_dir` raises SystemExit. Preflight is routinely run
        on machines that only ever edit the mod, and letting that escape would
        abort every other check after it."""
        report = civ6_preflight.Report()
        with mock.patch("civ6_control.install.signature_report",
                        side_effect=SystemExit("install not found")):
            civ6_preflight.check_bundle(report)

        self.assertEqual(report.failures, [])
        self.assertEqual(len(report.warnings), 1)


if __name__ == "__main__":
    unittest.main()
