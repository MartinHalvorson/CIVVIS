# Safe siege muster detours

The native Emperor attempt `civvis-20261014T195934Z` repeatedly held the
Lagash column more than ten tiles from its objective. This is a diagnostic
trigger, not proof that every hold was erroneous. The frozen event prefix,
recorded decisions and explanations are retained under
`~/civvis-tactics-results/2026-10-14/safe-muster-detour/native-prefix/` with
byte counts and SHA-256 digests. The source game continued unchanged.

The completed attempt lost to Scotland's Culture victory at turn 203.
Across 572 native state frames, Ur was first observed held at turn 148 and
Isin at turn 178; no foreign major original capital was observed held.
Antananarivo is a captured city-state capital and does not establish major
capital progress. The complete event stream's digest and outcome observation
are retained in `completed-native-observation.json` beside the prefix.

The existing `muster-walks-the-road` treatment ranks safe endpoints by a
terrain-and-access road that ignores enemy fire. A road through another
city's strike ring can therefore make every safe detour look farther away.
The muster then holds indefinitely even when a safe route reaches the band
that counts toward readiness. A deterministic scenario places a neighboring
walled city over the shortest road and a longer southern road whose first
step moves away from the objective.

Fresh reconstructions at turns 160 and 166 found zero danger on several
reachable candidate tiles. These reads do not reproduce the persistent native
AI memory or the exact decision frame that printed a hold. They do not prove
that enemy fire caused the native Lagash holds. The hold explanation now says
that no safe move completed, rather than asserting that every closer tile was
too dangerous.

The implementation uses a safe-road distance throughout a distant muster
approach under `muster-walks-the-road`. Its reverse flood starts on safe,
passable land within the existing ten-tile readiness band and excludes enemy
ranged or structure fire and danger above half the member's health. Existing
territory access rules remain authoritative. Only reachable, unoccupied safe
endpoints can be selected. The flood keeps the existing 128-step bound and
stops at the member; members already in the readiness band do not run it.

The first fallback prototype successfully moved two tiles onto the detour,
then followed the ordinary terrain distance back on the next turn. A retained
per-turn trace exposes four repetitions of that cycle. Consistent safe-road
ranking addresses the return step as well as the initial local minimum.
The regression requires the member to reach the readiness band over complete
turns while never ending under fire. Its cloned-board precondition verifies
that the first safe `MoveTo` actually executes. Blocked-detour and disabled
treatment cases must continue holding.

The controlled routing fixture disables random weather so its zero-danger
assertion measures the posed approach. All 16 muster tests passed in the
focused local run. The complete local `cargo test --profile ci --locked`
run passed all 5,918 tests, with 51 existing ignored tests and four ignored
documentation examples. Independent CI on `d323d2389` passed the same default
suite, 71 gene-screen, 21 live-differential and 24 evaluator tests, and all
other required checks. Logs remain beside the frozen evidence.

No native win-rate or advanced-difficulty improvement has been established
by this investigation. The existing treatment remains opt-in; native
adoption and outcome verification are separate work.
