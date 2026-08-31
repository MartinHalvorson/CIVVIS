#!/usr/bin/env python3
"""Every `--bin NAME` a document tells you to run must be a binary this tree ships.

⚠⚠ THIS CLASS OF DEFECT HAS ALREADY COST A SESSION. #1278 removed 31 binaries
for having "zero tests and zero invocations". The audit's question — who calls
it in the tree — had the answer *nobody*, and it was the wrong question:
everything depending on those tools depended on them **in prose**, which no
grep for callers can see. `victory_eval` was the first line of the battery at
the top of `docs/EVAL.md`, was cited by `src/elo.rs` for its turn limits, and
was the source of the measurement that justified which victory the live agent
plays for. It was restored in #1876 — but only because somebody tried to run it
and found it missing.

A prose citation of a tool that no longer exists is often fine: a record of what
was measured stays true after its instrument is retired. What is never fine is a
**runnable command** for a binary that cannot be built, because the reader
discovers it only by running it. This gate draws exactly that line: it reads
command lines, not mentions.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DOCS = REPO / "docs"

# `cargo run ... --bin NAME` and bare `--bin NAME` inside a command line.
BIN_FLAG = re.compile(r"--bin\s+([A-Za-z_][A-Za-z0-9_]*)")

# A binary a doc may name without shipping a source file for it: the ones
# `Cargo.toml` declares with an explicit path, and the feature-gated closed
# experiments, which are deliberately outside `src/bin`.
BIN_TABLE = re.compile(r'^\s*name\s*=\s*"([^"]+)"\s*$', re.MULTILINE)


def shipped_binaries() -> set[str]:
    """Every binary name `cargo build` could resolve."""
    names = {path.stem for path in (REPO / "src" / "bin").glob("*.rs")}
    names |= {path.stem for path in (REPO / "experiments" / "closed").glob("*.rs")}
    # `[[bin]]` entries carry names that need not match any path under src/bin —
    # `civvis` is `src/main.rs`.
    manifest = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    for block in manifest.split("[[bin]]")[1:]:
        found = BIN_TABLE.search(block.split("[", 1)[0] if "[" in block else block)
        if found:
            names.add(found.group(1))
    return names


def documented_binaries() -> dict[str, list[str]]:
    """Binary names appearing in a `--bin` command, mapped to where."""
    found: dict[str, list[str]] = {}
    for doc in sorted(DOCS.rglob("*.md")):
        for number, line in enumerate(
                doc.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            for name in BIN_FLAG.findall(line):
                found.setdefault(name, []).append(
                    f"{doc.relative_to(REPO)}:{number}")
    return found


# Binaries a document may still spell out a command for, because the command is
# the record of how a published result was measured and deleting it would delete
# the method. Each must carry, in the same document, the removal PR and a warning
# that the command does not run here — so a reader meets the caveat before the
# shell does.
#
# ⚠ This is a waiver list, not an exemption: `test_a_waiver_goes_stale_when_its_
# binary_returns` fails the moment one of these is restored, and a name that is
# not here at all still fails the gate above. Adding a row means writing down why.
REMOVED_TOOLING = {
    "age_census": "#1278",
    "ai_eval": "#2351",
    "evolve_probe": "#1278",
    "policy_eval": "#1278",
}


class DocsNameOnlyBinariesThatExist(unittest.TestCase):
    def test_every_documented_bin_command_can_actually_run(self):
        shipped = shipped_binaries()
        missing = {
            name: where
            for name, where in documented_binaries().items()
            if name not in shipped and name not in REMOVED_TOOLING
        }
        self.assertEqual(
            missing, {},
            "these documents give a `--bin` command for a binary this tree does "
            "not ship, so a reader following them gets a build error. Either "
            "restore the binary (see #1876), rewrite the line so it records what "
            "was measured instead of instructing a run, or add it to "
            "REMOVED_TOOLING with the PR that removed it and a warning in the "
            "document itself.")

    def test_a_waived_command_warns_the_reader_in_its_own_document(self):
        """The waiver buys nothing if the reader still meets the command first."""
        documented = documented_binaries()
        for name, removal in REMOVED_TOOLING.items():
            for location in documented.get(name, []):
                doc = REPO / location.split(":", 1)[0]
                text = doc.read_text(encoding="utf-8", errors="replace")
                with self.subTest(name=name, doc=doc.name):
                    self.assertIn(
                        removal, text,
                        f"{doc.name} spells out a `{name}` command without naming "
                        f"the PR that removed it")
                    self.assertIn(
                        "does not run against this tree", text,
                        f"{doc.name} spells out a `{name}` command without warning "
                        f"that it cannot run")

    def test_a_waiver_goes_stale_when_its_binary_returns(self):
        """Restoring a binary must retire its waiver, not leave it lying around
        to mask the next removal."""
        shipped = shipped_binaries()
        returned = sorted(name for name in REMOVED_TOOLING if name in shipped)
        self.assertEqual(
            returned, [],
            "these binaries are shipped again, so their REMOVED_TOOLING rows are "
            "stale and would hide a future removal; drop the rows and the "
            "documents' warnings with them")

    def test_the_gate_can_see_a_binary_that_is_only_in_cargo_toml(self):
        """`civvis` is `src/main.rs` with a `[[bin]]` name, and the docs are full
        of `--bin civvis`. A gate that missed it would be unusable."""
        self.assertIn("civvis", shipped_binaries())

    def test_the_gate_reads_commands_rather_than_mentions(self):
        """A retired instrument may still be named in a record of what it
        measured; only a runnable command is a broken instruction."""
        self.assertEqual(BIN_FLAG.findall("measured with `search_probe --outcome`"), [])
        self.assertEqual(
            BIN_FLAG.findall("cargo run --release --bin mapdump -- --maps 4"),
            ["mapdump"])


if __name__ == "__main__":
    unittest.main()
