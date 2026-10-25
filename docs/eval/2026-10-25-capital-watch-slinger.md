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

## Controlled Native proposals

The registered selection covers all sixteen trial starts: 279 frames and
290 distinct, complete captured reads. All 580 adapter queries succeeded;
all observed genomes match the actual played profile plus only the new bit.
The source is clean commit 589c7bda1aba21fd215a44780f5c0265d004ef9d,
using one frozen debug-profile adapter. These are fresh queries without the
historical controller's persistent memory, not executed Native turns.

Eleven reads change proposed orders. Nine add an upgrade proposal, covering
eight frames and four distinct historical Slingers in four games; two only
reorder upgrades the older logic already selected. All nine incremental
proposals concern 100-HP Slingers. The other 279 reads remain identical.
No changed read removes a purchase order, but several movement orders change,
including a Settler's route at 193342 turn 21. Civilian guard preservation
is separately checked by the negative test. Repeated proposals for the same
historical unit cannot be summed into executed upgrades.

The complete 1,487-member archive is 495,892 bytes, SHA-256
`46c60bac4d340a460580c26ef12dfc6ad4fec616812114d38683410aa760cd14`.
Every archived file is checked byte for byte. Raw result 0186, turn 20 of
193342, is the direct firing witness: the same input and genome differ only
by this bit, and ON adds UPGRADE for Slinger 196608. The firing gate reads
the raw result and verifies the archive hash and controls. This establishes
proposal activation only. Captured input bytes remain in the sixteen already
committed Culture-air cohort archives; the manifest identifies their hashes.

## Registered whole games

The model profile is frozen from `civvis-20261025T211349Z`, the first
unarmed start after task registration. Its actual binary revision is
e0ecf3f3a13b844ae9015c15d0fa6e3e23d825a4 and SHA-256
`db1f925fdbcfc425d77b03b2b843b7d3ee0e60b52b73cddd0491bc55e9ee5a92`.
The profile header SHA-256 is
`08108a6e751e2a066df64d69cfac0db6ad6e0cd4e83c5c5786ef8225bf86abba`.
It has 497 active treatments, 462 screenable in this build and 35 not
represented by that instrument, listed in the archive. The model batch
cannot be described as an exact Native controller replication.

Eight fixed sampled model worlds finished at seeds 59006400–59006407,
with one measured Domination chair and three Immortal rival chairs each.
All eight measured seats lose: three Religion, three Science and two Culture.
The sampled gene draw is one ON and seven OFF. Its independent score-share
contrast cannot establish activation or strength; it is not added to the
pricing ledger or used as the firing witness.

All eight original sampled games reproduce exactly in their matched arm,
including all four final seat rows apart from elapsed seconds. Starting worlds
are identical in every OFF/ON pair and every rival genome is fixed. Seven
pairs have byte-identical full action traces and terminal worlds. In seed
59006402, ON upgrades Slinger 99 at turn 33 and changes the trace and final
world: zero versus one city lost and eight versus four cities retained, but
both still lose to Science (turn 127 versus 133). This one pair does not
establish strength or a conquest win. Two ON policy entries are observed
across the eight worlds; one coincides with the older pass's same upgrade
and leaves its full trace unchanged. Journal overflow reaches 8,205 entries,
so the journal alone is not an exhaustive activation census. Full action/world
comparisons and original reproduction controls remain the stronger evidence.

The verified whole-game archive contains 141 members and
1,682,276 bytes, SHA-256
`08c322c44f5310745312882703bbb280096f143a605fb35a57090534ff8b9b62`. It preserves original sampled rows,
analysis, unchanged-gameplay instrumentation, all matched initial/final worlds
and action traces, journals, dropped-entry counts and reproduction controls.

## Prospective Native work

The companion observer preserves the registered sixteen-start cutoff and
exclusive lease semantics. Its five isolated lifecycle controls pass. No
prospective trial is activated in this development checkpoint. Qualification
must use an actual delivered driver and binary after shipping; a disposable
empty-mirror capability probe must read back the new bit. Installed Lua bytes
are recorded with loaded identity explicitly unattested.

## Integration validation

The full library run on clean source 589c7bda1aba21fd215a44780f5c0265d004ef9d
has 5,974 passing tests and one spectator-server failure: its ephemeral port
was already in use. The isolated test passes in 2.31 seconds. The complete
compiled suite then passes with four test threads: **5,975 passed, zero failed,
52 ignored**, in 142.57 seconds. The original failure and both retry logs are
retained, rather than presented as an initially green run.

The main merge preserves all 692 existing positional gene rows, appends the
candidate last and retains both independently added policy registrations.
The candidate module, its tests and the shared watch module are byte-identical
to the files used for the recorded queries and games. The deployment ledger
remains byte-identical. The integrated type check, scoped formatting, fourteen append controls,
firing gate, gene metadata and evaluation manifest all pass. The five observer
controls pass again from their committed destination. Required CI still gates
the final pushed head. No default promotion or verified
high-difficulty victory is claimed.

The 14-member integration archive preserves all local logs, the
original port failure, isolated/full retries, old Native capability rejection
and source/registry/ledger controls: 131,700 bytes, SHA-256
`e320d59d95a721192304e1120b62e107395bb687941e357bb86d3dacd601323e`.
