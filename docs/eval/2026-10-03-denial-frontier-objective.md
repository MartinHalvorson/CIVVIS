# Prewar denial objectives and declaration reach

The native King/Online/Pangaea/Tiny game `civvis-20261003T052455Z` lost to
Spain's Culture victory at turn 182. This task addresses one observed planning
inconsistency, not a demonstrated victory or win-rate improvement.

## Evidence and scope

The experimental native decider (`308e13764a38174bc0179ebbfa2cf83c07cb24da`)
switched its campaign from Persia to Spain at turn 161. Its journal named
A Coruña, then Valladolid at turn 166, while reporting that no Spanish city
was within 18 tiles of a friendly settlement. The declaration code actually
checks the selected objective, not every city belonging to the rival.

Applying `hex::offset_to_axial` and `hex::wdistance` with the exported width
of 60 to turn-start observations gives A Coruña a nearest-friendly-city
distance of 21 and Valladolid 29. Bilbao was 12, Seville 16, Cádiz 15, and
ỉwnw 14; newly observed Cartagena was 10 at turn 165. Thus the journal's
empire-wide claim was false. These distances do not establish a passable
march, adequate force, or a safe declaration. The army was dispersed and
the turn-start observations contained no friendly bomber.

The canonical source also reproduces the mismatch. A fresh, stateless mirror
census at the last recorded frame of turn 161 selected Toledo (distance 20),
despite mapped Theater Squares in Bilbao (12), Seville (16), and ỉwnw (14).
The same out-of-range objective appeared at turns 162 and 166. It did not
declare in these samples. This is deliberately not described as reproduction
of the experimental runtime's exact strategy or memory.

The baseline is production-identical to main
`d8ddf9732d6442831527718534ad0851259278f3`, built in the isolated PR #3873
worktree at claim commit `8bb263f3b0ac6f2e07ec15aea9eea528442ac9b2`.
Both census arms use the same 23 canonical supported forced treatments from
the preceding clock audit, not all 29 treatments of the experimental runtime.
Their map and state come from matching event prefixes. Every sample creates
a fresh AI and calls `plan_observed_turn`; these are observation-limited
model decisions, not new Firaxis games or persistent-decider replay results.

The candidate changes the three out-of-range peace-time samples:

| Turn, last frame | Baseline objective / distance | Candidate objective / distance |
|---|---|---|
| 161, 2 | Toledo / 20 | Seville / 16 |
| 162, 2 | Toledo / 20 | Cádiz / 15 |
| 166, 2 | Toledo / 20 | Bilbao / 12 |

The remaining seven samples (150, 160, 165, 167, 168, 175, 180) retain their
objectives. The pre/post war state is identical between arms in all ten
samples; neither arm opens the Spanish war at 161, 162, or 166. Cádiz is a
frontier fallback, not mapped Theater Square infrastructure. Spain is already
at war with the seat at frame 1 of turn 167 in the original history; these
observations do not establish who initiated that war.

## Change under test

At peace, `victory_suppression_city` must use the same
`city_within_declaration_range` predicate as the declaration it is preparing.
Within that eligible set it retains the existing infrastructure preference,
loyalty deferral, campaign cost, and deterministic tie-break. With no eligible
infrastructure city it returns no suppression override, letting the existing
selected-rival frontier logic choose the first objective.

At war it keeps distant infrastructure available: the opening gate no longer
applies, and changing that policy could redirect an ongoing siege. Urgency,
lane policy, declarations, force staging, local strength, capture capability,
treasury checks, peace deadlines, and third-party-war gates are unchanged.
The range predicate is a necessary condition for the current declaration,
not a new assertion of terrain reachability.

## Artifacts

