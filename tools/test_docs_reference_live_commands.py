#!/usr/bin/env python3
"""A doc that quotes a removed measurement command must say so up front.

#2351 removed `ai_eval` (the paired evaluator and its arm registry) and #2357
removed `civvis league`, `civvis tournament`, `civvis arena` and
`civvis rating` with the league and the Elo ledgers. Around twenty top-level
docs are measurement archives written while those commands ran; each keeps its
commands as the record of how a result was measured, behind a banner in the
file's head saying the commands do not run against this tree
(`docs/EVAL.md`'s banner is the model).

The convention only holds if something notices when it is broken: this test
fails when any top-level `docs/*.md` references a removed command without
carrying the banner marker in its head, so stale instructions cannot
accumulate. The token contexts are deliberately conservative — `ai_eval` as a
whole word and `civvis <subcommand>` in command position — so prose like
"colleague", "arena mode" or "a rating pool" cannot false-positive.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DOCS = REPO / "docs"

# The canonical banner marker. Every archival banner ends its first sentence
# with this exact phrase, and it must sit in the head of the file where a
# reader about to copy a command will see it.
BANNER_MARKER = "does not run against this tree"
BANNER_WINDOW = 15  # lines

# Whole-word / command-position contexts only.
REMOVED_COMMAND_PATTERNS = (
    re.compile(r"\bai_eval\b"),
    re.compile(r"\bcivvis\s+(?:--\s+)?(?:league|tournament|arena|rating)\b"),
)

# Docs that reference a removed command but must not be edited into carrying
# the banner. Each entry names its reason; an entry that stops matching any
# removed-command pattern is stale and fails the allowlist test below.
ALLOWLIST = {
    # Closed archive pinned by tools/test_eval_round.py — appending to it
    # fails CI, and it already opens with both removal banners (which carry
    # the marker). Allowlisted so no future drift in this test can demand an
    # edit to a file that must not grow.
    "EVAL.md",
    # Live replacement instrument, not an archive: its one reference is the
    # aside "(so did the retired `ai_eval`)".
    "GENE_SCREEN.md",
    # Live roadmap, not an archive: its one reference records the retirement
    # itself ("the `civvis arena` Elo batches are retired (#2351, #2357 ...").
    "ROADMAP.md",
}


def references_removed_command(text: str) -> bool:
    return any(p.search(text) for p in REMOVED_COMMAND_PATTERNS)


def head(text: str) -> str:
    return "\n".join(text.split("\n")[:BANNER_WINDOW])


class RemovedCommandsCarryTheBanner(unittest.TestCase):
    def test_every_referencing_doc_carries_the_banner_in_its_head(self):
        missing = []
        for doc in sorted(DOCS.glob("*.md")):
            if doc.name in ALLOWLIST:
                continue
            text = doc.read_text(encoding="utf-8")
            if not references_removed_command(text):
                continue
            if BANNER_MARKER not in head(text):
                missing.append(doc.name)
        self.assertEqual(
            missing,
            [],
            f"these docs reference a command removed in #2351/#2357 without "
            f"the archival banner in their first {BANNER_WINDOW} lines. Add "
            f"the banner used across docs/ (it must contain "
            f"{BANNER_MARKER!r}) — or, better, cite the live instrument "
            f"(`docs/GENE_SCREEN.md`) instead of a retired command.",
        )

    def test_the_marker_matches_the_model_banner(self):
        # docs/EVAL.md is the model this convention copies; if its banner and
        # this marker ever disagree, the marker drifted, not the archive.
        text = (DOCS / "EVAL.md").read_text(encoding="utf-8")
        self.assertIn(
            BANNER_MARKER,
            head(text),
            "docs/EVAL.md's head no longer carries the banner phrase this "
            "test checks for — realign BANNER_MARKER with the shipped "
            "banner wording.",
        )

    def test_the_allowlist_is_not_stale(self):
        for name in sorted(ALLOWLIST):
            doc = DOCS / name
            self.assertTrue(doc.is_file(), f"allowlisted docs/{name} is gone")
            self.assertTrue(
                references_removed_command(doc.read_text(encoding="utf-8")),
                f"docs/{name} no longer references a removed command — drop "
                f"it from the allowlist so the exemption cannot be inherited "
                f"by future content.",
            )


class TokensStayConservative(unittest.TestCase):
    def test_command_contexts_match(self):
        for sample in (
            "run `ai_eval advanced basic --pairs 10`",
            "`civvis league --season 3`",
            "civvis tournament ran overnight",
            "`civvis -- tournament` via cargo",
            "civvis  arena --elo",
            "civvis rating repin",
        ):
            self.assertTrue(references_removed_command(sample), sample)

    def test_prose_does_not_false_positive(self):
        for sample in (
            "a colleague reviewed the writeup",
            "the arena mode testbed",
            "a rating pool that carried negative information",
            "the league table below",
            "ai_evaluate is a different symbol",
            "civvis evolve and civvis benchmark still run",
        ):
            self.assertFalse(references_removed_command(sample), sample)


if __name__ == "__main__":
    unittest.main()
