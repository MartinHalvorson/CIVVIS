#!/usr/bin/env python3
"""Preflight must be able to tell "Steam is running" from "Steam can launch a game".

⚠ These are machine-independent on purpose. The bug being pinned was found on a
host that was signed out, and a test that reads the live client would pass or fail
for reasons that have nothing to do with the parsing.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_preflight  # noqa: E402


def _helpers(text: str):
    """Stand in for `pgrep -fl "Steam Helper"`."""
    return mock.patch.object(
        civ6_preflight.subprocess,
        "run",
        return_value=mock.Mock(stdout=text),
    )


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


if __name__ == "__main__":
    unittest.main()
