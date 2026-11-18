#!/usr/bin/env python3
"""The wedge restart must leave behind the state that explains the wedge."""

from __future__ import annotations

import json
import os
import re
import shutil
import signal
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

    def _pick(self, runs: Path) -> subprocess.CompletedProcess:
        """Run the watchdog's OWN three lines, as one pass of its poll loop.

        They end in `continue` when there is no run, so they sit in a loop of
        one pass here and `picked=` is printed only when a run was chosen.
        """
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        source = self._source()
        start = source.index("  newest_events=(")
        end = source.index("\n", source.index("  tag=${newest_events[1]:h:t}")) + 1
        script = ("set -u\nRUNS=$1\nreset_progress() { : }\n"
                  "for _ in 1; do\n" + source[start:end]
                  + '  print -r -- "picked=$tag"\ndone\n')
        return subprocess.run(["zsh", "-c", script, "zsh", str(runs)],
                              capture_output=True, text=True, timeout=30)

    def test_the_tag_comes_from_the_newest_events_file(self):
        source = self._source()
        self.assertIn('newest_events=("$RUNS"/civvis-*/events.jsonl(Nom[1]))',
                      source)
        self.assertIn("tag=${newest_events[1]:h:t}", source)

    def test_the_directory_listing_is_gone(self):
        """The old form promoted whichever run directory was touched last."""
        self.assertNotIn("""ls -t "$RUNS" 2>/dev/null | grep '^civvis-'""",
                         self._source())

    def test_it_picks_the_appended_run_over_a_touched_one(self):
        """The behaviour itself, through the same zsh the watchdog runs."""
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
            done = self._pick(root)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.strip(), "picked=civvis-live")

    def test_no_run_directory_is_no_run_and_no_complaint(self):
        """⚠ The `ls -t "$RUNS"/civvis-*/events.jsonl` form failed twice here.

        zsh refuses an unmatched pattern before `ls` ever runs, so the
        `2>/dev/null` on `ls` never saw the message: 784 lines of
        `civvis-agent-wedge-watchdog.sh:401: no matches found` reached the
        launch log, one per poll. And `${tag:h:t}` of the empty result is `.`,
        not empty, so the `-z` guard never fired and the loop went on to judge
        a run called `.`.
        """
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "empty").mkdir()
            # A run directory whose game has not written an event yet.
            (root / "unwritten" / "civvis-starting").mkdir(parents=True)
            for runs in (root / "empty", root / "unwritten", root / "absent"):
                with self.subTest(runs=runs.name):
                    done = self._pick(runs)
                    self.assertEqual(done.returncode, 0, done.stderr)
                    self.assertEqual(done.stderr, "",
                                     "an unmatched glob must not reach the launch log")
                    self.assertEqual(done.stdout, "",
                                     "with no events file there is no run to judge")


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
        self.assertIn("SILENCE_S=${CIVVIS_WEDGE_SILENCE_S:-60}", source)
        self.assertIn("SILENCE_CONFIRM=${CIVVIS_WEDGE_SILENCE_CONFIRM:-2}", source)

    def test_the_limit_clears_the_longest_healthy_silence_ever_measured(self):
        """81.2 s was the 2026-09-02 worst (capture stalls, removed by #3089).
        After the headroom governor and the order-queue stall fix (#3893), 66k
        gaps in civvis-20261004T094143Z / 100903Z / 100903Z-cont1 peaked at
        14.8 s with none over 30 s; 60 s keeps a 4x margin over that, and the
        forced-end-turn proof still has to fail before a restart."""
        source = self._source()
        limit = int(re.search(r"SILENCE_S=\$\{CIVVIS_WEDGE_SILENCE_S:-(\d+)\}",
                              source).group(1))
        self.assertGreaterEqual(limit, 45, "must clear the 14.8 s healthy worst case 3x over")
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

    def _run_branch(self, *, age_s: int, reported_at: int = 0,
                    summary: str | None = None, summary_age_s: int = 0) -> str:
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
            if summary is not None:
                receipt = events.with_name("summary.json")
                receipt.write_text(summary)
                summary_stamp = time.time() - summary_age_s
                os.utime(receipt, (summary_stamp, summary_stamp))
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

    def test_a_confirmed_stopped_run_is_not_an_unattended_stall(self):
        receipt = '{"tag":"civvis-test","game_stopped":true,"reason":"stopped"}'
        self.assertEqual(self._run_branch(age_s=900, summary=receipt).strip(), "")

    def test_uncertain_stop_receipts_keep_the_stall_diagnosis(self):
        for receipt in ('{', 'null', '[]', '{}',
                        '{"tag":"other-run","game_stopped":true}',
                        '{"tag":"civvis-test","game_stopped":false}',
                        '{"tag":"civvis-test","game_stopped":"true"}',
                        '{"tag":"civvis-test","reason":"stopped"}'):
            with self.subTest(receipt=receipt):
                self.assertIn("WEDGE UNATTENDED",
                              self._run_branch(age_s=900, summary=receipt))

    def test_events_after_a_stop_receipt_keep_the_stall_diagnosis(self):
        receipt = '{"tag":"civvis-test","game_stopped":true}'
        self.assertIn("WEDGE UNATTENDED", self._run_branch(
            age_s=900, summary=receipt, summary_age_s=1800))

    def test_a_live_unowned_run_says_nothing(self):
        """A run that is merely unowned is not news; only a silent one is."""
        self.assertEqual(self._run_branch(age_s=5).strip(), "")

    def test_the_report_is_rate_limited(self):
        """It must reach a human once, not fill the log every poll."""
        recent = int(time.time())
        self.assertEqual(self._run_branch(age_s=900, reported_at=recent).strip(),
                         "")


