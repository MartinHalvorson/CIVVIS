# Keep the opening army out of new Settler escort assignments

## Recorded failure

Native run `civvis-20260920T101201Z` declared against Spain on turn 29.
The opening had reserved three shooters and two melee bodies. On turn 30,
a newly departing Settler recruited Archer 655364 as its guard. The opening
already excludes existing civilian guards when choosing its force, but the
reverse ownership check was absent: a new civilian could recruit a unit
from the declared opening after its army had been committed.

The recorded Madrid attack never captured a city. This allocation bug is
one contributor, not proof that preserving this Archer alone wins the war.

## Change

New land escort selection excludes surviving members of a declared conquest
opening while the named war and enemy-owned city still stand. This applies
to ordinary formation escorts and the native bridge's formationless guards.
Existing civilian guards remain assigned. The restriction is absent with
the early-conquest flag off, before declaration, after peace, after capture,
or once the opening is released. No city-defense or combat-danger rules
change.

## Frozen observation experiment

Baseline and candidate were built from the same source revision with only
this change between them. Both replayed the native observation stream with
the same 19 forced verification genes and explicit Domination target.
All 496 frames through turn 180 exited successfully in both arms
(140.96 and 140.90 seconds; not a speed claim).
After removing telemetry (`order_failed`, `order_verified`, `turn_verified`),
12 exported frames and 10 internal native-action lists changed.

The first changed frame is turn 30/frame 0. Archer 655364's host movement
request changes from (26,20), back toward the Settler, to (25,24), toward
Madrid at (22,27). In axial hex distance, the new destination is four tiles
from Madrid instead of seven. The candidate's civilian path still invokes
the ordinary threat avoidance; its why ledger records a barbarian-reach
sidestep that turn.

These are alternate requests on frozen observations. Later input frames
still describe the original game; they do not prove host adoption, changed
city damage, a capture, or a domination win.

All exported changes are unit movement or fortification requests, on turns
30–33, 40, 41, 49 and 50. There are no later action differences through
turn 180. All five focused tests pass. The complete `cargo test --profile ci --locked`
run passes: 3,692 library tests and 204 binary tests; 49 library and four
documentation tests remain ignored. Formatting and diff checks pass.
Engine soak is not applicable: only AI assignment changes.
