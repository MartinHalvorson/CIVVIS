#!/usr/bin/env python3
"""What the deployed genome costs to run, gene by gene, and what each point buys.

⚠⚠ **THE DEPLOYED GENOME HAS A COMPUTE BILL AND NOTHING WAS READING IT.**
`gene_screen` has priced the runtime cost of every gene since the timing
estimator landed — `compute_cost_pct`, the percent change in wall seconds per
completed turn per enabled major seat — and `GENE_HEURISTIC_RANKING.md` prints
it in a column. Nothing adds it up, nothing compares it to the win it buys, and
nothing notices when it moves. The genome went 30 to 33 to 76 to 83 over three
weeks and no artifact in the repository says what that did to the cost of every
evaluation batch the fleet runs.

Read on 2026-08-26 against `docs/gene_ledger.json`: **76 default-on genes, of
which the twelve most expensive account for +19.3%**, and one gene —
`naval-threat-triage`, +7.87% ±0.56% — costs **more than three times the next**
and about **11.6% of a turn per point of win rate**. That is not an argument
for withdrawing it. It is an argument for the number existing somewhere a
reader can see it, which is what this file is.

## The device, and why it is this one and not a threshold

`docs/census.json` and `docs/speed_ledger.json` both solve the shape this has:
a quantity that is allowed to move, that nobody should be able to move
*silently*. Neither one holds a budget, because a budget invented without a
measurement behind it is the thing `AGENTS.md` calls a claim rather than a
check — and there is no measurement that says what a genome ought to cost.

So `genome_cost.py check` fails on exactly one condition: **the deployed gene
set has changed and the bill was not re-recorded.** Regenerating it is one
command; the diff is the signal, and it lands in the pull request that moved
the bill, next to the gene that moved it.

⚠ Deliberately NOT on the cost numbers. The reporting batches rotate several
times a day — three landed on 2026-08-26 alone — and every rotation reprices
every gene, so a check that compared the figures would be red almost
continuously and would teach the fleet to ignore it. That is the credibility
problem `rust-quality` cost this repository once already. The figures are
recorded as evidence and refreshed whenever anyone runs `write`; the *set* is
what is guarded, because a gene entering or leaving the genome is the event
that moves the bill on purpose.

⚠ **The sum is not a total and this file never calls it one.** Every reading is
a marginal cost measured with the rest of the genome enabled, so the figures do
not compose: enabling two genes that both walk the unit list costs less than
their sum, and two that contend for the same cache may cost more. Summing them
is an *indicator that tracks*, useful because it moves when the bill moves, and
it is labelled that way in the output and in the recorded file.

## Usage

    tools/genome_cost.py report          # the deployed bill, dearest first
    tools/genome_cost.py report --all    # every priced gene, on or off
    tools/genome_cost.py write           # record today's reading
    tools/genome_cost.py check           # fail if the record is stale
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# ⚠ IMPORTED, NEVER REIMPLEMENTED. `pooled_win_diff_pp`'s own docstring says
# the printed totals and the ledger's published *Diff* are "one arithmetic";
# a second copy here would be a third number claiming to be the same one.
import genes as gene_ledger_tool

ROOT = Path(__file__).resolve().parent.parent
LEDGER_JSON = ROOT / "docs" / "gene_ledger.json"
RECORD_JSON = Path(__file__).resolve().parent / "genome_cost_floor.json"

#: Where a screen's own analysis JSON keeps the per-gene timing estimate. Both
#: columns come from the same run; see `docs/GENE_SCREEN.md`.
COST = "compute_cost_pct"
COST_SE = "compute_cost_se_pct"
TIME = "time_cost_pct"

#: A reading whose one-standard-error bar is wider than the point estimate says
#: nothing about the sign, let alone the size. Kept in the table so the row is
#: not silently missing, and excluded from the summed indicator so noise cannot
#: walk it.
def resolved(point: Optional[float], se: Optional[float]) -> bool:
    return (point is not None and se is not None and math.isfinite(point)
            and math.isfinite(se) and abs(point) > se)


def history(ledger: dict) -> Dict[str, List[dict]]:
    """Per-gene measurement history — the ranking's own, not a second copy.

    ⚠⚠ `load_display_sources` is what `GENE_HEURISTIC_RANKING.md` builds every
    row from: the ledger's authoritative sources PLUS the fixed display
    batches, with a source that occupies both counted once. Walking
    `docs/gene_screens/` directly instead was the first version of this file,
    and it disagreed with the ranking's own Diff column by a rounding step on
    `boost-wait-research` (0.20pp against 0.21pp) — two numbers claiming to be
    the same one, which is the defect this file exists to avoid, committed by
    the file itself.

    ⚠ It matters that this is not `load_sources`, which is the *authoritative*
    history: that covers 44 of the 76 deployed genes. The other 32 — including
    `naval-threat-triage`, the dearest gene in the genome — are priced only by
    the display batches.
    """
    measured, _newest = gene_ledger_tool.load_display_sources(ledger)
    return measured


def newest_cost(rows: List[dict]) -> Optional[dict]:
    """The newest reading that carries a cost, oldest-first history.

    "Newest usable" rather than "newest": analysis JSON written before the
    timing estimator carries no cost at all, and a gene absent from the latest
    batch should report its last real measurement rather than a hole. Same rule
    as `genes.cost_cell`, which renders the ranking's column.
    """
    for row in reversed(rows):
        point, se = row.get(COST), row.get(COST_SE)
        if point is None or se is None:
            continue
        if math.isfinite(float(point)) and math.isfinite(float(se)):
            return {"compute_cost_pct": float(point),
                    "compute_cost_se_pct": float(se),
                    "time_cost_pct": (None if row.get(TIME) is None
                                      else float(row[TIME])),
                    "source": row.get("source")}
    return None


def bill(ledger: dict) -> dict:
    """Today's reading: the deployed set, each gene's cost, and the indicator."""
    deployed = list(ledger.get("rules", {}).get("deployment_genome", ()))
    measured = history(ledger)
    priced = {tag: found for tag, rows in measured.items()
              if (found := newest_cost(rows)) is not None}
    wins = {tag: gene_ledger_tool.pooled_win_diff_pp(rows)
            for tag, rows in measured.items() if rows}
    rows = []
    for tag in deployed:
        entry = priced.get(tag)
        win = wins.get(tag)
        row = {
            "tag": tag,
            "compute_cost_pct": None if entry is None else round(entry["compute_cost_pct"], 3),
            "compute_cost_se_pct": None if entry is None else round(entry["compute_cost_se_pct"], 3),
            # ⚠ Six places, not three: `diff_cell` renders 0.205118 as "0.21%"
            # and a value pre-rounded to 0.205 renders as "0.20%". Rounding
            # before formatting is how this file first disagreed with the
            # ranking it is supposed to agree with. `DIFF_PLACES` is 6.
            "win_diff_pp": None if win is None else round(float(win), 6),
            "resolved": bool(entry) and resolved(entry["compute_cost_pct"],
                                                 entry["compute_cost_se_pct"]),
            "source": None if entry is None else entry["source"],
        }
        row["cost_per_point"] = cost_per_point(row)
        rows.append(row)
    rows.sort(key=lambda one: (-(one["compute_cost_pct"] or 0.0), one["tag"]))
    resolved_rows = [one for one in rows if one["resolved"]]
    return {
        "what": (
            "The deployed genome's per-gene compute cost, from the screens "
            "docs/gene_ledger.json draws on. compute_cost_pct is the percent "
            "change in wall seconds per completed turn per enabled major seat "
            "(docs/GENE_SCREEN.md). Regenerate with `python3 "
            "tools/genome_cost.py write`; `check` fails when this file and the "
            "ledger disagree, so the bill cannot move without the diff showing "
            "which gene moved it."),
        "not_a_total": (
            "summed_cost_pct adds marginal costs each measured with the rest "
            "of the genome enabled. They do not compose — two genes that walk "
            "the same list cost less together than apart — so this is an "
            "indicator that TRACKS the bill, not the bill. Compare it with "
            "itself over time; never quote it as what the genome costs."),
        "deployed_genes": len(deployed),
        "priced": sum(1 for one in rows if one["compute_cost_pct"] is not None),
        "resolved_beyond_one_se": len(resolved_rows),
        "summed_cost_pct": round(sum(one["compute_cost_pct"] for one in resolved_rows), 3),
        "summed_positive_cost_pct": round(
            sum(one["compute_cost_pct"] for one in resolved_rows
                if one["compute_cost_pct"] > 0), 3),
        "genes": rows,
    }


def cost_per_point(row: dict) -> Optional[float]:
    """Percent of a turn spent per point of win rate the gene is worth.

    ⚠ Only for a gene that is measurably *both* costly and winning. A win at or
    below zero makes the ratio meaningless rather than large — dividing by a
    win of +0.01pp reports a gene as infinitely expensive when what is actually
    true is that its win is unresolved — so those return `None` and the table
    prints a dash. The threshold is the promotion rule's own: `docs/GENE_SCREEN.md`
    promotes on a Diff of at least 0.85pp, so a win below that is not a win this
    ratio is entitled to divide by.
    """
    cost, win = row.get("compute_cost_pct"), row.get("win_diff_pp")
    if not row.get("resolved") or cost is None or win is None:
        return None
    if cost <= 0 or win < PROMOTION_DIFF_PP:
        return None
    return round(cost / win, 2)


#: `docs/GENE_SCREEN.md` / #2458: a gene is promoted on a Diff of at least this.
PROMOTION_DIFF_PP = 0.85


#: A gene costing less than this per turn is not worth a reader's attention
#: whatever it wins: the median |cost| over every probe in `docs/gene_screens/`
#: is 0.37%, so this is roughly "above the middle of the distribution".
DEAR_PCT = 0.5


def dear_and_under_the_bar(reading: dict) -> List[dict]:
    """Deployed genes that cost real time and do not clear the promotion bar.

    ⭐ THE READING THIS FILE EXISTS TO PRODUCE. `docs/GENE_SCREEN.md` promotes
    on a pooled *Diff* of at least 0.85pp (#2458). A gene already deployed is
    not re-judged against that bar — the retained-selection policy carries the
    recorded genome forward — so a gene can sit in the genome at several
    percent of every turn on a win the rule would not promote it for today,
    and nothing anywhere says so.

    This is not a verdict. Several of these are operator pins, and an operator
    pin is a decision, not an oversight. It is the list a human should be shown
    before the next one is added.
    """
    return [row for row in reading["genes"]
            if row["resolved"] and (row["compute_cost_pct"] or 0) >= DEAR_PCT
            and (row["win_diff_pp"] is None or row["win_diff_pp"] < PROMOTION_DIFF_PP)]


def render(reading: dict, out=print) -> None:
    out("deployed genes: %d   priced: %d   resolved beyond 1 s.e.: %d"
        % (reading["deployed_genes"], reading["priced"],
           reading["resolved_beyond_one_se"]))
    out("summed marginal cost: %+.2f%%   (positive only %+.2f%%)  — an indicator, "
        "not a total; marginal costs do not compose"
        % (reading["summed_cost_pct"], reading["summed_positive_cost_pct"]))
    out("")
    out("%-44s %10s %9s %10s  %s" % ("gene", "cost", "win", "cost/point", "source"))
    for row in reading["genes"]:
        if row["compute_cost_pct"] is None:
            out("%-44s %10s %9s %10s  %s" % (row["tag"], "–", "–", "–", "unpriced"))
            continue
        ratio = row["cost_per_point"]
        out("%-44s %+9.2f%% %+8.2f %10s  %s%s"
            % (row["tag"], row["compute_cost_pct"], row["win_diff_pp"] or 0.0,
               ("–" if ratio is None else "%.1f" % ratio),
               (row["source"] or "")[:34],
               "" if row["resolved"] else "   (inside 1 s.e.)"))

    under = dear_and_under_the_bar(reading)
    if under:
        out("")
        out("%d deployed genes cost >= %.1f%%/turn and do NOT clear the %.2fpp "
            "promotion bar, %+.2f%% between them:"
            % (len(under), DEAR_PCT, PROMOTION_DIFF_PP,
               sum(row["compute_cost_pct"] for row in under)))
        for row in under:
            out("    %+6.2f%% for %+.2fpp   %s"
                % (row["compute_cost_pct"], row["win_diff_pp"] or 0.0, row["tag"]))
        out("  (a deployed gene is not re-judged against the bar; several of "
            "these are operator pins, which are decisions rather than oversights)")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    report = sub.add_parser("report", help="the deployed bill, dearest first")
    report.add_argument("--all", action="store_true",
                        help="every priced gene, deployed or not")
    sub.add_parser("write", help="record today's reading")
    sub.add_parser("check", help="fail if the recorded reading is stale")
    args = parser.parse_args(argv)

    ledger = json.loads(LEDGER_JSON.read_text())
    reading = bill(ledger)

    if args.command == "report":
        if args.all:
            deployed = set(ledger.get("rules", {}).get("deployment_genome", ()))
            measured = history(ledger)
            priced = {tag: found for tag, rows in measured.items()
                      if (found := newest_cost(rows)) is not None}
            wins = {tag: gene_ledger_tool.pooled_win_diff_pp(rows)
                    for tag, rows in measured.items() if rows}
            rows = []
            for tag, entry in priced.items():
                row = dict(entry, tag=tag, win_diff_pp=wins.get(tag),
                           resolved=resolved(entry["compute_cost_pct"],
                                             entry["compute_cost_se_pct"]))
                row["cost_per_point"] = cost_per_point(row)
                row["tag"] = ("* " if tag in deployed else "  ") + tag
                rows.append(row)
            rows.sort(key=lambda one: -one["compute_cost_pct"])
            render(dict(reading, genes=rows), out=print)
            print("\n* = in the deployed genome")
        else:
            render(reading)
        return 0

    text = json.dumps(reading, indent=1, sort_keys=False) + "\n"
    if args.command == "write":
        RECORD_JSON.write_text(text)
        print("wrote %s" % RECORD_JSON.relative_to(ROOT))
        render(reading)
        return 0

    if not RECORD_JSON.exists():
        print("genome cost: %s is missing; run `python3 tools/genome_cost.py write`"
              % RECORD_JSON.relative_to(ROOT))
        return 1
    recorded = json.loads(RECORD_JSON.read_text())
    was = [row["tag"] for row in recorded.get("genes", [])]
    now = [row["tag"] for row in reading["genes"]]
    if set(was) != set(now):
        joined = sorted(set(now) - set(was))
        left = sorted(set(was) - set(now))
        print("genome cost: the deployed genome changed and the bill was not "
              "re-recorded.\n  joined: %s\n  left:   %s\n"
              "  Run `python3 tools/genome_cost.py write` and keep the diff in "
              "this pull request — it prices what just entered the genome."
              % (", ".join(joined) or "(none)", ", ".join(left) or "(none)"))
        return 1
    print("genome cost: the deployed set is recorded (%d genes; the bill reads "
          "%+.2f%% today, %+.2f%% when last written)"
          % (reading["deployed_genes"], reading["summed_cost_pct"],
             recorded.get("summed_cost_pct", 0.0)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