HOST = OPS / "civvis-interactive-host.sh"


def _sandbox(tmp: str, *, blind: bool = False) -> tuple[Path, dict]:
    """A home of its own and an environment that names nothing on this machine.

    The watchdog and the host find their lock, log, run directory, pause file
    and intent file under `$HOME` unless a `CIVVIS_*` knob says otherwise. So
    the knobs this process inherited are dropped and `$HOME` is a temporary
    directory: a test here cannot read the live lane's intent or take its lock.

    `blind` also replaces `pgrep` with one that finds nothing, for a test that
    lets the watchdog's own loop run: no pass of it can then discover a real
    climb or player. The host must keep the real `pgrep`, which is how it finds
    the descendants it stops on exit.
    """
    home = Path(tmp) / "home"
    (home / "civvis-civ6-runs").mkdir(parents=True)
    (home / "civvis-civ6-mirror").mkdir()
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("CIVVIS_")}
    env["HOME"] = str(home)
    if blind:
        fake_bin = Path(tmp) / "fake-bin"
        fake_bin.mkdir()
        pgrep = fake_bin / "pgrep"
        pgrep.write_text("#!/bin/sh\nexit 1\n")
        pgrep.chmod(0o755)
        env["PATH"] = str(fake_bin) + os.pathsep + env.get("PATH", "")
    return home, env


def _wait_for(condition, timeout_s: float = 10.0) -> bool:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if condition():
            return True
        time.sleep(0.02)
    return bool(condition())


def _text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.exists() else ""


def _stop(process: subprocess.Popen) -> None:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)


def _accepted_intents(script: Path) -> list[str]:
    """Every pattern a script's `verification_intent_running` compares against."""
    source = script.read_text(encoding="utf-8")
    start = source.index("verification_intent_running() {")
    body = source[start:source.index("\n}\n", start)]
    return re.findall(r'"\$\(<"\$INTENTFILE"\)" == (\S+) \]\]', body)


