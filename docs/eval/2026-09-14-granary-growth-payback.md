# Granary reservations that accelerate growth

`first-granary-reserve` starts a Granary whenever population is within one
citizen of housing. That includes cities without surplus food, cities whose
amenities or loyalty stop growth, and queues too slow to deliver another
citizen soon. Its last recorded pooled on/off difference is -0.18 percentage
points. That historical reading motivates this candidate and does not prove
why the original underperformed.

Version two keeps the same reservation slot and the independently testable
original version. It only reserves a legal Granary when:

- the city has surplus food and extra housing increases its growth rate;
- it is not the plan's threatened city or recently attacked;
- construction finishes before the next citizen would already arrive;
- construction plus the subsequent growth delivers the next citizen within
  30 Standard turns, scaled to game speed and capped by the remaining clock;
- that citizen arrives at least two Standard turns sooner than without it.

The forecast uses current yields, banked food, remaining item production and
item production multipliers. It credits only the housing lift, excluding the
Granary's extra food and any better citizen assignment. Normal production can
still choose the building when it does not warrant this forced reservation.
No deployment default changes.

Growth uses one implementation: `Game::city_growth_surplus` is extracted from
`process_city_with_upkeep`, which calls it with its existing memoized yields,
housing and amenities. The gene supplies the same readings and substitutes the
planned housing. Housing, amenity, loyalty, governor, pantheon, industry and
other growth bonuses therefore have the same arithmetic in the forecast and
in turn processing. The multiplication and addition order are retained.

## Evaluation fixed before execution

Run 12 standard-shape Emperor games, seeds 914371000–914371011, with the
family `first-granary-reserve,first-granary-reserve-2`. Seats independently
draw the family level, with all other genes at the evaluator baseline.
This is a reach and execution sample, not enough to promote a version on win
rate. Retain and report all three family levels, including adverse results.

```sh
cargo test --profile ci --locked
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes first-granary-reserve,first-granary-reserve-2 \
  --games 12 --target-games 12 --start-seed 914371000 --jobs 4 \
  --difficulty emperor --out <run-dir>/granary-reach.jsonl
```

The initial recipe specified two workers. Before any game started, scheduling
changed to four workers on this busy host; the twelve games, seed window and
controller stayed fixed. The frozen sample started at 13:04:15 UTC.

Separately compare frozen original and candidate ordinary simulator binaries
on four matched seeds, 914372000–914372003, 120 turns, at the standard map
shape using `tools/speed_ab.py`. Neither gene version is enabled in that
controller. Every game report must match to establish that the shared growth
extraction preserves play. On this busy host timing is diagnostic only.
The family screen checks the new behavior; it is not used to claim exactness.

Required tests cover construction-before-growth timing, banked production,
low food, growth shutoffs, a housing lift too small to change the growth band,
the deadline, speed scaling, threatened cities, and the actual reservation
before an owed Library. The existing original Granary test must still pass.

## Results

The independently randomized family sample is still running. This is an
unpromoted candidate.

### Shared growth exactness

The four fixed pairs completed at 13:16:27 UTC with exit code zero and the
same game report on every seed. Both arms completed all 480 turns. The
baseline was clean source `2e972f414cf48c25bff8b290091ba986653de0d9`, whose
Housing candidate is disabled in the ordinary controller; the candidate was
clean source `c931228e3e87c7fdc484b67166253431e3896e09`, before its later merge
from main. The binaries were frozen before either run:

- Baseline SHA-256:
  `2c7d0719112fe63b0956957e8d72a218d5b950d2d9d7a1144c515dcea4152f5d`.
- Candidate SHA-256:
  `7386778b6eb06fd19ec18dcdf465cb732feacb5443ef93fb4c04d098c6286d33`.

The median CPU difference was -0.29% per turn, pooled -0.01%, with an IQR of
2.96 percentage points and an estimated resolution of ±2.20%. Host load
ranged from 137.37 at the start to a peak of 198.10. These timings do not
establish a performance improvement. Report equality supports unchanged
engine play on these four games, not a strength claim for the new gene.

### Local checks

All six focused Granary tests and both shared-growth tests passed. The
library suite passed 3,491 tests with 49 ignored. That executable predates
only module ordering and closure formatting, with no semantic difference.
A redundant full local Cargo run was interrupted under host contention after
the library pass; it is not recorded as a full-suite pass. CI on merged
source `69ce6a4a3` passed the Rust suite, documentation examples, tournament
and provenance regressions, lint/formatting and paired cost checks. Its
remaining failure was the then-missing V2 firing artifact; later CI steps
still need to pass after the completed artifact is committed.
