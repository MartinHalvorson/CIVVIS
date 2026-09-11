# Reading score share instead of wins: what it buys, and where it stops

`gene_screen` reports a gene's effect twice — as a change in **win rate** and as
a change in **score share**. Share is continuous and resolves an edge far more
tightly at the same seats, which is a standing argument for reading it when wins
are too expensive. `src/bin/gene_screen.rs` makes that argument in its own
header.

The argument skips a step. Share and wins are not the same quantity, so a share
reading is only worth something if it *predicts* the win reading, and nobody had
measured whether it does. `tools/share_predicts_win.py` measures it against the
committed screen corpus. **It runs no games.** Every number below comes from
screens already on disk: 9,099 gene-by-screen records, 373 genes, 82 screens.

## The answer in one line

**Share predicts wins with a slope near 3, and the prediction carries an
irreducible floor of about 0.47 win pp. So reading share is worth roughly 4× the
seats on a small screen, 1× at about 26,000 seats, and less than nothing above
that.**

## The decision rule

| share SE | ≈ seats | predicted win SE | direct win SE | advantage |
| ---: | ---: | ---: | ---: | ---: |
| 0.600 | 559 | 1.825 | 3.655 | **4.01×** |
| 0.400 | 1,259 | 1.265 | 2.437 | **3.71×** |
| 0.300 | 2,238 | 0.997 | 1.827 | **3.36×** |
| 0.200 | 5,035 | 0.750 | 1.218 | **2.64×** |
| 0.150 | 8,951 | 0.641 | 0.914 | 2.03× |
| 0.120 | 13,986 | 0.584 | 0.731 | 1.57× |
| 0.100 | 20,139 | 0.551 | 0.609 | 1.22× |
| 0.080 | 31,468 | 0.522 | 0.487 | 0.87× |
| 0.050 | 80,557 | 0.488 | 0.305 | 0.39× |

Crossover at share SE **0.0873 pp**, about **26,428 seats**.

⭐ **Read this as two different instructions for two different screens.**

- **A pilot or an expensive rung — hundreds to a few thousand seats — should be
  read on share.** That is 3–4× the seats for free, and it is exactly the regime
  where seats are the binding constraint.
- **The large standard screens this fleet already runs — 13,000 to 27,000 seats —
  are at or past the crossover.** Reading wins directly there is correct, and
  the existing practice needs no change. Above ~26,000 seats a share reading is
  strictly the worse instrument, and the gap widens.

## Why the advantage ends

Within a screen, the win reading's standard error is a median **6.09×** the
share reading's. That factor is fixed. But predicting a win effect from a share
effect costs a **structural residual of 0.466 win pp** — genes that move share by
the same amount do not move wins by the same amount, and that scatter is not
sampling noise. Seats shrink the direct reading's error without bound; they
cannot shrink the floor. So the two curves cross, and where they cross is an
arithmetic fact about this corpus rather than a matter of taste.

## ⚠⚠ The trap the tool is built around

Inside one screen, `share_delta_pp` and `win_delta_pp` are computed from the
**same seats**. Their sampling errors are correlated — a lucky draw of strong
seats lifts both — so fitting one against the other within a screen measures
that shared luck as though it were signal.

Every headline number therefore uses **disjoint sources**: a gene's share effect
is pooled from one random half of the screens it appears in and its win effect
from the other half, so the two errors are independent by construction. The
corpus supports it easily — the median gene appears in **25** screens, and 370
of 373 appear in at least two.

Two corrections then change the answer, and both need the reported standard
errors rather than an assumption:

1. **Attenuation.** The share axis is measured with error, which biases an
   ordinary slope toward zero. Its sampling variance is subtracted from the
   regressor's sum of squares.
2. **The residual is mostly noise until you remove it.** Raw scatter about the
   line contains the sampling error of both axes. Removing it leaves the
   structural part — the 0.466 pp above — and that single number decides the
   whole question.

## ⚠ Do not quote a slope off the near-zero genes

Restricting to genes with small |share| drives the corrected sum of squares
toward zero and the slope explodes — it reads above 20 on the lowest band. That
is the correction dividing by nearly nothing. `--bands` prints those rows and
marks any band under a signal-to-noise of 1.5 as unquotable rather than printing
a number somebody might carry away.

The bands that *can* be quoted agree with each other:

| band | genes | slope |
| --- | ---: | ---: |
| \|share\| 0.0–0.1 | 284 | ⚠ not quotable (SNR 0.20) |
| \|share\| 0.1–0.2 | 49 | 3.501 |
| \|share\| 0.2–0.4 | 22 | 3.497 |
| \|share\| 0.4+ | 15 | 2.807 |

Dropping the 1, 3, 5, 10 or 20 largest-effect genes moves the pooled slope only
between 3.1 and 3.6, so no handful of outliers is carrying it.

## ⚠⚠ This is a PRINCE calibration, and that is exactly the wrong place for it

Every screen in the corpus reports `overall_win` as exactly 1/6. That is not a
difficulty fact — it is the arithmetic of a six-player game in which one seat
wins and all six are measured. The corpus is the standard all-seats shape.

**So the calibration is measured where the win column is already readable, and
the regime where share would help most is the one it was not measured in.** At
Emperor the deployment shape handicaps only rivals, the measured base rate is
far from 1/6, and `docs/GENE_SCREEN.md` records that the win column cannot be
read there at all. That is a few-thousand-seat regime, where this table promises
3–4× — and it is an extrapolation.

Before leaning on share at Emperor, the slope and the floor have to be
re-measured there. `docs/GENE_SCREEN.md` already refuses the weaker version of
this comparison in its own words — one measured seat against five handicapped
Emperor rivals is **"not comparable to the self-play columns"** beside it — and
a calibration carried across that line is the same move wearing a regression.
**Do not treat the 2.94 as a constant of nature.** It is a constant of one shape.

## A caution about what the slope means

Share and winning are two summaries of "this seat did well," so part of the link
is definitional rather than mechanical. That does not weaken the *use* — a
surrogate has to predict, not to cause — but it does mean the slope is not
evidence that share is upstream of winning, and it should not be quoted as
though a gene that lifts share has been shown to lift wins *through* share.

## Running it

```
tools/share_predicts_win.py                 # the table above
tools/share_predicts_win.py --bands         # plus the magnitude bands
tools/share_predicts_win.py --json          # machine-readable
tools/share_predicts_win.py --screens 'docs/gene_screens/2026-09-*.json'
```

Screens are discovered, never listed, and an empty glob is an error rather than
an empty report.
