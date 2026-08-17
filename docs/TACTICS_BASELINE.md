# Tactics arena baseline

What the shipped controllers do on the arena, so a change to tactical AI can be
answered with a number. Regenerate with `tools/tactics_bench.py --write-baseline`
and quote the diff in the pull request that moves it.

**Measured on `5e3aa3cfe` (2026-08-17), 120 seat-mirrored games per matchup.** These figures describe
that revision and no other. `tactics_bench.py` prints how many commits have
landed since, because a table with no age on it reads as current — and this one
was not. See `staleness_note` in the benchmark for what that cost.

Every figure is the left controller's share of 120 seat-mirrored games on a
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
| 1 city per side | 75.8% | 61.7% (52.7–69.9) |
| no cities | 58.3% | 92.5% (86.4–96.0) |
| no cities, random era | 80.8% | 95.8% (90.6–98.2) |

<!-- measured: {"commit": "5e3aa3cfec2c9676165fae2b6fbfa3a8278d7d45", "date": "2026-08-17T13:20:40-04:00", "games": 120} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "basic", "pct": 75.83333333333333} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "advanced_v1", "pct": 61.7} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "basic", "pct": 58.333333333333336} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "advanced_v1", "pct": 92.5} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "basic", "pct": 80.83333333333333} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "advanced_v1", "pct": 95.8} -->
