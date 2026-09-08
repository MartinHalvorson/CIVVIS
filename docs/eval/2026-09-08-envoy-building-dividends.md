# Affordable envoy building dividends

The `envoy-building-dividends` opt-in gene prices 1/3/6 envoy packages using
Civvis's existing final-patch city-state rules. It is a tournament candidate;
its default remains off until the ordinary batch rule selects it.

The previous scorer subtracts seven points per envoy needed to take the
suzerainty even when the proposed placement only buys a building threshold.
It also awards identity/alignment points to placements that cannot reach a
new bonus. Consequently a rival's 100-envoy delegation can suppress our
University investment even though building income does not require suzerainty.

The candidate compares attainable packages by strategic value per placement.
It sums the actual active buildings and production queues across the empire,
considers later tiers when they fit the current pool, and prices takeover or
one-placement defense separately. Rival distance and shared suzerain bonuses
do not discount ordinary building income. With no positive affordable package,
it saves the pool for a future threshold. Geographic, victory-denial and
competition prizes remain eligible on attainable takeovers.

`Game::envoy_investment_options` is a read-only planner. Its placement math
matches `Game::do_send_envoy`: first raw placement gets `first_envoy_bonus`,
and `different_government_envoy_bonus` stops once the foreign suzerain is tied
or displaced. Amani's established delegation and multiplier come from
`Game::amani_envoy_terms`. Economic income uses `envoy_type_yields_for_count`,
the same implementation as turn income. No game rules or income changed.
`data/policies.json` supplies Diplomatic League's `first_envoy_bonus: 1`
and Containment's `different_government_envoy_bonus: 1`.
The installed `Base/Assets/Gameplay/Data/Policies.xml:1646–1648` confirms
`DIPLOMATICLEAGUE_DUPLICATEFIRSTINFLUENCETOKEN`, `Amount`, `1`.

## Arithmetic and behavioral validation

Ten active Libraries plus ten Universities yield +11 Science per turn at one
envoy and +31 at three, excluding Consulate and percentage modifiers.
Pillaging one University changes +31 to +29. Eight focused tests cover those
values, investing below a rival's unattainable suzerainty, opening the first
tier, saving until three is affordable, the six-envoy Lab tier, one-placement
defense, policy/Amani predictions against actual actions, and opt-in isolation.

## Tournament-format exercise

Predeclared size: 12 games, 72 seats, seeds 980908001 through 980908012. Only
this gene varies; the other genes remain at their deployment defaults. Use the
standard six-player Continents map, nine city-states, Online speed, 250 turns,
all victory types, Emperor difficulty. This small run verifies execution and
records descriptive outcomes; it cannot establish a win-rate improvement or
promote the gene.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --games 12 --jobs 4 --difficulty emperor \
  --genes envoy-building-dividends --start-seed 980908001 \
  --out /tmp/civvis-envoy-building-dividends.jsonl
target/ci/gene_screen --analyze /tmp/civvis-envoy-building-dividends.jsonl \
  --json docs/gene_screens/fires/envoy-building-dividends.json
```

## Regression checks

- `cargo test --profile ci --locked`: 3,177 passed, 50 ignored, zero failures.
- `cargo test --profile ci --locked envoy_dividends -- --nocapture`: all eight new tests passed.
- `python3 -m unittest discover -s tools -p test_treatment_append_points.py`: 14 passed.
- `python3 tools/genes.py check`: current.
- `python3 tools/rust_quality.py --base origin/main --head HEAD`: changed lines formatted and warning-free.

## Completed exercise

The validated reader reports all 12 games / 72 seats complete, one winner per
game, and an analysis matching the raw rows. All games ended in Science
victories. The gene was on for 16 seats (5 wins, 31.25%) and off for 56 seats
(7 wins, 12.5%). The difference is +18.75 percentage points with standard error
10.27 points; the reported 95% interval is approximately −1.4 to +38.9 points.
This is inconclusive (`~`), not evidence sufficient to change the default.
Score-share difference is +1.11 points, standard error 1.21 points.

The committed firing artifact records the clean build commit, executable and
gene-set hashes, seed range and complete analysis. `python3 tools/gene_fires.py
--max 0` now reports 270/270 genes with firing evidence and zero unproven.
`python3 tools/continuous_screen_status.py /tmp/civvis-envoy-building-dividends.jsonl
--analysis docs/gene_screens/fires/envoy-building-dividends.json` verifies the
complete game and seat counts. The evaluation inventory was regenerated;
all 21 `test_eval_manifest.py` tests pass.
