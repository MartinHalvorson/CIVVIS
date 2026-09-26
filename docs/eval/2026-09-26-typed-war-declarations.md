# Native war types — 2026-09-26

The live bridge discarded every casus belli and used the generic war
operation for major civilizations. The host export also used
`CanDeclareWarOn` as a veto on every declaration. These were two separate
obstacles to a Formal War against Canada.

## Native evidence

King / Gran Colombia run `civvis-20260926T185025Z` planned an Ottawa
campaign. At turn 78 its reasoning said “Declaring war on Canada,” but the
serialized native action was `denounce`, not a declaration. The host recorded
that denouncement from turn 79. Its `our_denounce_turn` remained 78 while
`can_declare` stayed false through turns 83–98. At turn 99 the denouncement
was gone. No war action was serialized in this run. The game ended at turn
135 with a rival Religious victory, 23 kills, seven losses, and no city
captures. Those combat results do not establish that the Canada campaign
started; the host's `at_war` remained false.

Source artifacts on this host are `~/civvis-civ6-runs/control/` followed by
the run ID and `events.jsonl`, `decisions.jsonl`, and `why.log`. The ladder
records decider revision `1db776f3667225ea7991387212363658621cb266`.
The old exports did not record typed permission, so they do **not** prove
which turn Formal War became legal. The counterfactual requires a new
native observation.

## Shipped contract and repair

`Base/Assets/UI/DiplomacyStatementSupport.lua:167` validates each major
action with
`IsDiplomaticActionValid(selection.DiplomaticActionType, otherPlayerID, true)`.
`Base/Assets/UI/DiplomacyActionView.lua:414–415` dispatches the selected
Formal War with `DiplomacyManager.RequestSession(..., "DECLARE_FORMAL_WAR")`.
The same view names the other base-game wars. The expansion replacement,
`DLC/Expansion2/UI/Replacements/DiplomacyActionView_Expansion1.lua:131–138`,
adds the expansion wars; Golden Age War specifically uses the session
`DECLARE_GOLDEN_WAR`, despite its action name `DECLARE_GOLDEN_AGE_WAR`.
`Base/Assets/UI/Popups/DeclareWarPopup.lua:76–80` reserves the bare player
operation for city-states.

The bridge now carries the chosen casus belli into the order, validates that
exact major action, and requests its native session. It never substitutes
another war type when permission fails. Unknown and joint-war forms cannot
silently become surprise declarations; a joint war needs its partner/deal
path. City-state declarations keep their existing operation and permission.

The exported `can_declare` now means at least one supported major war type
is valid. All refused means false; an unreadable permission with no known
legal type means unknown. The existing mirror veto still applies to an
explicit false. When the permission reopens, refusal cooldowns recognize
typed war orders. The postcondition still verifies the host's actual war
flag, independently of the session request.

## Validation and limits

The Lua regression failed against the original handler on the Canada-shaped
case: generic permission false, Formal War valid. It passes with the repair,
including exact-session dispatch, refusal of Surprise War, the Golden Age
session exception, treaty refusals, unavailable API, invalid target/type, and
city-state operation behavior. All 63 discovered Lua suites pass using
`lupa.lua51.LuaRuntime`; 130 control-installer tests pass. Rust coverage
checks the supported war names and aliases, host-seat mapping, rejected
unsupported forms, typed refusal reopening, and observed war postconditions.

No engine or strategic policy changes are made, so a simulator soak cannot
exercise this repair. Native follow-up must observe the merged bridge export
a legal typed action, dispatch the same type, and read back `at_war = true`.
This patch does not establish a win-rate gain, city captures, a domination
victory, or readiness to advance the difficulty.
