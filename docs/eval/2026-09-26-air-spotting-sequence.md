# Preserve cavalry spotting around bomber sorties

The requested maneuver is cavalry movement to reveal a city, bomber strikes,
then a capture approach or retreat using the cavalry's remaining movement.
`Game::do_air_strike` already checks `combat_target_visible_at`, which requires
current sight. Remembering a city's coordinates is insufficient.

The native order compressor kept an opening walk open across other units'
actions. For `cavalry MOVE_TO spot; bomber AIR_ATTACK city; cavalry MOVE_TO
retreat`, it replaced the first destination with the retreat destination and
deleted the last move. The aircraft never received the intended spotting
position in the exported sequence. With two initial approach steps, two bomber
sorties and a retreat, the same bug compressed all three cavalry moves into the
retreat position before either bomber action.

An AIR_ATTACK now closes the preceding open movement segments. Adjacent
approach steps still combine before that boundary; later cavalry moves remain
after the aircraft action on hosts supporting per-unit queues. Older hosts
retain the spot and defer the cavalry follow-up. Independent rebasing does not
close movement segments. This changes order compression only; it does not move
units, select a spotting tile, or make the asynchronous host execute different
units serially.

The air verb follows the shipped
`Base/Assets/UI/WorldInput.lua:2077–2078`:

```lua
if (UnitManager.CanStartOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, nil, tParameters)) then
    UnitManager.RequestOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, tParameters);
```

`CivvisQueue.drain` currently settles each unit's own orders. Preserving the
cross-unit list is necessary but does not by itself guarantee move → bomb →
capture/retreat execution. The coordinated policy must preserve cavalry movement,
observe real visibility before sending the sortie, and observe bombardment
before deciding whether to capture or withdraw. Existing same-turn replan and
combat frames provide the observation mechanism; their limits and triggers need
end-to-end verification. The ordinary planner currently finishes one unit's turn
at a time and ranks siege/air units before cavalry, so policy work remains.

Four regression tests exercise the exported order sequence: retreat after two
sorties, approach and capture after a sortie, older-host deferral, and ordinary
travel around an independent rebase. The first three fail on the unchanged
compressor; all four pass with the boundary. This is synthetic transport
evidence, not a demonstrated native combined maneuver or a win-rate result.
Full-suite and quality validation are pending. Native testing remains under the
active game agent's ownership; this work does not interact with the game tab,
order database or running controller.
