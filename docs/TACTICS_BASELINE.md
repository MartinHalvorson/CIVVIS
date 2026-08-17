# Tactics arena baseline

What the shipped controllers do on the arena, so a change to tactical AI can be
answered with a number. Regenerate with `tools/tactics_bench.py --write-baseline`
and quote the diff in the pull request that moves it.

**Measured on `79f2ab775` (2026-08-17), 240 seat-mirrored games per matchup.** These figures describe
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
| 1 city per side | 79.2% | 60.0% (53.7–66.0) |
| no cities | 87.9% | 99.6% (97.7–99.9) |
| no cities, random era | 95.0% | 98.8% (96.4–99.6) |

<!-- measured: {"commit": "79f2ab7750a85500ed8b746a7d95e0bf4d440f80", "date": "2026-08-17T14:31:02-04:00", "games": 240} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "basic", "pct": 79.16666666666667} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "advanced_v1", "pct": 60.0} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "basic", "pct": 87.91666666666667} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "advanced_v1", "pct": 99.6} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "basic", "pct": 95.0} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "advanced_v1", "pct": 98.8} -->
