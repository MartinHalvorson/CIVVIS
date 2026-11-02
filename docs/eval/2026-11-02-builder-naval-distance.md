# Let land Builders pass a distance-only naval threat

The Builder capture guard used a barbarian ship's movement allowance as a
capture radius when the host supplied no positive path. Unlike the existing
Settler guard, it did not exclude ordinary land. The repair applies that same
terrain check to Builders, after accepting any positive native path and
confirming that the ship has no active coastal-raiding ability. Water,
city centers, districts (including canals), unavailable terrain and land
threats retain their existing handling. Raid-capable ships and unavailable
ability or ability-metadata reads also keep the prior hold. This is a host actuation repair;
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
Its ordinary-land criterion is now also used in
`CivvisBoard.holdVisibleBuilderCaptureLegs`. The Builder guard additionally
reads the active host abilities through the API used by shipped
`Base/Assets/UI/Panels/UnitPanel.lua:2225,2267`. It follows their
`UnitAbilityModifiers` to `MODIFIER_UNIT_ADJUST_RAIDING`, as defined in
`Base/Assets/Gameplay/Data/UnitAbilities.xml:652–657,1053–1054`; it does not
infer raid capability from a list of ship names.

The harness loads the real control script and exercises its `applyOrders`.
Before the repair, five land-movement checks and the new classification-record
check fail, including the native turn-96 coordinates. An intermediate blanket naval exception fails eight
new raid/unknown-ability checks; those failures are retained. Afterward all
197 host-board checks pass. Both ship types also retain holds for water,
city, district, unknown terrain, positive native paths, raid abilities and
unknown ability reads. All 114
discovered mod test files pass locally under `lupa.lua51`; CI runs the
standalone Lua 5.1 interpreter. No Rust or simulated-engine behavior changes.
The 139 Python installation tests pass too. Their Settler-reach source check
now ends before the Builder function, rather than accidentally depending on
an unrelated local-variable assignment inside that function.
The workflow's type-name validator passes after giving the mock modifier IDs
a test prefix; their modifier types and tested behavior stay the same.

The companion JSON binds the 376,016-byte raw archive and all 24 members:
selected native rows, red/green logs, complete local Lua test results,
the shipped unit-row readout, baseline script, harness and source receipt.
This is evidence of the corrected host-harness movement decision, not actual
movement in the captured game or an increase in win rate. The captured holds
do not include the hostile ability list; actual live bypass still requires a
supported ability read, and unknown data preserves the hold. Future native
traces can use `builder_naval_land_clear` to verify that a
non-raiding classification was actually made; that event alone is not proof
of an executed move or improved yields. The Science fuel
pilot remains defined by its original plan; this change is recorded as a
separate runtime version if it reaches a later family.
