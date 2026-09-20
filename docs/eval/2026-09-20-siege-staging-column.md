# Siege staging through friendly columns

## Evidence

Native King Gran Colombia verification `civvis-20260920T122005Z-cont4`
recovered from turn 104 and passed a repeated turn-112 stall. During turns
123–128 its Ulundi siege remained in Stage with no city-center attacks.
This game remained pinned to `d9ee52e87` throughout recovery.

A frozen replay through turn 128 contains 2,864 events / 6,041,078 bytes,
SHA-256 `e562378b04ff2ba137ebdce9344f537dd57c98d69329318b987b30978e2dd1e8`.
All 65 baseline order frames reproduce the native decisions exactly. Replaying
with the already-shipped unopposed-rally fix changes the turn-126 rally toward
Ulundi but changes no exported orders. A new baseline built from `7cd566ebe`
also reproduces all 65 frames' exported orders.

Inspection found a separate movement gap: `siege_stage_step` uses a legal
single-step route and otherwise holds. The general mover already falls back
to `pass_through_destination` and `path_walk_to`, because a unit may cross
friendly occupants without ending on their stacking layer. Staging bypasses
that general mover.

## Change

After no usable single-step staging route is available, try the existing
friendly-column path helper. It chooses a reachable, legally stoppable tile
that makes progress and stays at least `STAGING_FAR` (five hexes) from the
target city. Execute through `path_walk_to`, preserving the existing movement
history and refusal bookkeeping. Ordinary steps and the already-staged hold
remain unchanged. This changes no movement rule or live game binary.

## Validation

A narrow mountain corridor places a catapult seven hexes from the city,
a friendly swordsman six hexes away, and an open staging tile five away.
The single-step router has no route, while the whole-path helper confirms a
legal destination. Before the fix, the staging action fails to advance; after
it, the gun reaches the open tile and the screen stays put. A companion case
with only one movement point verifies the gun cannot stop on its screen.
All 24 siege-train tests pass after the change.
`cargo test --profile ci --locked` passes 3,710 library and 205 binary tests
(3,915 active tests; 49 library and four documentation tests ignored).
Formatting and whitespace checks pass; the branch contains current
`origin/main` (`7cd566ebe`) before readiness.

The same 65-frame replay, comparing binaries built from the same base and
with the same release flags and 19 forced genes, changes seven exported
frames: five contain actual action changes (123/0, 125/0, 126/0, 126/1,
128/0), and two contain only verification bookkeeping. The first change
replaces horseman 3473410's move to native (30,22) with (31,18), from an
observed start at (30,23), toward Ulundi at (29,12).

Both replay processes exit successfully. The before/after times (5.91 and
7.58 seconds) are observations under concurrent native play and compilation,
not a controlled throughput benchmark. Frozen observations after divergence
are the original game trajectory: they do not prove native execution of the
new commands, city capture, or a victory.

Artifacts are retained in `/tmp/civvis-recovered-staging-replay/` on the
MacBook Pro: input receipt, events, baseline/patched executables, full exported
orders, explain logs, and `column-comparison.json`.
