#!/usr/bin/env python3
"""The fires gate's own gate.

`AGENTS.md`: *a guard you add runs in the same change that adds it.*
`test_the_ratchet_is_green_now` is that clause — it runs the real
`--max 0` over the real repository, so the tool cannot ship documenting a
ratchet nobody has ever satisfied, which is precisely what `civvis_inert.py`
did for months while Poland's Winged Hussar shipped with no unique ability.

The rest holds the two halves of the tool honest: that a zero-width screen row
is read as *never fired* rather than as a measurement, and that a waiver cannot
outlive the reason it was written for.
"""

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

import gene_fires


def _screen(rows):
    return {"kind": "gene_screen_analysis", "genes": rows}


def _row(tag, **stats):
    row = {"tag": tag, "pairs": 1800}
    for key in gene_fires.STATISTICS:
        row[key] = 0.0
    row.update(stats)
    return row


class Discovery(unittest.TestCase):
    """The gene set comes from the binary's own tables, not from a list here."""

    def test_the_three_doors_are_all_read(self):
        genes = gene_fires.gene_tables()
        self.assertGreater(len(genes), 90, "the scrape found almost nothing")
        # One known inhabitant of each table, so a scrape that silently reads
        # only one of the three fails here instead of under-reporting forever.
        self.assertEqual(genes["barbarian-hunt"], "ENGINE_REPAIR_TREATMENTS")
        self.assertEqual(genes["strategic-wonders"], "PRODUCTION_TREATMENTS")
        self.assertEqual(genes["holy-lane-parity"], "PRODUCTION_OPT_INS")

    def test_a_new_opt_in_row_reaches_the_gate_without_touching_this_tool(self):
        """The property that makes the gate survive the next gene."""
        treatments = gene_fires.TREATMENTS.read_text(encoding="utf-8")
        added = treatments.replace(
            '    ("promote_when_wounded", "promote-when-wounded",',
            '    ("a_brand_new_gene", "a-brand-new-gene", '
            "AdvancedAi::enable_a_brand_new_gene),\n"
            '    ("promote_when_wounded", "promote-when-wounded",')
        self.assertNotEqual(added, treatments, "the anchor row moved; fix this test")
        with TemporaryDirectory() as tmp:
            fake = Path(tmp) / "treatments.rs"
            fake.write_text(added, encoding="utf-8")
            with mock.patch.object(gene_fires, "TREATMENTS", fake):
                self.assertIn("a-brand-new-gene", gene_fires.gene_tables())

    def test_a_broken_scrape_raises_rather_than_reporting_zero(self):
        with TemporaryDirectory() as tmp:
            empty = Path(tmp) / "treatments.rs"
            empty.write_text("// nothing here\n", encoding="utf-8")
            with mock.patch.object(gene_fires, "TREATMENTS", empty):
                with self.assertRaises(SystemExit):
                    gene_fires.gene_tables()

    def test_an_empty_screen_directory_raises(self):
        with TemporaryDirectory() as tmp:
            with mock.patch.object(gene_fires, "SCREENS", Path(tmp)):
                with self.assertRaises(SystemExit):
                    gene_fires.firing_evidence()


class WhatCountsAsFiring(unittest.TestCase):
    """A zero-width interval is the never-fired signature, not a null."""

    def _evidence(self, rows):
        with TemporaryDirectory() as tmp:
            screens = Path(tmp)
            (screens / "probe.json").write_text(json.dumps(_screen(rows)),
                                                encoding="utf-8")
            with mock.patch.object(gene_fires, "SCREENS", screens):
                return gene_fires.firing_evidence()

    def test_a_row_that_moved_is_proof(self):
        fired, flat = self._evidence([_row("moved", win_se_pp=0.7)])
        self.assertIn("moved", fired)
        self.assertEqual(flat, [])

    def test_only_the_score_share_moving_is_still_proof(self):
        """A gene can fire and not change who won. That is still firing."""
        fired, _ = self._evidence([_row("shared", share_delta_pp=0.29)])
        self.assertIn("shared", fired)

    def test_a_zero_width_row_is_not_proof_and_is_reported(self):
        fired, flat = self._evidence([_row("flat")])
        self.assertNotIn("flat", fired)
        self.assertEqual([tag for _, tag in flat], ["flat"])

    def test_a_row_with_no_statistics_is_neither(self):
        """A screen file written before these fields existed says nothing."""
        fired, flat = self._evidence([{"tag": "silent", "pairs": 10}])
        self.assertNotIn("silent", fired)
        self.assertEqual(flat, [])

    def test_the_committed_screens_prove_a_gene_the_ledger_measured(self):
        fired, _ = gene_fires.firing_evidence()
        self.assertIn("barbarian-hunt", fired)
        self.assertIn("escort-unstick", fired)


class Waivers(unittest.TestCase):
    def test_every_waiver_names_a_gene_that_exists(self):
        genes = gene_fires.gene_tables()
        for tag in gene_fires.waivers():
            self.assertIn(tag, genes,
                          f"{tag} is waived but is not a gene any more")

    def test_every_waiver_gives_a_reason(self):
        for tag, reason in gene_fires.waivers().items():
            self.assertGreater(
                len(reason.strip()), gene_fires.REASON_CHARACTERS,
                f"{tag}'s waiver is too short to be a reason")

    def test_a_waiver_goes_stale_when_its_gene_is_proven(self):
        with mock.patch.object(gene_fires, "waivers",
                               lambda: {"barbarian-hunt": "x" * 60}):
            found = gene_fires.survey()
        self.assertEqual([tag for tag, _ in found["stale_waivers"]],
                         ["barbarian-hunt"])

    def test_a_waiver_for_a_departed_gene_goes_stale(self):
        with mock.patch.object(gene_fires, "waivers",
                               lambda: {"a-gene-that-left": "x" * 60}):
            found = gene_fires.survey()
        self.assertEqual([tag for tag, _ in found["stale_waivers"]],
                         ["a-gene-that-left"])


class TheRatchet(unittest.TestCase):
    def test_the_ratchet_is_green_now(self):
        """The guard runs in the change that adds it. See the module docstring."""
        self.assertEqual(gene_fires.main.__module__, "gene_fires")
        with mock.patch("sys.argv", ["gene_fires.py", "--max", "0"]):
            self.assertEqual(gene_fires.main(), 0,
                             "the repository does not satisfy its own ratchet")

    def test_an_unproven_gene_fails_the_ratchet(self):
        with mock.patch.object(gene_fires, "waivers", dict):
            with mock.patch.object(
                    gene_fires, "firing_evidence", lambda: ({}, [])):
                with mock.patch("sys.argv", ["gene_fires.py", "--max", "0"]):
                    self.assertEqual(gene_fires.main(), 1)

    def test_the_report_without_a_ratchet_never_fails(self):
        with mock.patch.object(gene_fires, "waivers", dict):
            with mock.patch.object(
                    gene_fires, "firing_evidence", lambda: ({}, [])):
                with mock.patch("sys.argv", ["gene_fires.py"]):
                    self.assertEqual(gene_fires.main(), 0)


if __name__ == "__main__":
    unittest.main()
