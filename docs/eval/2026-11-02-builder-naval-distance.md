# Let land Builders pass a distance-only naval threat

The Builder capture guard used a barbarian ship's movement allowance as a
capture radius when the host supplied no positive path. Unlike the existing
Settler guard, it did not exclude ordinary land. The repair applies that same
terrain check to Builders, after accepting any positive native path. Water,
city centers, districts (including canals), unavailable terrain and land
threats retain their existing handling. This is a host actuation repair;
there is no new strategy flag or registry default.

Configured Immortal Science game `civvis-20261102T202907Z` records three
Caravel distance-fallback holds on owned ordinary land. At turns 96 and 97,
Builder 2490377 at (26,31) was held from moving onto Iron on desert hills at
(27,31), for Caravel 8519700 at (29,31) and then (30,33). At turn 108 another
Builder's approach to owned tundra at (24,37) was held for a Caravel. The
preceding tile exports report water=false, and the own-city exports have no
city center or placed district on these destinations. The extractor preserves
the exact native rows, source line numbers and hashes. These observations
identify false holds; they do not quantify lost production or saved turns.

The shipped rule is `Base/Assets/Gameplay/Data/Units.xml:804`: Caravel is
`DOMAIN_SEA`, with `BaseMoves=4`. The same domain/terrain exception already
exists in `CivvisBoard.holdVisibleBarbarianCombatCaptureLegs` for Settlers.
It is now also used in `CivvisBoard.holdVisibleBuilderCaptureLegs`.

The harness loads the real control script and exercises its `applyOrders`.
Before the repair, the Galley and Caravel plain-land cases and a case using
the native turn-96 coordinates fail; all other checks pass. Afterward all
186 host-board checks pass. Both ship types also retain holds for water,
city, district, unknown terrain and positive native-path cases. All 114
discovered mod test files pass locally under `lupa.lua51`; CI runs the
standalone Lua 5.1 interpreter. No Rust or simulated-engine behavior changes.

The companion JSON binds the 370,764-byte raw archive and all 19 members:
selected native rows, red/green logs, complete local Lua test results,
the shipped unit-row readout, baseline script, harness and source receipt.
This is evidence of the corrected host-harness movement decision, not actual
movement in the captured game or an increase in win rate. The Science fuel
pilot remains defined by its original plan; this change is recorded as a
separate runtime version if it reaches a later family.
