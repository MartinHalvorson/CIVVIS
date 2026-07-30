# Simulator performance

This note records the July 2026 simulator profile, the changes kept from that
work, and the next optimization targets. Percentages below are diagnostic
signals, not an additive decomposition: library routines such as `memcmp` and
`memmove` are costs incurred by several higher-level systems.

## Representative workloads

Measurements used the `ci` Cargo profile and an otherwise idle serial CIVVIS
process as far as the shared development host allowed. The full-game comparison
used ten adjacent baseline/treatment pairs with four major civilizations, a
44-by-28 map, no city-states, a 200-turn limit, and seeds 7,310,000 through
7,310,009. CPU time is reported because unrelated jobs made wall time noisy.

The rollout comparison prepared the same four-player, 44-by-28 game through
turn 100 at seed 7,310,002, then took the median of four adjacent 5,000-sample
runs for each operation. Headless games disable fog memory, so the no-fog rows
most closely represent simulator and search use.

| Rollout operation | Baseline | Optimized | Latency change |
| --- | ---: | ---: | ---: |
| Clone only | 8.55 us | 8.50 us | -0.6% |
| Clone + move | 50.10 us | 32.15 us | -35.8% (1.56x throughput) |
| Clone + end turn | 301.90 us | 277.30 us | -8.1% |
| Clone + move, no fog memory | 38.60 us | 22.45 us | -41.8% (1.72x throughput) |
| Clone + end turn, no fog memory | 305.35 us | 286.35 us | -6.2% |

Across the ten paired full games, aggregate user CPU time fell from 16.96 to
15.76 seconds, a 7.1% reduction. Every seed was faster. After removing the
elapsed-time line, all ten baseline and optimized simulator reports were
identical. A separate optimized soak completed all 20 requested games.

## Changes retained

Three deliberately small changes produced that result:

1. Monopoly calculation now builds one connected-resource census per player
   and reuses it across every luxury and foreign player. Previously each query
   walked all owned city tiles again.
2. An ordinary adjacent `Move` no longer runs the first-monopoly transition
   scan after success. Moves onto tribal villages, meteor sites, or Barbarian
   Outposts still run it because their rewards can complete a technology and
   reveal a luxury resource. Other action kinds remain conservative.
3. `hex::disk` reserves its exact `1 + 3r(r + 1)` result size, avoiding growth
   copies in a primitive used by movement, combat, yields, and visibility.

Two broader prototypes were rejected rather than kept without evidence. A
borrowed replacement for `units_at` did not materially improve adjacent rollout
runs because most consumers still need a stable snapshot while mutating the
game. Filling `WorldMap::disk`'s final buffer directly was neutral to slightly
slower. An older sphere-cache experiment also remains rejected: admitted-row
lookup improved substantially but cold local lookup exceeded the project's
regression gate.

## Largest remaining opportunities

A post-change main-thread sample contained 8,282 active samples. The profile
was taken immediately before the small `hex::disk` reserve change, which does
not materially affect this ranking.

| Opportunity | Current signal | Promising next experiment | Main constraint |
| --- | --- | --- | --- |
| Intern effect and rules keys | `memcmp` 9.2%, `Name::new` 1.4%, and `SpecMap::position` 1.2% as exclusive leaves | Intern effect keys when loading rules, then carry typed IDs through the hottest effect queries while preserving lexical serialization | Broad representation change; compare saved comparisons against conversion and indirection cost |
| Cache production candidates | `can_produce` 3.4%, unit purchase cost 1.8%, plus wonder, district, unit, and building candidate scans; roughly an 8% family | Build a per-city, generation-keyed catalog of producible items and costs for AI selection | Invalidation spans technology, civics, resources, districts, queues, diplomacy, and policy effects |
| Reuse movement and routing state | Traversal class, neighbors, passability, path checks, entry checks, and routing zones form roughly a 6-7% family | Derive a traversal profile once per unit and use generation-stamped arrays/reused search buffers instead of tree maps and fresh vectors | Movement rules have many conditional abilities and diplomatic dependencies |
| Remove targeted allocation and copying | `memmove` 5.9% and allocator leaves are prominent; `units_at` alone is 1.4% | Add narrow `tile_occupied` and callback/iterator queries, then migrate read-only and `is_empty` callers individually | These costs overlap the systems above; a wholesale borrowed API showed no win |
| Reduce rollout clone cost | Clone alone is 8.5 us, 38% of the optimized no-fog clone-plus-move latency | Share more immutable state or prototype reversible branch deltas/undo for search | High correctness and determinism risk; cloning is currently the isolation boundary |
| Broaden visibility and yield memoization | Line-of-sight/world-stamp leaves are about 2-3%; yield and adjacency work is fragmented across roughly 3-5% | Extend epoch-keyed memoization and precompute stable adjacency facts | Smaller expected return and potentially high cache-invalidation complexity |

The first two rows are the largest plausible code-level gains. The string/key
work is cross-cutting but invasive; the production catalog is narrower and is
the better next benchmark-driven project. Movement reuse follows closely.
These should be measured independently because removing one source of
comparison or allocation cost will also shrink the apparent library leaves.

Operationally, independent games should continue to use the existing outer
`--jobs` parallelism. Production runs should use `--release`; its thin LTO and
single codegen unit are intentionally more optimized than the faster-building
`ci` profile used for the comparisons above.
