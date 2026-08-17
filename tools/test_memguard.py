"""The memory guard ships with the repository, and refuses to kill the wrong thing.

⚠ WHY THIS EXISTS. On 2026-08-10 two `civvis` benchmark processes reached a
**206 GB and a 205 GB physical footprint each on a 128 GB machine**, and the
kernel answered with a system-wide jetsam that terminated **14,818 processes**.
macOS honours neither `ulimit -v` nor `ulimit -d`, so nothing in the operating
system was ever going to stop it. The guard written that day lived in
`~/.local/bin` on the one laptop that had been hurt, installed by hand and
tracked by nothing — so every other machine in the fleet ran the same
benchmarks with no ceiling at all.

These cases are about the two ways this can go wrong: a guard that is not
installed, and a guard that kills something it must not.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))

import civvis_collab as collab  # noqa: E402

GUARD = TOOLS / "ops" / "memguard.py"


def load_guard():
    spec = importlib.util.spec_from_file_location("memguard", GUARD)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TheGuardIsInTheRepository(unittest.TestCase):
    def test_the_guard_is_versioned_and_runnable(self) -> None:
        self.assertTrue(GUARD.is_file(), f"{GUARD} must be committed")
        load_guard()  # importing it is the cheapest proof it is not broken

    def test_bootstrap_knows_where_it_is(self) -> None:
        self.assertEqual(collab.memguard_source(TOOLS.parent), GUARD)

    def test_a_missing_guard_is_an_error_not_a_silent_skip(self) -> None:
        with self.assertRaises(collab.CommandError):
            collab.install_memory_guard(Path("/nonexistent/repo"))


class TheJobItInstalls(unittest.TestCase):
    def plist(self) -> str:
        return collab.macos_memguard_plist(GUARD).decode()

    def test_the_job_carries_the_thresholds_and_the_guard_path(self) -> None:
        text = self.plist()
        self.assertIn(str(GUARD), text)
        self.assertIn(collab.MEMGUARD_HARD_GB, text)
        self.assertIn(collab.MEMGUARD_LABEL, text)

    def test_the_job_is_well_formed_plist(self) -> None:
        import plistlib
        parsed = plistlib.loads(collab.macos_memguard_plist(GUARD))
        self.assertEqual(parsed["Label"], collab.MEMGUARD_LABEL)
        self.assertEqual(parsed["StartInterval"], 10)
        self.assertIn("--hard-gb", parsed["ProgramArguments"])

    def test_the_ceiling_is_a_fraction_of_a_fleet_machine(self) -> None:
        # 32 GB on a 128 GB host: far above any legitimate run measured here,
        # far below the point where the kernel starts killing things nobody
        # asked it to. A ceiling set at the crash size protects nothing.
        self.assertLess(float(collab.MEMGUARD_HARD_GB), 64.0)
        self.assertGreater(float(collab.MEMGUARD_HARD_GB), 8.0)
        self.assertLess(float(collab.MEMGUARD_SOFT_GB),
                        float(collab.MEMGUARD_HARD_GB))


class ItRefusesToKillTheWrongThing(unittest.TestCase):
    """The kill is one line; the refusals are the reason it can run unattended."""

    def test_the_session_owning_processes_are_protected(self) -> None:
        guard = load_guard()
        # Killing these takes down every session on the machine, which is the
        # outcome the guard exists to prevent rather than to cause.
        for name in ("WindowServer", "Terminal", "loginwindow"):
            self.assertIn(name, guard.PROTECTED, f"{name} must never be killable")

    def test_a_protected_name_is_never_eligible(self) -> None:
        guard = load_guard()
        import os
        import subprocess
        # A real, live, third-party pid: not the guard's own and not its
        # parent, so the protected-name branch is the one under test rather
        # than the self-protection that comes before it.
        victim = subprocess.Popen(["sleep", "30"])
        self.addCleanup(victim.kill)
        ok, why = guard.eligible(victim.pid, "WindowServer", os.getuid())
        self.assertFalse(ok, why)
        self.assertIn("protected", why)
        # And the same pid under an ordinary name IS eligible, so the refusal
        # above is the name and not something incidental about the process.
        ok, why = guard.eligible(victim.pid, "sleep", os.getuid())
        self.assertTrue(ok, why)

    def test_another_users_process_is_never_eligible(self) -> None:
        guard = load_guard()
        # pid 1 is root's. A guard that reaps across users is a guard that
        # takes down the machine it is protecting.
        ok, _ = guard.eligible(1, "launchd", 501)
        self.assertFalse(ok)

    def test_it_reads_physical_footprint_not_rss(self) -> None:
        # The distinction is the whole point: at the moment of the 2026-08-10
        # jetsam the offenders held 20 GB resident with 186 GB in the
        # compressor, so an RSS-based limit would never have fired.
        source = GUARD.read_text()
        self.assertIn("footprint", source.lower())


if __name__ == "__main__":
    unittest.main()
