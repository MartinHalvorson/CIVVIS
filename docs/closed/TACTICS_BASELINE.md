# Tactics arena baseline (retired 2026-08-23)

> **Closed.** `tools/tactics_bench.py` and the `civvis tournament` harness it ran on were removed on 2026-08-23 (#2357, operator: *"lets remove the league /
> elo work for now"*). The gene screen (`docs/GENE_SCREEN.md`) prices
> behaviours; nothing rates named agents against each other for now, and the
> deployment genome is the gene ledger's default-on set. A rating system for
> finished genomes is planned to return — see `docs/ROADMAP.md`. This document
> is kept as the record of how the retired instrument worked and what it
> measured.

What the shipped controllers do on the arena, so a change to tactical AI can be
answered with a number. Regenerate with `tools/tactics_bench.py --write-baseline`
and quote the diff in the pull request that moves it.

**Measured on `e66bd0a28` (2026-08-19), 240 seat-mirrored games per matchup.** These figures describe
that revision and no other. `tactics_bench.py` prints how many commits have
landed since, because a table with no age on it reads as current.

⚠ **Compare like with like.** A row here is only comparable to a row measured
at the same sample size. On 2026-08-17 a 40-game reading of `1 city per side`
(97.5%) was compared against a 120-game one (75.8%) and reported as a
21.7-point regression; rebuilding the older commit and measuring it at 480
games gave **81.2%**, so about sixteen of those points were never there and the
remainder is not statistically distinguishable (p = 0.136). `--write-baseline`
now refuses fewer than 120 games for exactly that reason.

Every figure is the left controller's share of 240 seat-mirrored games on a
20x20 arena, with a 95% Wilson interval. Seat-mirrored means each
controller plays both ends of every draw, so a starting-corner advantage cannot
read as a controller advantage. The arena economy is pinned by the battery
(`ECONOMY` in `tools/tactics_bench.py`) rather than taken from the stock arena,
so the rows stay comparable when the stock arena moves.

## Regimes

- **1 city per side** — a static objective: the battle is decided by taking the enemy city
- **no cities** — pure combat: the objective is the enemy army, and it moves
- **no cities, random era** — pure combat across the whole unit roster rather than one era's

## Opponents

`basic` is the informative opponent. `advanced_v1` is a frozen copy of the live
controller, and nearly everything separating them is empire-level machinery an
arena never exercises — so that column sits near 50% whatever the tactical AI
does. **A near-50% result against `advanced_v1` is the expected null, not a
finding.** Read the `basic` column.

## Results

| regime | advanced vs basic | advanced vs advanced_v1 |
| --- | --- | --- |
| 1 city per side | 75.8% | 62.1% (55.8–68.0) |
| no cities | 91.7% | 100.0% (98.4–100.0) |
| no cities, random era | 95.4% | 97.9% (95.2–99.1) |

<!-- measured: {"commit": "e66bd0a289cd3cb1cae449aaa37963f74dec65d4", "date": "2026-08-19T01:18:58-04:00", "games": 240} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "basic", "pct": 75.83333333333333} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "advanced_v1", "pct": 62.1} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "basic", "pct": 91.66666666666667} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "advanced_v1", "pct": 100.0} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "basic", "pct": 95.41666666666667} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "advanced_v1", "pct": 97.9} -->
