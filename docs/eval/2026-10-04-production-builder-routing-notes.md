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
