"""The heuristic gene ranking is derived from the ledger's sources and must
not fall behind them."""
from __future__ import annotations

import io
import json
import math
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gene_ledger  # noqa: E402
import heuristic_gene_ranking as ranking  # noqa: E402


class TheTableIsDerived(unittest.TestCase):
    EXPECTED_COLUMNS = (
        "| Rank | Gene | Description | Default | ± Wins / 10k seats | ± Wins / 10k seats prior | "
        "Total (on) Win rate | Total (off) Win rate | Diff | "
        "Posterior (95% CI) | P(>0) | Share Δpp (z) | "
        "cost (compute) | cost (time) |"
    )

    def test_the_checked_in_table_matches_the_ledgers_sources(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.assertEqual(
            ranking.render(ledger),
            ranking.RANKING_MD.read_text(),
            "HEURISTIC_GENE_RANKING.md is stale: run tools/heuristic_gene_ranking.py --write",
        )

    def test_every_screenable_gene_is_visible(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_sources(ledger)
        text = ranking.RANKING_MD.read_text()
        for tag in ranking.screenable_tags():
            if tag in measured:
                self.assertIn(f"`{tag}`", text, tag)
            else:
                self.assertIn("## Awaiting measurement", text)
                self.assertIn(f"| `{tag}` | off (unmeasured) |", text, tag)
        self.assertNotIn("`step-and-reassess` | ", text, "a host-only flag is not ranked")

    def test_descriptions_come_from_the_toggle_docs(self):
        desc = ranking.descriptions()
        self.assertGreater(len(desc), 50)
        self.assertTrue(desc["recon-replacement"].startswith("Rebuild the recon arm"))
        self.assertTrue(desc["loyalty-rate-alarm"].startswith("Rank loyalty emergencies"))

    def test_operator_columns_are_in_the_requested_order(self):
        header = next(
            line for line in ranking.RANKING_MD.read_text().splitlines()
            if line.startswith("| Rank |")
        )
        self.assertEqual(header, self.EXPECTED_COLUMNS)

    def _ranked_rows(self):
        """Every ranked row of the main table, split into its cells."""
        lines = ranking.RANKING_MD.read_text().splitlines()
        start = lines.index(self.EXPECTED_COLUMNS) + 2
        rows = []
        for line in lines[start:]:
            if not line.startswith("| "):
                break
            rows.append([c.strip() for c in line.strip().strip("|").split(" | ")])
        return rows

    def test_diff_is_the_on_rate_minus_the_off_rate(self):
        """The column that replaced the pooled seat count (operator, 2026-08-22).

        It is the WHOLE on−off difference, so it sits at roughly twice the
        scale of the win columns beside it and must be judged against a
        screen's difference band, not the halved column band the table prints.
        """
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_sources(ledger)
        rows = self._ranked_rows()
        self.assertGreater(len(rows), 50)
        for cells in rows:
            history = measured[cells[1].strip("`")]
            on_seats = sum(m["n_on"] for m in history)
            off_seats = sum(m["n_off"] for m in history)
            on = sum(m["win_on"] * m["n_on"] for m in history) / on_seats
            off = sum(m["win_off"] * m["n_off"] for m in history) / off_seats
            self.assertEqual(cells[8], ranking.diff_cell(history), cells[1])
            self.assertRegex(cells[8], r"^-?\d+\.\d\d%$", cells[1])
            # Taken off the unrounded rates, so it can land a hundredth away
            # from subtracting the two printed cells by eye — 0.01% against a
            # band of half a point. Never further: that would be a real slip.
            shown = float(cells[6].split("%")[0]) - float(cells[7].split("%")[0])
            self.assertAlmostEqual(100 * (on - off), shown, delta=0.011, msg=cells[1])
        self.assertNotIn("Total seats (on+off)", ranking.RANKING_MD.read_text())

    def test_diff_cell_is_a_percent_and_keeps_a_negative_sign(self):
        def arms(on, off):
            return [{"win_on": on, "win_off": off, "n_on": 1000, "n_off": 1000}]

        self.assertEqual(ranking.diff_cell(arms(0.17, 0.15)), "2.00%")
        self.assertEqual(ranking.diff_cell(arms(0.15, 0.17)), "-2.00%")

    def test_the_printed_diff_is_the_figure_the_ledger_vetoes_on(self):
        """One arithmetic, not two: the *Diff* cell and `win_diff_pp` are the
        same call, so a gene cannot read positive in the table while the
        deployment rule sees a negative record."""
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        measured, _ = ranking.load_sources(ledger)
        recorded = {g["tag"]: g["win_diff_pp"] for g in ledger["genes"]}
        self.assertGreater(len(recorded), 50)
        for cells in self._ranked_rows():
            tag = cells[1].strip("`")
            self.assertEqual(cells[8], f"{recorded[tag]:.2f}%", tag)
            self.assertEqual(recorded[tag], ranking.pooled_win_diff_pp(measured[tag]), tag)

    def test_each_win_rate_cell_carries_its_own_sample_size(self):
        """`n` is per arm, not one pooled figure: the arms are equal only while
        every screen that measured a gene split them evenly, and the row reads
        them from `n_on`/`n_off` separately so an uneven screen shows up."""
        for cells in self._ranked_rows():
            for cell in (cells[6], cells[7]):
                self.assertRegex(cell, r"^\d+\.\d\d% \(n=[\d,]+\)$", cells[1])

    def test_descriptions_print_whole(self):
        """The Description column was widened 160 → 480 characters, which is
        past the longest first sentence in the registry — so nothing clips."""
        longest = max(len(d) for d in ranking.descriptions().values())
        self.assertLess(longest, ranking.DESCRIPTION_CHARS)
        for cells in self._ranked_rows():
            self.assertNotIn("\u2026", cells[2], cells[1])

    def test_cost_uses_the_newest_real_measurement_and_never_invents_zero(self):
        history = [
            {"compute_cost_pct": 90.0, "compute_cost_se_pct": 4.0},
            {"compute_cost_pct": None, "compute_cost_se_pct": None},
            {"compute_cost_pct": 1.234, "compute_cost_se_pct": 0.456},
        ]
        self.assertEqual(
            ranking.cost_cell(history, "compute_cost_pct", "compute_cost_se_pct"),
            "+1.23% ±0.46%",
        )
        self.assertEqual(
            ranking.cost_cell([{}], "compute_cost_pct", "compute_cost_se_pct"),
            "–",
        )

    def test_the_band_is_the_columns_own_scale_not_the_differences(self):
        """A column is half the on−off difference, so its band is half too.

        The regression this guards: the header quoted ±110/10k — correct for
        `win_delta_pp`, twice too wide for the column beside it — and #2266
        removed eight genes calling readings up to that "inside the noise".
        """
        self.assertAlmostEqual(ranking.column_se(1.0), ranking.PER / 200.0)
        # Proved from a screen's own numbers rather than asserted: a foldover
        # holds the arms symmetric about chance, so `column / column_se` must
        # reproduce the screen's `win_z` exactly, which it can only do if the
        # column and its error are on the same (halved) scale.
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        source = ledger["sources"][-1]
        data = json.loads((ranking.ROOT / source["path"]).read_text())
        chance = 1.0 / int(data["profile"]["players"])
        for gene in data["genes"]:
            column = (float(gene["win_on"]) - chance) * ranking.PER
            se = ranking.column_se(float(gene["win_se_pp"]))
            self.assertAlmostEqual(column / se, float(gene["win_z"]), places=6, msg=gene["tag"])

    def test_every_screen_prints_its_own_band_and_its_shape(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        rows = ranking.resolutions(ledger)
        self.assertEqual(len(rows), len(ledger["sources"]))
        text = ranking.RANKING_MD.read_text()
        for row in rows:
            # ⭐ The shape is printed beside the band because a `legacy` row is
            # a reading from the retired Pangaea instrument, not from the
            # screen the ledger now accepts.
            self.assertIn(f"`{row['name']}` | {row['shape']} | {row['genes']} |", text, row["name"])
        # Screens are not interchangeable, so no single number serves — and
        # none of them is the retired ±110.
        self.assertGreater(len({round(r["band"]) for r in rows}), 1)
        self.assertTrue(all(round(r["band"]) != 110 for r in rows))
        self.assertIn("twice too wide", text)

    def test_the_band_is_not_explained_by_gene_count(self):
        """The table's own rows make gene count look causal; this holds the
        falsifier the prose cites, so neither can rot silently.

        `h1` carries ONE gene over 7,200 pairs and still resolves wider, at a
        lower pairing gain, than four-gene `s6` over 6,000 — because its gene
        changes nearly every game and `s7`'s rarely fires. A foldover cancels
        only what the arms play in common.
        """
        def read(name):
            data = json.loads((ranking.ROOT / "docs" / "gene_screens" / name).read_text())
            errors = sorted(ranking.column_se(float(g["win_se_pp"])) for g in data["genes"])
            median = errors[len(errors) // 2]
            pairs = int(data["complete_pairs"])
            players = int(data["profile"]["players"])
            gain = ranking.unpaired_constant(players) / (median * (pairs ** 0.5))
            return len(errors), pairs, ranking.POWER_80 * median, gain

        h1 = read("2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json")
        s6 = read("2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json")
        self.assertLess(h1[0], s6[0], "h1 must carry fewer genes")
        self.assertGreater(h1[1], s6[1], "h1 must carry more pairs")
        self.assertGreater(h1[2], s6[2], "yet h1 must resolve WIDER — the whole point")
        self.assertLess(h1[3], s6[3], "and at a lower pairing gain")
        self.assertIn("Pairing gain", ranking.RANKING_MD.read_text())


    def test_the_table_is_the_first_thing_in_the_file(self):
        """Operator, 2026-08-22: "i want the table on top."

        Twenty-two lines of preamble used to stand between the title and the
        first row. The reference did not go away — it is carried under the
        tables — but nothing may get back in front of them.
        """
        lines = ranking.RANKING_MD.read_text().splitlines()
        self.assertEqual(lines[0], "# The heuristic gene ranking")
        self.assertEqual(lines[1], "")
        self.assertTrue(lines[2].startswith("| Rank | Gene |"), lines[2])
        self.assertTrue(lines[3].startswith("|---:|"), lines[3])
        self.assertTrue(lines[4].startswith("| 1 | `"), lines[4])

    def test_the_reference_is_carried_under_the_tables_not_deleted(self):
        """Moving the preamble must not become dropping it.

        Every derived paragraph the header used to open with is load-bearing —
        the band correction in particular is why a culled gene came back — so
        each is asserted present, and after the last table rather than before
        the first.
        """
        text = ranking.RANKING_MD.read_text()
        self.assertIn("## How to read this", text)
        for phrase in (
            "Reading the table",
            "What each screen resolves",
            "Pairing gain",
            "twice too wide",
            "**Cost.**",
            "Regenerate with",
        ):
            self.assertIn(phrase, text, phrase)
        self.assertLess(
            text.index("| Rank | Gene |"),
            text.index("## How to read this"),
            "the reference must sit under the table, not over it",
        )
        for heading in ("## Awaiting measurement", "## Removed from the code"):
            if heading in text:
                self.assertLess(text.index(heading), text.index("## How to read this"), heading)


class ThePosteriorIsPublishedAndNotInForce(unittest.TestCase):
    """The precision-weighted posterior: printed beside the win columns,
    deciding nothing, with the delta it would make published under it."""

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.measured, _ = ranking.load_sources(self.ledger)
        self.text = ranking.RANKING_MD.read_text()

    def _rows(self):
        lines = self.text.splitlines()
        start = lines.index(TheTableIsDerived.EXPECTED_COLUMNS) + 2
        rows = []
        for line in lines[start:]:
            if not line.startswith("| "):
                break
            rows.append([c.strip() for c in line.strip().strip("|").split(" | ")])
        return rows

    def test_the_printed_posterior_is_the_figure_the_ledger_records(self):
        """One arithmetic, not two — the same rule the *Diff* column follows.
        A gene cannot print one interval here and have the switch read
        another."""
        recorded = {g["tag"]: g for g in self.ledger["genes"]}
        seen = 0
        for cells in self._rows():
            tag = cells[1].strip("`")
            posterior = ranking.posterior_of(self.measured[tag])
            self.assertEqual(cells[9], ranking.posterior_cell(posterior), tag)
            self.assertEqual(cells[10], ranking.probability_cell(posterior), tag)
            self.assertEqual(posterior["effect"], recorded[tag]["posterior_pp"], tag)
            self.assertEqual(posterior["se"], recorded[tag]["posterior_se_pp"], tag)
            seen += 1
        self.assertGreater(seen, 50)

    def test_the_shrinkage_is_visible_in_the_probability_not_the_point(self):
        """The operator's own framing: a +30 from a ±64 screen and a +30 from
        a ±29 screen must not read the same. They print the same point and
        different `P(>0)`, which is the column that decides anything."""
        wide = ranking.posterior_of([{"win_delta_pp": 0.6, "win_se_pp": 0.4576,
                                      "shape": "legacy"}])
        tight = ranking.posterior_of([{"win_delta_pp": 0.6, "win_se_pp": 0.2052,
                                       "shape": "legacy"}])
        self.assertEqual(ranking.posterior_cell(wide).split(" ")[0],
                         ranking.posterior_cell(tight).split(" ")[0])
        self.assertNotEqual(ranking.probability_cell(wide),
                            ranking.probability_cell(tight))

    def test_nothing_the_posterior_publishes_decides_a_default(self):
        """★ The hard constraint of this change. The ranking's *Default*
        column is the ledger's, under the threshold rule, gene for gene."""
        recorded = {g["tag"]: g for g in self.ledger["genes"]}
        self.assertEqual(self.ledger["rules"]["authority"], "columns")
        for cells in self._rows():
            gene = recorded[cells[1].strip("`")]
            self.assertEqual(
                cells[3], "**on**" if gene["default_on"] else "off", cells[1])
            self.assertEqual(
                gene["default_on"],
                gene_ledger.default_from_columns(
                    gene["wins_last_10k"], gene["wins_prior_10k"],
                    gene["win_diff_pp"]),
                cells[1])

    def test_every_authority_is_published_with_what_it_would_ship(self):
        self.assertIn("## What the posterior would change", self.text)
        for candidate in ranking.AUTHORITIES:
            self.assertIn(f"| `{candidate}`", self.text, candidate)
        self.assertIn("`columns` **(in force)**", self.text)
        rows = ranking.authority_table(self.ledger, self.measured)
        for candidate in ranking.AUTHORITIES:
            self.assertEqual(
                sum(r[f"would/{candidate}"] for r in rows),
                self.ledger["counts"][f"default_on_under_{candidate}"],
                candidate)

    def test_the_three_way_call_covers_every_priced_gene(self):
        rows = ranking.authority_table(self.ledger, self.measured)
        calls = [r["call"] for r in rows]
        self.assertEqual(len(rows), len(calls))
        self.assertGreater(calls.count("on"), 0)
        self.assertGreater(calls.count("unresolved"), calls.count("on"))
        self.assertIn("### What the posterior can decide at all", self.text)
        for row in rows:
            if row["call"] != "unresolved":
                self.assertIn(f"| `{row['tag']}` |", self.text, row["tag"])
        # A host-only flag carries a ledger row from a retired native
        # stand-in and the ledger never governs it, so it is in no table here.
        self.assertNotIn("step-and-reassess", {r["tag"] for r in rows})

    def test_the_shapes_are_published_apart(self):
        self.assertIn("## The two shapes, apart", self.text)
        self.assertIn("| standard |", self.text)
        self.assertIn("| legacy |", self.text)
        # Today every source is legacy; the file must say so rather than let a
        # reader take a Pangaea column for the deployment shape.
        shapes = {s["shape"] for s in self.ledger["sources"]}
        if shapes == {"legacy"}:
            self.assertIn("No `standard` source is in the ledger yet", self.text)


class TheBoundarySet(unittest.TestCase):
    """`--boundary`: the genes whose interval straddles the decision line,
    ranked by what one single-gene direct arm would buy, printed as an
    argument list."""

    maxDiff = None

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.measured, _ = ranking.load_sources(self.ledger)

    def test_the_boundary_is_exactly_the_straddling_intervals(self):
        rows, arm = ranking.boundary_table(self.ledger, self.measured)
        self.assertIsNotNone(arm)
        every = ranking.authority_table(self.ledger, self.measured)
        self.assertEqual({r["tag"] for r in rows},
                         {r["tag"] for r in every if r["call"] == "unresolved"})
        for row in rows:
            self.assertLessEqual(row["posterior"]["lo"], 0.0, row["tag"])
            self.assertGreaterEqual(row["posterior"]["hi"], 0.0, row["tag"])

    def test_it_is_ranked_by_what_an_arm_buys_and_the_top_is_a_disagreement(self):
        rows, _ = ranking.boundary_table(self.ledger, self.measured)
        self.assertEqual([r["buys"] for r in rows],
                         sorted((r["buys"] for r in rows), reverse=True))
        # An arm buys most where the genome and the evidence disagree: the
        # leader is held OFF by the rule while the posterior leans positive.
        top = rows[0]
        self.assertFalse(top["shipped"], top["tag"])
        self.assertGreater(top["posterior"]["effect"], 0.0, top["tag"])

    def test_a_bigger_arm_buys_more_and_resolves_more(self):
        small, _ = ranking.boundary_table(self.ledger, self.measured, arm_pairs=2000)
        large, _ = ranking.boundary_table(self.ledger, self.measured, arm_pairs=40000)
        by_small = {r["tag"]: r["buys"] for r in small}
        for row in large:
            self.assertGreaterEqual(row["buys"] + 1e-9, by_small[row["tag"]], row["tag"])

    def test_the_output_is_a_genes_argument_list(self):
        out = io.StringIO()
        with redirect_stdout(out):
            ranking.print_boundary(self.ledger, ranking.ARM_PAIRS,
                                   ranking.FEASIBLE_ARM_PAIRS)
        text = out.getvalue()
        self.assertIn("boundary genes ·", text)
        line = next(l for l in text.splitlines() if l.startswith("--genes "))
        tags = line.removeprefix("--genes ").split(",")
        self.assertLessEqual(len(tags), ranking.BOUNDARY_SUGGESTIONS)
        known = set(ranking.screenable_tags())
        for tag in tags:
            self.assertIn(tag, known, tag)
        # Only genes one batch could actually resolve are proposed.
        needs = {r["tag"]: r["needs"]
                 for r in ranking.boundary_table(self.ledger, self.measured)[0]}
        for tag in tags:
            self.assertLessEqual(needs[tag], ranking.FEASIBLE_ARM_PAIRS, tag)
        self.assertIn("gene_screen --genes ", ranking.RANKING_MD.read_text())

    def test_the_two_stage_arithmetic_is_recorded_where_it_will_be_read(self):
        """⚠ The efficient plan is two stage and NOT a partial foldover. The
        ranking says so; `docs/GENE_SCREEN.md` carries the arithmetic."""
        self.assertIn("two stage", ranking.RANKING_MD.read_text())
        screen = (ranking.ROOT / "docs" / "GENE_SCREEN.md").read_text()
        for phrase in ("two-stage", "±145", "partial", "blocked", "8× the games"):
            self.assertIn(phrase, screen, phrase)

    def test_the_eight_times_figure_is_the_screens_own_arithmetic(self):
        """±146 is not quoted, it is derived — and this recomputes it from the
        ledger's own screens so the paragraph cannot rot.

        Split p10's budget into one screen per gene: 17,574 / 75 = 234 pairs
        each. Even at the best single-gene pairing gain the repository has
        measured (`s7`'s 3.32× against p10's 1.09×) that resolves ±146, which
        is 2.9× wider than p10's ±51 — 8× the games for the same band.
        """
        rows = {r["name"].split("-native")[0].split("-holy")[0].split("-idle")[0]:
                r for r in ranking.resolutions(self.ledger)}
        p10 = next(r for k, r in rows.items() if "p10" in k)
        s7 = next(r for k, r in rows.items() if "s7" in k)
        self.assertEqual(round(p10["band"]), 51)
        self.assertEqual(round(s7["band"]), 29)
        per_gene_pairs = p10["pairs"] / p10["genes"]
        self.assertAlmostEqual(per_gene_pairs, 234.32, places=1)
        # p10's own error per pair, improved by the single-gene gain ratio.
        constant = p10["se"] * math.sqrt(p10["pairs"]) * p10["gain"] / s7["gain"]
        band = ranking.POWER_80 * constant / math.sqrt(per_gene_pairs)
        self.assertAlmostEqual(band, 145, delta=1)
        self.assertAlmostEqual(band / p10["band"], 2.84, places=2)
        # Error falls with the square root of the games, so 2.84x the width is
        # 8.1x the games for the same band.
        self.assertAlmostEqual((band / p10["band"]) ** 2, 8.1, places=1)


class TheLaneGenes(unittest.TestCase):
    """A lane gene is discovered from the code, and its share reading is
    published beside its win columns."""

    def test_the_lane_set_is_read_off_victory_lane_rs(self):
        tags = ranking.lane_tags()
        self.assertGreaterEqual(len(tags), 6)
        for tag in ("lane-congress-ballot", "lane-great-people", "lane-policy-deck",
                    "lane-space-race", "lane-culture-spending", "lane-congress-favor"):
            self.assertIn(tag, tags, tag)
        # Discovered, not listed: a gene joins by being read in the module.
        reg = ranking.registry()
        read = ranking.LANE_MODULES[0].read_text()
        for tag in tags:
            self.assertIn(f"self.{reg[tag][0]}", read, tag)
        self.assertNotIn("wide-map-capacity", tags)

    def test_every_lane_gene_appears_with_its_axis(self):
        text = ranking.RANKING_MD.read_text()
        self.assertIn("## Lane genes and the share axis", text)
        for tag in ranking.lane_tags():
            self.assertIn(f"| `{tag}` |", text, tag)
        self.assertIn("science 0/8", text)
        self.assertIn("t283", text)

    def test_the_share_cell_carries_the_reading_and_its_verdict(self):
        self.assertEqual(ranking.share_verdict(2.0), "helps *")
        self.assertEqual(ranking.share_verdict(-2.0), "hurts *")
        self.assertEqual(ranking.share_verdict(1.99), "~")
        history = [{"share_delta_pp": -1.024, "share_z": -15.92}]
        self.assertEqual(ranking.share_cell(history), "-1.02 (z -15.92) hurts *")

    def test_the_pre_registered_rule_is_written_down_before_the_screen(self):
        screen = (ranking.ROOT / "docs" / "GENE_SCREEN.md").read_text()
        self.assertIn("Pre-registered", screen)
        self.assertIn("lane gene", screen)
        # The axis that decides is still WINS.
        self.assertIn("decision axis stays", screen)


class TheStandardScreenPreview(unittest.TestCase):
    """⭐ `docs/gene_ranking_notes.md` records what the first standard-shape
    screen says, and the ranking carries it verbatim. That screen is NOT a
    ledger source yet — no analysis JSON for it exists — so every figure in
    that note is computed by hand from its published table. This recomputes
    them, so the note cannot go stale silently while it is the only place the
    disagreement is written down.

    Source: `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`
    (PR #2323): 3,937 complete map pairs, 23,622 matched seat comparisons per
    gene, 74x46 Continents / 9 CS / Online-250, all six lanes, best-genome
    baseline, all-seats foldover, source commit `b3ad9f00`.
    """

    #: tag -> (on−off win Δpp, win z), read off that document's table.
    STANDARD = {
        "governor-victory-lanes": (-4.73, -15.37),
        "governor-every-lane": (-4.68, -15.12),
        "governor-expansion-lane": (-0.55, -1.76),
        "war-economy": (2.35, 7.50),
        "apostle-promotion-by-role": (0.32, 1.02),
        "theology-for-founders": (0.45, 1.43),
        "settler-site-agreement": (-0.46, -1.47),
        "settler-target-hysteresis": (-0.36, -1.16),
        "housing-research": (-0.35, -1.10),
        "religion-sues-peace": (-0.36, -1.14),
    }
    #: The eight defaults the pooled-`Diff` veto alone would move if that
    #: screen entered the ledger, and whether the reading behind each is a
    #: signal or a coin flip.
    VETO_FLIPS = ("governor-victory-lanes", "war-economy",
                  "settler-site-agreement", "settler-target-hysteresis",
                  "housing-research", "religion-sues-peace",
                  "apostle-promotion-by-role", "theology-for-founders")

    def setUp(self):
        self.ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.measured, _ = ranking.load_sources(self.ledger)
        self.notes = ranking.NOTES_MD.read_text()

    def reading(self, tag):
        """That screen's row as a history entry: a foldover holds the arms
        symmetric about chance, so `win_on = 1/6 + Δ/200`."""
        delta, z = self.STANDARD[tag]
        chance = 1.0 / gene_ledger.SCREEN["players"]
        return {"win_on": chance + delta / 200.0, "win_off": chance - delta / 200.0,
                "n_on": 23622, "n_off": 23622, "win_delta_pp": delta,
                "win_se_pp": abs(delta / z), "shape": "standard"}

    def pools(self, tag):
        history = self.measured[tag]
        return (gene_ledger.pooled_posterior(history, ("legacy",)),
                gene_ledger.pooled_posterior([self.reading(tag)]),
                gene_ledger.pooled_posterior(list(history) + [self.reading(tag)]))

    def test_governor_victory_lanes_is_the_largest_correctable_defect(self):
        """RESOLVED 2026-08-23. It shipped ON, promoted on P10's single +46
        column; the deployment shape read it at −237 [−267, −206]; and the
        pre-registered direct arm `g1` (600 map pairs, seeds 150000000+,
        disjoint from the whole-genome screen's maps) confirmed −4.78 pp at
        win z −6.11. The threshold rule then wrote it **off** — both clauses
        agreeing, so it does not rest on the marginal Diff veto."""
        row = next(g for g in self.ledger["genes"]
                   if g["tag"] == "governor-victory-lanes")
        self.assertFalse(row["default_on"], "g1 resolved it off")
        self.assertEqual(row["verdict"], "hurts")
        # The +46 that promoted it is now the PRIOR column; g1 is the latest.
        self.assertEqual(row["wins_last_10k"], -239)
        self.assertEqual(row["wins_prior_10k"], 46, "the column that promoted it")
        legacy, standard, pooled = self.pools("governor-victory-lanes")
        self.assertEqual((round(legacy["effect"]), round(legacy["lo"]),
                          round(legacy["hi"])), (46, 9, 82))
        self.assertEqual((round(standard["effect"]), round(standard["lo"]),
                          round(standard["hi"])), (-236, -267, -206))
        # The two intervals do not merely disagree, they do not come close.
        self.assertGreater(legacy["lo"] - standard["hi"], 200)
        # So the pool is a warning, not an answer. Assert the figure the
        # LEDGER publishes (its two screens: P10 legacy and g1 standard),
        # not a recomputation over the note's preview as well -- since
        # 2026-08-23 that would pool three readings and report a different
        # tau for a quantity nothing ships.
        self.assertEqual(round(row["posterior_tau_pp"]), 199)
        self.assertEqual(row["posterior_screens"], 2)
        self.assertEqual(gene_ledger.posterior_call(pooled["effect"], pooled["se"]),
                         "unresolved")
        self.assertEqual(gene_ledger.posterior_call(standard["effect"],
                                                    standard["se"]), "off")
        for phrase in ("-237", "[-267, -206]", "tau = 199", "-15.37"):
            self.assertIn(phrase, self.notes, phrase)

    def test_the_legacy_share_axis_already_said_it(self):
        """P10 read this gene win z +2.46 / share z −15.92 — a recorded
        `conflict`, because the rule reads the win axis only. The share axis
        was right a day before the win axis caught up, and g1's own arm now
        agrees on BOTH axes, so the conflict is gone."""
        row = next(g for g in self.ledger["genes"]
                   if g["tag"] == "governor-victory-lanes")
        # g1 is the current screen: both axes negative, no conflict left.
        self.assertFalse(row["conflict"], "both axes now agree")
        self.assertAlmostEqual(row["screen"]["win_z"], -6.11, places=2)
        self.assertAlmostEqual(row["screen"]["share_z"], -23.76, places=2)
        # P10's share axis (−15.92) landed within half a sigma of what the
        # deployment shape's WIN axis said a day later (−15.37).
        self.assertLess(abs(15.92 - abs(self.STANDARD["governor-victory-lanes"][1])),
                        0.6)
        self.assertIn("-15.92", self.notes)

    def test_the_composite_harm_is_carried_by_one_named_half(self):
        composite = self.STANDARD["governor-every-lane"][0]
        victory = self.STANDARD["governor-victory-lanes"][0]
        expansion = self.STANDARD["governor-expansion-lane"][0]
        self.assertLess(abs(composite - victory), 0.1, "the half is the composite")
        self.assertLess(abs(expansion), abs(victory) / 5, "the other half is cheap")
        self.assertLess(abs(victory + expansion - composite), 0.7, "roughly additive")
        # The harmful half was the only one that shipped, until g1 resolved
        # it off on 2026-08-23. All three governor genes now default off.
        by_tag = {g["tag"]: g for g in self.ledger["genes"]}
        self.assertFalse(by_tag["governor-victory-lanes"]["default_on"])
        self.assertFalse(by_tag["governor-every-lane"]["default_on"])
        self.assertFalse(by_tag["governor-expansion-lane"]["default_on"])

    def test_six_of_the_eight_veto_flips_are_decided_at_z_about_one(self):
        """The sharpest argument for the posterior, in the rule's own numbers:
        the veto reads the sign of a difference that carries no error, and on
        the very next screen six of its eight decisions come from |z| ≈ 1."""
        signal = [tag for tag in self.VETO_FLIPS
                  if abs(self.STANDARD[tag][1]) >= 3.0]
        noise = [tag for tag in self.VETO_FLIPS
                 if abs(self.STANDARD[tag][1]) < 2.0]
        self.assertEqual(sorted(signal), ["governor-victory-lanes", "war-economy"])
        self.assertEqual(len(noise), 6)
        for tag in noise:
            self.assertLess(abs(self.STANDARD[tag][1]), 1.5, tag)

    def test_read_standard_only_the_posterior_resolves_exactly_the_two(self):
        """And read pooled it resolves none, because tau swamps both. Both
        halves are the recommendation the note makes."""
        resolved_standard, resolved_pooled = [], []
        for tag in self.VETO_FLIPS:
            _, standard, pooled = self.pools(tag)
            if gene_ledger.posterior_call(standard["effect"], standard["se"]) != "unresolved":
                resolved_standard.append(tag)
            if gene_ledger.posterior_call(pooled["effect"], pooled["se"]) != "unresolved":
                resolved_pooled.append(tag)
        self.assertEqual(sorted(resolved_standard),
                         ["governor-victory-lanes", "war-economy"])
        self.assertEqual(resolved_pooled, [])
        self.assertIn("resolves exactly two of the eight", self.notes)
        self.assertIn('POSTERIOR_SHAPES = ("standard",)', self.notes)

    def test_the_note_is_carried_into_the_published_ranking(self):
        text = ranking.RANKING_MD.read_text()
        self.assertIn("23,622", text)
        self.assertIn("not a ledger source", text)
        self.assertIn("governor-victory-lanes", text)


if __name__ == "__main__":
    unittest.main()
