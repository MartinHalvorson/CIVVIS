#!/usr/bin/env python3
"""Every toggle no screen can reach carries a reason, and the reason cannot rot.

`docs/EVAL_STATUS.md` publishes an OVER-count on purpose: *"some toggles are
host-only or bundle plumbing that no native screen could price and nothing here
tries to tell them apart. So the last number is a ceiling on the work, not a
floor."* A ceiling nobody has examined is the same shape as the defect the
section was published for — `precise_evacuation` shipped ON for every seat,
held roughly half the simulator's main thread, and had no gene row, no
evaluator arm and no mention in any round, and nothing said so.

`docs/genome_reach_debt.json` is the examination: one line per unreachable
toggle saying why it is not a gene. This test is what keeps it true. The file
must cover **exactly** the list `eval_manifest.genome_coverage` computes —
so a toggle that gains a gene row must LOSE its entry here, and a toggle that
arrives unreachable must gain one. Neither a stale reason nor a silent new
debt survives a run.

Discovered, never listed: the expected set is recomputed from the repository's
own tables on every run.
"""

import json
import unittest
from pathlib import Path

import eval_manifest

ROOT = Path(__file__).resolve().parent.parent
DEBT = ROOT / "docs" / "genome_reach_debt.json"

# The groups the file's own `_doc` defines. A new group is a deliberate edit
# here, not a typo that quietly creates a category of one.
GROUPS = {"bundle", "host-only", "live-bridge-row", "production-on",
          "configured-on", "infrastructure", "already-on"}

# Long enough to be a reason. `test_ci_wiring.py` and `gene_fires.py` use the
# same bar, well clear of "host-only" or "n/a".
REASON_CHARACTERS = 60


def recorded() -> dict[str, dict[str, str]]:
    return json.loads(DEBT.read_text(encoding="utf-8"))["toggles"]


def unreachable() -> list[str]:
    return eval_manifest.genome_coverage(ROOT)["unreachable_toggles"]


class TheResidualIsExamined(unittest.TestCase):
    def test_the_scrape_found_something(self):
        """A census that returns nothing is a broken census, not a clean repo."""
        self.assertGreater(len(unreachable()), 0)
        self.assertGreater(len(recorded()), 0)

    def test_every_unreachable_toggle_has_a_reason(self):
        missing = sorted(set(unreachable()) - set(recorded()))
        self.assertEqual(
            missing, [],
            "these toggles are unreachable by any screen and nothing says why. "
            "Give each a line in docs/genome_reach_debt.json, or wire it as a "
            "gene: " + ", ".join(missing))

    def test_no_reason_outlives_its_toggle(self):
        stale = sorted(set(recorded()) - set(unreachable()))
        self.assertEqual(
            stale, [],
            "these have a reason for being unreachable and are not unreachable "
            "any more (or no longer exist). Delete the entry: "
            + ", ".join(stale))

    def test_every_reason_is_a_reason(self):
        for toggle, entry in recorded().items():
            with self.subTest(toggle=toggle):
                self.assertIn(entry["group"], GROUPS)
                self.assertGreater(len(entry["reason"].strip()),
                                   REASON_CHARACTERS)

    def test_the_published_count_and_this_file_agree(self):
        """The number `docs/EVAL_STATUS.md` prints is the number examined here."""
        coverage = eval_manifest.genome_coverage(ROOT)
        self.assertEqual(coverage["unreachable_by_any_screen"], len(recorded()))


if __name__ == "__main__":
    unittest.main()
