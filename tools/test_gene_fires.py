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
import copy
import hashlib
import io
import tarfile
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
        self.assertEqual(genes["camp-party"], "Kind::Repair(Axis::War)")
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
        self.assertIn("escort-unstick", fired)


class ProbesAreNotLedgerSources(unittest.TestCase):
    """Three map pairs must never price a gene, and only a name stops them.

    A single-gene probe runs at the screen's own profile, so its file says
    `"shape": "standard"` and `tools/genes.py`'s shape guard — the one
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
                if probe.get("kind") == "captured_native_action_pairs_v1":
                    # Captured reads count proposals, not played seats.
                    self.assertNotIn("seats", probe)
                    continue
                # The screen's unit is the seat (#2349); a legacy probe
                # recorded pairs, two seats each.
                seats = (probe["seats"] if "seats" in probe
                         else 2 * probe["complete_pairs"])
                self.assertLess(seats, 400)


class CapturedActionPairs(unittest.TestCase):
    def record(self):
        off = {"arm": "off", "exit": 0, "argv": ["frozen-decider", "--mirror", "/off"],
               "genome": {"treatments": ["old"], "forced": ["old"], "lane": "domination"},
               "payload": {"orders": [{"op": "hold"}]}}
        on = copy.deepcopy(off)
        on.update(arm="on", argv=["frozen-decider", "--mirror", "/on", "--with", "new"])
        on["genome"]["treatments"].append("new")
        on["genome"]["forced"].append("new")
        on["payload"]["orders"] = [{"op": "spy_travel"}]
        return {"complete": True, "reads": [{"complete": True, "bytes": 123,
                "sha256": "a" * 64, "arms": [off, on]}]}

    def evidence(self, record, artifact_change=None, corrupt=False):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "raw.tar.xz"
            raw = json.dumps(record).encode()
            with tarfile.open(archive, "w:xz") as tar:
                info = tarfile.TarInfo("result.json")
                info.size = len(raw)
                tar.addfile(info, io.BytesIO(raw))
            artifact = {"kind": "captured_native_action_pairs_v1", "tag": "new",
                        "archive": {"path": "raw.tar.xz", "sha256": hashlib.sha256(archive.read_bytes()).hexdigest()},
                        "members": ["result.json"]}
            if artifact_change:
                artifact_change(artifact)
            screens = root / "screens"
            screens.mkdir()
            (screens / "pair.json").write_text(json.dumps(artifact))
            if corrupt:
                archive.write_bytes(archive.read_bytes() + b"changed")
            with mock.patch.object(gene_fires, "ROOT", root), mock.patch.object(gene_fires, "SCREENS", screens):
                return gene_fires.firing_evidence()

    def test_raw_changed_order_proves_recorded_activation(self):
        fired, flat = self.evidence(self.record())
        self.assertIn("new", fired)
        self.assertEqual(flat, [])

    def test_identical_orders_or_only_journal_changes_do_not_prove_activation(self):
        record = self.record()
        record["reads"][0]["arms"][1]["payload"] = copy.deepcopy(record["reads"][0]["arms"][0]["payload"])
        record["reads"][0]["arms"][1]["journal"] = ["new fired"]
        self.assertNotIn("new", self.evidence(record)[0])

    def test_bad_controls_fail_closed(self):
        changes = [
            lambda r: r.update(complete=False),
            lambda r: r["reads"][0].update(sha256="missing"),
            lambda r: r["reads"][0].update(bytes=0),
            lambda r: r["reads"][0]["arms"][1].update(exit=1),
            lambda r: r["reads"][0]["arms"][1]["genome"]["treatments"].append("also-new"),
            lambda r: r["reads"][0]["arms"][1]["genome"].update(lane="science"),
            lambda r: r["reads"][0]["arms"][1]["argv"].append("--other-option"),
            lambda r: r["reads"][0]["arms"][1]["argv"].__setitem__(0, "different-binary"),
        ]
        for change in changes:
            with self.subTest(change=change):
                record = self.record()
                change(record)
                with self.assertRaises(SystemExit):
                    self.evidence(record)

    def test_corrupt_or_escaping_archive_fails_closed(self):
        with self.assertRaises(SystemExit):
            self.evidence(self.record(), corrupt=True)
        with self.assertRaises(SystemExit):
            self.evidence(self.record(), lambda a: a["archive"].update(path="../raw.tar.xz"))

    def test_missing_member_fails_closed(self):
        with self.assertRaises(SystemExit):
            self.evidence(self.record(), lambda a: a.update(members=["missing.json"]))


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
                               lambda: {"escort-unstick": "x" * 60}):
            found = gene_fires.survey()
        self.assertEqual([tag for tag, _ in found["stale_waivers"]],
                         ["escort-unstick"])

    def test_a_waiver_for_a_departed_gene_goes_stale(self):
        with mock.patch.object(gene_fires, "waivers",
                               lambda: {"a-gene-that-left": "x" * 60}):
            found = gene_fires.survey()
        self.assertEqual([tag for tag, _ in found["stale_waivers"]],
                         ["a-gene-that-left"])


class PruningProvenWaivers(unittest.TestCase):
    def test_only_waivers_a_screen_proves_are_deleted(self):
        with TemporaryDirectory() as temporary:
            waivers = Path(temporary) / "gene_fire_waivers.json"
            waivers.write_text(json.dumps({"_doc": ["kept"], "waivers": {
                "proven-gene": "x" * 50, "still-waived": "y" * 50, "retired-gene": "z" * 50,
            }}, indent=2) + "\n", encoding="utf-8")
            found = {"stale_waivers": [("proven-gene", "now proven by batch.json"),
                                       ("retired-gene", "no longer a gene")]}
            with mock.patch.object(gene_fires, "WAIVERS", waivers), \
                    mock.patch.object(gene_fires, "survey", return_value=found):
                self.assertEqual(gene_fires.prune_proven_waivers(), ["proven-gene"])
            data = json.loads(waivers.read_text(encoding="utf-8"))
            self.assertEqual(data["_doc"], ["kept"])
            self.assertEqual(set(data["waivers"]), {"still-waived", "retired-gene"})
            self.assertTrue(waivers.read_text(encoding="utf-8").endswith("}\n"))

    def test_nothing_proven_leaves_the_file_untouched(self):
        with TemporaryDirectory() as temporary:
            waivers = Path(temporary) / "gene_fire_waivers.json"
            waivers.write_text('{"waivers": {"a": "b"}}', encoding="utf-8")
            with mock.patch.object(gene_fires, "WAIVERS", waivers), \
                    mock.patch.object(gene_fires, "survey", return_value={"stale_waivers": []}):
                self.assertEqual(gene_fires.prune_proven_waivers(), [])
            self.assertEqual(waivers.read_text(encoding="utf-8"), '{"waivers": {"a": "b"}}')


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
