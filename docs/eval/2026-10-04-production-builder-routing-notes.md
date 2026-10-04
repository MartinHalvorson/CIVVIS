# Builder routing diagnostic under the enabled live controller

Two previously consumed Deity maps (61007101 and 61007102) replayed with the
focal public Domination controller using `enable_live_bridge()` and fixed stock
rivals. Their 66,455 authoritative actions and both final saves match the
frozen control byte for byte. Journal recording changed no gameplay in these
replays. This is diagnostic coverage, with zero fresh strength samples and no
verification in Firaxis Civilization VI.

In map 61007101, Builder 362 stood on a worked hill at (29,6) from turns 63–69.
The initial-turn decision view accepted building a Mine there, adding one
modeled Production. Planning repeatedly preferred the Horses at (31,9), but
no authoritative action for that Builder was recorded during those seven
turns. At turn 70, it moved toward that resource. The resource premium could
be useful for military supply; these traces do not establish that the chosen
resource job is unnecessary. The mismatch between planning and execution is
the next diagnostic target, before changing job scores.

Builder 182's adjacent capital hill also costs its full movement allowance to
enter, so its Mine cannot be constructed in that same turn. At turn 82, when
it reached the hill, the planner recorded a capture threat and it retreated.
Several other departures from productive tiles also record a capture threat.
No safety threshold has been relaxed.

Initial-turn clone legality does not establish future capture safety. The
nearby hostile list omits enemies beyond three tiles and remembered threats.
Thoughts record planner proposals, including hypothetical actions the executor
may refuse or skip; they are not a record of executed orders. The raw files
and hashes are catalogued in the adjacent trace-results JSON. Map 61007101 evicted 750 older thoughts from the bounded journal ring, but
the per-turn drains captured contiguous IDs 1–6750 through the final cursor.
Map 61007102 captured IDs 1–4434 with no ring evictions. Both report zero
truncated turns; no thought IDs are missing from the saved streams.

## Deferred-order projection

A second observer ran a separately cloned first-frame plan on turns 60–100 of
map 61007101 and replayed that frame on an authoritative clone, following the
native executor's stopping rules. The original game still used the unchanged
production executor. Its 34,454 action bytes and final save match the original
trace exactly. These 41 projections are not telemetry from later real replans.

On each turn 63–69, the first-frame clone refused a zero-Gold peace proposal
to player 2 with `invalid diplomatic deal`, stopping before Builder 362's move
to (29,7). The builder had a proposed move on every one of those seven tails.
In `src/game/actions.rs`, `do_propose_deal` validates before creating a pending
deal; every error returns before the state writes. A refused proposal changes
neither diplomatic access nor the tactical map. The executor currently treats
that refusal as a reason to stop all subsequent unit orders, unlike a refused
financial trade. A narrow executor repair can therefore preserve independent
Builder and military work after the refusal and refresh after the batch.

This finding applies to the native observed-player executor. The live Civ6
bridge exports peace proposals as host peace orders through its own adapter;
this diagnostic does not demonstrate that adapter has the same failure.
