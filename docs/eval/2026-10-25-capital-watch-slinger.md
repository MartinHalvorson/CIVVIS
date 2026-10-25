# Candidate: arm a healthy Slinger under the capital warning

Default-off `capital-watch-arms-the-slinger` considers one nearby healthy
Slinger-to-Archer (or the civilization's Archer replacement) upgrade while a
watched stronger major remains at peace. The existing sighting/power warning
can arrive while the assigned Domination seat is still expanding, before its
named-offensive/wartime upgrade pass applies. The generic peaceful pass also
keeps a 120-Gold floor.

This candidate keeps city-saving purchases first, requires normal upgrade
permission and the actual offer, preserves civilian-bound guards and appointed
war/air packages, and stands down during major war, Recovery or other victory
lanes. HP must be at least 75 and the Slinger within eight tiles of the watched
capital. The remaining cash must cover 1.5 times the projected unit upkeep
bill, with strictly positive projected income after the existing discounted
upkeep delta. The successor's upkeep remains modeled, not a Native quote.
Only one proposal is allowed per turn. The new warning observation does not
implicitly enable the older wall research or production policy.

Native start 193342 is exploratory: its healthy Slinger has a 30-Gold quote
and 38/44 Gold at turns 20/21, is outside friendly territory at 22/23, falls
to 2 HP by 24 and upgrades after returning to friendly territory then.
Both budget and caller context need checking; this does not establish a
winning counterfactual. The existing offer reader follows the host's strict
upgrade verdict and price from `Base/Assets/UI/Panels/UnitPanel.lua:468–483`.
A selected local action is still only a proposed Native command.

## Development controls

Initial fixture compilation failed on a string/Name mismatch, retained in
the raw log. The corrected behavior-free hook on dfe61d5f8 passes four
controls and fails exactly five intended upgrade cases, including real turn
dispatch. Candidate version one passes all nine. Later versions preserve
the old wall behavior, exclude free-city war from major war, protect Recovery
and appointed budgets, and restrict the successor to the Archer family.
The final focused run passes **17 tests, zero failed**: nine new tests and
eight existing matching capital-related checks. Fourteen append-point
controls also pass. These are mechanism checks, not full Rust or strength
validation.

After inspecting native-permission precedence, the missing-technology
negative fixture first removes the authoritative host permission. The
original and updated test sources remain preserved; a contradictory modeled
technology state does not override a real host verdict in the offer reader.
The deployment ledger remains byte-identical and no gene positions move.
Generated metadata lists the candidate off and unmeasured.

The verified 48-member development archive preserves source versions, raw
failed/passing logs, the fixed selection and Native profile: 620,704 bytes,
SHA-256
`fc045600bf8e890ef636763abf755b7af574323c1846019a74deb52c5e0cb064`.

## Registered comparisons, not yet completed

The complete fixed selection covers all sixteen Native trial starts with
279 frames and 290 distinct captured reads. Every capture is complete;
there are no ambiguous observed frames or duplicate decision frames in the
selected set. Refused/territory cases and first/final/upgrade controls are
retained. No treated query has run yet. See the companion protocol for the
exact selection and planned Native/model comparisons.

The model profile is frozen from `civvis-20261025T211349Z`, the first
unarmed start after task registration. Its actual binary revision is
e0ecf3f3a13b844ae9015c15d0fa6e3e23d825a4 and SHA-256
`db1f925fdbcfc425d77b03b2b843b7d3ee0e60b52b73cddd0491bc55e9ee5a92`.
The profile header SHA-256 is
`08108a6e751e2a066df64d69cfac0db6ad6e0cd4e83c5c5786ef8225bf86abba`.
It has 497 active treatments, 462 screenable in this build and 35 not
represented by that instrument, listed in the archive. The model batch
cannot be described as an exact Native controller replication.

Adapter firing, whole games, full Rust/repository validation and a qualified
prospective Native trial remain pending. No default promotion or verified
high-difficulty victory is claimed.
