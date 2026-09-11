#!/usr/bin/env python3
"""Is SCORE SHARE a usable stand-in for WIN RATE, and up to how many seats?

`gene_screen` reports both. Share is the continuous outcome and resolves an edge
far more tightly than a win/loss count at the same seats, which is a standing
argument for reading it instead. This asks the question that argument skips:
share and wins are not the same quantity, so how much does a share reading
actually tell you about the win reading, and where does the trade stop paying?

Run it against the committed screen corpus:

    tools/share_predicts_win.py
    tools/share_predicts_win.py --screens 'docs/gene_screens/*.json' --splits 400

Nothing here runs a game. Every number comes from screens already on disk.

## ⚠⚠ The trap this is built around

Inside ONE screen, `share_delta_pp` and `win_delta_pp` are computed from the
SAME seats. Their sampling errors are therefore correlated -- a lucky draw of
strong seats lifts both -- and regressing one on the other within a screen
measures that shared luck as though it were signal.

So every headline number uses DISJOINT SOURCES: a gene's share effect is pooled
from one random half of the screens it appears in, and its win effect from the
other half. Those errors are independent by construction. The within-screen fit
is computed too and printed beside it, so the size of the inflation is visible
rather than argued about.

## The two corrections that change the answer

1. **Attenuation.** The share axis is itself measured with error, which biases
   an ordinary slope toward zero. The screens report `share_se_pp`, so the
   sampling variance is subtracted from the regressor's sum of squares instead
   of being assumed away.
2. **The residual is mostly noise until you remove it.** The raw scatter about
   the line contains the sampling error of BOTH axes. Subtracting it leaves the
   STRUCTURAL residual -- the part that no number of seats removes -- and that
   single number is what decides the whole question, because it puts a floor
   under any win estimate made from a share reading.

## ⚠ Do not read the slope off a low-signal subset

Restricting to genes with small |share| drives the corrected `Sxx` toward zero
and the slope explodes. That is the correction dividing by nearly nothing, not
a steeper relationship. `--bands` prints those rows on purpose, and marks the
ones whose signal-to-noise is too low to quote.
"""
from __future__ import annotations

import argparse
import collections
import glob
import json
import math
import random
import statistics
import sys

#: Screens are discovered, never listed. A hand-written list of files is
#: complete the day it is written and silently shrinks afterwards; `AGENTS.md`
#: records that failure costing this repository three times. An empty glob is
#: an error here, not an empty report.
DEFAULT_SCREENS = "docs/gene_screens/*.json"

#: A gene needs at least this many screens to be split into two disjoint sides.
MIN_SCREENS_PER_GENE = 2

#: Below this |z| on the pooled share axis, a magnitude band carries so little
#: signal that the attenuation correction divides by nearly zero. Such bands are
#: printed with a marker instead of a number anybody might quote.
BAND_MIN_SNR = 1.5


def load_screens(pattern):
    """Every (gene, screen) record that carries both readings and both errors."""
    paths = sorted(glob.glob(pattern))
    if not paths:
        raise SystemExit(f"no screens matched {pattern!r} -- nothing to analyse")
    rows, skipped = [], 0
    for path in paths:
        try:
            doc = json.load(open(path))
        except (OSError, ValueError):
            skipped += 1
            continue
        if not isinstance(doc, dict):
            skipped += 1
            continue
        for gene in doc.get("genes") or []:
            if not isinstance(gene, dict):
                continue
            sd, ss = gene.get("share_delta_pp"), gene.get("share_se_pp")
            wd, ws = gene.get("win_delta_pp"), gene.get("win_se_pp")
            if None in (sd, ss, wd, ws):
                continue
            if not (ss > 0 and ws > 0):
                continue
            rows.append(
                dict(screen=path, tag=gene.get("tag"), share=sd, share_se=ss,
                     win=wd, win_se=ws, seats=gene.get("seats") or 0)
            )
    if not rows:
        raise SystemExit(f"{len(paths)} screens matched {pattern!r} but none "
                         "reported share and win with standard errors")
    return rows, paths, skipped


