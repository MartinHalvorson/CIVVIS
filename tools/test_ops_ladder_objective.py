#!/usr/bin/env python3
"""The ladder's objective is one fact, stated once, and the loops obey it.

Every real ladder attempt runs down one chain — `civ6_civvis_climb.py` forwards
`--victory` to `civ6_play.py --civvis-victory`, which forwards it to
`civ6_brain.py`, which forwards it to `civvis_orders --victory`. The *list* of
lanes was collapsed to one source of truth after three of the six became
unreachable from the live seat; the *default* was not, and it fragmented in
exactly the same way:

* `civ6_play.py`, `civ6_civvis_climb.py` and `civ6_brain.py` each declared
  `science`;
* `tools/ops/civvis-batch-loop.sh` wrote `--victory civvis` into three places;
* `tools/ops/civvis-game-supervisor.sh`, the loop actually installed as a
  launchd service, passed nothing and inherited `science` in silence.

So the two production supervisors were running two different experiments into
one ledger, and 307 recorded attempts went to the one lane `victory_eval`
completes **0/16** at the profile the ladder plays (`docs/EVAL.md`).

These tests are structural on purpose. They do not check that the objective is
any particular good one — they check that there is only one of it, that a shell
script cannot quietly hold a second, and that the value is a lane the chain can
actually carry.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_brain  # noqa: E402
import civ6_civvis_climb  # noqa: E402
import civ6_play  # noqa: E402

OPS = Path(__file__).resolve().parent / "ops"

# A `--victory` whose value is a bare word: the literal the launchers drifted on.
# `--victory $lane`, `--victory "$VICTORY"` and `${VICTORY:+--victory "$VICTORY"}`
# all name a variable and are what this file exists to require.
HARDCODED_LANE = re.compile(r"--victory[=\s]+(?![\"']?[$])([A-Za-z][\w-]*)")


class TheDefaultObjectiveHasOneHome(unittest.TestCase):
    def test_every_launcher_reads_the_same_object(self):
        """Not merely equal — the same object, so a copy cannot be introduced.

        Equality would pass again the moment someone re-declared the literal in
        a second module, which is the precise failure being locked out here.
        """
        self.assertIs(civ6_civvis_climb.DEFAULT_VICTORY,
                      civ6_play.DEFAULT_CIVVIS_VICTORY)
        self.assertIs(civ6_brain.DEFAULT_VICTORY,
                      civ6_play.DEFAULT_CIVVIS_VICTORY)

    def test_the_default_is_a_lane_the_chain_accepts(self):
        self.assertIn(civ6_play.DEFAULT_CIVVIS_VICTORY, civ6_play.VICTORY_LANES)

    def test_the_default_is_the_measured_one(self):
        """A deliberate pin, and the second time this value has been argued.

        `docs/EVAL.md` 2026-08-17, at the ladder's own profile (6 players, 250
        turns, Online): science completes **0/16**, diplomatic **14/16**,
        culture 12/16, religious 8/16, domination 2/16 across 96 games on two
        disjoint seed streams; and all four named lanes beat the
        science-targeted incumbent under `ai_eval --deployment-comparison`.
        Diplomacy is chosen among the lanes that land on the HOST's own census —
        `docs/CIV6_LADDER.md` ranks 199 terminal events diplomatic 41 > culture
        24 > religious 5 — and not on that margin, which measures science's
        floor rather than Diplomacy's strength.

        Moving this is allowed and is meant to cost one deliberate edit here.
        """
        self.assertEqual(civ6_play.DEFAULT_CIVVIS_VICTORY, "diplomatic")

    def test_the_help_text_does_not_restate_the_value(self):
        """The help said "defaults to Science" for as long as the value was
        `science`, and would have gone on saying it afterwards."""
        for module, path in ((civ6_play, "civ6_play.py"),
                             (civ6_civvis_climb, "civ6_civvis_climb.py")):
            source = (Path(module.__file__)).read_text()
            with self.subTest(path=path):
                self.assertNotIn("defaults to Science", source)


class NoOperationalScriptHoldsALaneOfItsOwn(unittest.TestCase):
    def test_the_scripts_are_discovered_not_listed(self):
        """A hand-written list of files to check shrinks silently; glob instead."""
        self.assertTrue(list(OPS.glob("*.sh")), f"no shell scripts under {OPS}")

    def test_no_ops_script_writes_a_victory_lane_by_hand(self):
        offenders = []
        for script in sorted(OPS.glob("*.sh")):
            for number, line in enumerate(script.read_text().splitlines(), 1):
                if line.lstrip().startswith("#"):
                    continue
                for match in HARDCODED_LANE.finditer(line):
                    offenders.append(f"{script.name}:{number}: --victory {match.group(1)}")
        self.assertEqual(offenders, [], "\n".join(
            ["an ops script names a victory lane itself; ask the tree instead "
             "(`civ6_play.DEFAULT_CIVVIS_VICTORY`) or take one from CIVVIS_VICTORY:"]
            + offenders))

    def test_the_installed_supervisor_can_state_an_objective(self):
        """It could not, which is why its objective was whatever it inherited."""
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("VICTORY=${CIVVIS_VICTORY:-}", source)
        self.assertIn('${VICTORY:+--victory} ${VICTORY:+"$VICTORY"}', source)
        self.assertIn("RESTART_BELOW_LEADER_RATIO=${CIVVIS_RESTART_BELOW_LEADER_RATIO:-}",
                      source)

    def test_the_supervisors_optional_flags_reach_the_climb_as_words(self):
        """⚠ zsh does not word-split an unquoted `${VAR:+--flag "$VAR"}`: set,
        it reached the climb as ONE argument, `--victory science`, which
        argparse rejects as unrecognized. The victory form had never been
        exercised; the abandon floor was, 2026-08-19 17:00Z, and four starts
        in a row played nothing. Run the script's OWN knob lines and the
        optional-flag lines of its climb invocation under zsh, with and
        without the knobs, and read the words that come out."""
        if shutil.which("zsh") is None:
            # The supervisor is a zsh script and only ever runs on the macOS
            # hosts that have it; the literal pin above is the guard on a
            # runner without one.
            self.skipTest("zsh is not installed here")
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        knob_lines = [line for line in source.splitlines()
                      if re.match(r"^(VICTORY|ABANDON_BELOW|RESTART_BELOW_LEADER_RATIO)=\$\{CIVVIS_", line)]
        self.assertEqual(len(knob_lines), 3, knob_lines)
        invocation = EveryLadderLoopCanAskForTheRungAndTheLane._invocation(source)
        flag_lines = [line.strip().rstrip("\\").strip()
                      for line in invocation.splitlines()
                      if ":+--" in line]
        self.assertEqual(len(flag_lines), 3, invocation)
        script = "\n".join(knob_lines) + (
            "\nfor w in " + " ".join(flag_lines) + "; do print -r -- \"$w\"; done\n")
        for knobs, expected in (
            ({}, []),
            ({"CIVVIS_VICTORY": "science"}, ["--victory", "science"]),
            ({"CIVVIS_ABANDON_BELOW_WIN_RATE": "0.05"},
             ["--abandon-below-win-rate", "0.05"]),
            ({"CIVVIS_RESTART_BELOW_LEADER_RATIO": "0.70"},
             ["--restart-below-leader-ratio", "0.70"]),
            ({"CIVVIS_VICTORY": "culture", "CIVVIS_ABANDON_BELOW_WIN_RATE": "0.1",
              "CIVVIS_RESTART_BELOW_LEADER_RATIO": "0.70"},
             ["--victory", "culture", "--abandon-below-win-rate", "0.1",
              "--restart-below-leader-ratio", "0.70"]),
        ):
            with self.subTest(knobs=knobs):
                env = {k: v for k, v in os.environ.items()
                       if not k.startswith("CIVVIS_")}
                env.update(knobs)
                done = subprocess.run(["zsh", "-c", script], env=env,
                                      capture_output=True, text=True,
                                      check=True)
                words = [w for w in done.stdout.split("\n") if w != ""]
                self.assertEqual(words, expected)

    def test_the_installed_supervisor_forwards_a_named_live_withhold_as_words(self):
        """The live A/B gate is a list of argv words, never one shell string.

        The runner already records `withheld` in each game summary, but this
        is the only hop that can otherwise lose the operator's named arm.  In
        particular, zsh does not split a quoted comma-list for us: turn it
        into repeated `--without TREATMENT` pairs before the climb starts.
        """
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed here")
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        start = source.index("WITHHELD=${CIVVIS_WITHOUT:-}")
        end = source.index("# Attempts per cycle.", start)
        gate = source[start:end]
        invocation_start = source.index("python3 -u tools/civ6_civvis_climb.py")
        invocation_end = source.index("# \"Played a turn\"", invocation_start)
        self.assertIn('"${WITHOUT_ARGS[@]}"', source[invocation_start:invocation_end])
        script = gate + '\nfor word in "${WITHOUT_ARGS[@]}"; do print -r -- "$word"; done\n'
        for raw, expected in (
            (None, []),
            ("war-economy", ["--without", "war-economy"]),
            ("war-economy,garrison-walls",
             ["--without", "war-economy", "--without", "garrison-walls"]),
        ):
            with self.subTest(raw=raw):
                env = {key: value for key, value in os.environ.items()
                       if not key.startswith("CIVVIS_")}
                if raw is not None:
                    env["CIVVIS_WITHOUT"] = raw
                done = subprocess.run(["zsh", "-c", script], env=env,
                                      capture_output=True, text=True, check=True)
                words = [word for word in done.stdout.split("\n") if word]
                self.assertEqual(words, expected)

    def test_the_installed_supervisor_forwards_live_timeout_budgets_as_words(self):
        """A slow GUI host can extend both linked watchdog budgets per batch.

        The defaults still belong to ``civ6_civvis_climb.py``; an absent
        operator knob must therefore add no argument. Values are array words,
        not a shell fragment, so each number remains one argparse value.
        """
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed here")
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        start = source.index("PLAY_TIMEOUT=${CIVVIS_PLAY_TIMEOUT:-}")
        end = source.index("SUP=$LOGS/supervisor.log", start)
        gate = source[start:end]
        invocation_start = source.index("python3 -u tools/civ6_civvis_climb.py")
        invocation_end = source.index("# \"Played a turn\"", invocation_start)
        self.assertIn('"${TIMEOUT_ARGS[@]}"',
                      source[invocation_start:invocation_end])
        script = gate + (
            '\nfor word in "${TIMEOUT_ARGS[@]}"; do print -r -- "$word"; done\n'
        )
        for knobs, expected in (
            ({}, []),
            ({"CIVVIS_PLAY_TIMEOUT": "10800"}, ["--timeout", "10800"]),
            ({"CIVVIS_PLAY_TIMEOUT": "10800",
              "CIVVIS_PLAY_TIMEOUT_CEILING": "14400"},
             ["--timeout", "10800", "--timeout-ceiling", "14400"]),
        ):
            with self.subTest(knobs=knobs):
                env = {key: value for key, value in os.environ.items()
                       if not key.startswith("CIVVIS_")}
                env.update(knobs)
                done = subprocess.run(["zsh", "-c", script], env=env,
                                      capture_output=True, text=True, check=True)
                words = [word for word in done.stdout.split("\n") if word]
                self.assertEqual(words, expected)

    def test_the_installed_supervisor_forwards_a_named_ledger_force_on_or_file_as_words(self):
        """A force-on arm stays explicit through either approved control path.

        The deployment genome withholds an unresolved gene by default.  The
        supervisor is the only safe owner of this host's Civ VI slot, so it
        must be able to pass the deliberately named `--with` verification arm
        through as repeated argv words rather than silently leaving a force-on
        experiment impossible to schedule.  The GUI host cannot change its
        inherited environment during a long-running session, so an absent-by-
        default batch file is the second path; its value must be the same arm
        and a disagreement fails closed before a game can launch.
        """
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed here")
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        start = source.index("FORCED_ENV=${CIVVIS_WITH:-}")
        end = source.index("# Attempts per cycle.", start)
        gate = source[start:end]
        invocation_start = source.index("python3 -u tools/civ6_civvis_climb.py")
        invocation_end = source.index("# \"Played a turn\"", invocation_start)
        self.assertIn('"${WITH_ARGS[@]}"', source[invocation_start:invocation_end])
        self.assertIn("resolve_forced_arm", gate)
        resolve_call = source.index("if ! resolve_forced_arm;")
        build_call = source.index("if ! cargo build --release --bin civvis_orders")
        self.assertLess(resolve_call, build_call,
                        "the arm must be resolved before this batch can build or launch")
        script = (
            "say() { :; }\n" + gate
            + '\nresolve_forced_arm || exit $?\n'
            + 'for word in "${WITH_ARGS[@]}"; do print -r -- "$word"; done\n'
        )
        for env_arm, file_arm, expected in (
            (None, None, []),
            (None, "", []),
            ("amenity-project-preemption", None,
             ["--with", "amenity-project-preemption"]),
            (None, "amenity-project-preemption",
             ["--with", "amenity-project-preemption"]),
            ("amenity-project-preemption",
             "amenity-project-preemption,idle-walkers-close-the-pipeline",
             None),
            ("amenity-project-preemption,idle-walkers-close-the-pipeline",
             "amenity-project-preemption,idle-walkers-close-the-pipeline",
             ["--with", "amenity-project-preemption",
              "--with", "idle-walkers-close-the-pipeline"]),
            (None, "amenity-project-preemption\nidle-walkers-close-the-pipeline", None),
        ):
            with self.subTest(env_arm=env_arm, file_arm=file_arm):
                with tempfile.TemporaryDirectory() as directory:
                    force_file = Path(directory) / "force-on"
                    if file_arm is not None:
                        force_file.write_text(file_arm)
                    env = {key: value for key, value in os.environ.items()
                           if not key.startswith("CIVVIS_")}
                    env["CIVVIS_WITH_FILE"] = str(force_file)
                    if env_arm is not None:
                        env["CIVVIS_WITH"] = env_arm
                    done = subprocess.run(["zsh", "-c", script], env=env,
                                          capture_output=True, text=True)
                    if expected is None:
                        self.assertNotEqual(done.returncode, 0, done.stderr)
                    else:
                        self.assertEqual(done.returncode, 0, done.stderr)
                        words = [word for word in done.stdout.split("\n") if word]
                        self.assertEqual(words, expected)

    def test_the_installed_supervisor_uses_the_evidence_gated_rung(self):
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("civ6_ladder_policy.py", source)
        self.assertIn('--runs "$RUNS_DIR" target', source)
        self.assertIn('--difficulty "$DIFFICULTY"', source)
        self.assertIn("CIVVIS_DIFFICULTY", source)
        self.assertIn("ATTEMPTS=${CIVVIS_PLAY_ATTEMPTS:-3}", source)

    def test_head_pin_refreshes_the_detached_checkout_before_a_batch(self):
        """A detached runner must not silently replay a stale ladder rung.

        The live runner sat at 24e5c068 while its `origin/main` ref was fifteen
        commits ahead: its uninspected `git pull` did not move detached HEAD,
        so it also missed the supervisor's ladder-policy wiring.  This is a
        source contract because the production loop is shell; it locks both the
        explicit update sequence and the read-back that rejects any mismatch.
        """
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        start = source.index('rm -f status.json')
        end = source.index('if ! cargo build', start)
        sync = source[start:end]

        self.assertIn('git -c gc.auto=0 fetch --quiet origin main', sync)
        self.assertIn('git checkout --quiet --detach origin/main', sync)
        self.assertIn('ORIGIN_MAIN_SHA=$(git rev-parse origin/main', sync)
        self.assertIn('"$HEAD_SHA" != "$ORIGIN_MAIN_SHA"', sync)
        self.assertIn('refusing to run a stale head batch', sync)
        self.assertNotIn('git pull -q --ff-only origin main', sync)
        self.assertLess(sync.index('fetch --quiet origin main'),
                        sync.index('checkout --quiet --detach origin/main'))
        self.assertLess(sync.index('checkout --quiet --detach origin/main'),
                        sync.index('"$HEAD_SHA" != "$ORIGIN_MAIN_SHA"'))


class EveryLadderLoopCanAskForTheRungAndTheLane(unittest.TestCase):
    """`docs/CIV6_LADDER.md` records wins per (victory type, difficulty), so a
    loop that cannot vary those two cannot move the ladder at all.

    Both axes have now been silently fixed at a launcher default by a loop that
    produces ladder rows. The objective was, until #1960 — 307 attempts aimed at
    the one lane `victory_eval` completes 0/16. The rung was, until #1969 gave
    the installed supervisor a policy — and `civvis-batch-loop.sh` still could
    not express it afterwards, so the two production loops would have been
    climbing on two different rules with only one of them able to reach
    Chieftain.

    Discovered, not listed: a script counts as a ladder loop when it invokes
    `civ6_civvis_climb.py`, so a new one is held to this the day it is written.
    """

    #: A script that RUNS the climb, not one that merely names it.
    #: `civvis-supervisor-safe-reload.sh` matches the name in a `ps` pattern and
    #: `civvis-tcc-probe.sh` in prose; neither starts an attempt, and holding
    #: them to this would be a guard that has to be argued with.
    INVOKES = re.compile(r"^\s*[^#\n]*python3[^\n#]*civ6_civvis_climb\.py",
                         re.MULTILINE)

    def ladder_loops(self) -> list:
        return [path for path in sorted(OPS.glob("*.sh"))
                if self.INVOKES.search(path.read_text(encoding="utf-8"))]

    def test_at_least_one_ladder_loop_is_found(self):
        """If this ever empties, the discovery rule stopped matching rather than
        the loops going away."""
        self.assertTrue(self.ladder_loops(),
                        "no ops script invokes civ6_civvis_climb.py")

    @staticmethod
    def _invocation(source: str) -> str:
        """The climb command itself, following backslash continuations.

        ⚠ NOT "the flag appears somewhere in the file". `civvis-batch-loop.sh`
        also PRINTS its command into a provenance file, by hand, beside the
        command it claims to describe — and that line's own comment records it
        saying `--war-from-plan` for hours after the flag was removed below. A
        guard satisfied by the description rather than the invocation would
        certify exactly that.
        """
        lines = source.splitlines()
        start = next(i for i, line in enumerate(lines)
                     if "civ6_civvis_climb.py" in line
                     and "python3" in line
                     and not line.lstrip().startswith("#"))
        command = []
        for line in lines[start:]:
            command.append(line)
            if not line.rstrip().endswith("\\"):
                break
        return "\n".join(command)

    def test_each_one_passes_both_axes(self):
        missing = []
        for script in self.ladder_loops():
            command = self._invocation(script.read_text(encoding="utf-8"))
            for flag in ("--victory", "--difficulty"):
                if flag not in command:
                    missing.append(f"{script.name}: never passes {flag}")
        self.assertEqual(missing, [], "\n".join(
            ["a loop that writes ladder rows cannot ask for an axis the ladder "
             "records, so no row can ever carry a different value:"] + missing))

    def test_neither_axis_is_written_as_a_literal(self):
        """A pinned rung is the same defect as a pinned lane, one axis over."""
        literal = re.compile(
            r"--difficulty[=\s]+(?![\"']?[$])(DIFFICULTY_[A-Z]+)")
        offenders = []
        for script in self.ladder_loops():
            for number, line in enumerate(script.read_text().splitlines(), 1):
                if line.lstrip().startswith("#"):
                    continue
                for match in literal.finditer(line):
                    offenders.append(f"{script.name}:{number}: {match.group(1)}")
        self.assertEqual(offenders, [], "\n".join(
            ["an ops script pins the rung by hand; take it from the ladder "
             "policy or from CIVVIS_DIFFICULTY:"] + offenders))


if __name__ == "__main__":
    unittest.main()
