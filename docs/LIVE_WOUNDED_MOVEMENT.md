# Wounded live movement

A sequence of planned movement steps used to become a single host destination
for every unit. Firaxis then selected its own route to that destination. For
a wounded military unit within three hexes of a visible military enemy this
can discard the safe detour CIVVIS evaluated.

The bridge now preserves that unit's individual planned steps. Current hosts
execute them using the existing per-unit queue, which waits for the previous
operation to complete. An older host receives only the first step; dependent
orders wait for a fresh observed board. Healthy travel and wounded travel away
from visible enemies keep the existing coalescing behavior. Peaceful rival
units do not trigger this guard. The reply reports `wounded_local_routes`.

This is an execution repair, not a new retreat policy or a claim that every
death is avoidable. Compare the retained postcondition and evacuation counts
alongside city retention in complete games. Host grounding: shipped
`Base/Assets/UI/Civ6Common.lua:163` calls
`UnitManager.RequestOperation(kUnit, UnitOperationTypes.MOVE_TO, tParameters)`;
the operation specifies a destination, not CIVVIS's evaluated sequence.
