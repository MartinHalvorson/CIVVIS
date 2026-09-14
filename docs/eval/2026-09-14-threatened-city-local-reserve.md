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
  --games 12 --target-games 12 --start-seed 914374000 --jobs 4 \
  --difficulty emperor --out <run-dir>/reserve-reach.jsonl
```

The initial recipe specified two workers. Before any game started, scheduling
changed to four workers while retaining the fixed sample size, seeds and
controller. The frozen sample started at 13:59:06 UTC. Its header confirms
the planned map, difficulties, independent seats and all victory lanes.

Required checks cover the actual reserve wrapper, local discounts and host
refusals, unavailable purchase slots, low treasury, exclusion of siege/recon,
the unchanged threat signal and higher stock floors, version isolation and
the original regression test. The complete Rust suite runs in CI before
integration. Record the analyzer artifact and repository gene checks too.

## Results

The fixed sample completed all twelve games and 72 distinct seats on
2026-09-14 at 14:30:29 UTC; both simulation and analysis exited successfully.
It used clean source `5c8e75a7d6632ae5d218fcc31e13a809a5215c63`. The frozen
simulator SHA-256 is
`800bff49ce16088aa416cecd9affc92a66b45a08ebbdfbcdaa40aacc4bb6f348`;
the complete raw JSONL SHA-256 is
`363acd6bdf3c8e6f4843acc7ff025d5d4f78cb2db8e64e03295f77e8c162b81c`.
The analyzer is committed at
`docs/gene_screens/fires/2026-09-14-threatened-city-local-reserve.json`.

| Family level | Seats | Wins | Win rate | Score share |
|---|---:|---:|---:|---:|
| off | 50 | 7 | 14.00% | 16.10% |
| threatened-city-reserve | 13 | 3 | 23.08% | 17.98% |
| threatened-city-reserve-2 | 9 | 2 | 22.22% | 17.93% |

| Contrast | Win difference, pp [approx. 95% interval] | Score-share difference, pp [approx. 95% interval] |
|---|---:|---:|
| V1 minus off | +9.08 [-21.18, +39.33] | +1.89 [-1.95, +5.72] |
| V2 minus off | +8.22 [-26.17, +42.61] | +1.83 [-3.20, +6.87] |
| V2 minus V1 | -0.85 [-38.95, +37.24] | -0.05 [-6.67, +6.56] |

Intervals use analyzer standard errors clustered by game, without a multiple
comparison correction. These small-sample intervals can be unreliable. V2
executed in nine seats, but its point estimates are slightly below V1 and
the uncertainty is wide. This sample does not establish improved strength.
The candidate remains unpromoted, and the firing artifact is not added to
deployment ledger sources.

### Validation

All seven new tests passed locally through Cargo. The existing treasury
regression passed from the same built library test executable (one test,
zero failures). Its duplicate Cargo invocation was waiting behind the
simulator build; after the direct regression passed, only that redundant
waiter was interrupted. Both successful logs were preserved, and the build
continued without a restart.

CI on `5c8e75a7d` passed the Rust suite, documentation examples, tournament
and provenance regressions, lint/formatting and paired cost checks. Its
remaining failure was the then-missing V2 firing artifact. All fourteen
registry append-point tests and the generated ledger and evaluation manifest
checks passed locally. After integrating main, all fourteen append-point
tests passed again and `cargo check --locked --features developer-tools
--all-targets` passed. The complete firing artifact is checked before shipping;
final integration remains gated by required CI checks.
