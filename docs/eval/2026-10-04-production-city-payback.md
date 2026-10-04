# Early city production payback

Status: fixed prototype planned; not a strength claim.

## Hypothesis and frozen protocol

The existing named-lane production reservation pays for legal industrial
buildings but cannot start an Industrial Zone. Test a Domination-only
reservation that starts one district-plus-Workshop chain at a time, only
when its full remaining costs can repay before the game clock. Include
build time and subtract production from a worked plot being replaced.
Keep existing commitments and earlier emergency, growth, research and
solvency reservations. Require at least two cities, maintenance coverage,
and at least one fielded-or-queued military unit per city.

Control: unmodified source at the launcher checkpoint on main cbc4cf533.
Pilot: four Emperor seeds 61005800–61005803 and four Deity seeds
61005900–61005903. No tuning after observing these games. If a useful
early ramp survives without material science/culture or survival loss,
confirm on twelve fresh Emperor seeds 61005600–61005611 and eight fresh
Deity seeds 61005700–61005707; those reserved seeds were not run for the
previous experiment.

Native configuration: four majors, 60×38 Pangaea, six city states, Online
speed, all victory conditions, 150-turn cap, Gran Colombia focal seat with
Domination target and applied gene ledger; random fleet rivals, only the
focal seat exempt from the difficulty handicap. Capture every final
outcome and explicit missing checkpoints. These are native simulations,
not controlled Firaxis A/B games.

## Recorded Firaxis observations

`-host-observations.json` extracts the first complete state at turns 75,
100, 125 and 150 from the archived continuation of run
`civvis-20261004T070716Z`. Another writer controls that game; this task
only reads the recording. This private pinned controller differs from
public main, so it is diagnostic context, not validation of this patch.

At turn 75, five of six cities had housing growth multipliers below one.
At turn 100, all eight cities lacked a completed Industrial Zone; four
cities quoted a legal district at 102 production, requiring 7–17 turns.
Production was 75.70312 versus Scotland’s public total of 111.992 across
six cities. Own city sources contained Palace production, worked plots,
a domestic route, and amenity penalties, but no industrial contribution.
This establishes an unbuilt opportunity, not that building industry
causes a better result. Growth and terrain also limit the ramp.
