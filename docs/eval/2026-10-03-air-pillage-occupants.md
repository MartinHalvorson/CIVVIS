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
(turn 182/frame 0 and turn 196/frame 0) are not explained or claimed repaired
by this change.
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

## Recorded-history result and local validation

Both persistent deciders exited zero after 584 observation frames, including
583 board decisions. Both consumed the full original history with the exact
source hash above. The arms used this task's own ci-profile builds of canonical
main `f5e8c3092d01edb54281bae9d683b4c1a893dccf` and the correction, not
a sibling worktree's build cache. The matching-board census changes legality
from true to false without changing range, distance, visibility, or occupants.

Ordered host actions differ on 12 frames; internal actions differ on four.
The first host difference is turn 182/frame 1: our Cuirassier `9109526` now
stands on Campus `(36,17)`. Baseline proposes air pillage through that friendly
land occupant; candidate targets unoccupied `(35,17)`, distance 8. This is a
later recorded frame, not an explanation for the preceding native refusal at
182/frame 0, when no occupant was exported at that target.

At all three frames of 187, baseline reproduces the refused Builder-occupied
Campus request. Candidate targets `(34,13)`, distance 9, with no observed
occupant. Across the entire replay the host-order multiset removes nine and
adds seven `AIR_ATTACK` requests, all for Bomber `8650780`. Four removals are
occupied targets (one friendly Cuirassier, three Builder retries). The other
five removals, at frames 0 through 4 of turn 201, are range-valid requests
against an unoccupied target. Those dropped requests are not claimed as
beneficial: the arms' different request histories change their cooldown state
while both still receive the original game's acknowledgments.

All seven additions fit observed range 10 and target plots without observed
occupants. Three additions at 188 return to `(36,14)` after the Builder leaves.
The 12 changed frames span four aircraft unit-turns; repeated frames are not
independent available attacks. After removing `AIR_ATTACK` and verification
telemetry, every ordered host-action list is identical across the arms.
None of the replacement requests were executed in Firaxis. Full native target
permission, consumption, infrastructure damage, survival, conquest, denial,
and a better terminal outcome remain unproven.

Local validation:

- `cargo test --profile ci --locked`: 4,476 passed, zero failed, 53 existing
  ignores (49 library and four documentation).
- Seven focused regressions passed, after four failed first on old production
  code. Changed-line Rust quality passed on all three changed Rust files;
  seven CI-wiring tests passed.
- Four 6-major/9-city-state, 74x46 Continents/Online simulator health games,
  250-turn cap, seeds 261003750–753, jobs 2: all four completed. These are
  simulator health checks, not native strength measurements.
- Full-clock paired cost reading, four pairs at the same 250-turn shape,
  seeds 261003770–773: both arms' game reports match and each completes 967
  turns. Median +0.60% CPU per completed turn is inside the +/-1% noise floor.
  The 7.36pp IQR resolves only +/-5.45%, wider than the 5% budget on this busy
  host (load 42.56 -> 26.91). This does not establish a speed change or rule out
  a 5% regression; independent CI cost validation is still required.

Frozen baseline/candidate orders-binary SHA-256 values are respectively
`6bbc47900dbbbc48743f6f20817a5ef335aef81d92456be8d1d285cd9c0628af` and
`92853ec56455838d73cd44bce13c57aff27928d4578d7a4de293d9d6a45e15a7`.
Frozen baseline/candidate actual library SHA-256 values are respectively
`5de160fc65803080bba468597b59f1ff4d7a77a3ed17449a5667159355072930` and
`8f6632f7f9c4d2ff9538b3241aa15d500cdfd0a50d968a13d2df44cd97e402d8`.
Force-file SHA-256:
`a79c0e54e708a92ccf8cd8b286da851fea43b643dff58cdedb8905ce74dfec3b`.

The broader objective of superhuman performance in real Firaxis verification
games remains active and unproven. The newer completed run
`civvis-20261003T074953Z`, experimental runtime `b5fa2206`, also lost to
Culture at turn 166; it did not contain this correction.
