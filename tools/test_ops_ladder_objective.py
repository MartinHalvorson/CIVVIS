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

import re
import sys
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
        self.assertIn('${VICTORY:+--victory "$VICTORY"}', source)

    def test_the_installed_supervisor_uses_the_evidence_gated_rung(self):
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("civ6_ladder_policy.py", source)
        self.assertIn('--runs "$RUNS_DIR" target', source)
        self.assertIn('--difficulty "$DIFFICULTY"', source)
        self.assertIn("CIVVIS_DIFFICULTY", source)
        self.assertIn("ATTEMPTS=${CIVVIS_PLAY_ATTEMPTS:-3}", source)


if __name__ == "__main__":
    unittest.main()


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
