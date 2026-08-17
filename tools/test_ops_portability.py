#!/usr/bin/env python3
"""Operational scripts must run on the host that installs them.

`tools/ops/` was written on one machine and grew a habit of naming that
machine's home directory outright. That is invisible on the machine it was
written on and total everywhere else, and on 2026-08-17 it cost the project
14.3 hours of Civilization VI attempts: `civvis-game-supervisor.sh` carried
`REPO=/Users/martin/CIVVIS`, so on this host it reached `cd "$REPO"`, logged
"no tree at ...", slept, and did that forever. It could never be installed as a
service, so the ladder loop was hand-started from a terminal session instead —
and when that session ended, so did the loop, with nothing watching.

Two rules, because the debt is real and paying all of it here would be a
different change than the one that fixes the supervisor:

* a script installed as a launchd service must have zero hardcoded homes;
* every other script is held to a recorded count that may fall and never rise.
"""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_collab  # noqa: E402

OPS = Path(__file__).resolve().parent / "ops"

# `/Users/$USER/` and `/Users/${HOME}` are derivations, not hardcoded homes.
HARDCODED_HOME = re.compile(r"/Users/(?!\$)[A-Za-z][A-Za-z0-9._-]*/")

# Installed by `civvis_collab.py bootstrap` as launchd services. A service that
# only works on its author's machine is a service that silently does nothing.
MANAGED = {
    "civvis-game-supervisor.sh",
    "ladder_watchdog.py",
}

# Everything else, at the count measured on 2026-08-17. These are hand-run
# operator scripts, not installed services, so they are ratcheted rather than
# blocked: lower a number when you fix one, and never raise one.
LEGACY_DEBT = {
    "civvis-keeper.sh": 9,
    "civvis-chain-status.sh": 8,
    "civvis-batch-loop.sh": 7,
    "civvis-interactive-host.sh": 7,
    "civvis-refresh.sh": 5,
    "civvis-popup-keeper.sh": 4,
    "civvis-overnight-audit.sh": 3,
    "civvis-overnight-watchdog.sh": 3,
    "civvis-challenger-guard.sh": 2,
    "civvis-item6-rerun.sh": 2,
    "civvis-goal-report.sh": 1,
    "civvis-goal-watch.sh": 1,
    "civvis-mirror-keeper.sh": 1,
    "civvis-tabs.sh": 1,
    "civvis-tcc-probe.sh": 1,
}


def hardcoded_homes(path: Path) -> list[str]:
    """Hardcoded home paths in executable lines. Comments may quote history."""
    found = []
    for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        for hit in HARDCODED_HOME.findall(line):
            found.append(f"{path.name}:{number}: {hit}")
    return found


class ServicesRunWhereTheyAreInstalled(unittest.TestCase):
    def test_managed_scripts_name_no_home_directory(self):
        for name in sorted(MANAGED):
            path = OPS / name
            self.assertTrue(path.is_file(), f"{name} is installed but missing")
            found = hardcoded_homes(path)
            self.assertEqual(found, [], f"{name} is installed as a launchd "
                                        f"service and must derive its paths:\n  "
                                        + "\n  ".join(found))

    def test_the_supervisor_derives_its_tree(self):
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("HEAD_REPO=", source)
        self.assertIn("REPO=$HEAD_REPO", source,
                      "the `head` pin must resolve to the derived tree")


class LegacyDebtOnlyFalls(unittest.TestCase):
    def test_no_script_gains_a_hardcoded_home(self):
        for name, allowed in sorted(LEGACY_DEBT.items()):
            path = OPS / name
            if not path.is_file():
                continue
            actual = len(hardcoded_homes(path))
            self.assertLessEqual(
                actual, allowed,
                f"{name} went from {allowed} hardcoded home paths to {actual}. "
                f"Derive the path instead — see the supervisor's HEAD_REPO.")

    def test_a_fixed_script_lowers_its_recorded_number(self):
        for name, allowed in sorted(LEGACY_DEBT.items()):
            path = OPS / name
            if not path.is_file():
                self.fail(f"{name} is gone; drop it from LEGACY_DEBT")
            actual = len(hardcoded_homes(path))
            self.assertEqual(
                actual, allowed,
                f"{name} now has {actual} hardcoded home paths, not {allowed}. "
                f"Someone fixed some: set LEGACY_DEBT['{name}'] = {actual} so "
                f"the ratchet holds the new floor.")

    def test_a_new_script_is_classified(self):
        known = MANAGED | set(LEGACY_DEBT)
        for path in sorted(OPS.iterdir()):
            if not path.is_file() or path.name in known:
                continue
            self.assertEqual(
                hardcoded_homes(path), [],
                f"{path.name} is new and names a home directory. Derive it, or "
                f"add it to LEGACY_DEBT with a reason.")


class ManagedServicesCanBeUpdated(unittest.TestCase):
    """`write_managed_service` refuses to replace a plist without its marker.

    It makes that check before the "identical, nothing to do" check, so a
    managed plist written without the marker installs once and can then never
    be changed. The memory guard shipped exactly that way on 2026-08-17: every
    later `bootstrap` raised the moment the content had to move.
    """

    def _plists(self):
        repo = Path(__file__).resolve().parent.parent
        return {
            "memguard": civvis_collab.macos_memguard_plist(Path("/x/memguard.py")),
            "ladder": civvis_collab.macos_ladder_plist(
                Path("/x/civvis-game-supervisor.sh"), repo),
            "ladder-watchdog": civvis_collab.macos_ladder_watchdog_plist(
                Path("/x/ladder_watchdog.py")),
        }

    def test_every_managed_plist_carries_the_marker(self):
        marker = civvis_collab.FRESHNESS_MARKER.encode("utf-8")
        for name, data in self._plists().items():
            self.assertIn(marker, data,
                          f"the {name} plist cannot be updated after install")

    def test_a_managed_plist_can_be_rewritten(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as raw:
            for name, data in self._plists().items():
                path = Path(raw) / f"{name}.plist"
                self.assertTrue(civvis_collab.write_managed_service(path, data))
                changed = data.replace(b"<integer>10</integer>",
                                       b"<integer>11</integer>") + b"\n"
                civvis_collab.write_managed_service(path, changed)

    def test_the_supervisor_job_keeps_itself_alive(self):
        repo = Path(__file__).resolve().parent.parent
        plist = civvis_collab.macos_ladder_plist(Path("/x/sup.sh"), repo).decode()
        self.assertIn("<key>KeepAlive</key>", plist)
        self.assertIn("<key>ThrottleInterval</key>", plist,
                      "a supervisor that dies instantly must not spin")

    def test_the_watchdog_is_a_separate_job(self):
        """It has to outlive the thing it watches, so it cannot live inside it."""
        self.assertNotEqual(civvis_collab.LADDER_LABEL,
                            civvis_collab.LADDER_WATCHDOG_LABEL)
        watchdog = civvis_collab.macos_ladder_watchdog_plist(
            Path("/x/w.py")).decode()
        self.assertIn("<key>StartInterval</key>", watchdog)
        self.assertNotIn("<key>KeepAlive</key>", watchdog)


if __name__ == "__main__":
    unittest.main()
