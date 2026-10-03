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

Validation and candidate measurements will be added before this PR is ready.
