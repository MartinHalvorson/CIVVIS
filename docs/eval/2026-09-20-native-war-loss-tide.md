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
and nonzero-local-host seats); three initial controls pass. All eight final
AI regressions pass, covering positive/negative exchanges, rival attribution,
duplicate evidence/frames, native seat mapping, future observation bounds,
peace and redeclaration, and simulator-versus-host casualty authority.
Existing parser tests also assert both opponent identities, repeated-event
idempotence, and selected-frame isolation; existing settler memory tests pass.

After merging #3635 (`8f6a078`), `cargo test --profile ci --locked` passes:
3,762 library tests plus 205 binary/integration tests; 49 library and four doc
tests remain ignored. `cargo fmt --all -- --check` and `git diff --check`
pass. No simulator rule changes: this is native observation and AI ledger
routing, so an engine-only crash soak would not exercise the change.

## Frozen native replay

Freeze the original game's prefix through 207/2. Replay its 577 unique
awaited frames with the same Domination/Gran Colombia arguments and 19 forced
genes, a fresh persistent brain per binary. Baseline `280b3b7` and candidate
`71a3f96cc` differ only by this observation/ledger fix and its tests/docs.
The subsequent #3635 integration is covered by the full suite above.
Both exit 0: baseline 222.40 seconds, candidate 252.98. These timings are
informational, not a controlled performance benchmark.

Excluding `order_failed`, `order_verified`, and `turn_verified`, 16 frames
change exported orders. The candidate adds 15 peace requests to Germany:
104/0, 112/1, 129/0, 139/0, 144/0, 149/0, 154/0, 159/0, 164/0, 174/0,
179/0, 184/0, 189/0, 198/0, and 203/0. The baseline has none. The first
explanation reads an infeasible siege and a negative exchange; the rout
explanation appears at turn 143. The existing request cooldown determines
which repeated planner offers are exported.

Six rout requests carry the existing bounded tribute caps (8–39 Gold); the
other nine request white peace. The existing Suzerain peace-to-envoy handoff
also defers six envoy requests at 134/0 and three at 174/0, as identified by
`envoy_suzerain_reclaim_peace` in the bridge note. All internal native-action
lists remain identical on all 577 frames. No unit, production, or research
order category changes.

A request is not an accepted peace treaty, paid tribute, recovered city, or
avoided loss. The frozen host remains at war and loses at turn 209. Native
verification must establish whether the rival accepts a proposal and whether
the resulting recovery improves a later campaign.

Local artifacts: `/tmp/civvis-native-war-loss-replay/`,
`/tmp/civvis-native-war-loss-frame-replay.py`,
`/tmp/civvis-3636-comparison.json`,
`/tmp/civvis-143728-combat-deaths.json`, and
`/tmp/civvis-3636-{red,focused,full,merged-full,fmt}.log`.
