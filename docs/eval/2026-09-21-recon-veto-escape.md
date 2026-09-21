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
has no corresponding escape. The proposed change preserves the attack veto
and considers a legal move to a tile with no predicted reply damage before
ending the Scout's turn. Existing civilian reservations and frozen exploration
settings remain authoritative.

Validation is in progress. The native input is frozen at
`/tmp/civvis-recon-veto-replay/native-events.jsonl`, SHA-256
`7c177196218c286d3954211dbc8826a477c8e333a4131503f9dc2c6a64ddd7e9`.
The baseline binary uses source `80ff7be762aa0bfcfc1af101b1b78a5f10579ab9`,
verified identical in Rust, data, and Cargo files to this task's starting tree.
A paired replay must show actual movement changes before this proposal is
accepted. Replaying recorded future boards cannot prove earlier discoveries,
new captures, or a victory.
