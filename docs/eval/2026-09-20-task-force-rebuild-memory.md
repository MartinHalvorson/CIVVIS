# Task-force identity across native board rebuilds

The verification run `civvis-20260920T053144Z` uses `--fresh-board` and native
revision `73f12461d`. At turn 136 it still musters forces for Thăng Long, with
no captured city. Inspection found a concrete continuity defect, though that
observation alone does not prove it caused the stalled offensive.

The live bridge remaps unit memory, early-conquest memory, and the city campaign
across each reconstruction. The objective board also persists, but its forces,
objective keys, dependencies, city damage history, and requisition destinations
still held ephemeral board IDs. A reused ID could name a different soldier or
city; a changed city key could dissolve an otherwise surviving force.

The new bridge hook remaps cities by location and our units by the existing
host correspondence. Destroy objectives additionally use foreign host-unit
facts keyed by owner and native ID: the supplied unit correspondence only
covers our own units. Spatial objectives retain their keys, missing identities
are dropped, and surviving force IDs and formation turns persist. The board's
once-per-turn assessment clock is retained; derived force groups are rebuilt.
This is separate from PR #3601's siege-train memory and uses a separate bridge
insertion after the conquest-memory call.

Two regressions failed with the remapping hook empty: a force retained
`Siege(6)` when its actual city had become `Siege(5)`, and the home city's
pressure history was absent under its new ID. The tests exercise a real board
assessment before reconstruction and a new-turn reassessment afterwards, plus
all objective key types, dependencies, missing identities, and foreign ID
namespaces. One missing-city fixture initially removed only the city record;
it was corrected to use `mirror_remove_city` so the spatial index is consistent.
That fixture failure is not counted as production regression evidence.

The full suite passed before integration: 3,639 library and 204 binary tests,
49 library and four doc tests ignored. All six new identity regressions passed.
Integration validation is in progress.

A controlled replay compared `f04bd6633` with only this identity change, using
turns 1–160 of the frozen native export and the same 19 verification genes,
Gran Colombia, domination target, and `--serve --fresh-board --explain`. Both
binaries returned 160 decisions and exited successfully. Distinct land-force
IDs reported for the Thăng Long siege fell from 88 to four. Final actionable
orders changed on 63 turns, excluding `order_verified`, `order_failed`, and
`turn_verified` telemetry. This exclusion also corrects the earlier air-package
evaluation to eight changed-order turns; its three-to-zero Aerodrome
cancellation result is unchanged.

Artifacts: `/tmp/civvis-task-force-replay/` contains the frozen export, arguments,
binaries, and baseline/patched orders and reasoning logs. The export SHA-256 is
`5b1a6099604232f58c2a220b6f23838f2eaa9968a999eeadb3a7e21cfe73664f`.

Stable identities and changed recommendations do not prove every tactical
change is beneficial. Frozen exports cannot establish a capture or domination
win. Native outcome verification remains required.
