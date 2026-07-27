# Evolution fitness and racing evidence

`civvis evolve` has two separate jobs:

1. rank a population cheaply enough to breed promising genomes; and
2. replace the saved champion only after an independent win-based SPRT and a
   fixed-map holdout both pass.

Those jobs should not use the same noisy statistic. Before this change, the
population objective was

```text
50 * players * score_share
+ 100 * outright_win
+ 12 * players * combat_share
```

The 100-point Bernoulli term was larger than the continuous score and combat
signal combined. It made eight-game population rankings unstable even though
all genomes already shared map seeds, seats, horizons, and opponents.

The breeder and fixed-map holdout now use only the score and combat terms.
Outright wins still exclusively decide champion promotion in `sprt_confirm`.
The 65-point development screen is the old threshold with its parity win bonus
removed, so its margin above the continuous objective's roughly 62-point table
parity is unchanged.

## Exact-path measurement

`evolve_probe` calls the same game evaluator and common-random-number schedule
as a production generation. It applies the four bounded doctrine perturbations
to one base genome and reports marginal and paired uncertainty, prefix ranks,
disjoint block winners, and alternative objectives.

```sh
cargo run --profile ci --bin evolve_probe -- \
  --games 64 --players 4 --width 24 --height 16 --turns 200 \
  --seed 99000 --jobs 4

cargo run --profile ci --bin evolve_probe -- \
  --games 64 --players 4 --width 24 --height 16 --turns 200 \
  --seed 100000 --jobs 4
```

Across the two disjoint 64-game schedules, the historical objective's eventual
leader won only 8 of 16 independent eight-game blocks. Removing the win bonus
raised that to 12 of 16. Its paired leader-versus-runner standard error fell
from 3.659 to 2.193 on seed 99000 and from 5.121 to 1.958 on seed 100000.
Score-only reached 10 of 16; an 80%-score/20%-win blend remained at 8 of 16.

This does not make an eight-game generation certain. Doubling the disjoint
blocks to 16 games left the repaired objective at 6 of 8 eventual-leader
selections, the same 75% agreement, so the default was not doubled without a
measured return. Evolution instead prints the selected genome's marginal
standard error and its paired edge over the runner-up, and appends both to
`history.csv`. Those numbers make selection drift visible without pretending
that a Bernoulli win-rate error describes the statistic being ranked.

## Search and promotion check

The repaired objective was exercised in a fresh four-generation production
configuration:

```sh
cargo run --profile ci --bin civvis -- evolve \
  --generations 4 --pop 16 --games 8 --players 4 \
  --width 24 --height 16 --turns 200 --seed 101000 --threads 12 \
  --dir /tmp/civvis-scorecombat-evolve
```

The screen proposed candidates in all four generations. None replaced the
champion:

| generation | best / population mean | fixed holdout candidate / champion | promotion SPRT |
| ---: | ---: | ---: | ---: |
| 0 | 93.76 / 79.67 | 51.19 / 48.73 | 7-28, reject |
| 1 | 74.90 / 65.14 | 66.82 / 48.73 | 58-136, reject |
| 2 | 74.95 / 62.29 | 61.94 / 48.73 | 43-104, reject |
| 3 | 85.00 / 78.10 | 65.10 / 48.73 | 4-22, reject |

The high continuous validation values were not treated as proof of stronger
play. The independent win gate rejected them and `best.json` remained the
incumbent. This is the intended failure mode: search can learn from dense
signals, while deployment still requires decisive wins.

Generation 3 also demonstrated why uncertainty is printed: the selected
genome's 85.0 score had a 9.6 standard error, and its paired edge over the
runner-up was only 0.7 with a 5.9 standard error.

## Existing artifact check

The repository's older generation-7 `evolved/best.json` predates the current
genome schema. A fresh mirrored evaluation against `advanced` produced 33-27
on 30 maps (seed 97000), then exactly 120-120 on a disjoint 120-map gate (seed
98000), with 17-17 directional map wins. It is not evidence of a stronger
current player and was not promoted or copied into production.

The probe and uncertainty output are retained so future objective, budget, and
agent changes can be measured on the statistic evolution actually consumes.
