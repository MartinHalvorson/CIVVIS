# Luxury research with a usable payoff

Version one of `connect-the-luxury` ranks an improvement's printed technology
cost whenever it lists an owned, unimproved luxury. The resource can already
be supplied by a city center, another improved copy, a trade, or a city-state;
the tile can be flooded, occupied by a district, the wrong land/water domain,
or refused by the host. The rule still interrupts the lane's research.

`connect-the-luxury-2` is a separate, off-by-default challenger. It requires a
current empire Amenity deficit and a missing, visible, unbanned luxury. If any
owned copy is already connectable, research is not that resource's bottleneck.
For each candidate technology, it adds the missing prerequisite chain on a
speculative board and asks `Game::valid_improvements` for a Builder connection.
This retains the engine's civic, unique-improvement, terrain and host-refusal
rules. It does not alter the real board during planning.

The new rule ranks distinct connectable luxuries per remaining Science, capped
by the empire's deficit and a luxury's city reach. The bill includes game
speed, prerequisites, earned Eurekas, China's extra boost, and current research
progress without crediting a boost twice. A detour must fit within twelve
Standard turns of current city science. Ties prefer lower cost, then the
technology's name. This is a research heuristic: Builder travel and production
remain the existing Builder planner's responsibility.

## Fixed evaluation plan

Registered before games run on 2026-09-14. No sample size or seed window changes
to obtain a favorable sign:

1. A 12-game family probe on seeds 914356700–914356711 at the standard Emperor
   shape, naming both `connect-the-luxury` and `connect-the-luxury-2` so a held-on
   sibling cannot erase the treatment. This establishes that the challenger
   reaches complete games; it is too small to establish strength.
2. A 120-game whole-registry screen on disjoint seeds 914357000–914357119 at
   the same shape and ordinary independent random-genome draw. The family table
   reports each version against off and v2 against v1. Improvement remains
   unproven unless both win contrasts are positive; uncertainty is reported
   regardless of sign. This exploratory sample does not move a deployment
   default or erase the original version.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes connect-the-luxury,connect-the-luxury-2 \
  --games 12 --start-seed 914356700 --jobs 3 --difficulty emperor --out FAMILY_ROWS
target/ci/gene_screen --analyze FAMILY_ROWS \
  --json docs/gene_screens/fires/connect-the-luxury-2.json
target/ci/gene_screen --games 120 --start-seed 914357000 --jobs 4 \
  --difficulty emperor --out SCREEN_ROWS
target/ci/gene_screen --analyze SCREEN_ROWS --json SCREEN_SUMMARY
```

## Validation and results

Pending execution. Focused tests cover the actual research entry point,
first-copy and imported resources, Congress bans, tile/host legality,
already-connectable resources, distinct-resource valuation, prerequisite and
Eureka costs, and mutually exclusive version toggles. The existing v1 research
test remains unchanged. Full Rust and repository gene gates will run before
integration.
