#!/usr/bin/env python3
"""Per-gene paired contrast on a recorded QUANTITY from a `gene_screen` run.

`gene_screen --analyze` answers on two axes: the win rate and the score share.
Both are terminal outcomes, and a treatment aimed at one economy has to move a
whole game's result before either can see it — which is why every science
treatment measured so far has come back `~` unresolved. `docs/eval/` records the
same wall from the other direction for `ai_eval`, whose own note says to read
the per-arm `tech` column rather than the victory rate.

Every `kind: "game"` row a screen writes already carries `techs`, `cities`,
`score`, `faith` and `military`. This reads the same rows on any one of those
axes, which is far more sensitive than a win: a gene that reliably buys two more
technologies is visible long before the wins it might eventually buy are.

**The estimator is the screen's own.** The foldover makes arm 1 the exact
complement of arm 0, so the pair DIFFERENCE keeps main effects and cancels every
two-factor term (`docs/GENE_SCREEN.md`). For a gene at sign x in {-1, +1} the
marginal is `mean((y0 - y1) * x0) / 2`, and errors are clustered by game pair
because an all-seats run puts every major seat of one game in the same cluster.

Run with `--metric win` and it reproduces `gene_screen --analyze`'s win column to
the printed precision — that is the check `--self-test` performs, and the reason
a reading on `techs` from the same rows can be trusted.

    python3 tools/gene_quantity_contrast.py RUN.jsonl --metric techs
    python3 tools/gene_quantity_contrast.py RUN.jsonl --metric win --self-test
"""

from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from pathlib import Path

# `win` is a bool on the row; every other axis is already numeric.
BOOL_METRICS = {"win", "alive", "founded_religion", "inquisition"}


def read(paths):
    """Return (gene names, game rows) from one or more screen JSONL files.

    ⚠ A long run's file can carry MORE THAN ONE header. `gene_screen` writes
    one per window, so rebuilding the binary mid-run to add a gene makes every
    later window announce a different list. The 83,000,000 run did exactly
    that, and the first draft of this reader refused the whole file.

    A row's signs are read against ITS OWN header, so a gene APPENDED to the
    end is harmless: the genome string is positional and every earlier gene
    keeps its index. Pooling is refused only when the shorter list is not a
    prefix of the longer, which is a genuine reorder — position `i` means two
    different genes and no care could line the matrix up.

    ⭐ And that is what the 83,000,000 run turned out to be: the new gene was
    inserted into the MIDDLE of the treatment table, beside its siblings, not
    appended. The refusal was right, and the run had to be analysed as two
    segments. **Append a gene at the END of the table if a screen is already
    running against it**, or expect to split the file.
    """
    genes, rows = None, []
    for path in paths:
        current = None
        for line in Path(path).read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            if row.get("kind") == "header":
                current = row["genes"]
                shorter, longer = sorted((genes or [], current), key=len)
                if genes is not None and longer[: len(shorter)] != shorter:
                    raise SystemExit(
                        "refusing to pool rows whose gene lists disagree on "
                        "order: position i would mean two different genes"
                    )
                # Keep the longest list seen; it names every gene screened.
                genes = longer
            elif row.get("kind") == "game":
                if current is None:
                    raise SystemExit("a game row before any header")
                row["_genes"] = current
                rows.append(row)
    if genes is None:
        raise SystemExit("no header row; is this a gene_screen --out file?")
    return genes, rows


def seat_pairs(rows):
    """Complete (arm 0, arm 1) pairs, keyed by the seed and seat they share."""
    by_key = defaultdict(dict)
    for row in rows:
        by_key[(row["seed"], row["seat"])][row["arm"]] = row
    return [(v[0], v[1]) for v in by_key.values() if 0 in v and 1 in v]


def value(row, metric):
    raw = row[metric]
    return float(raw) if metric not in BOOL_METRICS else (1.0 if raw else 0.0)


def contrast(pairs, genes, metric, gene):
    """(estimate, standard error, z, clusters) for one gene on one axis.

    The estimate is per unit of the +/-1 coding, so the on-minus-off difference
    a screen prints is twice it.

    The sign is read against each row's own header (see `read`), so a window
    written before the gene existed contributes nothing rather than reading
    some other gene's bit.
    """
    per_cluster = defaultdict(list)
    for arm0, arm1 in pairs:
        names = arm0.get("_genes", genes)
        if gene not in names:
            continue
        index = names.index(gene)
        sign = 1.0 if arm0["genome"][index] == "1" else -1.0
        per_cluster[arm0["pair"]].append(
            (value(arm0, metric) - value(arm1, metric)) * sign / 2.0
        )
    means = [sum(v) / len(v) for v in per_cluster.values()]
    n = len(means)
    if n < 2:
        return None
    mean = sum(means) / n
    variance = sum((v - mean) ** 2 for v in means) / (n - 1)
    se = math.sqrt(variance / n)
    return mean, se, (mean / se if se > 0 else 0.0), n


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("run", nargs="+", help="gene_screen --out JSONL file(s)")
    ap.add_argument("--metric", default="techs")
    ap.add_argument("--genes", help="comma-separated; default every screened gene")
    ap.add_argument(
        "--self-test",
        action="store_true",
        help="also print the on-minus-off difference, which on --metric win is "
        "what gene_screen --analyze prints",
    )
    args = ap.parse_args(argv)

    genes, rows = read(args.run)
    pairs = seat_pairs(rows)
    wanted = args.genes.split(",") if args.genes else genes
    # A gene held at baseline all run has no sign to read; skip it silently.
    varying = [
        g
        for g in wanted
        if g in genes
        and len(
            {
                r["genome"][r.get("_genes", genes).index(g)]
                for r in rows
                if g in r.get("_genes", genes)
            }
        )
        > 1
    ]
    print(f"{len(pairs)} complete seat-pairs · metric {args.metric}")
    results = []
    for gene in varying:
        got = contrast(pairs, genes, args.metric, gene)
        if got:
            results.append((got[2], gene, *got))
    results.sort(key=lambda row: -row[0])

    head = f"{'gene':<32}{'Δ' + args.metric:>11}{'±se':>9}{'z':>8}"
    if args.self_test:
        head += f"{'on−off':>10}"
    print(head + "   pairs")
    for z, gene, mean, se, _z, n in results:
        flag = "**" if abs(z) >= 2.5 else ("*" if abs(z) >= 2 else "")
        line = f"{gene:<32}{mean:>+11.3f}{se:>9.3f}{z:>+8.2f}"
        if args.self_test:
            line += f"{2 * mean:>+10.3f}"
        print(f"{line} {flag:<2}  {n}")
    if not results:
        print("(no gene varied in these rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
