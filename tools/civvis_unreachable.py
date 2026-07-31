#!/usr/bin/env python3
"""Find engine code and ruleset rows nothing can reach.

One class of dead weight slips past the other checks. `civvis_inert.py` finds
effect *keys* with no consumer and `civvis validate` finds dangling references,
but neither notices a correct function that no production caller ever invokes.
Arms Control was the case that prompted this: a complete, correct World
Congress handler that no session could propose, because the resolution was
missing from the roster the Congress draws its slate from.

Ruleset rows are deliberately not scanned. Most tables are consumed generically
by iteration, so "the engine never spells this name" says nothing about whether
a row can be reached; `civvis validate` already reports the rows whose gates
cannot be satisfied.

Run from the repository root:

    python3 tools/civvis_unreachable.py
"""

from __future__ import annotations

import pathlib
import re
import sys

# Entry points the outside world calls: agents, the browser, the binaries.
PUBLIC_API = {
    "apply",
    "legal_actions",
    "new_with_setup",
    "positions",
    "speed_cost_mult",
    "tourism_per_turn",
    "religious_tourism_per_turn",
    "unit_base_max_moves",
    # A read-only "how dangerous is this strike" query. It shares its body with
    # `resolve_air_interceptions` through `air_interceptors`, so the number the
    # tests measure is the number the game fights with.
    "air_interception_strength",
}

# Machinery that is deliberately test-driven for now, with a FIDELITY.md entry
# explaining why. Natural disasters have describable effects and unpublished
# frequencies, so they are exercised by tests until a rate can be sourced.
DOCUMENTED_DORMANT = {
    "resolve_drought",
    "clear_drought",
    "resolve_coastal_flooding",
}


def test_mask(lines: list[str]) -> list[bool]:
    """Which lines sit inside a `#[cfg(test)]` module."""
    mask = [False] * len(lines)
    depth = 0
    started: object = None
    for index, line in enumerate(lines):
        if started is None and re.match(r"\s*#\[cfg\(test\)\]", line):
            started = "pending"
        if started == "pending" and re.search(r"\bmod\s+\w+\s*\{", line):
            started = depth + 1
        depth += line.count("{") - line.count("}")
        if isinstance(started, int):
            mask[index] = True
            if depth < started:
                started = None
    return mask


def unreachable_functions(root: pathlib.Path) -> list[tuple[str, int, int]]:
    files = sorted(root.glob("src/**/*.rs"))
    texts = {path: path.read_text().split("\n") for path in files}
    masks = {path: test_mask(lines) for path, lines in texts.items()}

    engine = root / "src/game.rs"
    definitions: dict[str, int] = {}
    for index, line in enumerate(texts[engine]):
        match = re.match(r"\s*(pub(\([a-z()]*\))?\s+)?fn\s+([a-z_0-9]+)\s*[(<]", line)
        if match and not masks[engine][index]:
            definitions.setdefault(match.group(3), index + 1)

    dead = []
    for name, defined_at in definitions.items():
        if name in PUBLIC_API or name in DOCUMENTED_DORMANT:
            continue
        production = tests = 0
        call = re.compile(r"\b" + re.escape(name) + r"\s*\(")
        for path, lines in texts.items():
            for index, line in enumerate(lines):
                if path == engine and index + 1 == defined_at:
                    continue
                if not call.search(line):
                    continue
                if masks[path][index]:
                    tests += 1
                else:
                    production += 1
        if production == 0 and tests > 0:
            dead.append((name, defined_at, tests))
    dead.sort(key=lambda entry: -entry[2])
    return dead


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    dead = unreachable_functions(root)
    print(f"{len(dead)} engine functions with no production caller:")
    for name, line, tests in dead:
        print(f"  {name:45s} src/game.rs:{line:<6} {tests} test call sites")
    return 1 if dead else 0


if __name__ == "__main__":
    sys.exit(main())
