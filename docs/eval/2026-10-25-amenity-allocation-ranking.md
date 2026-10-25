# Luxury ranking: remove repeated intermediate map lookups

## Locator and proposed change, before behavior edits

A five-second CPU sample of 200 fresh cloned controllers on the first frozen
Aztec witness prefix (turn 185) points to city yields, citizen choices and
Amenity calculations. This locator uses frozen source `1bc9c751f`, with the
existing v1 policy, and supplies no speedup or whole-game claim. Its raw
source, invocation and sample are retained in the CPU-profile archive
(117688 bytes, SHA-256
`3d04715baf40577dda201b95ed5f7850a3a14f7328678263ee1661f8b499f4d3`).

The proposed change is confined to `luxury_amenity_allocations_uncached`.
Keep each city's need, ID and accumulated allocation together in a vector.
Sort by the existing integer need and city-ID key for each luxury, then
increment the distinct selected cities and their attached counters. Build
the final ordered map once. Preserve luxury order/copies, four/six-city
reaches, the all-cities shortcut and the order of local-Amenity evaluation.
A unique city ID resolves every tie, so an unstable tuple sort has the same
city order as the existing stable key sort. This is the existing model's
allocation; it proposes no new Amenities mechanic.

The clean behavior parent is `8b131f11070379326ef98b6b6fc95ce0e5cd5e86`. Existing independent ranked-oracle
and shortcut/mutation-scope tests in `src/game/luxury_allocation_tests.rs`
cover the allocation and remain the reference. Open #4134 and #3585 claim
`src/game.rs`; inspection of their actual patches finds no luxury-allocation
hunk overlap. The source claim is this routine only.

## Fixed comparison contract

Reuse the complete frozen corpus from #4124: eight verified native prefixes
and 48 assigned simulation snapshots, of which six were not reached. Every
verified file's bytes/hash rechecks before this experiment. Retain the six
missing assignments and every subsequent failure; use no replacement cases.
The native and observed-simulation probe sources are copied without changes
so both artifacts execute the same configured planner and timing boundaries.
The old `reservation_active` field is retained as a legacy annotation, not
an exposure or inclusion rule for this optimization.

Build and freeze the unmodified parent's CI-profile library and both probes
before editing production source. Use the same compiler, optimization,
debug assertions, source and dependency fingerprints for the candidate.
Qualify every reached case by complete resulting board, applied actions,
plan and reasoning equality. A discrepancy or 120-second call failure is
retained and stops that measurement attempt.

For both corpora, run three parent-versus-parent control rounds and five
parent-versus-candidate rounds, with one fresh controller per measured call.
Alternate order by case/round parity. Reconstruct and clone outside the
planner CPU clock. Retain every round, case, semantic output, failure and
negative timing. Report per-round totals, median total change and per-case
changes beside the same-binary noise. There is no whole-game, native-win or
cold-native-process speed claim from these fixed-board measurements.

Run the ranked-oracle tests and full `cargo test --profile ci --locked` on
the completed source. No implementation change or benchmark result exists
in this registration; the baseline library build is next.
