# Feed confirmed native unit losses into the war tide

## Native failure

Native run `civvis-20260920T143728Z` and its continuation used controller
`44a39496100fbc9adaa4715f8128a6820d01f7dc`. It ended at turn 209 in a
German Technology victory (host victory 5, team 3, `won:false`,
2026-09-20T15:15:08.507Z). No military capture by our seat was verified.

The captured combat trace through turn 207 reports 52 of our units killed
by Germany and 16 German units killed by us. Around turn 143, the Online
four-turn window contains five of our losses and no German loss: a net -5
against the existing rout threshold of -4. The planner produced no peace
offer in this recorded prefix.

The native mirror applies `at_war` flags but does not reconstruct the
simulator's `Game::wars` loss ledger. Both one-war retreat and the objective
board's peace policy read that ledger through `one_war_ledger`, so recorded
native casualties previously left their exchange counter at zero. The
board's deployed policy replaces the simple military-power-ratio peace
term, making the missing tide evidence consequential.

## Host contract and change

The existing exporter in `tools/civ6_control/mod/CivvisControlAgent.lua`
11071–11089 derives killed flags from each post-combat component's `gone`
result and emits both original participants:

```lua
attacker = combat.attacker, defender = combat.defender,
defender_killed = defenderKilled, attacker_killed = attackerKilled,
```

Retain the other combatant's owner alongside each already-confirmed unit
death. At the existing host-death observation boundary, map native major
player IDs to model seats and rebuild cumulative losses by victim/opponent.
Use those confirmed unit counts in both war-tide readers. The mirror and
simulator war records are not fabricated or modified. Simulated games retain
the engine ledger; host observations never borrow speculative unit kills
from the throwaway planning pass.

Unknown opponents, unmapped barbarians/minors, same-owner events, future
observations, and duplicate unit evidence do not create major-war casualties.
The existing state-frame boundary still excludes combat events that occurred
after the selected observation. A new war seeds its counter from the current
totals, so casualties from the previous war are not charged again.

This adds unit-loss evidence only. It does not infer native city captures or
casualties from missing fogged units, change the retreat thresholds, force
another civilization to accept peace, or establish that retreat would have
prevented the recorded Technology loss.

## Validation

Two positive regressions fail before the AI ledger is connected (ordinary
and nonzero-local-host seats); three initial controls pass. Final focused,
full-suite, and frozen native replay results will be recorded before shipping.
No simulator rule changes: this is native observation and AI ledger routing.
