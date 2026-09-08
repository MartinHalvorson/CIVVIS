"""Regression coverage for the unattended GUI-lane stall guards."""

from __future__ import annotations

import importlib.util
import json
import select
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
OPS = TOOLS / "ops"
STATE_PATH = OPS / "civvis_watchdog_state.py"
SPEC = importlib.util.spec_from_file_location("civvis_watchdog_state", STATE_PATH)
assert SPEC is not None and SPEC.loader is not None
watchdog_state = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(watchdog_state)


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class KernelOwnershipTest(unittest.TestCase):
    SCRIPTS = ("civvis-interactive-host.sh", "civvis-game-supervisor.sh")

    def guard(self, name: str) -> str:
        source = (OPS / name).read_text()
        start = source.index("acquire_kernel_owner_lock() {")
        end = source.index("\n}\n", start) + 3
        call = source.index('acquire_kernel_owner_lock "', end)
        legacy = (source.index('if ! mkdir "$LOCK"', call)
                  if "interactive-host" in name else
                  source.index("acquire_supervisor_lock\n", call))
        self.assertLess(call, legacy)
        return "say() { :; }\n" + source[start:end]

    def test_unpublished_pid_cannot_admit_a_second_owner_and_crash_releases(self):
        for name in self.SCRIPTS:
            with self.subTest(script=name), tempfile.TemporaryDirectory() as tmp:
                # The first owner is deliberately paused before PID publication.
                legacy = Path(tmp) / "owner.lock"
                legacy.mkdir()
                lock = str(legacy) + ".owner"
                guard = self.guard(name)
                acquire = guard + '\nacquire_kernel_owner_lock "$1" || exit $?\n'
                holder = subprocess.Popen(
                    ["zsh", "-c", acquire + 'print ready; read -r release',
                     "test-owner", lock], stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    ready, _, _ = select.select([holder.stdout], [], [], 5)
                    self.assertTrue(ready, "owner never acquired the kernel lock")
                    self.assertEqual(holder.stdout.readline().strip(), "ready")
                    contender = subprocess.run(
                        ["zsh", "-c", acquire + 'print admitted',
                         "test-contender", lock], capture_output=True,
                        text=True, timeout=5)
                    self.assertEqual(contender.returncode, 2, contender.stderr)
                    self.assertNotIn("admitted", contender.stdout)
                    self.assertTrue(legacy.is_dir())
                    holder.kill()
                    holder.wait(timeout=5)
                    successor = subprocess.run(
                        ["zsh", "-c", acquire + 'print admitted',
                         "test-successor", lock], capture_output=True,
                        text=True, timeout=5)
                    self.assertEqual(successor.returncode, 0, successor.stderr)
                    self.assertEqual(successor.stdout.strip(), "admitted")
                finally:
                    if holder.poll() is None:
                        holder.kill()
                    holder.communicate(timeout=5)

    def test_unusable_lock_path_fails_closed(self):
        for name in self.SCRIPTS:
            with self.subTest(script=name), tempfile.TemporaryDirectory() as tmp:
                result = subprocess.run(
                    ["zsh", "-c", self.guard(name) +
                     '\nacquire_kernel_owner_lock "$1"', "test-owner",
                     str(Path(tmp) / "missing" / "owner")],
                    capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 70)


class RepeatingUnitBlockerTest(unittest.TestCase):
    def test_counts_only_the_latest_unit_blocker_on_its_turn(self) -> None:
        events = [
            {"kind": "turn", "turn": 152},
            {"kind": "blocked", "turn": 152,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "turn", "turn": 153},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "dismissed", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
        ]
        self.assertEqual(
            watchdog_state.repeating_unit_blocker(events),
            (153, "ENDTURN_BLOCKING_UNITS", 3),
        )

    def test_non_unit_blockers_do_not_trigger_the_unit_recovery(self) -> None:
        events = [
            {"kind": "turn", "turn": 153},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_PRODUCTION"},
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_PRODUCTION"},
        ]
        self.assertIsNone(watchdog_state.repeating_unit_blocker(events))

    def test_a_later_turn_or_outcome_clears_an_old_blocker_signal(self) -> None:
        self.assertIsNone(watchdog_state.repeating_unit_blocker([
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "turn", "turn": 154},
        ]))
        self.assertIsNone(watchdog_state.repeating_unit_blocker([
            {"kind": "blocked", "turn": 153,
             "blocker": "ENDTURN_BLOCKING_UNITS"},
            {"kind": "outcome", "turn": 153},
        ]))


