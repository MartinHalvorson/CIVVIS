#!/usr/bin/env python3
"""Reject Lua that Civilization VI's 5.1 runtime cannot compile.

`luac -p` is not a syntax check for this mod. Homebrew ships Lua 5.5; the game
runs 5.1. Anything added to the language after 5.1 parses cleanly on the way in
and then refuses to load inside the game, where the only symptom is the mod
being absent.

That is not a hypothetical. `goto continue` was added to
`CivvisControlAgent.lua` on 2026-07-29, passed `luac -p` under Lua 5.5, and cost
run `settler-20260729T224210Z`: it sat in a correctly configured game for half an
hour emitting nothing but `autoclose` events, because that context lives in a
different file and kept working while the agent never loaded. It reads exactly
like a stalled game.

Run it over the mod before shipping a change:

    python3 tools/civ6_control/check_lua51.py

Exit 0 clean, 1 with findings. It is deliberately a lexical check rather than a
parser: a wrong-version parser is what caused the problem in the first place,
and this only has to catch the handful of constructs newer than 5.1.
"""
from __future__ import annotations

import pathlib
import re
import sys

# Each entry is (regex, what it is, which Lua version introduced it).
FORBIDDEN = [
    (re.compile(r"\bgoto\s+\w"), "goto statement", "5.2"),
    (re.compile(r"::\s*\w+\s*::"), "goto label", "5.2"),
    (re.compile(r"\bgoto\b\s*$"), "goto statement", "5.2"),
    (re.compile(r"[^/]//[^/]"), "integer division //", "5.3"),
    (re.compile(r"[\w\)\]]\s*<<\s*[\w\(]"), "left shift <<", "5.3"),
    (re.compile(r"[\w\)\]]\s*>>\s*[\w\(]"), "right shift >>", "5.3"),
    (re.compile(r"\bmath\.(tointeger|type|ult)\b"), "math integer helper", "5.3"),
    (re.compile(r"\btable\.(move|pack|unpack)\b"), "table helper", "5.2"),
    (re.compile(r"\bos\.exit\s*\(\s*\w+\s*,"), "os.exit second argument", "5.2"),
    (re.compile(r"\brawlen\b"), "rawlen", "5.2"),
    (re.compile(r"<\s*(const|close)\s*>"), "attribute syntax", "5.4"),
]

# `--` comments are stripped before matching so a comment *about* goto — such as
# the one warning future readers off it — does not fail the check that exists to
# enforce that warning.
COMMENT = re.compile(r"--.*$")


def scan(path: pathlib.Path) -> list[str]:
    findings: list[str] = []
    for number, raw in enumerate(path.read_text(errors="replace").splitlines(), 1):
        line = COMMENT.sub("", raw)
        if not line.strip():
            continue
        for pattern, what, version in FORBIDDEN:
            if pattern.search(line):
                findings.append(
                    f"{path}:{number}: {what} is Lua {version}; "
                    f"Civilization VI runs 5.1\n    {raw.strip()[:100]}"
                )
    return findings


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent / "mod"
    files = sorted(root.rglob("*.lua"))
    if not files:
        print(f"no .lua files under {root}", file=sys.stderr)
        return 1
    findings = [f for path in files for f in scan(path)]
    for finding in findings:
        print(finding)
    print(
        f"checked {len(files)} file(s) for constructs newer than Lua 5.1: "
        f"{len(findings)} finding(s)"
    )
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