Retained outside the repository at
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-03/denial-frontier-pr3873/`:
frozen baseline executable and actual dependency library, census source,
forced-treatment file, and raw baseline census. The root library alias was
not used as the library provenance after subsequent Cargo test builds.

| Input | SHA-256 |
|---|---|
| Original native events | `d5dde288a1250d6f7820aab4a15af11aa15e3d5615965f52c1e224d3ef82a9dd` |
| Baseline `civvis_orders` | `2a706d61af66b2c09e7313d99ae5cc76189ad502f6d5c74ab7d4a99e19e4b059` |
| Baseline dependency library | `77893a54be80434cd65c5a0fc1d3c60d5ba6b5cea9ad525d81dd9bddeb1af8ed` |
| Candidate `civvis_orders` | `c75de362bd412dc2950cf69ab536383c1518a2a60d436034a8fedd209b46ee88` |
| Candidate dependency library | `383e55a81c937ef8db520733fcb489c250b9e711deadb28c5be5fa5b151895bc` |
| Baseline census output | `c97c3bf3aab4142bc36789a2f76f0db2e8caca541497575ea66dbba4b5f0c77f` |
| Candidate census output | `aa04f434a4f8663d2ba0ecb3f8c481162fb062ef8d069b9786db70cd3c9b845d` |
| Forced canonical treatments | `a79c0e54e708a92ccf8cd8b286da851fea43b643dff58cdedb8905ce74dfec3b` |

## Persistent native-history replay

Both frozen deciders also processed the complete unchanged native history
with `tools/native_city_route_replay.py`, the same forced treatments,
`--fresh-board`, and separate persistent AI memory. Both exited successfully:
526 state frames, one initial frame with no terrain/board, and 525 decisions
per arm. The full consumed-prefix hash matches the original event hash above.
Rebuilding the final source after the test-fixture corrections produces a
byte-identical `civvis_orders` executable to the frozen candidate above.

There are 18 changed actionable-order frames, beginning at turn 161 frame 1,
and 12 changed native-action frames. The host-order multiset differences are
39 added / 34 removed `MOVE_TO` orders and five added / two removed `FORTIFY`
orders. After excluding those two verbs and verification telemetry, the
remaining ordered host-action lists are identical in every frame. Neither
arm issues a war declaration. No combat, production or diplomacy difference
is being claimed. Later movement differences carry forward the differing
prewar decisions and AI memory; the fresh census's unchanged wartime samples
do not imply identical persistent trajectories.

These orders were not executed in Firaxis. Both arms receive the original
board at each frame, including the original outcome. More moves, fewer
internal move actions, and a different first objective are not evidence of
less travel, better survival, captures, disrupted tourism, or victory.

## Validation

- Five new tests pass. The initial fixture version failed three core
  assertions before the implementation; two compatibility tests already
  passed. The fallback fixture was subsequently strengthened to include
  two friendly cities and explicit urgency / not-staged assertions.
- The first full local run exposed four existing failing tests in two
  infrastructure-preference fixtures: 4224 library tests passed, four failed,
  and 49 were ignored. The old fixtures put their preferred objective outside
  declaration range. Their locations are now explicitly reachable and assert
  the range precondition; their lane, urgency, infrastructure, and target
  assertions are preserved. The new tests separately cover the far-only case.
- Corrected `cargo test --profile ci --locked`: 4464 passed, zero failed,
  53 existing ignores across six test groups. This is not a claim that
  deliberately ignored research tests have been run.
- `python3 tools/test_ci_wiring.py`: seven tests passed.
- The first independent CI paired-cost run at implementation head
  `58e8f5d65` passed: five pairs / 600 turns, median +0.13% per completed turn,
  inside its ±1% noise floor. No performance change in either direction is
  established. This is a computational-cost gate, not a strength evaluation.
- The source change is AI objective selection only, so engine soak validation
  is not applicable. The live game, native bundle and other writers' trees
  remain untouched.

Full local logs, the initial CI failure, cost output, replay decisions and
provenance are retained in the artifact directory. Fresh paired Firaxis
outcomes remain required for a native-strength claim.