class SynchronizedProgressTokenTest(unittest.TestCase):
    def test_arms_only_when_this_run_agrees_with_the_live_mirror(self) -> None:
        status = {
            "server_instance": 321,
            "turn": 184,
            "frame_sequence": 912,
        }
        events = [
            {"kind": "turn", "turn": 183},
            {"kind": "turn", "turn": 184},
        ]
        self.assertEqual(
            watchdog_state.synchronized_progress_token(status, events),
            (321, 184, 912, 184),
        )

    def test_does_not_arm_from_setup_or_a_stale_mirror(self) -> None:
        status = {
            "server_instance": 321,
            "turn": 184,
            "frame_sequence": 912,
        }
        self.assertIsNone(watchdog_state.synchronized_progress_token(status, []))
        self.assertIsNone(watchdog_state.synchronized_progress_token(
            status, [{"kind": "turn", "turn": 170}],
        ))

    def test_same_turn_blocked_events_do_not_look_like_progress(self) -> None:
        status = {
            "server_instance": 321,
            "turn": 184,
            "frame_sequence": 912,
        }
        first = [{"kind": "turn", "turn": 184}]
        blocked = [
            *first,
            {"kind": "blocked", "turn": 184,
             "blocker": "ENDTURN_BLOCKING_SPY_CHOOSE_ESCAPE_ROUTE"},
        ]
        self.assertEqual(
            watchdog_state.synchronized_progress_token(status, first),
            watchdog_state.synchronized_progress_token(status, blocked),
        )

    def test_terminal_or_malformed_status_disarms_progress_guard(self) -> None:
        status = {
            "server_instance": 321,
            "turn": 184,
            "frame_sequence": 912,
        }
        self.assertIsNone(watchdog_state.synchronized_progress_token(
            status, [{"kind": "turn", "turn": 184},
                     {"kind": "victory", "turn": 184}],
        ))
        self.assertEqual(
            watchdog_state.synchronized_progress_token(
                status, [{"kind": "turn", "turn": 184},
                         {"kind": "defeat", "turn": 184, "ours": False}],
            ),
            (321, 184, 912, 184),
        )
        self.assertIsNone(watchdog_state.synchronized_progress_token(
            {"server_instance": True, "turn": 184, "frame_sequence": 912},
            [{"kind": "turn", "turn": 184}],
        ))

    def test_progress_cli_uses_the_current_run_as_its_stale_mirror_fence(self) -> None:
        status = {
            "server_instance": 321,
            "turn": 184,
            "frame_sequence": 912,
        }
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(json.dumps({"kind": "turn", "turn": 184}) + "\n")
            command = [
                sys.executable, str(STATE_PATH), "--progress", str(events),
            ]
            live = subprocess.run(
                command, input=json.dumps(status), text=True, check=True,
                capture_output=True,
            )
            self.assertEqual(live.stdout, "321 184 912 184\n")
            events.write_text(json.dumps({"kind": "turn", "turn": 170}) + "\n")
            stale = subprocess.run(
                command, input=json.dumps(status), text=True, check=True,
                capture_output=True,
            )
            self.assertEqual(stale.stdout, "")


class WatchdogWiringTest(unittest.TestCase):
    def test_agent_watchdog_escalates_an_explicit_repeating_unit_blocker(self) -> None:
        source = (OPS / "civvis-agent-wedge-watchdog.sh").read_text()
        self.assertIn("CIVVIS_WEDGE_BLOCKER_STREAK", source)
        self.assertIn("civvis_watchdog_state.py", source)
        self.assertIn("repeating unit blocker", source)
        self.assertLess(
            source.index("repeating unit blocker"),
            source.index("mirror_status=$(curl"),
        )

    def test_agent_watchdog_requires_synchronized_no_progress_before_recovery(self) -> None:
        source = (OPS / "civvis-agent-wedge-watchdog.sh").read_text()
        self.assertIn("CIVVIS_WEDGE_PROGRESS_CONFIRM", source)
        self.assertIn("--progress", source)
        self.assertIn("NO GAME PROGRESS confirmed", source)
        self.assertIn("synchronized progress", source)
        self.assertLess(
            source.index("NO GAME PROGRESS confirmed"),
            source.index("gap=$(( mirror_turn - agent_turn ))"),
        )

    def test_agent_watchdog_never_signals_a_direct_unowned_player(self) -> None:
        source = (OPS / "civvis-agent-wedge-watchdog.sh").read_text()
        self.assertIn("owned_climb_and_player()", source)
        self.assertIn("descends_from", source)
        self.assertIn("unowned direct civ6_play", source)
        self.assertIn('restart_attempt "$tag', source)
        self.assertLess(
            source.index("ownership=$(owned_climb_and_player || true)"),
            source.index("blocker_signal=\"\""),
        )
        self.assertNotIn(
            'local play=$(pgrep -f "[c]iv6_play\\.py"',
            source,
        )

    def test_interactive_host_keeps_the_agent_watchdog_alive(self) -> None:
        source = (OPS / "civvis-interactive-host.sh").read_text()
        self.assertIn("WEDGE_WATCHDOG", source)
        self.assertIn("start_wedge_watchdog", source)
        self.assertIn("wedge_watchdog_owned", source)


if __name__ == "__main__":
    unittest.main()
