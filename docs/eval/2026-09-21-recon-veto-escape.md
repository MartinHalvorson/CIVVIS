# Recon escape after a lethal-attack veto

Native run `civvis-20260921T200755Z`, pinned to `0f410c021`, ended at turn 194
with Sweden's Religious victory (`victory: 4`, `team: 3`, `won: false`). Our
first Bomber appeared at turn 192 and we held no enemy original capital at the
end. This is a verified loss, not evidence of improved win rate.

The original Scout, native id `196608`, remained at offset `(7, 25)` from turn
54 through 103, at 100 HP with four movement points. The journal repeatedly
says its attacks would be fatal and holds it in place; reported standing
danger was 41. A fortified barbarian Spearman and naval contacts occupied the
nearby area. The first observed rival city was Xi'an at turn 121; Linköping
appeared at turn 132. These dates identify a reconnaissance failure, but do
not prove that freeing this Scout would have discovered those cities earlier.

`rotate_wounded` holds a healthy unit whose attacks were all vetoed. It already
allows a safe approach step for a siege reinforcement, but a free Recon unit
has no corresponding escape. The change preserves the attack veto
and considers a legal move to a tile with no predicted reply damage before
ending the Scout's turn. Existing civilian reservations and frozen exploration
settings remain authoritative.

## Validation

- The focused regression fails on the old code because the Scout stays put,
  despite a reachable zero-danger tile. Five focused tests pass, including
  civilian reservations, frozen exploration, no safe exit, and non-Recon units.
- A multi-turn test runs the battle planner and explorer for eight turns. The
  Scout reveals more than ten new tiles, ends at least five tiles from its
  starting point, and leaves the enemy unharmed. It does not merely relocate
  the indefinite hold to an adjacent tile.
- Integrated full Rust suite: 4,120 passed, zero failed, 53 ignored across six
  suites (`cargo test --profile ci --locked`). Fourteen append-point checks pass.
- Eight four-player, 180-turn simulation soak games completed, seeds
  369000–369007. This ran on the same AI before integrating the parser-only
  change #3689; it is a stability check, not a native victory measurement.

## Paired native replay

The frozen 535-frame input is
`/tmp/civvis-recon-veto-replay/native-events.jsonl`, SHA-256
`7c177196218c286d3954211dbc8826a477c8e333a4131503f9dc2c6a64ddd7e9`.
The final pair compares parent `965c350d50238693701fb3f5388387faed274377`
with integrated candidate `6ef99cca6` (the subsequent commit only adds the
multi-turn test). Both contain the parser optimization. Binary hashes are
recorded in the replay directory's `provenance.json`.

Both sides complete all 535 frames. The candidate changes 27 exported frames
and 42 internal-action frames. Scout commands change in 18 frames: twelve new
`MOVE_TO` orders, fourteen new `FORTIFY` orders, and one removed `MOVE_TO`.
At turn 54 it orders offset `(8, 24)` instead of holding at `(7, 25)`. The
planner predicts no reply damage at the destination. Heavy Chariot routes also
change on four frames at turns 95–96 as the projected unit positions change.
The earlier isolated pair produced exactly the same comparison.

The recorded future still leaves the Scout on its original tile, so eleven
new `did_not_move` receipts are counterfactual replay artifacts, not observed
execution failures of these new orders. The replay demonstrates changed
orders on the failing boards; the multi-turn test demonstrates resumed
exploration in the simulator. Neither proves earlier native discoveries,
new captures, or a victory. The original run remains a Religious loss.

Final comparison: `/tmp/civvis-3690-final-comparison.json`. Runtime durations
are not a performance comparison because background loads were uncontrolled.
