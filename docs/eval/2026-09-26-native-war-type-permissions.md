# Exact native war permissions

## New live evidence

King / Gran Colombia / four-major Tiny Pangaea run
`civvis-20260926T205122Z` is pinned to `4c3938afd`, including the
typed declaration dispatch fix (#3768). At turn 53, Inca's export said
`can_declare: true`, `DIPLO_STATE_FRIENDLY`, and both denouncement turns
`-1`. Three actual `war_refused` events named `DECLARE_FORMAL_WAR`,
with `permission_known: true`, while `at_war` stayed false.

The planner's religious-interception reasoning reported a successful
condemnation on its speculative board. That was not a native success:
Formal War was refused before the dependent action could be legal.
The archive establishes that Formal War was disallowed; it does not
establish which alternative declaration was allowed.

Source files are `events.jsonl` and `why.log` under
`~/civvis-civ6-runs/control/civvis-20260926T205122Z/`.

## Cause and correction

The exporter reported only whether **any** supported declaration was legal.
An older mirror compatibility shortcut turned aggregate permission into
`denounced_until = turn + 1` without a start turn. The engine consequently
treated Formal War as available and preferred it over Surprise War.

Shipped `Base/Assets/UI/DiplomacyStatementSupport.lua:167` checks each
selection with
`IsDiplomaticActionValid(selection.DiplomaticActionType, otherPlayerID, true)`.
`Base/Assets/UI/DiplomacyActionView.lua:414–415` dispatches the selected
Formal War session, not an arbitrary currently legal war.

The new `rivals[].war_declarations` list carries every supported native
statement and its optional boolean permission. A missing, throwing or
non-boolean read remains unknown. An observed typed list authorizes only
explicit true entries; unknown/missing entries are not permission.
Legacy exports without this field retain their prior behavior.

Both mirror paths refresh actor/target-mapped permissions on every board,
including same-turn frames, and clear omitted observations. New typed exports
never fabricate denouncement. Exact native facts apply only on the observed
turn and are not checkpointed. Normal simulations without host observations
keep their existing rules. Contact, living major targets, friendship,
alliance, peace treaty and existing-war guards remain in force.

The declaration name mapping is shared between legality and wire translation.
The native order handler still rechecks exactly the selected type and does
not silently substitute another war. Refusal cooldowns reopen only when the
same type changes from false to true; an aggregate change or unknown result
cannot reopen a different refused type.

## Validation and limits

Focused regressions cover rebuild/sync, host-seat mapping, no fake clock,
explicit Formal/Surprise permission, unknown facts, omission, expiry,
actor/target scope, apply/enumeration agreement, preserved guards, legacy
behavior and exact-type cooldown reopening. The Lua suite executes the
shipped exporter and verifies false/unknown distinction, stable ordering,
coverage of every supported statement and wiring into the rival export.

Full-suite and deployment results are recorded in the PR after validation.
This corrects an observed obstacle to war; it does not establish successful
native war/condemnation, city capture or a domination win. Those require
readback in a normally deployed game.
