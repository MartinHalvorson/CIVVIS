# Gran Colombia native trait fidelity

The shipped civilization attaches exactly two Ejercito Patriota modifiers:
`TRAIT_EJERCITO_PATRIOTA_EXTRA_MOVEMENT` and `TRAIT_PROMOTE_NO_FINISH_MOVES`.
The movement modifier grants `ABILITY_EJERCITO_PATRIOTA_EXTRA_MOVEMENT`, tagged
`CLASS_ALL_UNITS`; its unit modifier is `MODIFIER_PLAYER_UNIT_ADJUST_MOVEMENT`
with `Amount=1`. These are installed rows in
`DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml`.

Current CIVVIS instead records unconditional combat strength +2 and combat
experience +25%, and omits the movement point. The promotion half was repaired
in #4129. This follow-up will replace the unsupported bonuses with movement
and retain that promotion behavior. Native positive `GetMaxMoves` readbacks
already include the civilization bonus and must remain authoritative.

Registration before behavioral edits: run the eleven new trait regressions
against unchanged production behavior, retain every failure, then implement
the minimal trait hook and repeat them together with the six existing promotion
regressions, rules validation and the ruleset fingerprint test. Full hosted
Rust tests and changed-line quality are required. Two fixed CI-profile engine
smoke games use seeds 59010610 and 59010611, Battlefield 40x28, Gran Colombia
versus Rome, Deity, Online, all victories, 80 turns, best-of-one. These are
stability checks, with no causal strength or native victory inference.

The first game delivered #4129 (`civvis-20261025T101851Z`) is fixed descriptive
evidence. Its initial Settler and Warrior both export three movement points.
Its first emitted movement follow-up after promotion and first attack follow-up
are archived separately with all intervening events. At turn 23 Scout 196608
gains Alpine and moves from (19,6) to (19,7) in the immediate next frame. At
turn 35 Archer 327683 gains Garrison, moves from (15,14) to (14,14), and its
native combat event deals 34 damage to Warrior 196610 (57 to 23 HP). These are
execution witnesses for the previous correction, not paired treatment results.
This same game still lost to rival Science on turn 205, with one city taken
and five lost. Its native seat readback confirms Emperor, Gran Colombia/Simon,
four players, Tiny Pangaea, Online Gathering Storm and all victories. It was
the off arm of the independent `unwalled-ring-steps-in` screen. The archive
preserves that context and outcome; it supplies no result for this new fix.

`2026-10-25-gran-colombia-trait.native.json.gz` contains the eleven relevant
shipped rows as facts, the full source-file digest, native seat/first-state/
victory events and full game summary (13,506 bytes; SHA-256
`bc6b44fcc851db8f92ba3738626411912b7a62846dfe332eeb5c8464369f619e`).
`2026-10-25-gran-colombia-trait.execution.json.gz` contains the two execution
witnesses, their complete immediate state intervals, orders, all intervening
failures and 106 independently verified raw blobs (37,880 bytes; SHA-256
`9a5a053bdb20c7c0d055bedd222e94c18ea4194cd0949dad9be5720e4a4eec93`).

A read-only transaction against the loaded `DebugGameplay.sqlite` independently
returns the same two trait attachments, three modifiers and three arguments.
The fourteen database rows are retained in
`2026-10-25-gran-colombia-trait.database.json.gz` (709 bytes; SHA-256
`c191a1327bc3cd3b0a597084fedb33299fc2ddc0d20eb57114bb5eff12b82afe`).

Implementation, validation and exact artifact hashes are pending.
