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

Implementation and validation are in progress. No native win-rate or
advanced-difficulty improvement has been established by this investigation.
