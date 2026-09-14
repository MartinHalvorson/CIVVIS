# An emergency reserve at the threatened city's actual price

`threatened-city-reserve` holds the dearest ranged land unit buildable anywhere
in the empire, otherwise its dearest melee unit, otherwise a fixed fallback.
Its production-cost-times-four quote ignores purchase discounts, local
purchase availability and host refusals. Siege and recon units can set that
bill. The historical pooled win difference is -0.04 percentage points in the
ranking; that reading motivates a candidate and does not identify a cause.

Version two preserves the existing threat signal and one-defender scope. For
each owned city named by the plan or the selected native emergency signal,
it asks `Game::unit_purchase_cost` for a Gold quote for ordinary land military
units, excluding siege and recon. It chooses the greatest of melee and ranged
strength, breaking a strength tie by lower price and then stable unit name.
The largest such bill across threatened cities sets the additional floor.

The engine quote includes local discounts, construction and resource gates,
the combat purchase slot, host prices and refusals. It does not require an
already sufficient bank. If no threatened city has a purchasable defender,
this gene adds no floor; existing stock and war reserves still bind. The
separate emergency buyer and treasury-at-work policy are unchanged. A current
defender or queued defender can therefore release this extra purchase reserve;
version one keeps its implementation for comparison.

This strongest-unit rule is a heuristic, not an optimal answer to every enemy
composition. It prices ordinary units, not Corps or Armies. The new version
is opt-in and both enable methods enforce one active version at a time.
No deployment default changes.

## Evaluation fixed before execution

Run twelve independently randomized family games at the standard shape:
six majors, Continents 74×46, nine city-states, Online 250 turns, all victories,
Emperor majors and Deity barbarians. Use seeds 914374000–914374011, all other
genes at the evaluator baseline. Retain off, version one and version two
cells, including adverse outcomes. This checks reach and execution and is
too small to establish stronger play or justify promotion.

```sh
cargo test --profile ci --locked --features developer-tools --lib threatened_reserve
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes threatened-city-reserve,threatened-city-reserve-2 \
  --games 12 --target-games 12 --start-seed 914374000 --jobs 2 \
  --difficulty emperor --out <run-dir>/reserve-reach.jsonl
```

Required checks cover the actual reserve wrapper, local discounts and host
refusals, unavailable purchase slots, low treasury, exclusion of siege/recon,
the unchanged threat signal and higher stock floors, version isolation and
the original regression test. The complete Rust suite runs in CI before
integration. Record the analyzer artifact and repository gene checks too.

## Results

Pending execution. This is an unpromoted candidate.
