# A cheap first Seowon before a repeated Settler

The qualified fresh Deity Korea Science pilot `civvis-20261105T114538Z-current-writing-pilot-01` ended in a rival Science victory at turn 178, with no cities lost. It completed Education at 75, Chemistry at 133, Rocketry at 145, its first Spaceport at 165 and Satellite at 175. The completed independent audit verifies 453 canonical decisions and 7,878 actual captured reads. This one combined Writing/Foundation exploratory game establishes no individual attribution or population win rate.

At native 46/0, Gangneung had population 5 and production 11. Its first Seowon was legal in four native sites, cost 28 and took three native turns. Its queued Settler cost 85, had 20 production invested and took six more turns. Three cities were founded and another Settler was already walking. The existing Campus claim repeatedly journaled that it postponed the three-turn Campus because of the early Settler floor. This is a missed scheduling opportunity under the existing priority, not an incorrectly modelled game rule.

## Bounded change

Under the existing `campus-claims-the-free-slot-2` option, Korea pursuing Science can briefly pause that longer repeated Settler for a first Campus when its native quote is at most three turns. The early floor must still be open; at least two cities and a walking Settler must already exist. The plan must be Science or Expansion, without a rush, threatened city or major war. The native Settler quote must exceed the Campus quote. Missing host quotes preserve the old behavior.

The ordinary Campus picker and other claim protections still run. The six-city target is unchanged. The first expansion, a Settler finishing as soon as the Campus, other civilizations and victory targets, version one, and the midgame floor receive no new pause. A short queued Seowon is also protected against the early floor on the next native replan, when its foundation may exist with zero paid production. The existing production action banks the paused Settler's work.

## Native witness and original controls

The neighboring `.native.tar.xz` is 109812 bytes, SHA-256 `6f2aeb40a13c9bc50df64c04eb5093ff93baed02669f8037b4c0d219a8ada09e`. Extract it and run `python3.12 verify-witness.py .`; verification also passed after independent extraction. It contains the original canonical decision, captured read archive, actual observed genome/context, mechanical selection protocol, preparation source, native quotes and raw original-source cold replies.

The selected request is the earliest chronological frame-zero witness meeting the predeclared mechanical conditions and a byte-stable actual read sequence in that completed pilot. All 18 original reads match input SHA-256 `0991aec842a14c03e6ac47dbf203eeb018dfd17e99c62cdce3e916debfd75ce4`. Original source is `0ab72aa31ec43b2d127bf86ba67b7b1ee1a28c15`, executable SHA-256 `632d442409e028d97d4f411a6ba64eb8a88e57e6e89f61d4731307a4a9fd8c45`.

Both predeclared original-binary cold controls retain the Settler: neither proposes a production change in Gangneung. Each uses 17 actual reads identical to the selected prefix, its actual genome matches the observed pilot genome, and the full repeated orders/decision replies are stable. These fresh-private controls do not establish the original planner's complete private-history parity.

The future changed-candidate comparison is fixed to this input, with OFF/ON/OFF/ON roles for the existing Campus version-two flag. All roles must be retained before judging the effect; no replacement input is selected. There has been no changed-candidate query at this checkpoint. Native execution, saved research turns, expansion timing and victory strength remain unproved.

## Validation checkpoint

Scoped rustfmt and `git diff --check` pass. Four new tests cover the actual queue switch with banked Settler progress, zero-progress Campus protection with an OFF control that actually reclaims the queue, the first-city boundary, and the scope/expansion safety cases. The Campus module now has eleven tests; the early floor has four existing regressions. Rust execution, full CI, focused Mac tests and the fixed-input candidate comparison remain pending. No engine rule, native Lua, registry row, new flag or runtime force setting is changed.

## First CI correction

The first implementation at `024cd6460c34a86f3da63faab2acdc700a117752` failed the `no walker` guard in [CI42027767001](https://github.com/MartinHalvorson/CIVVIS/actions/runs/42027767001). The general empire census counts queued units as well as field units, so its Settler total did not prove a Settler was walking. The pause now reads the dedicated walking-only census. This is a definition error, not a stale cache.

The unshippable Mac run `42027911145` was canceled after this concrete failure and before any candidate query. Its original protocol and cancellation receipt remain retained. The selected native 46/0 prefix and original-source controls are unchanged; the repaired candidate must use the same input and predeclared four-role comparison. No replacement input or favorable result was selected.
