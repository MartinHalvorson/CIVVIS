# Hostile memory revises cleared sightings

The existing `hostile-memory` versions preserve an old threat for four turns
even when the entire area of their forecast is visible and the enemy has not
been seen again. The native projection also reads an unseen unit's current
kind, owner, and movement allowance, while the live mirror has only the old
record. These are separate sources of needless avoidance and inconsistent
forecasts.

`hostile-memory-3` retains v2's protection for land escorts approaching naval
threats. After the observation pass refreshes visible enemies, it retires an
older sighting only when every tile in its bounded forecast is currently in
sight. A hole in that visibility preserves the record. Current-turn records
survive speculative kills, and recon and naval-raider records survive tile
visibility because those unit classes can hide. Both native and live missing
units use the recorded kind and owner, with the same static movement rule.

This is still the family's bounded risk model, with one hex of additional
uncertainty per elapsed turn. It is not an exhaustive movement forecast;
roads, promotions, and a rapidly moving enemy can escape that model. The new
gene does not change that horizon or claim a cleared forecast proves a death.

The gene is an exclusive, default-off third version. The existing versions
remain available for comparison and deployment defaults are preserved.

## Predeclared experiments

Before running any games:

- An activation probe: six games, seeds 914356600–914356605, only
  `hostile-memory-3` varied, four workers.
- A family comparison: 192 games, disjoint seeds 914366600–914366791, all three
  hostile-memory versions varied, four workers.

Both use the standard six-player, 74×46 Continents, nine-city-state,
Online-speed 250-turn shape, all victory conditions and Emperor difficulty.
The family comparison uses the standard genome probabilities. These small
experiments can expose behavior and gross regressions; neither authorizes a
default change or establishes superiority from a positive point estimate.

Commands and results will be recorded here after the clean source checkpoint
has been built and the experiments have finished.
