# Native Bomber air-pillage target occupancy

This repairs an observed illegal mission. It does not establish stronger
native play, a successful replacement sortie, or a better game outcome.

## Native evidence and authority

The completed `civvis-20261003T060034Z` game was configured
King/Online/Tiny/Pangaea, Gathering Storm, Gran Colombia pursuing Domination,
with all victories enabled and no game modes. It used experimental decider
`14c3399f3730b8c6a3488d32dcc187022549c452`, not canonical main, and lost to
Sweden's Culture victory at turn 205.

At all three frames of turn 187, Bomber `8650780` had 100 HP, native range
10, 11 movement remaining, and one attack. Its base was offset `(27,17)`.
It requested `AIR_ATTACK` against the visible Canadian Campus `(36,14)`,
wrapped distance 10. Canadian Builder `8912902` occupied that Campus in
every decision-frame snapshot. Firaxis refused all three requests with
`can_start=false,no_reasons [p4r]`. These are three retries for one aircraft
unit-turn, not three independent available attacks, deaths, or lost sorties.

The canonical recorded-history decider also chooses `Action::AirPillage`
there. An independent matching last-frame native-board rebuild at turn 187
reproduces the error: Builder `114` (model ID), owner 3, at axial `(29,14)`,
Bomber range 10/distance 10, visible target, yet `AirPillage` is legal.

`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:1041` defines strategic
bombing eligibility: "if no land unit currently occupies the target location".
The old implementation reused ground pillage eligibility, which excludes only
the district owner's military garrison on districts, and does not exclude
occupants at all on improvements. That is insufficient for bombing a Campus
occupied by a Builder.

Air pillage still translates to the correct native `AIR_ATTACK` operation:
`Base/Assets/UI/Panels/UnitPanel.lua:3629` explicitly includes air-pillage plots
in the air-attack interface, and `Base/Assets/UI/WorldInput.lua:2077-2078`
requests `UnitOperationTypes.AIR_ATTACK` using target X/Y. There is no separate
air-pillage operation in shipped `UnitOperations.xml`. The initial suspicion
of an incorrect verb was ruled out before changing production code.

## Correction and regression scope

`Game::air_pillageable_at` now additionally rejects land-domain occupants,
including civilians and support units, regardless of owner, on both districts
and improvements. Ordinary modeled land units have no explicit domain;
explicit `land` is also recognized. It uses the existing tile unit index,
not a new full-board unit scan. Existing camp exclusion, ownership, war,
remaining pillage layers, health, range, and visibility checks are unchanged.

Both full legal-action enumeration and the aircraft doctrine enumerate through
this helper; direct action application checks it too. Seven regression tests
exercise occupied districts, occupied improvements, friendly/neutral occupants,
eligibility returning after the occupant leaves, unchanged ground pillage,
unchanged air strikes on garrisons, and air/sea domains not triggering the new
land-occupant gate. Four fail first against old production code; three controls
already pass. The air/sea controls isolate this gate on an improvement fixture;
they do not certify every native naval/air occupancy interaction.

No Lua mod, native game, orders database, runtime pin, or other writer's
checkout is modified. The other two in-range refusals in this original game
(turns 182 and 196) are not explained or claimed repaired by this change.
The distance-11 requests at 195 were separately addressed by PR #3874.

## Artifact scope

Source native events SHA-256:
`ad52fdcadb7de9f90ddfd7167350c792408d691d537573ef7f77bf3fda901516`.
Owned isolated-worktree builds, frozen binaries, native-evidence audit,
matching-board census, regression logs, and paired recorded-history outputs
are retained outside Git at
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-03/air-pillage-pr3875/`.

Recorded-history replay compares requests against identical original event
prefixes. Both arms carry persistent AI memory but receive unchanged original
acknowledgments. The same 23 supported canonical forced treatments are used,
not the original experimental runtime's entire 30-treatment bundle.
It is not a new Firaxis game or a counterfactual continuation of the original.