#: How the interactive host tells the watchdog it starts that it is the
#: Terminal window's own child. See `verification_intent_running` in the script.
TERMINAL_WINDOW_FLAG = "CIVVIS_WEDGE_TERMINAL_WINDOW"


class TerminalControlKeepsItsWedgeWatchdog(unittest.TestCase):
    """⚠⚠ UNDER TERMINAL CONTROL THE LANE HAD NO WEDGE WATCHDOG FOR SIX DAYS.

    #5167 (2026-11-12) added the intent `terminal`: the loop a Terminal window
    hosts may play, and no background keeper may start or recover it. The
    interactive host accepted it. The wedge watchdog the host starts as its own
    child still asked for the exact value `running`, so on a
    `CIVVIS_CONTROL=terminal` host it logged "verification intent is not
    running; exiting without touching a game" and exited at once, and the host
    restarted it on every five-second poll: about 52,000 times by 2026-11-18.
    No wedged game was rescued on that host in that time.

    The host's watchdog belongs to the window's own loop and starts no game: it
    signals the one climb-owned pair, and what follows comes from the climb and
    the supervisor in the same window. So the host marks the watchdog it starts
    (`CIVVIS_WEDGE_TERMINAL_WINDOW=1`) and that copy accepts `terminal`. A copy
    started by anything else still needs the exact value `running`, which is
    what #5167 promised of everything outside the window.
    """

    def test_the_hosts_own_watchdog_accepts_exactly_what_the_host_accepts(self):
        """One definition on both sides, or the host starts what then refuses."""
        self.assertEqual(_accepted_intents(HOST), ["(running|terminal)"])
        self.assertEqual(_accepted_intents(WATCHDOG), ["(running|terminal)", "running"],
                         "the host's pattern under the flag, exact `running` without it")
        source = WATCHDOG.read_text(encoding="utf-8")
        start = source.index("verification_intent_running() {")
        body = source[start:source.index("\n}\n", start)]
        self.assertLess(body.index('if [[ "$TERMINAL_WINDOW" == 1 ]]; then'),
                        body.index("(running|terminal)"))
        self.assertLess(body.index("  else\n"), body.index("== running ]]"))
        self.assertIn("TERMINAL_WINDOW=${" + TERMINAL_WINDOW_FLAG + ":-0}", source)

    def test_every_intent_check_goes_through_that_one_function(self):
        source = WATCHDOG.read_text(encoding="utf-8")
        self.assertEqual(source.count('"$(<"$INTENTFILE")" =='),
                         len(_accepted_intents(WATCHDOG)),
                         "a comparison outside the function can disagree with it")
        self.assertGreaterEqual(source.count("if ! verification_intent_running; then"), 3,
                                "startup, every poll and every recovery ask")

    def test_only_the_interactive_host_starts_it(self):
        """Discovered, not listed. No keeper, installer or launchd job in
        `tools/ops` names the watchdog; one that starts to has to answer for a
        background watchdog on a host under terminal control."""
        naming = sorted(
            path.name for path in OPS.iterdir()
            if path.is_file() and path != WATCHDOG
            and WATCHDOG.name in path.read_text(encoding="utf-8", errors="replace"))
        self.assertEqual(naming, ["civvis-interactive-host.sh"])

    def test_only_the_interactive_host_claims_the_terminal_window(self):
        """Discovered, not listed. The flag is a claim to be the window's own
        child, so a second script that sets it is a background process letting
        itself in under `terminal`."""
        repo = OPS.parent.parent
        claiming = sorted(
            str(path.relative_to(repo))
            for root in (repo / "tools", repo / "deploy")
            for path in root.rglob("*")
            if path.is_file() and "__pycache__" not in path.parts
            and not path.name.startswith("test_")
            and TERMINAL_WINDOW_FLAG in path.read_text(encoding="utf-8",
                                                       errors="replace"))
        self.assertEqual(claiming, ["tools/ops/civvis-agent-wedge-watchdog.sh",
                                    "tools/ops/civvis-interactive-host.sh"])
        host = HOST.read_text(encoding="utf-8")
        self.assertEqual(host.count(TERMINAL_WINDOW_FLAG), 1)
        # On the watchdog's own command, never exported to the host's other
        # children: the supervisor and everything under it would inherit it.
        self.assertIn(TERMINAL_WINDOW_FLAG + '=1 /bin/zsh "$WEDGE_WATCHDOG"', host)

    def test_it_names_no_program_that_starts_a_game(self):
        """Discovered, not listed. What makes `terminal` safe to accept is that
        the watchdog launches nothing: it runs its own state reader and the
        end-turn keystroke, and names the two harnesses only to find and prove
        the processes it may signal. A program added to it has to be added
        here, with the answer to "does this start a game outside the window?"."""
        code = [line.replace("[c]iv6", "civ6").replace("\\.", ".")
                for line in WATCHDOG.read_text(encoding="utf-8").splitlines()
                if not line.lstrip().startswith("#")]
        named: dict[str, list[str]] = {}
        for line in code:
            for name in re.findall(r"[\w-]+\.(?:py|sh)\b", line):
                named.setdefault(name, []).append(line)
        self.assertEqual(sorted(named), [
            "civ6_civvis_climb.py",     # recognised, never run
            "civ6_nudge_end_turn.py",   # SHIFT+RETURN to the running game
            "civ6_play.py",             # recognised, never run
            "civvis_watchdog_state.py",  # reads events.jsonl
        ])
        for harness in ("civ6_civvis_climb.py", "civ6_play.py"):
            for line in named[harness]:
                self.assertTrue("pgrep -f" in line or "is_python_harness" in line,
                                f"{harness} may only be looked for: {line.strip()}")
        text = "\n".join(code)
        for launcher in ("open -", "osascript", "launchctl", "nohup", "caffeinate"):
            self.assertNotIn(launcher, text)
        self.assertEqual([line.strip() for line in code if re.search(r"\bexec\b", line)],
                         ['exec /bin/zsh "$SELF_PATH"'],
                         "the only thing it may replace itself with is itself")

    def test_running_and_terminal_keep_the_hosts_watchdog_up_and_withdrawing_either_ends_it(self):
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        # (the intent, whether the host started it). `running` needs no host.
        for value, hosted in (("running", False), ("running", True), ("terminal", True)):
            with self.subTest(intent=value, hosted=hosted), \
                    tempfile.TemporaryDirectory() as tmp:
                home, env = _sandbox(tmp, blind=True)
                if hosted:
                    env[TERMINAL_WINDOW_FLAG] = "1"
                intent = home / ".civvis-operator-intent"
                # Exactly what `civvis-games on` writes, newline included.
                intent.write_text(value + "\n")
                log = home / "civvis-civ6-runs" / "agent_wedge_watchdog.log"
                lock = home / ".civvis-agent-wedge-watchdog.lock"
                complaints = Path(tmp) / "watchdog.stderr"
                with complaints.open("w") as sink:
                    watchdog = subprocess.Popen(
                        ["zsh", str(WATCHDOG)],
                        env={**env, "CIVVIS_WEDGE_POLL_S": "0.1"},
                        stdout=subprocess.DEVNULL, stderr=sink)
                    try:
                        self.assertTrue(
                            _wait_for(lambda: "watchdog up" in _text(log)),
                            f"intent={value} must start the watchdog: {_text(log)}")
                        self.assertEqual((lock / "pid").read_text().strip(),
                                         str(watchdog.pid))
                        # Several passes of the real loop, with no run directory.
                        time.sleep(0.6)
                        self.assertIsNone(
                            watchdog.poll(),
                            f"intent={value} must keep it up: {_text(log)}")
                        intent.write_text("stopped\n")
                        self.assertEqual(watchdog.wait(timeout=10), 0)
                    finally:
                        _stop(watchdog)
                self.assertIn("verification intent is not running (intent=stopped); "
                              "exiting without touching a game", _text(log))
                self.assertFalse(lock.exists(), "its lock must go with it")
                self.assertEqual(_text(complaints), "",
                                 "a poll with no run directory must say nothing")

    def test_any_other_intent_exits_before_the_lock(self):
        """Fail closed, as before: only the exact values authorize it, and
        `terminal` only the copy the Terminal window's host started."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        cases = [(value, hosted)
                 for value in (None, "", "stopped", "Terminal", "terminal window",
                               "running\nterminal", "unattended")
                 for hosted in (False, True)]
        # What a keeper, a launchd job or a stray shell would be under
        # terminal control: not the window's child, so not authorized.
        cases.append(("terminal", False))
        for value, hosted in cases:
            with self.subTest(intent=value, hosted=hosted), \
                    tempfile.TemporaryDirectory() as tmp:
                home, env = _sandbox(tmp, blind=True)
                if hosted:
                    env[TERMINAL_WINDOW_FLAG] = "1"
                if value is not None:
                    (home / ".civvis-operator-intent").write_text(value + "\n")
                done = subprocess.run(
                    ["zsh", str(WATCHDOG)], env={**env, "CIVVIS_WEDGE_POLL_S": "0.1"},
                    capture_output=True, text=True, timeout=30)
                log = _text(home / "civvis-civ6-runs" / "agent_wedge_watchdog.log")
                self.assertEqual(done.returncode, 0, done.stderr)
                self.assertIn("verification intent is not running", log)
                self.assertIn("exiting without touching a game", log)
                self.assertNotIn("watchdog up", log)
                self.assertFalse((home / ".civvis-agent-wedge-watchdog.lock").exists())
                if value == "terminal":
                    self.assertIn("intent=terminal, which authorizes only the watchdog "
                                  "the interactive host starts with "
                                  + TERMINAL_WINDOW_FLAG + "=1", log)


class TheHostDoesNotHotLoopItsWedgeWatchdog(unittest.TestCase):
    """⚠⚠ 52,000 RESTARTS AND NOT ONE WATCHDOG.

    The host's poll said "agent wedge watchdog exited; restarting" every five
    seconds for six days and never asked why the last one had lived for none.
    A watchdog that exits at once is now restarted on the next poll once, then
    after a wait that doubles to a ceiling; one that lived is still restarted
    on the next poll, which is what the climb's batch-start retire relies on.
    """

    def test_the_knobs_are_named_and_the_line_clears_the_watchdogs_first_poll(self):
        """A watchdog that dies in its first pass must still count as a quick
        exit, or one that crashes there is restarted every minute for ever."""
        host = HOST.read_text(encoding="utf-8")
        self.assertIn("WEDGE_BACKOFF_S=${CIVVIS_WEDGE_BACKOFF_S:-10}", host)
        self.assertIn("WEDGE_BACKOFF_MAX_S=${CIVVIS_WEDGE_BACKOFF_MAX_S:-600}", host)
        quick = int(re.search(
            r"WEDGE_QUICK_EXIT_S=\$\{CIVVIS_WEDGE_QUICK_EXIT_S:-(\d+)\}", host).group(1))
        poll = int(re.search(r"POLL_S=\$\{CIVVIS_WEDGE_POLL_S:-(\d+)\}",
                             WATCHDOG.read_text(encoding="utf-8")).group(1))
        self.assertGreater(quick, poll)

    def _host(self, tmp: str, watchdog: Path, **knobs: str):
        if not (Path("/bin/zsh").exists() and Path("/usr/bin/caffeinate").exists()):
            self.skipTest("the interactive host is a macOS zsh script")
        home, env = _sandbox(tmp)
        (home / ".civvis-operator-intent").write_text("terminal\n")
        sleeper = "#!/bin/zsh\nwhile true; do sleep 1; done\n"
        # The host recognises a live supervisor by this file name.
        supervisor = Path(tmp) / "civvis-game-supervisor.sh"
        mirror_keeper = Path(tmp) / "fake-mirror-keeper.sh"
        for script in (supervisor, mirror_keeper):
            script.write_text(sleeper)
        gamelock = Path(tmp) / "gamelock.py"
        gamelock.write_text("raise SystemExit(1)\n")  # no operator halt stands
        env.update(
            CIVVIS_SUPERVISOR=str(supervisor),
            CIVVIS_MIRROR_KEEPER=str(mirror_keeper),
            CIVVIS_WEDGE_WATCHDOG=str(watchdog),
            CIVVIS_GAMELOCK=str(gamelock),
            CIVVIS_GAMELOCK_PYTHON=sys.executable,
            # No visual popup keeper: it is not what these tests are about.
            CIVVIS_CAPTURE_FREE="1",
            CIVVIS_INTERACTIVE_HOST_POLL_S="0.05",
            WEDGE_STARTS=str(Path(tmp) / "wedge.starts"),
            **knobs)
        complaints = (Path(tmp) / "host.stderr").open("w")
        self.addCleanup(complaints.close)
        host = subprocess.Popen(["/bin/zsh", str(HOST)], env=env,
                                stdout=subprocess.DEVNULL, stderr=complaints)
        return host, home, home / "civvis-civ6-runs" / "interactive_host.log"

    def _stop_quietly(self, tmp: str, host: subprocess.Popen) -> None:
        """Stop the host; a zsh error in it does not stop it, so read stderr."""
        _stop(host)
        self.assertEqual(_text(Path(tmp) / "host.stderr"), "",
                         "the host wrote to stderr")

    @staticmethod
    def _starts(tmp: str) -> list[int]:
        return [int(line) for line in _text(Path(tmp) / "wedge.starts").split()]

    def test_under_terminal_control_the_hosts_own_watchdog_stays_up(self):
        """The defect itself, with the real watchdog under the real host."""
        with tempfile.TemporaryDirectory() as tmp:
            # A long poll: the watchdog only has to start and stay.
            host, home, log = self._host(tmp, WATCHDOG, CIVVIS_WEDGE_POLL_S="30")
            wedge_log = home / "civvis-civ6-runs" / "agent_wedge_watchdog.log"
            pid_file = home / ".civvis-agent-wedge-watchdog.lock" / "pid"
            try:
                self.assertTrue(_wait_for(lambda: "watchdog up" in _text(wedge_log)),
                                f"host: {_text(log)}\nwatchdog: {_text(wedge_log)}")
                watchdog_pid = int(pid_file.read_text())
                time.sleep(1.0)  # several host polls
                self.assertIsNone(host.poll(), _text(log))
                os.kill(watchdog_pid, 0)  # raises if the watchdog is gone
                text = _text(log)
                self.assertEqual(text.count("started agent wedge watchdog pid"), 1, text)
                self.assertNotIn("agent wedge watchdog exited", text)
                self.assertNotIn("not running", _text(wedge_log))
            finally:
                self._stop_quietly(tmp, host)

            def gone() -> bool:
                try:
                    os.kill(watchdog_pid, 0)
                except ProcessLookupError:
                    return True
                return False

            self.assertTrue(_wait_for(gone, 5.0),
                            "closing the window's host must take its watchdog with it")

    def test_a_watchdog_that_exits_at_once_is_not_restarted_every_poll(self):
        with tempfile.TemporaryDirectory() as tmp:
            # What the watchdog did under `terminal` before the fix.
            refuser = Path(tmp) / "fake-wedge-refuser.sh"
            refuser.write_text('#!/bin/zsh\nprint -r -- $$ >> "$WEDGE_STARTS"\nexit 0\n')
            host, _, log = self._host(tmp, refuser, CIVVIS_WEDGE_BACKOFF_S="1",
                                      CIVVIS_WEDGE_BACKOFF_MAX_S="2")
            try:
                self.assertTrue(_wait_for(lambda: len(self._starts(tmp)) >= 1),
                                _text(log))
                time.sleep(2.5)  # dozens of host polls
                self.assertIsNone(host.poll(), "the host itself must keep running")
                starts, text = len(self._starts(tmp)), _text(log)
            finally:
                self._stop_quietly(tmp, host)
        # The first start, the one free restart, then waits of 1 s and 2 s.
        self.assertGreaterEqual(starts, 2, text)
        self.assertLessEqual(starts, 5,
                             f"{starts} starts in 2.5 s is the hot loop again")
        self.assertEqual(text.count("agent wedge watchdog exited; restarting"), 1, text)
        self.assertIn("times running (status 0, intent=terminal); next start in 1s",
                      text)
        self.assertLessEqual(text.count("agent wedge watchdog exited"), 5, text)

    def test_a_missing_watchdog_backs_off_the_same_way(self):
        with tempfile.TemporaryDirectory() as tmp:
            host, _, log = self._host(tmp, Path(tmp) / "absent-watchdog.sh",
                                      CIVVIS_WEDGE_BACKOFF_S="1",
                                      CIVVIS_WEDGE_BACKOFF_MAX_S="2")
            try:
                self.assertTrue(_wait_for(lambda: "cannot start agent wedge watchdog"
                                          in _text(log)), _text(log))
                time.sleep(2.0)
                self.assertIsNone(host.poll(), "the host itself must keep running")
                text = _text(log)
            finally:
                self._stop_quietly(tmp, host)
        self.assertLessEqual(text.count("cannot start agent wedge watchdog"), 5, text)
        self.assertIn("next start in 1s", text)

    def test_a_retired_watchdog_is_back_on_the_next_poll(self):
        """The climb stops the watchdog at every batch start so the host brings
        it back on the current build (`retire_stale_wedge_watchdog`). A
        watchdog that had lived is restarted at once, however often; only
        exits that follow their own start closely are made to wait, and one
        long life clears that count."""
        with tempfile.TemporaryDirectory() as tmp:
            stayer = Path(tmp) / "fake-wedge-stayer.sh"
            stayer.write_text('#!/bin/zsh\nprint -r -- $$ >> "$WEDGE_STARTS"\n'
                              'while true; do sleep 1; done\n')
            host, _, log = self._host(tmp, stayer, CIVVIS_WEDGE_QUICK_EXIT_S="2",
                                      CIVVIS_WEDGE_BACKOFF_S="1",
                                      CIVVIS_WEDGE_BACKOFF_MAX_S="1")

            def retire_and_await(count: int, within_s: float) -> None:
                os.kill(self._starts(tmp)[-1], signal.SIGTERM)
                self.assertTrue(_wait_for(lambda: len(self._starts(tmp)) >= count,
                                          within_s),
                                f"no start number {count} within {within_s}s: {_text(log)}")

            try:
                self.assertTrue(_wait_for(lambda: len(self._starts(tmp)) >= 1),
                                _text(log))
                time.sleep(2.2)              # it has lived past the quick-exit line
                retire_and_await(2, 1.0)     # so it is back on the next poll
                self.assertNotIn("next start in", _text(log))
                retire_and_await(3, 1.0)     # first quick exit: still at once
                self.assertNotIn("next start in", _text(log))
                retire_and_await(4, 3.0)     # second in a row: after the wait
                self.assertEqual(_text(log).count("next start in 1s"), 1, _text(log))
                time.sleep(2.2)              # a long life clears the count
                retire_and_await(5, 1.0)
                retire_and_await(6, 1.0)     # so the next quick exit is free again
                self.assertEqual(_text(log).count("next start in"), 1, _text(log))
                self.assertIsNone(host.poll(), _text(log))
            finally:
                self._stop_quietly(tmp, host)


if __name__ == "__main__":
    unittest.main()
