# Simulator performance

This note records the July 2026 simulator profile, the changes kept from that
work, the production-catalog follow-up, and the next optimization targets.
Percentages below are diagnostic signals, not an additive decomposition:
library routines such as `memcmp` and `memmove` are costs incurred by several
higher-level systems.

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

Four deliberately small changes produced that result:

1. Monopoly calculation now builds one connected-resource census per player
   and reuses it across every luxury and foreign player. Previously each query
   walked all owned city tiles again.
2. An ordinary adjacent `Move` no longer runs the first-monopoly transition
   scan after success. Moves onto tribal villages, meteor sites, or Barbarian
   Outposts still run it because their rewards can complete a technology and
   reveal a luxury resource. Other action kinds remain conservative.
3. `hex::disk` reserves its exact `1 + 3r(r + 1)` result size, avoiding growth
   copies in a primitive used by movement, combat, yields, and visibility.
4. A per-player/city production catalog retains the complete legal `Item` list
   across the separate read-only passes a city governor makes before acting.
   A successful `Game::apply` clears it, and game clones and saves start empty,
   so normal mutation and search branches cannot reuse a stale menu.

## Production-catalog follow-up

The production catalog was the narrow first experiment from the profile. Its
target is a real multi-city decision path: repairs, products, and wonder
fallbacks can each ask for the same city's legal production menu before a
single action is committed. Caching only the fully derived menu preserves the
existing item order and legality checks.

On a release paired measurement of 20 fixed-seed games (four major
civilizations, 44-by-28 map, no city-states, 200-turn limit, seeds 7,310,200
through 7,310,219), aggregate serial wall time fell from 44.57 to 41.34
seconds: **7.2%**, with a 6.8% median per-seed reduction. Baseline ran first
for ten seeds and the catalog build first for the other ten; the two subsets
improved 7.1% and 7.4% respectively. After removing the elapsed-time field,
all 20 paired terminal reports were identical. A separate 40-game,
eight-worker soak on the same map shape also had identical normalized reports
and reduced accumulated per-game time from 230.54 to 221.51 seconds (3.9%).

This is intentionally a targeted result, not a claim that every tiny
production query is faster. A short two-player throughput run was not
consistently improved; the retained cache earns its cost where a developed,
multi-city controller repeats the catalog scan.

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
| Reuse movement and routing state | Traversal class, neighbors, passability, path checks, entry checks, and routing zones form roughly a 6-7% family | Derive a traversal profile once per unit and use generation-stamped arrays/reused search buffers instead of tree maps and fresh vectors | Movement rules have many conditional abilities and diplomatic dependencies |
| Remove targeted allocation and copying | `memmove` 5.9% and allocator leaves are prominent; `units_at` alone is 1.4% | Add narrow `tile_occupied` and callback/iterator queries, then migrate read-only and `is_empty` callers individually | These costs overlap the systems above; a wholesale borrowed API showed no win |
| Reduce rollout clone cost | Clone alone is 8.5 us, 38% of the optimized no-fog clone-plus-move latency | Share more immutable state or prototype reversible branch deltas/undo for search | High correctness and determinism risk; cloning is currently the isolation boundary |
| Broaden visibility and yield memoization | Line-of-sight/world-stamp leaves are about 2-3%; yield and adjacency work is fragmented across roughly 3-5% | Extend epoch-keyed memoization and precompute stable adjacency facts | Smaller expected return and potentially high cache-invalidation complexity |

The string/key work is cross-cutting but invasive. With the production catalog
now retained, movement reuse is the next narrower benchmark-driven project.
These should be measured independently because removing one source of
comparison or allocation cost will also shrink the apparent library leaves.

Operationally, independent games should continue to use the existing outer
`--jobs` parallelism. Production runs should use `--release`; its thin LTO and
single codegen unit are intentionally more optimized than the faster-building
`ci` profile used for the comparisons above.

## Where the allocations are (2026-07-31)

Every earlier profile here is a *time* profile — `sample`'s leaf ranking, or
inclusive timers around suspect functions. Allocation had only ever been
inferred from `memmove`/`malloc` leaves, which say how much it costs but not
who is doing it. This is the direct measurement.

**Method, and it is cheap to repeat.** A counting `GlobalAlloc` plus a global
slot index and an RAII `Tag` that swaps the slot and restores it on drop, so a
tagged function is charged exclusively and nested tags are charged to the
innermost. It needs no thread-local — the probe is run under `--jobs 1`, and a
lazily-initialised thread-local risks allocating inside the allocator. Roughly
sixty lines in a scratch module plus one tag line per suspect, applied and
stripped by a script; it must not reach a commit.

**One 6p 74×46 150-turn game allocates 13,728,753 times for 3.09 GB.** Game
setup is 142k of that, so essentially all of it is play. That is about 5,700
allocations per seat-turn, and at a plausible 30 ns per malloc/free pair it is
roughly a tenth of the game's runtime — consistent with the allocator and
`memmove` leaves in the time profile.

| site | allocations | share | bytes |
| --- | ---: | ---: | ---: |
| `advanced_units` | 5,842,083 | **42.6%** | 1.70 GB |
| the rest of `AdvancedAi::take_turn` | 3,038,944 | 22.1% | 597 MB |
| `city_yields` | 1,256,361 | 9.2% | 129 MB |
| `units_at` | 1,076,368 | 7.8% | 4.5 MB |
| `wdisk` | 817,304 | 6.0% | 128 MB |
| `begin_turn` | 606,103 | 4.4% | 69 MB |
| `legal_actions_within` | 325,139 | 2.4% | 51 MB |
| `player_unit_ids` | 213,378 | 1.6% | 9 MB |
| `player_city_ids` | 182,495 | 1.3% | 7 MB |
| `route_step` | 89,702 | 0.7% | 292 MB |

**Two thirds of all allocation is the AI's own decision code**, and
`advanced_units` alone is 42.6% of it. That agrees with the long-standing
finding that roughly two thirds of runtime is `AdvancedAi`'s deliberation
rather than the engine's rules, and it says the largest remaining allocation
work is in `src/ai/advanced.rs`, not in `src/game.rs`.

**Read the count column, not the byte column.** `route_step` is the warning:
292 MB — the largest byte figure outside the AI — from only 89,702
allocations, because each search allocates two dense per-tile vectors. Its
leaf time is about 1%. Large, short-lived, lazily-faulted buffers are cheap;
volume of small allocations is what shows up in the allocator leaves.
`units_at` is the mirror image: 1.08M allocations for 4.5 MB, an average of
four bytes, because it clones a one- or two-element `Vec<u32>` out of the
occupancy map.

**What this does and does not license.** It ranks allocation, which is one of
the two things this codebase has repeatedly found to be worth removing (the
other being an expensive derivation that is recomputed). It does not by itself
predict a win: the record already contains a neutral result for removing ~30
`String` allocations per citizen plan, and a neutral result for a wholesale
borrowed `units_at`. Take the count column as a ranking of *candidates* and
still A/B each one interleaved.