def inverse_variance_mean(values, errors):
    weights = [1.0 / (e * e) for e in errors]
    total = sum(weights)
    mean = sum(v * w for v, w in zip(values, weights)) / total
    return mean, math.sqrt(1.0 / total)


def disjoint_points(by_gene, seed):
    """One (share, win) point per gene, from non-overlapping screens."""
    rng = random.Random(seed)
    points = []
    for tag, records in sorted(by_gene.items()):
        if len(records) < MIN_SCREENS_PER_GENE:
            continue
        order = list(range(len(records)))
        rng.shuffle(order)
        half = len(order) // 2
        left = [records[i] for i in order[:half]]
        right = [records[i] for i in order[half:]]
        if not left or not right:
            continue
        x, x_se = inverse_variance_mean([r["share"] for r in left],
                                        [r["share_se"] for r in left])
        y, y_se = inverse_variance_mean([r["win"] for r in right],
                                        [r["win_se"] for r in right])
        points.append((tag, x, x_se, y, y_se))
    return points


def corrected_fit(points):
    """Slope with the regressor's sampling variance removed, and the residual
    with the sampling variance of BOTH axes removed."""
    n = len(points)
    if n < 3:
        return None
    mx = sum(p[1] for p in points) / n
    my = sum(p[3] for p in points) / n
    sxx_observed = sum((p[1] - mx) ** 2 for p in points)
    sxx_noise = sum(p[2] ** 2 for p in points)
    sxx_true = sxx_observed - sxx_noise
    if sxx_true <= 0:
        return None
    sxy = sum((p[1] - mx) * (p[3] - my) for p in points)
    slope = sxy / sxx_true
    intercept = my - slope * mx
    residuals = [p[3] - (intercept + slope * p[1]) for p in points]
    var_observed = sum(r * r for r in residuals) / (n - 2)
    var_noise = (sum(p[4] ** 2 for p in points) / n
                 + slope * slope * sum(p[2] ** 2 for p in points) / n)
    return dict(slope=slope, intercept=intercept, n=n,
                structural_sd=math.sqrt(max(var_observed - var_noise, 0.0)),
                reliability=sxx_true / sxx_observed,
                snr=math.sqrt(sxx_true / sxx_noise) if sxx_noise > 0 else float("inf"))


def within_screen_fit(rows):
    """The inflated comparison: both axes from the same seats."""
    n = len(rows)
    weights = [1.0 / (r["win_se"] ** 2) for r in rows]
    total = sum(weights)
    mx = sum(r["share"] * w for r, w in zip(rows, weights)) / total
    my = sum(r["win"] * w for r, w in zip(rows, weights)) / total
    sxx = sum(w * (r["share"] - mx) ** 2 for r, w in zip(rows, weights))
    sxy = sum(w * (r["share"] - mx) * (r["win"] - my) for r, w in zip(rows, weights))
    return dict(slope=sxy / sxx if sxx else float("nan"), n=n)


