# Preserve wounded routes beside hostile cities

## Native evidence and change

In King game `civvis-20261001T050754Z`, turn 78 frame 0, Swordsman
`2883605` starts at offset `(13,23)` with 75 HP. Its recorded plan walks
through `(14,22)` to `(13,21)` beside Västerås `(14,20)`, whose visible
ranged strength is 25 and remaining outer defense is 44/100. No enemy
military unit is inside the bridge's three-hex route guard. The emitted
walk names only `(13,21)`; host movement events before the next state
instead record `(13,22)` then `(13,21)`. This proves the intermediary
step differed, not that this difference caused damage or a loss.

At turn 151 frame 1, Skirmisher `4653081` has 4 HP at `(46,17)` beside
Mainz `(46,18)`, with no visible military units belonging to that rival.
Its planned steps `(46,16)`, `(45,15)` collapse to one destination. The
host reports a no-op before the next exported state. A whole-turn join
would mix movement from other decision frames; it cannot establish that
this particular request took an unsafe route.

The existing guard preserves the planned steps of wounded military units
near visible enemy units. It now also considers known hostile city centers,
completed unpillaged Encampments, and the visible military units of hostile
minors. These are positions already supplied to the planner. Remembered
city positions remain eligible without inferring their present strength.
The planner, goals, healing thresholds, and three-hex guard radius are
unchanged. Healthy travel still coalesces. Hosts with a per-unit queue get
the exact planned steps followed by any action; older hosts get the first
step and replan from the observed position.

The shipped game uses positional movement requests in
`Base/Assets/UI/Civ6Common.lua:163`:
`UnitManager.RequestOperation(kUnit, UnitOperationTypes.MOVE_TO, tParameters)`.
City and district strikes are native commands;
`Base/Assets/UI/WorldView/CityBannerManager.lua:1559` calls
`CityManager.CanStartCommand(pCityOrDistrict, CityCommandTypes.RANGE_ATTACK)`.
This patch preserves the planner's waypoints, rather than reproducing
Firaxis's pathfinding or guessing a city strike verdict.

## Recorded-history comparison, specified before replay

Baseline source is integrated main
`5f9beba1d3528c5d0406d3074d4be384ede4481b`. Its baseline binary was built
from the empty claim commit `60d0d3024861efb6d245651344ed8f853e14aafe`,
before adding the tests or changing Rust. SHA-256:
`be16fde397d95445cc69eb1f000b1ad19a144cc50214592927bc9ca6b4c3171c`.

Replay both complete historical games `civvis-20261001T050754Z` and
`civvis-20261001T024402Z`, using `tools/native_city_route_replay.py`.
Both arms receive the same event prefixes, a persistent agent, a fresh
observed board for each state frame, Gran Colombia, an assigned Domination
target, and the unchanged 23-option `deploy/live-force-on.txt` bundle.
Retain every response, source and binary hash, return code, and changed
actionable order. Count changes to the planner's native actions separately
from changes to emitted orders and verification receipts.

These games are reused recorded histories. Their future states remain fixed;
changed commands do not establish improved arrival, survival, city capture,
or native win probability. No controller option is promoted by this replay.

## Validation

Five of the six new regressions fail before the implementation. They cover
the two recorded city cases, hostile minor cities and units, a remote
Encampment, and the old host's first-step behavior. The ordinary-travel
control passes. After the change, all 201 `civvis_orders` tests pass.

The complete `cargo test --profile ci --locked --jobs 3` suite passes:
4,454 tests, zero failures, 53 existing ignores. The CI-wiring suite passes
seven tests. A simulator soak is inapplicable: the engine and AI action
choice are unchanged; the change is in native order translation.

## Completed recorded-history comparison

Both replay arms exit zero on both complete histories. There are 498 state
frames in `050754Z` and 465 in `024402Z`. Each history's first state precedes
its terrain export and produces identical empty replies, leaving 961 actual
board decisions across 963 requested state frames. The source hashes match
the replay-prefix hashes, so the complete event streams were retained.
The genome readbacks match exactly, and all planned native actions match
between arms.

Gameplay commands change on 34 frames in `050754Z` and 18 in `024402Z`.
Only movement changes: there are respectively 54 and 29 additional planned
`MOVE_TO` steps, and all commands other than movement match. This is command
conformance evidence on recorded states, not a survival or strength estimate.
The current planner does not reproduce every historical plan: on turn 78
it chooses a different destination from the archived controller. The regression
uses the recorded historical steps; the replay measures current source.

The candidate binary SHA-256 is
`e1dc944e2b001321d9310d3a5180dd341813f81a5d14040443e07e15e9a69b34`.
The unchanged 23-option force file SHA-256 is
`a79c0e54e708a92ccf8cd8b286da851fea43b643dff58cdedb8905ce74dfec3b`.
Source/prefix hashes are
`99bb98e4535e44911cd061b160bf814dc1f9d0a33fdb86dad3049956361f47a7`
for `050754Z` and
`9cf4452826704620e881c856516feea25cb1b8e32058096f8c723f6e91edb3e5`
for `024402Z`.

`command-comparison.json` independently counts gameplay changes from the
complete saved replies. The initial replay counter omitted `turn_verified`
from its metadata exclusions and reported 52/27 changed frames including
aggregate verification receipts. Those original receipts are preserved;
the final utility excludes all three `CivvisVerify.isVerdict` kinds and the
gameplay-only totals above are the independently recounted 34/18. Two initial
harness attempts also remain preserved: they stopped on the initial empty
board reply, before adding support for missing decision objects.

Evidence root:
`~/civvis-tactics-results/2026-10-02/wounded-city-routes-pr3870/`.
The live verification checkout is independently pinned to another writer's
branch. Its active game and checkout were not modified by this task.
