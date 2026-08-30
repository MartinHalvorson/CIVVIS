#!/usr/bin/env python3
"""The wedge restart must leave behind the state that explains the wedge."""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

OPS = Path(__file__).resolve().parent / "ops"
WATCHDOG = OPS / "civvis-agent-wedge-watchdog.sh"


class AWedgeRestartLeavesEvidence(unittest.TestCase):
    """⭐⭐ Two games wedged on 2026-08-28 and left nothing to compare.

    A Prince run at t34 and a King run at t44 both stopped writing events
    mid-turn. Afterwards all that survived was "no synchronized progress" and a
    dead process: the last events differed between the two, `stdout.log` ended
    on an unremarkable line both times (its no-path sentinels are ordinary — a
    run that never wedged carried 31 of them), and every theory about the cause
    was therefore unfalsifiable.

    `sample` answers the one question that matters: where is the game stuck.
    """

    def _source(self) -> str:
        return WATCHDOG.read_text(encoding="utf-8")

    def test_the_sample_is_taken_before_anything_is_killed(self):
        source = self._source()
        sampled = source.index("/usr/bin/sample")
        killed = source.index('kill -TERM "$climb"')
        self.assertLess(sampled, killed,
                        "after the kill there is nothing left to sample")

    def test_the_sample_lands_in_the_run_directory(self):
        self.assertIn('sample_file="$RUNS/$tag/wedge-sample.txt"',
                      self._source())

    def test_a_failed_sample_never_blocks_the_restart(self):
        """Evidence is worth having, never worth an outage."""
        source = self._source()
        # ⚠ The boundary is the first `say "$reason` — the end of the SAMPLE
        # handling — not the `kill`. A deliberate recovery that returns instead
        # of killing now sits between the two (the deep-game handoff below), and
        # slicing to the kill made this read as the very defect it guards.
        block = source[source.index("game_pid=$(pgrep -x Civ6_Exe_Child"):
                       source.index('say "$reason;')]
        self.assertIn("restarting without it", block)
        # No `return` or `exit` may sit inside the sample handling itself.
        self.assertNotIn("return", block)
        self.assertNotIn("exit", block)

    def test_the_sample_length_is_a_named_knob(self):
        self.assertIn(
            "WEDGE_SAMPLE_SECONDS=${CIVVIS_WEDGE_SAMPLE_SECONDS:-2}",
            self._source())

    def test_the_script_is_valid_zsh(self):
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed here")
        done = subprocess.run(["zsh", "-n", str(WATCHDOG)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_the_sample_branch_runs_and_writes_a_file(self):
        """Run the watchdog's OWN branch under zsh against a live process."""
        if shutil.which("zsh") is None or not Path("/usr/bin/sample").exists():
            self.skipTest("zsh and /usr/bin/sample are needed here")
        source = self._source()
        start = source.index("  local game_pid sample_file")
        end = source.index('  say "$reason; restarting')
        branch = (source[start:end]
                  .replace("local ", "")
                  .replace("say ", "print -r -- "))
        with tempfile.TemporaryDirectory() as tmp:
            runs = Path(tmp)
            (runs / "civvis-test").mkdir()
            # `sleep` stands in for the game: a real process this user owns.
            sleeper = subprocess.Popen(["sleep", "30"])
            try:
                script = (
                    f'RUNS={runs}\ntag=civvis-test\n'
                    f'WEDGE_SAMPLE_SECONDS=1\n'
                    f'pgrep() {{ print -r -- {sleeper.pid}; }}\n' + branch)
                done = subprocess.run(["zsh", "-c", script],
                                      capture_output=True, text=True,
                                      timeout=120)
            finally:
                sleeper.terminate()
                sleeper.wait(timeout=10)
            # ⚠ Inside the `with`: the directory these assertions read is the
            # temporary one, and it is gone the moment the block exits.
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("sampled wedged Civ 6", done.stdout)
            written = runs / "civvis-test" / "wedge-sample.txt"
            self.assertTrue(written.is_file(),
                            "the sample must land where the run's evidence lives")
            self.assertIn("Call graph", written.read_text(errors="replace"))




class DeepWedgeIsHandedToTheClimb(unittest.TestCase):
    """⭐⭐⭐ A DEEP WEDGED GAME IS RELOADED, NOT THROWN AWAY.

    Civ 6 autosaves every turn and `civ6_civvis_climb.py` can reload one into a
    FRESH Civ 6 — the only thing that recovers a parked core, whose own process
    answers no input at all (an external SHIFT+RETURN was measured and ignored
    in two clean trials). That path runs only while the CLIMB is alive, and this
    watchdog killed the climb first: seven `<tag>-contN` runs exist, all from
    08-17..19, none since it began signalling. Games as deep as t179 with 15
    cities at 0.763 of the leader were discarded with their save on disk.
    """

    def _source(self) -> str:
        return WATCHDOG.read_text(encoding="utf-8")

    def test_a_deep_game_signals_the_player_and_leaves_the_climb(self):
        source = self._source()
        handoff = source.index('t${turn} is worth reloading')
        killed = source.index('kill -TERM "$climb"')
        self.assertLess(handoff, killed,
                        "the handoff must be considered before the kill")
        block = source[handoff:killed]
        self.assertIn('kill -INT "$play"', block)
        self.assertNotIn('kill -TERM', block)
        self.assertIn("return 0", block)

    def test_the_floor_matches_the_climbs_own_resume_floor(self):
        """Below it the climb redoes the game from scratch anyway, so handing
        one over would only cost the reload."""
        self.assertIn("RESUME_FLOOR=${CIVVIS_WEDGE_RESUME_MIN_TURN:-20}",
                      self._source())
        climb = (WATCHDOG.parent.parent / "civ6_civvis_climb.py").read_text(
            encoding="utf-8")
        self.assertIn("RESUME_MIN_TURN = 20", climb)

    def test_a_second_wedge_on_the_same_tag_is_not_handed_over_again(self):
        """One reload attempt per run. If the game wedges again under the same
        tag the handoff did not take, and the climb is killed as before."""
        self.assertIn('[[ "$tag" != "$handoff_tag" ]]', self._source())

    def test_a_climb_that_never_reloads_is_terminated_after_all(self):
        """⚠ This watchdog exists for a climb that is ITSELF blocked, so the
        handoff must never become a way for one to sit forever."""
        source = self._source()
        self.assertIn("HANDOFF_GRACE=${CIVVIS_WEDGE_HANDOFF_GRACE:-12}", source)
        self.assertIn('kill -TERM "$handoff_climb"', source)


if __name__ == "__main__":
    unittest.main()
