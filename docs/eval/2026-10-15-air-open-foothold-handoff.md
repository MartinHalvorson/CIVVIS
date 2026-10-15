# One objective for the open foothold and its air support

Native Emperor attempt `civvis-20261015T030910Z` used the complete private
publisher `73d32f7fae9127982fcad968b0a5b58fad9abc6c`. It ended at turn 179
with a rival Culture victory (`won=false`) and no recorded city captures.
At turn 179 the journal repeatedly selected open Almaliq, then mustered task
force 144 for walled Qaraqoto. These observations identify an objective
conflict; they do not establish the cause of the defeat.

The public air overlay can replace a selected land objective with its ready
appointment. The subsequent campaign-adoption pass returns once the wing is
ready, so it cannot reconcile a newly selected open foothold. The regression
poses the existing public land predicate, a healthy capture force, a ready
Bomber wing, and an open city in range belonging to the same at-war rival.
It exercises both overlay and adoption in production order and requires one
shared objective while preserving the appointment clocks and phase.

Checkpoint `82f92c751` adds the regression before the repair. GitHub run
`38679267582` reached the intended target assertion after its fixture proved
wing readiness, capture-body readiness, air reach and the existing land
predicate. The overlay returned city 7 while the selected open city was 8.
The repair now reconciles the ready version-two
appointment before its overlay can erase the selected land objective. It
requires the exact existing capture-opportunity choice, the same at-war
rival, air reach and a land staging route. It recomputes readiness for the
new city while preserving the appointment phase and clocks. A city that
stops the rival's victory retains priority over this handoff.

Seven authored controls cover the shared objective, repeated frames,
existing siege and victory guards, home Recovery, another rival, air reach,
a launched rival's Spaceport, and incomplete or still-building packages.
All seven controls and the full 5,932-test GitHub suite passed on
`c1a83c989` in run `38680696825`, with 51 existing skips. The local full suite
will validate the integrated current-main source before readiness. The native cohort
also contains conflicts before Bombers exist, so the air overlay cannot be
assumed to explain every campaign/force mismatch. No native capture,
win-rate or highest-level improvement is established.
