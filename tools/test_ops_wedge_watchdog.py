#!/usr/bin/env python3
"""The wedge restart must leave behind the state that explains the wedge."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
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

    def test_the_handoff_is_written_down_before_the_player_is_signalled(self):
        """⚠⚠ THE CLIMB READS THE MARKER, NOT THE CLOCK.

        The climb recognised a handoff only by the player exiting with its turn
        stale past 240 s — true of the five no-progress samples, false of the
        unit-blocker streak, which the mod feeds every few seconds. Run
        `civvis-20260831T085324Z-cont1` was handed over two minutes after its
        first turn, read as an ordinary exit, and filed as killed at t120 with
        two resumes unspent. The marker is written BEFORE the INT so it is on
        disk however fast the player goes.
        """
        source = self._source()
        handoff = source.index('t${turn} is worth reloading')
        block = source[handoff:source.index('kill -TERM "$climb"')]
        self.assertIn('"$RUNS/$tag/$HANDOFF_MARKER"', block)
        self.assertLess(block.index("$HANDOFF_MARKER"),
                        block.index('kill -INT "$play"'),
                        "the marker must be on disk before the player is signalled")

    def test_the_marker_name_is_the_climbs(self):
        """One name on both sides, or the handoff is written and never read."""
        self.assertIn("HANDOFF_MARKER=${CIVVIS_WEDGE_HANDOFF_MARKER:-wedge-handoff.json}",
                      self._source())
        climb = (WATCHDOG.parent.parent / "civ6_civvis_climb.py").read_text(
            encoding="utf-8")
        self.assertIn('WEDGE_HANDOFF_MARKER = "wedge-handoff.json"', climb)

    def test_the_handoff_branch_writes_a_marker_the_climb_can_read(self):
        """The branch itself, under the zsh the watchdog runs: the file lands
        in the run directory, parses as JSON, and names the turn and reason."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed here")
        source = self._source()
        start = source.index('    if [[ -d "$RUNS/$tag" ]]; then')
        end = source.index('    player_uses_tag "$play" "$tag" \\\n      && kill -INT')
        branch = source[start:end].replace("say ", "print -r -- ")
        with tempfile.TemporaryDirectory() as tmp:
            runs = Path(tmp)
            (runs / "civvis-test").mkdir()
            script = (f'RUNS={runs}\ntag=civvis-test\nturn=120\n'
                      f'HANDOFF_MARKER=wedge-handoff.json\n'
                      f'reason=\'civvis-test repeating unit blocker '
                      f'ENDTURN_BLOCKING_UNITS at t120 (19 sightings)\'\n'
                      + branch)
            done = subprocess.run(["zsh", "-c", script], capture_output=True,
                                  text=True, timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("wrote civvis-test/wedge-handoff.json", done.stdout)
            written = json.loads(
                (runs / "civvis-test" / "wedge-handoff.json").read_text())
        self.assertEqual(written["tag"], "civvis-test")
        self.assertEqual(written["turn"], 120)
        self.assertIn("ENDTURN_BLOCKING_UNITS", written["reason"])
        self.assertRegex(written["utc"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")


if __name__ == "__main__":
    unittest.main()


class TheLiveRunIsPickedByItsEvents(unittest.TestCase):
    """⚠⚠ A READ-ONLY QUERY MUST NOT REDIRECT THE WATCHDOG.

    The run was chosen by directory mtime. A directory's mtime changes whenever
    an entry is created in it, and opening a run's `orders.sqlite` — even
    read-only — creates `-shm`/`-wal` beside it. So any analysis tool run
    against a FINISHED game promoted that game to "newest", and the watchdog
    spent every poll saying "does not match the proven climb-owned player;
    leaving it alone" while the live game went unguarded.

    Observed 2026-08-30: two runs from hours earlier were touched by a
    read-only query and the watchdog followed them instead of the live t57
    game. `events.jsonl` is appended by the mod itself, so its mtime is the
    game actually being played.
    """

    def _source(self) -> str:
        return WATCHDOG.read_text(encoding="utf-8")

    def test_the_tag_comes_from_the_newest_events_file(self):
        source = self._source()
        self.assertIn('tag=$(ls -t "$RUNS"/civvis-*/events.jsonl', source)
        self.assertIn("tag=${tag:h:t}", source)

    def test_the_directory_listing_is_gone(self):
        """The old form promoted whichever run directory was touched last."""
        self.assertNotIn("""ls -t "$RUNS" 2>/dev/null | grep '^civvis-'""",
                         self._source())

    def test_it_picks_the_appended_run_over_a_touched_one(self):
        """The behaviour itself, through the same zsh the watchdog runs."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            live, stale = root / "civvis-live", root / "civvis-stale"
            for d in (live, stale):
                d.mkdir()
                (d / "events.jsonl").write_text("{}\n")
            # The stale run's DIRECTORY is touched after the live run's events
            # — exactly what a read-only sqlite open does.
            os.utime(live / "events.jsonl", (2_000_000_000, 2_000_000_000))
            (stale / "orders.sqlite-wal").write_text("")
            os.utime(stale / "events.jsonl", (1_000_000_000, 1_000_000_000))
            picked = subprocess.run(
                ["zsh", "-c",
                 f'tag=$(ls -t {root}/civvis-*/events.jsonl 2>/dev/null | head -1); print "${{tag:h:t}}"'],
                capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(picked, "civvis-live")


class SilenceIsAFasterWedgeSignal(unittest.TestCase):
    """★★★★★ A PARKED GAME IS SILENT; A SLOW ONE IS NOT.

    The turn-progress rule waits five one-minute samples because a late-game
    turn can legitimately take minutes. But a run that is only slow keeps
    writing events the whole time, and one whose Game Core has parked writes
    nothing.

    Measured over 50,405 consecutive event gaps in seven live runs on
    2026-09-02: 35 gaps reached 60 s, the longest was 81.2 s, and NOT ONE
    reached 90 s. Every gap over a minute was the desktop-rescue capture stall
    that #3089 removes; without it the longest silence in those runs is 25 s.

    Run civvis-20260902T162829Z-cont3 wrote its last event at 17:35:51Z and did
    not reach its fifth strike until 17:41:23Z -- 332 s of a roughly seven
    minute handoff spent waiting to be sure.
    """

    def _source(self) -> str:
        return WATCHDOG.read_text(encoding="utf-8")

    def test_the_silence_limit_and_its_confirmations_are_named_knobs(self):
        source = self._source()
        self.assertIn("SILENCE_S=${CIVVIS_WEDGE_SILENCE_S:-120}", source)
        self.assertIn("SILENCE_CONFIRM=${CIVVIS_WEDGE_SILENCE_CONFIRM:-2}", source)

    def test_the_limit_clears_the_longest_healthy_silence_ever_measured(self):
        """81.2 s was the worst, and that cause is being removed. 120 s keeps a
        margin without spending five minutes on a dead game."""
        source = self._source()
        limit = int(re.search(r"SILENCE_S=\$\{CIVVIS_WEDGE_SILENCE_S:-(\d+)\}",
                              source).group(1))
        self.assertGreaterEqual(limit, 100, "must clear the 81.2 s worst case")
        self.assertLessEqual(limit, 240, "or it saves nothing over the turn rule")

    def test_silence_only_ever_lowers_the_bar(self):
        """It must not make a talkative but stalled game harder to catch."""
        self.assertIn("(( SILENCE_CONFIRM < PROGRESS_CONFIRM ))", self._source())

    def test_a_silent_run_still_has_to_fail_the_forced_end_turn(self):
        """The nudge is the proof; silence only decides when to ask for it."""
        source = self._source()
        threshold = source.index("if (( progress_strikes >= progress_needed ))")
        nudge = source.index("if nudge_end_turn; then")
        restart = source.index("restart_attempt \"$tag NO GAME PROGRESS")
        self.assertLess(threshold, nudge, "the nudge must follow the threshold")
        self.assertLess(nudge, restart, "nothing may restart before the nudge")

    def test_the_rule_that_fired_is_written_down(self):
        """Two rules reach one recovery; the log has to say which one did."""
        source = self._source()
        self.assertIn('progress_rule="silent for ${silence}s"', source)
        self.assertIn('progress_rule="no synchronized progress"', source)
        self.assertIn('${progress_rule} (${progress_signal}) strike', source)
        self.assertIn('at t${mirror_turn} (${progress_rule})', source)

    def test_silence_is_measured_from_the_events_file_this_run_flushes(self):
        source = self._source()
        self.assertIn('events_path="$RUNS/$tag/events.jsonl"', source)
        self.assertIn("/usr/bin/stat -f '%m' \"$events_path\"", source)

    def test_an_unreadable_events_file_never_accelerates_a_restart(self):
        """A missing or unreadable file must read as 'no evidence', not
        'silent forever' -- the watchdog's own bug class."""
        source = self._source()
        block = source[source.index("silence=-1"):
                       source.index("progress_needed=$PROGRESS_CONFIRM")]
        self.assertIn("silence=-1", block)
        self.assertIn('if [[ -r "$events_path" ]]', block)

    def test_the_watchdogs_own_branch_lowers_the_bar_only_when_silent(self):
        """The real block, sliced out of the script and run under zsh, against
        a fresh events file and a stale one."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        source = self._source()
        branch = source[source.index("  silence=-1"):
                        source.index('\n  if [[ "$progress_signal" =~')]
        with tempfile.TemporaryDirectory() as tmp:
            runs = Path(tmp)
            (runs / "civvis-test").mkdir()
            events = runs / "civvis-test" / "events.jsonl"
            events.write_text("{}\n")

            def decide() -> tuple[str, str]:
                script = (f'RUNS={runs}\ntag=civvis-test\n'
                          'PROGRESS_CONFIRM=5\nSILENCE_S=120\nSILENCE_CONFIRM=2\n'
                          + branch +
                          '\nprint -r -- "$progress_needed|$progress_rule"\n')
                out = subprocess.run(["zsh", "-c", script], capture_output=True,
                                     text=True, timeout=60)
                self.assertEqual(out.returncode, 0, out.stderr)
                needed, rule = out.stdout.strip().split("|", 1)
                return needed, rule

            needed, rule = decide()
            self.assertEqual(needed, "5", "a live run keeps the patient rule")
            self.assertEqual(rule, "no synchronized progress")

            old_time = time.time() - 600
            os.utime(events, (old_time, old_time))
            needed, rule = decide()
            self.assertEqual(needed, "2", "ten minutes of silence lowers the bar")
            self.assertTrue(rule.startswith("silent for "), rule)

            events.unlink()
            needed, rule = decide()
            self.assertEqual(needed, "5", "no events file is no evidence")
            self.assertEqual(rule, "no synchronized progress")

    def test_the_silence_arithmetic_runs_in_the_watchdogs_own_zsh(self):
        """The behaviour, not the text: a fresh file is not silent and an old
        one is, through the same expression the script uses."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        with tempfile.TemporaryDirectory() as tmp:
            events = Path(tmp) / "events.jsonl"
            events.write_text("{}\n")
            script = (
                'events_path=$1; silence=-1\n'
                'if [[ -r "$events_path" ]]; then\n'
                "  events_mtime=$(/usr/bin/stat -f '%m' \"$events_path\" 2>/dev/null || print -r -- \"\")\n"
                '  if [[ "$events_mtime" =~ \'^[0-9]+$\' ]]; then\n'
                '    silence=$(( $(date -u +%s) - events_mtime ))\n'
                '    (( silence < 0 )) && silence=0\n'
                '  fi\n'
                'fi\n'
                'print -r -- "$silence"\n'
            )

            def silence_of(path: Path) -> int:
                out = subprocess.run(["zsh", "-c", script, "zsh", str(path)],
                                     capture_output=True, text=True, check=True)
                return int(out.stdout.strip())

            self.assertLess(silence_of(events), 5, "a just-written file is live")
            old = time.time() - 600
            os.utime(events, (old, old))
            self.assertGreaterEqual(silence_of(events), 590)
            self.assertEqual(silence_of(Path(tmp) / "absent.jsonl"), -1,
                             "no file means no evidence, not a wedge")


class AnUnownedRunIsStillReported(unittest.TestCase):
    """★★★★★ STANDING DOWN IS NOT THE SAME AS LOOKING AWAY.

    Refusing to SIGNAL an unowned `civ6_play` is right -- a protected autosave
    continuation and an operator's own session both look like one. But the
    branch also reset its strikes and continued, so an unowned run got no
    DETECTION either.

    Run `civvis-20260903T135954Z-cont2`, 2026-09-03: logged once as unowned,
    then silent for hours while it turned at a 485.7 s median against 3.8-6.5 s
    in healthy runs, and finally parked at t72 on `ENDTURN_BLOCKING_UNITS` --
    brain answering in 0.11 s, Civ VI at 98 % CPU, sixteen minutes with no
    event. Nothing said so.
    """

    def _source(self) -> str:
        return WATCHDOG.read_text(encoding="utf-8")

    def test_the_unowned_report_knobs_are_named(self):
        source = self._source()
        self.assertIn("UNOWNED_SILENCE_S=${CIVVIS_WEDGE_UNOWNED_SILENCE_S:-$SILENCE_S}",
                      source)
        self.assertIn(
            "UNOWNED_REPORT_EVERY_S=${CIVVIS_WEDGE_UNOWNED_REPORT_EVERY_S:-300}",
            source)

    def test_the_signal_is_still_withheld(self):
        """The diagnosis is added; the refusal to signal must be untouched."""
        source = self._source()
        self.assertIn("watchdog will not signal it", source)
        # ⚠ Anchor on the executable line, not the phrase: the rationale
        # comment above quotes it, and `index` would slice from there --
        # swallowing `restart_attempt` and its kills into "the branch".
        start = source.index('      say "$tag has an unowned direct civ6_play')
        branch = source[start:source.index('    if [[ -n "$handoff_tag" ]]; then',
                                           start)]
        for verb in ("kill ", "kill -", "nudge_end_turn", "osascript"):
            self.assertNotIn(verb, branch,
                             "the unowned branch must never signal the game")

    def _run_branch(self, *, age_s: int, reported_at: int = 0) -> str:
        if shutil.which("zsh") is None:
            self.skipTest("zsh is needed here")
        source = self._source()
        start = source.index("    unowned_silence=-1")
        end = source.index("    if [[ -n \"$handoff_tag\" ]]; then")
        branch = source[start:end].replace("say ", "print -r -- ")
        with tempfile.TemporaryDirectory() as tmp:
            runs = Path(tmp)
            (runs / "civvis-test").mkdir()
            events = runs / "civvis-test" / "events.jsonl"
            events.write_text('{"kind":"state"}\n')
            stamp = time.time() - age_s
            os.utime(events, (stamp, stamp))
            script = (f'RUNS={runs}\ntag=civvis-test\n'
                      f'UNOWNED_SILENCE_S=120\n'
                      f'UNOWNED_REPORT_EVERY_S=300\n'
                      f'unowned_reported_at={reported_at}\n' + branch)
            done = subprocess.run(["zsh", "-c", script], capture_output=True,
                                  text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def test_a_silent_unowned_run_is_named_in_the_log(self):
        out = self._run_branch(age_s=900)
        self.assertIn("WEDGE UNATTENDED", out)
        self.assertIn("civvis-test", out)

    def test_a_live_unowned_run_says_nothing(self):
        """A run that is merely unowned is not news; only a silent one is."""
        self.assertEqual(self._run_branch(age_s=5).strip(), "")

    def test_the_report_is_rate_limited(self):
        """It must reach a human once, not fill the log every poll."""
        recent = int(time.time())
        self.assertEqual(self._run_branch(age_s=900, reported_at=recent).strip(),
                         "")


if __name__ == "__main__":
    unittest.main()
