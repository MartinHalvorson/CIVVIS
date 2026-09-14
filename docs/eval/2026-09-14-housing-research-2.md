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
Then use 180 disjoint games, seeds 914369000–914369179, for a family comparison.
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
  --games 180 --target-games 180 --start-seed 914369000 --jobs 2 \
  --difficulty emperor --out <run-dir>/comparison.jsonl
```

Unit tests must verify the original goal remains unchanged, the cheaper
Granary is usable after its unlock, the Sewer is reachable when Aqueducts
cannot be placed, missing building prerequisites block the detour, the full
ancestor cost matters, known remedies remain with production, and the
counterfactual leaves the game untouched. Run the repository suite and its
generated-artifact and gene-reach checks before integration.

## Results

Pending execution. The rewrite is a hypothesis, not a promoted genome.
