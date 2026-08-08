# Tactics arena baseline

What the shipped controllers do on the arena, so a change to tactical AI can be
answered with a number. Regenerate with `tools/tactics_bench.py --write-baseline`
and quote the diff in the pull request that moves it.

Every figure is the left controller's share of 40 seat-mirrored games on a
20x20 arena, with a 95% Wilson interval. Seat-mirrored means each
controller plays both ends of every draw, so a starting-corner advantage cannot
read as a controller advantage.

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
| 1 city per side | 97.5% | 40.0% (26.3–55.4) |
| no cities | 30.0% | 77.5% (62.5–87.7) |
| no cities, random era | 52.5% | 90.0% (76.9–96.0) |

<!-- bench: {"regime": "capture", "left": "advanced", "right": "basic", "pct": 97.5} -->
<!-- bench: {"regime": "capture", "left": "advanced", "right": "advanced_v1", "pct": 40.0} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "basic", "pct": 30.0} -->
<!-- bench: {"regime": "attrition", "left": "advanced", "right": "advanced_v1", "pct": 77.5} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "basic", "pct": 52.5} -->
<!-- bench: {"regime": "attrition-eras", "left": "advanced", "right": "advanced_v1", "pct": 90.0} -->