def seat_scale(rows):
    """Calibrate seats <-> share SE from the corpus itself, so the seat column
    is this project's arithmetic and not a constant somebody typed once."""
    sized = [r for r in rows if r["seats"] and r["share_se"] > 0]
    if not sized:
        return None
    # share SE scales as 1/sqrt(seats); take the median of se * sqrt(seats).
    return statistics.median(r["share_se"] * math.sqrt(r["seats"]) for r in sized)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--screens", default=DEFAULT_SCREENS)
    ap.add_argument("--splits", type=int, default=200,
                    help="random disjoint splits to median over")
    ap.add_argument("--bands", action="store_true",
                    help="also fit within |share| magnitude bands")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    args = ap.parse_args(argv)

    rows, paths, skipped = load_screens(args.screens)
    by_gene = collections.defaultdict(list)
    for r in rows:
        by_gene[r["tag"]].append(r)

    fits = [corrected_fit(disjoint_points(by_gene, s)) for s in range(args.splits)]
    fits = [f for f in fits if f]
    if not fits:
        raise SystemExit("no split produced a usable fit")
    slope = statistics.median(f["slope"] for f in fits)
    structural = statistics.median(f["structural_sd"] for f in fits)
    genes_used = fits[0]["n"]

    ratio = statistics.median(r["win_se"] / r["share_se"] for r in rows)
    scale = seat_scale(rows)

    # The surrogate beats a direct win reading while
    #     ratio * s  >  sqrt(slope^2 s^2 + structural^2)
    crossover = (structural / math.sqrt(ratio ** 2 - slope ** 2)
                 if ratio ** 2 > slope ** 2 else float("nan"))

    if args.json:
        json.dump(dict(records=len(rows), genes=len(by_gene), screens=len(paths),
                       genes_used=genes_used, slope=slope,
                       structural_residual_pp=structural, win_se_over_share_se=ratio,
                       crossover_share_se_pp=crossover,
                       crossover_seats=(scale / crossover) ** 2 if scale and crossover == crossover else None),
                  sys.stdout, indent=2)
        print()
        return 0

    print(f"{len(rows)} gene-by-screen records | {len(by_gene)} genes | "
          f"{len(paths)} screens" + (f" | {skipped} unreadable" if skipped else ""))
    print()
    w = within_screen_fit(rows)
    print("WITHIN ONE SCREEN -- both readings from the SAME seats, so the errors")
    print("are correlated and this fit is inflated. Shown only for contrast.")
    print(f"   n={w['n']}   slope {w['slope']:.3f} win pp per share pp")
    print()
    print(f"DISJOINT SOURCES -- share from one half of a gene's screens, win from")
    print(f"the other. Median of {args.splits} random splits, {genes_used} genes.")
    print(f"   slope                {slope:.3f} win pp per share pp")
    print(f"   structural residual  {structural:.3f} win pp")
    print( "                        the scatter left after removing the sampling")
    print( "                        error in BOTH axes -- a floor no seat count lifts")
    print(f"   share-axis reliability {statistics.median(f['reliability'] for f in fits):.3f}")
    print()
    print(f"WITHIN A SCREEN the win reading's SE is {ratio:.2f}x the share reading's.")
    print()
    print("SO: how many times more seats a DIRECT win reading needs to match a win")
    print("PREDICTED from share -- above 1.00x read share, below 1.00x read wins.")
    print()
    print(f"   {'share SE':>9} {'seats':>9} {'predicted':>10} {'direct':>8} {'advantage':>10}")
    for s in (0.60, 0.40, 0.30, 0.20, 0.15, 0.12, 0.10, 0.08, 0.05):
        predicted = math.sqrt(slope * slope * s * s + structural * structural)
        direct = ratio * s
        seats = (scale / s) ** 2 if scale else float("nan")
        print(f"   {s:9.3f} {seats:9.0f} {predicted:10.3f} {direct:8.3f} "
              f"{(direct / predicted) ** 2:9.2f}x")
    if crossover == crossover:
        seats = (scale / crossover) ** 2 if scale else float("nan")
        print()
        print(f"   CROSSOVER at share SE {crossover:.4f} pp, about {seats:,.0f} seats.")
        print( "   Below that a direct win reading is strictly the better instrument.")

    if args.bands:
        print()
        print("BY |share| MAGNITUDE  (a band whose signal-to-noise is under "
              f"{BAND_MIN_SNR} cannot be quoted:")
        print("the attenuation correction is dividing by nearly zero there, which")
        print("inflates the slope without meaning anything.)")
        points = disjoint_points(by_gene, 0)
        for lo, hi in ((0.0, 0.1), (0.1, 0.2), (0.2, 0.4), (0.4, float("inf"))):
            sub = [p for p in points if lo <= abs(p[1]) < hi]
            fit = corrected_fit(sub) if len(sub) > 8 else None
            label = f"   |share| in [{lo}, {hi})".ljust(28)
            if not fit:
                print(f"{label} n={len(sub):3}  too few genes")
            elif fit["snr"] < BAND_MIN_SNR:
                print(f"{label} n={len(sub):3}  ⚠ SNR {fit['snr']:.2f} -- not quotable")
            else:
                print(f"{label} n={len(sub):3}  slope {fit['slope']:.3f}"
                      f"  (SNR {fit['snr']:.2f})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
