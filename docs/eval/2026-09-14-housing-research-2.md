# Usable housing research, version two

The original `housing-research` scans Aqueduct and Neighborhood district
names, then chooses a printed technology cost. Neighborhoods require a civic,
and no building is scanned, so it always returns Engineering or nothing.
Its preceding production gate checks `spec.housing > 0`, which misses the
ordinary Aqueduct's water-housing gain. It can therefore research Engineering
while Pottery would unlock a Granary, and can promise an Aqueduct without a
legal site in any capped city. The current ranking reads -0.17 percentage
points pooled on/off for version one; that historical measurement motivates
investigation and does not measure this rewrite.

Version two retains the empire gate (at least two capped cities and at least
half of the empire) and the existing priority after victory and science
research goals. It discovers housing buildings and districts, including the
Aqueduct's water gain. A capped city that can already build housing stays with
production. For the remaining cities, a private board unlocks each candidate
technology's missing ancestor path and asks the engine whether its housing
construction is legal. Missing civics, district sites, prerequisite buildings,
unique replacements and host refusals stay binding.

Candidates are priced by remaining research per useful housing across those
cities, capped at the housing needed to restore full growth. Research counts
every missing ancestor once and credits banked boosts and active progress;
active research already includes its boost. Version one keeps its implementation.
The new version follows the other repair-family challengers as `Kind::OptIn`,
so adding it does not replace version one in the unfiltered repair bundle.
Both enable methods enforce one version at a time.

## Evaluation fixed before execution

Run the existing independently randomized screen at its standard shape:
six seats, Continents 74×46, nine city-states, Online, 250 turns, all victories,
Emperor majors. The family levels are off, `housing-research` and
`housing-research-2`; all other genes follow the evaluator baseline.

First use 12 games, seeds 914368000–914368011, as a reach and execution check.
Use 180 disjoint games, seeds 914369000–914369179, for a family comparison.
These sizes are fixed before reading an outcome; neither sample changes a
deployment default. A claim of improved strength requires version two's win
contrast to be positive against both version one and off; uncertainty and
score share must be reported beside it. A sign alone at this sample size is
exploratory. Preserve adverse and inconclusive readings.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes housing-research,housing-research-2 \
  --games 12 --target-games 12 --start-seed 914368000 --jobs 2 \
  --difficulty emperor --out <run-dir>/reach.jsonl
target/ci/gene_screen --genes housing-research,housing-research-2 \
  --games 180 --target-games 180 --start-seed 914369000 --jobs 8 \
  --difficulty emperor --out <run-dir>/comparison.jsonl
```

The comparison started alongside the reach sample with eight workers after
the first reach games established the busy host's throughput. This changed
only scheduling: the fixed sample sizes, seeds, controller and binary stayed
the same, and neither sample was selected or shortened based on its results.

Unit tests must verify the original goal remains unchanged, the cheaper
Granary is usable after its unlock, the Sewer is reachable when Aqueducts
cannot be placed, missing building prerequisites block the detour, the full
ancestor cost matters, known remedies remain with production, and the
counterfactual leaves the game untouched. Run the repository suite and its
generated-artifact and gene-reach checks before integration.

## Results

### Completed reach sample

The fixed twelve-game reach sample completed with all 72 unique seats and
all reserved seeds, with no restarts or omitted games. The analyzer artifact
is `docs/gene_screens/fires/2026-09-14-housing-research-2.json`. It records the
original clean implementation, before the later merge from main. The raw
JSONL SHA-256 is `6e5006a1b5bfb89b7f17e9bdc9353fa8e967023c4def9d25b020c950832481d0`.

12 games; 72 seats; seeds [914368000, 914368011].
Source 2e972f414cf48c25bff8b290091ba986653de0d9; clean=True; binary SHA-256 922ff29a1bed35c5b8ef893b2a27c278d9de126eee637255cfc847984ade9589.

| Family level | Seats | Wins | Win rate | Score share |
|---|---:|---:|---:|---:|
| off | 55 | 9 | 16.36% | 16.55% |
| housing-research | 8 | 1 | 12.50% | 18.81% |
| housing-research-2 | 9 | 2 | 22.22% | 15.49% |

| Contrast | Win Δ, pp [approx. 95% interval] | Share Δ, pp [approx. 95% interval] |
|---|---:|---:|
| housing-research minus off | -3.86 [-31.92, +24.19] | +2.26 [-4.14, +8.66] |
| housing-research-2 minus off | +5.86 [-24.64, +36.36] | -1.06 [-4.44, +2.33] |
| housing-research-2 minus housing-research | +9.72 [-31.57, +51.02] | -3.32 [-8.74, +2.11] |

Intervals use the analyzer’s standard errors clustered by game. They are exploratory, without correction for multiple comparisons; small-sample intervals can be unreliable.

Version two's win signs are positive against both alternatives, but its score
share is lower than both. With only nine V2 seats, these readings do not
establish stronger play or justify promotion. The artifact satisfies the
execution-screen requirement; actual changed research choices are covered by
the focused regression tests. It is not added as a deployment-ledger source.

### Larger comparison failure and replacement

The original 180-game comparison used the same clean source and binary as
the reach sample, with the disjoint pre-registered seeds 914369000–914369179.
It hit an existing settlement ownership panic at `advanced.rs:31242`: a tile
named a city that was absent from the city table. The old parallel reporter
could leave the remaining workers alive after that panic. At 13:05:58 UTC the
failed process was stopped; its 17 reported games / 102 seats remain intact
in `housing-comparison.jsonl`, SHA-256
`2ef356c82d8f4dce886a78deb2b93beedff0c72d92d9dd5deb6086e97f0d64bb`.
No strength estimate is taken from that failed prefix.

Main PR #3564 already fixed that exact lookup, with a regression test, and
merge `6ab320a58` includes the fix in this candidate. The corrected
`6ab320a58` simulator build finished and was frozen, but the long replacement
sample has not started. Upstream PR #3578 has also tested a separate missing
Builder-owner lookup repair after another probe failed. The full replacement
will use the integrated engine, the same 180 seeds and eight workers in
`housing-comparison-fixed.jsonl`, with its own binary and header. Failed prefix
rows are never combined with the replacement. Its result remains pending,
and the candidate remains opt-in and unpromoted.

### Validation

All ten Housing-related focused tests passed. The full local Rust suite
passed, including 3,492 library tests with 49 ignored, and the integration
and binary suites. CI on the merged source also passed its Rust tests,
documentation examples, lint/formatting and paired cost checks. The existing
settlement failure came from the old comparison binary, not the merged
candidate source. No engine rule is changed by this gene.
