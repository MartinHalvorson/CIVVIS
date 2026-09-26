# Actual movement allowance for visible foreign units

## Observation and scope

Historical King game `civvis-20260926T211003Z`, turn 158 frame 0, exposed
Cuirassier 3997696 with zero remaining movement but no `max_moves`. The
export already included `GetMaxMoves()` for own units and barbarian hostiles,
but omitted it from both major-civilization and city-state unit records.
The Rust mirror already accepts the optional fact for all three rosters.

This is a two-line export correction, not a movement-rule or retreat-policy
change. Read the actual host allowance only for units the existing roster
visibility filter permits. A missing, throwing, or nil API stays unknown;
do not infer allowance from remaining movement, a nearby general, or combat.

Shipped authority on this Mac:
`Base/Assets/UI/Panels/UnitPanel.lua:2242` reads
`kSubjectData.MaxMoves = unit:GetMaxMoves();`.
The same script at line 3447 reads the defender's `GetMaxMoves()`.

## Regression coverage

The Lua regression executes both actual exported table expressions. It
failed before the patch with `foreign roster omitted the native allowance`.
Afterward it preserves synthetic allowances 6, 2.5 and 0 independently of
zero remaining moves, and leaves nil, throwing and absent APIs unknown.
CI discovers this `*_test.lua` through its existing glob.

Rust tests deserialize both foreign rosters, verify the reported allowance
on fresh reconstruction, same-turn synchronization and next-turn refresh,
and check that absent/invalid values clear an older host allowance back to
the existing rules fallback. Six in these tests is synthetic, not a claim
about the historical Cuirassier.

## Evidence limits

The old archive cannot recover a fact that was never exported. This patch
does not establish why the Cuirassier reached the retreating Bombard, nor
that a different move would have saved it. Actual new native exports must
be inspected after normal deployment; no mid-game reload is requested.
There is no native capture, survival, or domination-win claim.

Future verification uses the operator's updated Prince difficulty, Simón,
four-player Pangaea and domination target. Archived King evidence remains
labeled King. A simulation strength comparison is not applicable to the
Lua-only production change: simulated games never call this exporter.
