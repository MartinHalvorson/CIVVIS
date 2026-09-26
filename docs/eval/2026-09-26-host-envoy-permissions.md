# Preserve native envoy permissions — 2026-09-26

Carry the host's own envoy permission into the mirrored planner. This is a
legality/fidelity repair, not a new envoy valuation policy or evidence of a
domination win. Ordinary simulations and older exports keep the existing rule.

## Observed fuel interruption

King / Gran Colombia / four-player Tiny Pangaea native run
`civvis-20260926T190901Z`, pinned to `01385ba`, depended on Taruga's Aluminum.
The public state exports and actual envoy requests show:

| Observation | Our / leading envoys | Suzerain | Aluminum income |
|---|---:|---|---:|
| Turn 164 | 10 / 10 | Us | 2 |
| Turn 165, at war | 10 / 11 | Egypt | 0 |
| Turn 167 frame 2, after peace | 10 / 11 | Egypt | 0 |
| Turn 168, after one envoy | 11 / 11 | None | 0 |
| Turn 170, after two more envoys | 13 / 13 | Us | 2 |
| Turn 171, rival ties | 13 / 13 | None | 0 |
| Turn 176, lead recovered | 14 / 14 | Us | 2 |

One available envoy was held during turns 165–167 while the planner excluded
Taruga for being at war. The recorded placement was after peace, not evidence
that wartime placement was legal. The old export did not record the permission
needed to answer that question. A tie removes the rival's suzerainty but does
not restore our resource income. None of this proves an extra envoy would have
prevented the loss or that this source could be defended cheaply indefinitely.

Read-only evidence is retained under
`~/civvis-civ6-runs/control/civvis-20260926T190901Z/`: `events.jsonl`,
`orders.sqlite` and `why.log`. No live controls were used for this change.

## Native authority and implementation

The installed game's `Base/Assets/UI/PartialScreens/CityStates.lua` reads
`CanGiveInfluence()` at line 1429 and `CanGiveTokensToPlayer(iPlayer)` at line
1499, separately from `IsAtWarWith(iPlayer)` at line 1508. Its envoy buttons
use `CanReceiveTokensFrom` at lines 676–677. War status alone therefore cannot
replace the host permission, whether the actual host answer is true or false.

The control mod exports their combined boolean as `minors[].can_send_envoy`.
Missing, throwing or non-boolean accessors yield an absent field, not a made-up
permission. It reads only already-met minors. The existing envoy order handler
still independently rechecks both native permissions before issuing the order.

Both mirror construction and incremental sync map the permission to the board's
city-state identity. Every new snapshot refreshes it, including same-turn
frames; omission clears the previous observation. `Game::can_send_envoy` uses
an explicit host answer only on the observed turn, for that actor/target pair.
It still requires contact, a living non-barbarian minor and an available envoy.
Absent or expired facts use the prior simulation rule. No general wartime
envoy rule is changed and no war is removed to manufacture legality.

## Validation

Focused Rust tests cover accepted wartime placement through enumeration and
apply, explicit peacetime refusal, expiry of both answers, actor/target scope,
contact/supply/life guards, host-id remapping, rebuild and same-turn refresh,
clearing omitted observations, and old-export compatibility. A Lua 5.1 test
executes the shipped exporter and verifies true, false, unknown and error
cases plus its wiring into the minor record. Full-suite and quality results
are recorded in the pull request after completion.

Native acceptance remains unmeasured. After integration and a normal game
boundary, verify the exported permission, emitted envoy order, next-snapshot
envoy counts, suzerain and Aluminum income separately. A permitted request is
not proof of placement, resource recovery, city capture or victory. Resource
source valuation and protection remain a separate policy task.
