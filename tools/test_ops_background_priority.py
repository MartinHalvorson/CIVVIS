#!/usr/bin/env python3
"""A live Civilization VI game must not start demoted by the shell that backgrounds it.

zsh sets ``BG_NICE`` by default, so every ``&`` job a zsh script starts runs at
nice +5, and the whole subtree inherits it. ``civvis-interactive-host.sh``
backgrounds the game supervisor that way, so from 2026-08-11 every live game
ran underneath every nice-0 ``cargo build`` on the box (9-11 s/turn on a quiet
host, ~18 s/turn under fleet load) -- and macOS refuses to lower a nice once
set, so a game born demoted stays demoted for its whole run. The exhibition
lane had already found and fixed this (``civvis-keeper.sh``: ``unsetopt
BG_NICE``); the live lane kept paying it because nothing checked.

Two rules, discovered rather than listed so a new script cannot quietly
reintroduce the demotion:

* every zsh script under ``tools/ops`` that backgrounds a job turns ``BG_NICE``
  off before its first ``&``;
* the fix does what it claims -- probed against the real shell where one is
  installed, not asserted from the option's documentation.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

OPS = Path(__file__).resolve().parent / "ops"

BG_NICE_OFF = re.compile(r"^\s*(unsetopt\s+BG_NICE|setopt\s+NO_BG_NICE)\b", re.IGNORECASE)
# Lines that merely PRINT a `... &` command for an operator to type are
# instructions, not jobs.
OUTPUT_COMMANDS = ("say", "print", "printf", "err", "echo", "log", "warn", "note")


def logical_lines(text: str) -> list[tuple[int, str]]:
    """Join backslash-continued lines, keeping the number of their first line."""
    lines: list[tuple[int, str]] = []
    buffer = ""
    start = 0
    for number, raw in enumerate(text.splitlines(), start=1):
        if not buffer:
            start = number
        if raw.rstrip().endswith("\\"):
            buffer += raw.rstrip()[:-1] + " "
            continue
        lines.append((start, buffer + raw))
        buffer = ""
    if buffer:
        lines.append((start, buffer))
    return lines


def background_jobs(text: str) -> list[int]:
    """Line numbers at which a script backgrounds a command with `&`."""
    jobs = []
    for number, line in logical_lines(text):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.endswith("&&") or not stripped.endswith("&"):
            continue
        first = stripped.split()[0]
        if first in OUTPUT_COMMANDS:
            continue
        jobs.append(number)
    return jobs


def is_zsh(text: str) -> bool:
    first = text.splitlines()[0] if text else ""
    return first.startswith("#!") and "zsh" in first


def zsh_scripts_with_background_jobs() -> dict[Path, list[int]]:
    found: dict[Path, list[int]] = {}
    for path in sorted(OPS.glob("*.sh")):
        text = path.read_text()
        if not is_zsh(text):
            continue
        jobs = background_jobs(text)
        if jobs:
            found[path] = jobs
    return found


def first_bg_nice_off(text: str) -> int | None:
    for number, line in enumerate(text.splitlines(), start=1):
        if BG_NICE_OFF.match(line):
            return number
    return None


class EveryBackgroundingScriptTurnsBgNiceOff(unittest.TestCase):
    def test_the_glob_finds_the_scripts_it_guards(self) -> None:
        found = zsh_scripts_with_background_jobs()
        self.assertTrue(found, "no zsh script under tools/ops backgrounds a job -- "
                               "the glob or the detector moved")
        names = {path.name for path in found}
        # The two launch sites the live ladder actually depends on.
        self.assertIn("civvis-interactive-host.sh", names)
        self.assertIn("civvis-verification-relaunch.sh", names)

    def test_bg_nice_is_off_before_the_first_background_job(self) -> None:
        failures = []
        for path, jobs in zsh_scripts_with_background_jobs().items():
            off_at = first_bg_nice_off(path.read_text())
            if off_at is None:
                failures.append(f"{path.name}: backgrounds a job at line {jobs[0]} and "
                                "never turns BG_NICE off")
            elif off_at > jobs[0]:
                failures.append(f"{path.name}: BG_NICE is turned off at line {off_at}, "
                                f"after the first background job at line {jobs[0]}")
        self.assertEqual(failures, [], "\n".join(failures))

    def test_instructions_that_tell_an_operator_to_background_the_loop_say_so(self) -> None:
        """A human typing `nohup ... &` into Terminal demotes the loop the same way."""
        offenders = []
        for path in sorted(OPS.glob("*.sh")):
            for number, line in enumerate(path.read_text().splitlines(), start=1):
                if "nohup /bin/zsh" in line and line.rstrip().endswith('&"') \
                        and "BG_NICE" not in line:
                    offenders.append(f"{path.name}:{number}")
        self.assertEqual(offenders, [], "operator instructions that background the loop "
                                        f"without turning BG_NICE off: {offenders}")

    def test_the_supervisor_reports_its_own_priority(self) -> None:
        text = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("ps -o ni= -p $$", text)
        self.assertIn("supervisor priority nice", text)


class TheDetectorReadsShellNotProse(unittest.TestCase):
    def test_a_printed_instruction_is_not_a_job(self) -> None:
        text = 'say "  start it: nohup /bin/zsh ~/loop.sh >> log 2>&1 &"\n'
        self.assertEqual(background_jobs(text), [])

    def test_a_continued_command_is_found_on_its_first_line(self) -> None:
        text = ("#!/bin/zsh\n"
                "set -u\n"
                "nohup python3 tools/thing.py \\\n"
                "    --flag >> log 2>&1 &\n")
        self.assertEqual(background_jobs(text), [3])

    def test_and_and_is_not_a_background_job(self) -> None:
        self.assertEqual(background_jobs("a &&\n  b\n"), [])


@unittest.skipUnless(shutil.which("zsh"), "zsh is not installed here")
class TheOptionActuallyChangesThePriority(unittest.TestCase):
    """Probe the real shell: the number the game inherits, not the manual."""

    @staticmethod
    def child_nice(prelude: str) -> int:
        script = f"{prelude}sleep 2 & ps -o ni= -p $!; kill $! 2>/dev/null; wait 2>/dev/null"
        result = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                                timeout=20, check=True)
        return int(result.stdout.strip())

    def test_a_job_backgrounded_with_bg_nice_off_keeps_the_parent_priority(self) -> None:
        self.assertEqual(self.child_nice("unsetopt BG_NICE; "), os.nice(0))

    def test_a_job_backgrounded_by_default_is_at_or_below_the_parent(self) -> None:
        # Documentation only: zsh's default demotes. Not asserted as +5 because the
        # size of the step is the shell's, not this repository's.
        self.assertGreaterEqual(self.child_nice(""), os.nice(0))


if __name__ == "__main__":
    unittest.main()
