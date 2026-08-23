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

    def test_every_screenable_kind_is_read(self):
        genes = gene_fires.gene_tables()
        self.assertGreater(len(genes), 90, "the scrape found almost nothing")
        # One known inhabitant of each screenable kind, so a scrape that
        # silently lost a kind fails here instead of under-reporting forever.
        self.assertEqual(genes["barbarian-hunt"], "Kind::Repair(Axis::War)")
        self.assertEqual(genes["strategic-wonders"], "Kind::Production")
        self.assertEqual(genes["holy-lane-parity"], "Kind::OptIn")
        self.assertNotIn("land-grab", genes, "a plain host-only gene is never screened")

    def test_a_new_row_reaches_the_gate_without_touching_this_tool(self):
        """The property that makes the gate survive the next gene."""
        registry = gene_fires.REGISTRY.read_text(encoding="utf-8")
        anchor = '    Gene { tag: "promote-when-wounded",'
        added = registry.replace(
            anchor,
            '    Gene { tag: "a-brand-new-gene", field: "a_brand_new_gene", kind: Kind::OptIn, '
            "enable: AdvancedAi::enable_a_brand_new_gene, disable: AdvancedAi::disable_a_brand_new_gene },\n"
            + anchor)
        self.assertNotEqual(added, registry, "the anchor row moved; fix this test")
        with TemporaryDirectory() as tmp:
            fake = Path(tmp) / "genes.rs"
            fake.write_text(added, encoding="utf-8")
            with mock.patch.object(gene_fires, "REGISTRY", fake):
                self.assertIn("a-brand-new-gene", gene_fires.gene_tables())

    def test_a_broken_scrape_raises_rather_than_reporting_zero(self):
        with TemporaryDirectory() as tmp:
            empty = Path(tmp) / "genes.rs"
            empty.write_text("// nothing here\n", encoding="utf-8")
            with mock.patch.object(gene_fires, "REGISTRY", empty):
                with self.assertRaises(SystemExit):
                    gene_fires.gene_tables()


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


class ProbesAreNotLedgerSources(unittest.TestCase):
    """Three map pairs must never price a gene, and only a name stops them.

    A single-gene probe runs at the screen's own profile, so its file says
    `"shape": "standard"` and `tools/gene_ledger.py`'s shape guard — the one
    that refuses a probe — would let it through. What actually keeps it out is
    that the ledger takes its sources by name. That is a convention until
    something checks it, and eighteen seat pairs entering the ledger would
    move a default.
    """

    def test_no_fires_probe_is_a_ledger_source(self):
        ledger = json.loads(
            (gene_fires.ROOT / "docs" / "gene_ledger.json").read_text(
                encoding="utf-8"))
        sources = [source["path"] for source in ledger["sources"]]
        probes = [path for path in sources if "/fires/" in path]
        self.assertEqual(
            probes, [],
            "a fires probe is being used to price genes; it is three map "
            "pairs and proves only that the gene moves a game: " + str(probes))

    def test_a_probe_is_small_enough_that_this_matters(self):
        """If a probe ever grows into a real screen, revisit the guard above."""
        directory = gene_fires.SCREENS / "fires"
        if not directory.is_dir():
            self.skipTest("no probes committed yet")
        for path in sorted(directory.glob("*.json")):
            with self.subTest(probe=path.name):
                probe = json.loads(path.read_text(encoding="utf-8"))
                self.assertLess(probe["complete_pairs"], 200)


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
