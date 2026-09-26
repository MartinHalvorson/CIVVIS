# Prepare and sustain the Domination bomber wing

The supply purchase pass waited for both Advanced Flight and a finished
Aerodrome. Any positive Aluminum stockpile, any positive income, or an already
owned healthy mine then suppressed it. Those conditions do not establish that
a two-plane launch wing, or the aircraft subsequently built, can be supplied.

The pass now acquires a safe, reachable Aluminum plot while a live air beeline
has a finished or queued Aerodrome and the resource has been revealed. After
the breakthrough, the existing finished-field path continues without requiring
an active appointment. It prices upkeep for standing and queued resource users,
plus any missing members of the two-Bomber launch wing. A stockpile no longer
vetoes acquiring sustainable income. Existing unmined or pillaged deposits still
take precedence; a functioning mine can be supplemented when it is insufficient.
The same Domination, home-safety, treasury, nearby Builder, legal-purchase and
safe-route checks remain in force. Ground-unit resource purchasing retains its
previous thresholds and phase restrictions.

Read-only native evidence comes from King Gran Colombia four-player Tiny
Pangaea run `civvis-20260926T173411Z`. Its milestones were Industrialization
117, Flight 142, Radio 148, Advanced Flight 156, first Aerodrome 159, first
Bomber 172, second Bomber 184, and fourth Bomber 216. At turn 219 it held four
Bombers and a Helicopter, reported Aluminum income 2 and stock 23; the stock
fell by 2 each turn to zero at 231. All four Bombers had upgraded to Jet Bombers
by 226. This establishes a real late-game supply shortfall. It does not prove
that this map offered an additional safe, purchasable deposit or that this
change would have prevented the loss.

The same run proposed 78 AirPillage, eight AirStrike and 17 AirRebase actions,
yet recorded no city captures before the turn-235 Science loss. Proposal counts
are not distinct accepted sorties. The AIR_ATTACK receipt categories overlap
across frames and must not be combined into an acceptance denominator. In
particular, a zero-damage preview or `target_unharmed` receipt is not proof of a
failed pillage: Campus (22,15) changes from unpillaged at 174 to pillaged on the
next observed turn; an offshore Oil Rig at (14,22) similarly becomes pillaged
after the turn-223 request. The verifier's `target_harmed` checks units and city
health, not infrastructure layers. The shipped
`Base/Assets/UI/WorldInput.lua:2055–2078` uses AIR_ATTACK with X/Y, and
`Base/Assets/UI/Panels/UnitPanel.lua:3629` explicitly includes air-pillage in that
interface. These observations do not justify changing the native operation.
City targeting, infrastructure layers and capture-unit coordination still need
separate investigation; supplying aircraft alone has not demonstrated conquest.

During development, the untouched active run `civvis-20260926T190901Z` reached
its first Bomber at 159 with two finished Aerodromes, Aluminum stock 14 and
income 2. No game tab, native order database, live process or runtime was
modified. Runtime integration belongs to the game agent at a completed-game
boundary. Native summaries and input hashes are retained on the verification
host under `civvis-tactics-results/2026-09-26/bomber-aluminum/`.

By turn 165 that game had two Bombers but no Aluminum income. The visible
connected mine at (40,24) belonged to Taruga. At 160 we were its suzerain with
10 envoys versus Egypt's five; at 165 Egypt had 11 versus our 10, Taruga was
at war with us, and the resource income was gone. The observed resource census
offered no unowned Aluminum plot. Plot purchasing therefore cannot address this
specific interruption: protecting or reacquiring city-state supply is a separate
remaining requirement for the overall bomber strategy.

Validation: four new positive regressions fail on the original implementation;
all 16 resource-purchase tests pass with the change. They exercise pre-breakthrough
acquisition with finished and queued fields, small and large stockpiles, a
growing wing with an existing mine, queued Bombers/Fighters/Helicopters, and
the resource-reveal, commitment, sufficient-income, repair and unmined-source
controls. Existing Builder, treasury, threat, lane and ground-upgrade controls
also pass. `cargo test --profile ci --locked` passes 4,285 tests with 53 existing
ignored tests. Rust formatting and `git diff --check` pass; fetched main was
already integrated. No engine rules changed, so an engine-stability soak is not
required for this purchase-policy change. Integration validation is pending.

This is a supply-policy repair, not a measured win-rate improvement. The full
objective remains reaching the beeline reliably and converting the wing into
rapid major-city and capital captures in native verification games.
