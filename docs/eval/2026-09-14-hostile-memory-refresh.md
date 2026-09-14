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
  hostile-memory versions varied. Before starting it, the worker count was
  raised from four to eight after observing the activation probe's runtime;
  the seeds, sample size and genome probabilities were unchanged.

Both use the standard six-player, 74×46 Continents, nine-city-state,
Online-speed 250-turn shape, all victory conditions and Emperor difficulty.
The family comparison uses the standard genome probabilities. These small
experiments can expose behavior and gross regressions; neither authorizes a
default change or establishes superiority from a positive point estimate.

## Activation result

The six-game probe completed all 36 intended seats on clean source
`269e2d3ce892a1f6ff18033a437df29c2ffab34a`, binary SHA-256
`cb08759a9ae6f9af4a243a591c292c4855e9d6759d5028b94f52bbe95cacf516`.
The recorded gene fingerprint is
`94a1032880b1044b8557a9b9aa049a791724f89515fc31813d64903ffc7587d8`.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes hostile-memory-3 --games 6 \
  --start-seed 914356600 --jobs 4 --difficulty emperor --out activation.jsonl
target/ci/gene_screen --analyze activation.jsonl \
  --json docs/gene_screens/fires/hostile-memory-3.json
```

Eleven seats drew v3 and 25 did not. The win difference was +2.18 percentage
points with a 14.44-point standard error; score share was +3.05 points with a
1.71-point standard error. Both are unresolved. Three games ended in science,
one in religion, one in culture, and one reached the score clock. The artifact
passes the repository's gene-firing evidence gate; the six targeted tests
separately establish the changed behavior and its fog/camouflage exclusions.
This result does not establish that v3 wins more than off or either old version.

The first attempt at the preregistered 192-game comparison hit an existing
missing-city lookup panic in settlement candidate filtering on its old frozen
source. Only two games reached its ordered output writer. That incomplete
attempt is excluded from strength analysis; it retains its original header
and raw rows externally. Upstream `7be82dfeec458b89be3a825ba610ecacb658aab1`
fixes that exact unchecked lookup and has been merged into this candidate.

The complete 192-game seed range will be replayed on the clean corrected
build, with `--genes hostile-memory,hostile-memory-2,hostile-memory-3 --games
192 --start-seed 914366600 --jobs 8 --difficulty emperor`. No seed is dropped
or substituted. The completed activation probe above remains a separately
identified result on its original build. No deployment default changes.
