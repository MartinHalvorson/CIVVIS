# Unit movement and combat on the live seat: the pipeline, its leaks, the program

2026-08-19. Companion to `docs/TACTICS.md` (the engine's tactical search and
its bench) and `docs/CIV6_COMPUTER_CONTROL.md` (the order channel). This
document is about the *other* half of fighting a real Civilization VI game:
what happens to a tactical decision between `AdvancedAi` and the host board,
where the value leaks, and the ranked program that closes the leaks. Each step
of the program lands as its own PR and appends its measured result here; a
step that is not yet measured says so.

## 1. What the record says

The engine's tactical brain is strong: on the Tactics arena the joint search
beats the frozen `advanced_v1` controller **99.6 %** of the time in pure
combat (`docs/TACTICS_BASELINE.md`). The live seat does not get that strength.
The host's own Hall of Fame for the twelve finished live games of 2026-08-01/02
has our seat losing **343 units and killing 61** (0.18 kills per loss) while
the sixty Firaxis-AI seats in the same games ran **1.71** the other way, at
Settler difficulty; **0** cities conquered in eleven declared wars. The code
comments written since carry the same shape: **7 melee ATTACK orders against
1,546 MOVE_TO** in 188 turns of war, 35 % of military unit-turns hovering 2–4
hexes from a target under an *Engage* posture, 67 % of wartime moves
arriving; and in the 54 completed runs since 2026-08-16 four sieges reduced a
city to 180–190/200 and captured none (`siege_is_progress`, #2031).

## 2. Where a decision leaks

Trace one wartime turn from the brain to the board:

1. **Board export, once per turn.** Own units carry position, hp, xp,
   promotions, fortify state; hostiles carry no id; terrain crosses every 25
   turns (`--tile-export-every`), roads never (`tile.road = 0`).
2. **Mirror rebuild** hands every unit its *full* movement (`mirror_unit_moves`)
   whatever the host already spent on a queued path; Civ 6 city-centre stacking
   drops units the engine's stacking law cannot seat (`tile_taken`).
3. **One CIVVIS turn on a clone of the mirror.** The joint search plans units
   within ≤3 tiles of contact, one turn deep, with clairvoyant rolls; the
   per-unit picker attacks only from the tile the unit stands on; `route_step`
   prices every edge at 1 and, beyond the first step, ignores units, ZOC and
   cliffs. No friendly pass-through, no swap — a friendly in the way is a hold.
4. **Translate and coalesce.** `coalesce_unit_paths` collapsed a unit's walk to
   its furthest hex and **deferred the first non-move and everything after it
   to the next turn** — every ATTACK, RANGE_ATTACK, FORTIFY or PILLAGE that
   followed a step. The joint search's lines are `[Move, Attack]`
   (`tactics.rs`), so on the bridge they executed as a step: the unit walked
   into contact and stood there, unstruck, through the enemy's turn. Only the
   finishing volley (`live_finishing_candidates`) sent approach+blow as one
   `MOVE_TO` onto the defender, and only for proved kills on wounded units.
   `Action::Pillage`, Heal, Alert, swap and formation combine were not
   translated at all.
5. **Lua applies the list, sequentially, open-loop.** Every order is gated on
   `CanStartOperation`, but later orders never see earlier results. Any combat
   unit CIVVIS did not mention was handed to `UNITOPERATION_AUTOMATE_EXPLORE`.
6. **The host rolls its own dice** and the enemy takes its turn.
7. **Next turn's export is the first feedback.** No unit-killed, unit-lost,
   combat or capture event exists; `move_refused` is written by the mod and
   read by nothing in Rust; the ladder records `applied_pct`.

## 3. The program, ranked

| # | step | what it removes | measured by |
|---|---|---|---|
| 1 | **Sequenced, closed-loop actuation** — per-unit order queues in the mod with tick-level readback, proved approach+blow as one host order, no explore hand-off inside an engagement, Pillage translated; later a mid-turn combat frame | one order per unit per turn; step-then-strike losing the strike; the host scattering held units | share of planned strikes landing the same turn; hover share; Hall-of-Fame exchange ratio |
| 2 | **Host-grounded planning board** — seat-turn-start movement, embarked and ZOC state, front-line terrain and roads every turn, the host's own reachable set, engine parity for pass-through/swap/stacking, refusals consumed as facts | invented MP; stale front; the 12.5 % of MOVE_TOs that never moved; "next tile refuses the unit" holds | arrival ledger (planned vs actual), no-op share |
| 3 | **Engagement-window planner** — `tactics.rs` grown to own every unit within R of contact, lines from `reachable()`, two of our turns deep, expected-value combat, city-assault terms | one-turn horizon; approach from four tiles out; the 60 % one-city arena regime | `battle_bench` + a city-assault cell in `tactics_bench`; live captures |
| 4 | **Operational choreography** — army plan with per-unit ETAs, go/no-go on the muster ring, siege as ring → walls → capture, wounded rotation | piecemeal arrival; sieges walked away | first live capture; arrival spread |
| 5 | **Live tactical ledger + replay bench** — kill/loss/combat/capture events, hostile ids, predicted-vs-actual per order, Hall-of-Fame per run, offline replay of recorded frames | the reconstruction this document had to do by hand | the next report cites the ledger |

Everything is bridge-scoped: the native controller keeps its rating anchor,
and the whole-game evidence that tactical quality does not move native Elo
(`docs/TACTICS.md` §6–7) is left undisturbed. Do not re-attempt the recorded
nulls: the adjacent-support term, `tile_defense_bonus` in the closed-form
reply, budget above the knee in the current line space, promoting bridge war
repairs natively, alpha-beta/MCTS over raw actions.

## 4. Step 1 — sequenced unit orders (this change)

**Mechanism.** The mod now keeps a per-unit **order queue** (`CivvisQueue`).
`applyOrders` runs the first order for each unit exactly as before; every
later order for the same unit — the strike after the walk, the fortify after
the strike, the found after the walk — is queued with the position the earlier
order was expected to leave the unit at. The queue drains from
`Events.UnitMoveComplete` / `Events.UnitOperationDeactivated` for that unit and
from the ordinary tick, re-resolving the unit handle every time (a cached
handle SIGSEGVs the game core; see `applyOrder`). A queued order runs when the
unit is where the previous order meant it to be, or has no movement left, or
has stopped operating; it is refused by name (`queue_no_moves`,
`queue_stalled`, `unit_gone:<id>`) when it cannot. `settleTurn` holds the turn
open while a queue is pending, bounded by `OrderQueueMaxTicks` (default 240
ticks ≈ well under the stall watchdog), so a wedged operation costs decision
quality, never progress — the same floor every other wait in this file has.

`civvis_orders` stops deferring the follow-ups when the run's `seat` event
says `order_queue = true`; against an older mod it behaves exactly as before.
`coalesce_unit_paths` still folds a walk into one `MOVE_TO`, and every order
after the walk now rides along in sequence.

**Two more leaks closed in the same change.**
- Units CIVVIS did not mention are no longer handed to the host's explore
  automation when a visible hostile combat unit or an at-war city stands
  within `ExploreGuardRadius` (4) tiles — a held soldier stays held; the
  automation keeps its peacetime job for units far from any enemy.
- `Action::Pillage` translates to `PILLAGE` and the mod issues
  `UNITOPERATION_PILLAGE`, so light cavalry's pillage-before-combat and
  pillage-to-heal exist on the live seat.

**Instrumentation.** `orders` gains `queued`; a per-turn `orders_queue` event
reports `applied`, `refused`, `refusals` (by reason), `waited` ticks and how
many queued orders were **strikes that landed on the turn they were planned**
(`strikes_landed`) — the number this step exists to move (≈0 before).

**Not measured yet.** A live game has not run with this change; the offline
Lua regression (`order_queue_test.lua`) proves the sequencing, refusal naming
and explore guard against a stubbed host, and the Rust tests prove the
capability gate. The first live runs should be read for `orders_queue` and
for the exchange ratio; a wedge shows as `queue_stalled` with `waited` at the
cap.

## 5. Step 5 — the tactical ledger (landed with step 1's successor change)

**Mechanism.** The mod (`CivvisLedger`) writes the combat record the host
already knows into the run's own event stream: `combat` at
`Events.CombatVisEnd` — attacker and defender (player, id, kind, plot), hit
points read back at Begin and End, damage both ways, kills, the
`UnitDamageChanged` deltas observed while the combat was open, and the strike's
host preview joined on; `strike` before every ATTACK / RANGE_ATTACK with
`CombatManager.SimulateAttackInto`'s predicted damage — the same call the
shipped UnitPanel makes to draw the combat preview; `unit_lost` for our units
leaving the map, with the last known kind and the treasury (a bankruptcy
disband and a battlefield loss are one field apart); `city_occupation` when a
city changes hands. Hostile and rival units carry the host's unit id in the
export, so a combat and a next-frame sighting name the same unit.

`tools/civ6_tactics_ledger.py <run-dir> [--hof HallofFame.sqlite]` reads one
run into one report: orders and the queue's strikes planned / landed, the
MOVE_TO arrival ledger against the next frame (arrived, short, did not move —
with or without movement at export —, gone; by unit kind), combat and the
host-preview error, the roster of military units gone with their last
context, the hover share, and the Hall of Fame. Where the recording mod did
not emit an event it says `(mod predates the ledger)` rather than printing a
zero.

**Measured on the recorded runs (2026-08-01/02, pre-queue mod):** run
`live-head-rome-20260802T164220Z` — 1,608 unit orders on 235 turns (6.84 per
turn), 0 unit-turns with more than one order; 1,397 first moves judged:
arrived 82.3 %, did not move 12.5 % (159 with movement at export, 15
without); 78 military units left the board (43 at full hp with the treasury
empty, 19 beside a hostile, 16 with no visible threat); 99 of 384 near-hostile
unit-turns hovered (25.8 %); Hall of Fame across the 12 local games: 0.18
kills per loss. These are the numbers the program started from, now
reproducible from the ledger in one command.

**Not verified in-game yet.** `CombatVisBegin/End`, `UnitDamageChanged`,
`SimulateAttackInto` and `CityOccupationChanged` are read the way the shipped
UI reads them, and both readings (hit-point readback and damage deltas) are
recorded side by side so the first live run says which one is right. The
offline regression (`combat_ledger_test.lua`) proves the events are shaped as
the ledger tool expects.

## 6. Step 2 — a host-grounded board (movement, roads, queued paths)

**What was wrong.** `mirror_unit_moves` handed every mirrored unit its full
allowance every turn, because the export's `moves` had misled twice — and it
misled because the host had already spent the movement before the brain
could act: a `MOVE_TO` whose host path outran the turn was *queued*, and the
host walked the unit along it at the start of the next turn, before
`beginTurn` exports (turn 31 of run `civvis-20260730T120107Z`: 7 of 8 units at
`moves: 0` at the start of the turn). Roads were never exported and the board
wrote `road = 0` everywhere, so every march was priced across roadless ground.

**Mechanism.** The mod (`CivvisBoard`):
- caps every `MOVE_TO` to the furthest plot on the host's own path that the
  unit reaches *this* turn (`UnitManager.GetMoveToPathEx` — `plots` and
  per-plot `turns`, the shipped WorldInput's own path); a walk that would
  take two turns is sent as its first turn's leg and the brain re-plans the
  rest from the real position next turn, so no path is left queued to walk
  the unit somewhere stale; a move whose first step is already next turn is
  refused by name (`move_no_moves_this_turn`); a melee ATTACK (a `MOVE_TO`
  onto the defender) is never capped; the row's own destination is rewritten
  so the per-unit queue expects the capped plot; counted as `move_capped` /
  `move_no_reach` on the `orders` event and detailed in `move_capped` events;
- cancels combat units' queued host paths at turn start
  (`UNITCOMMAND_CANCEL`; civilians keep theirs — a settler's long walk is what
  a queued path is for) and reports `queued_paths {found, cancelled}`;
- exports each unit's `queued_dest` and `embarked`, and each tile's route
  (`rt`, by name from `GameInfo.Routes`) and `rp` (pillaged);
- advertises `moves_at_turn_start` on the `seat` event.

The mirror maps `rt` onto the engine's route ladder (`route_level`: Ancient 1
… Railroad 5, a pillaged road 0) and, **only when the seat advertises
`moves_at_turn_start`**, starts each unit with `min(allowance, moves)` —
`mirror_unit_moves_for` — on both the fresh-board and the persistent path;
against an older mod every unit keeps its full allowance exactly as before.
`moves_short=N` in the decide note counts units the host had already walked
before the frame; zero is the healthy reading once every move is capped.

**Not measured yet.** Offline: `host_board_test.lua` (cap, refuse, attack
uncapped, queue expectation follows the cap, cancel only combat units) and
the mirror tests (`host_routes_land_on_the_engines_ladder`,
`exported_routes_reach_the_board`,
`exported_movement_is_trusted_only_with_the_seat_capability`). The first live
runs should be read for `move_capped` / `move_no_reach` on `orders`,
`queued_paths`, `moves_short` in the decide notes, and the arrival ledger's
did-not-move share (12.5 % before).

## 7. Step 3, first cut — the joint search reaches as far as the unit does

The engagement-window planner grows in steps; this is the first: approach
lines from the engine's exact reach flood instead of two hand-built steps
(`docs/TACTICS.md` §17). Measured on `battle_bench` at 300 paired seeds a cell
on two disjoint blocks: the four foot compositions hold within one standard
error; the two mounted compositions gain (cavalry +398 → +476 and +352 → +410;
mounted medieval +496 → +590 and +601 → +622), because a four-move unit's
third and fourth hexes now exist for our own lines as they already did for
the enemy's reply. Still ahead in this step: the window (units four-plus tiles
out with no strike this turn), a second ply for set-up lines, expected-value
combat for the host's rolls, and the city-assault terms the one-city arena
regime measures.

## 8. Step 1, second cut — the mid-turn combat frame (flag-gated, default off)

**What was wrong.** The whole turn is planned once, on the opening board,
with the engine's own rolls; the host's roll differs (it has left "sure"
kills alive at 1, 3, 6, 8, 16 and 20 HP), and the next export is next turn.
Nothing could react to what the first volley actually did.

**Mechanism.** With `CombatFrames ≥ 1` (`--combat-frames N`; default **0**),
once the opening orders and their per-unit queue have settled on a turn that
issued a strike, the mod (`CivvisFrames`) exports the board again stamped
`frame: 1` (`combat_frame` event), re-arms the handshake, and waits — with its
own short budget (`CombatFramePolls`, 20) and no stale answer and no fallback
ladder; past it the frame is abandoned by name (`combat_frame_timeout`) and
the turn ends as before. The brain (`civ6_brain.py`) answers a `state` with
`frame ≥ 1` for a turn it has already served: it asks the decider for the same
turn again — the decider reads the newest state for that turn, which is the
frame — and writes the answer beside the opening board's (`orders.frame`,
`ready.frame`; a database from before the column is migrated in place). The
mod's readers select by frame and treat a channel without the column as frame
0. On a frame `applyOrders` hands no unit to explore automation and writes no
`turn` record. Units export `attacks_remaining`; under `moves_at_turn_start`
the mirror starts a unit with the host's count, so a frame's re-plan spends no
strike twice (and its movement is the host's, from step 2).

**Not measured; deliberately off.** A second round trip per contact turn is a
second place for the loop to wedge, and the round trip is the one thing the
offline harness cannot exercise. `combat_frame_test.lua` drives the frame
handshake, the frame-aware readers (with and without the column), the
no-explore / no-turn-record rules and the default-off; `test_civ6_brain.py`
covers the channel and the migration; the mirror tests cover
`attacks_remaining`. Turn it on for one live run (`--combat-frames 1`) and
read `combat_frame`, `combat_frame_timeout`, `orders.frame`, and the ledger's
strikes-landed and exchange ratio against a run without it.

## 9. Every step behind a switch the ledger records — how to A/B each one

Each step ships **on** (except the combat frame) and can be withheld without
a rebuild, and every live row says which switches were on: the mod-side
switches ride in the summary's `mod_arms` (→ `docs/civ6_ladder.json`), the
brain-side withholds in `withheld`. Read a comparison with
`tools/civ6_tactics_ledger.py` on each arm's runs.

| step | switch (default) | withhold | where it is recorded |
|---|---|---|---|
| 1 · per-unit order queue | `OrderQueue` (on) | `civ6_play.py --no-order-queue` — the mod applies one order per unit per turn and the seat stops advertising `order_queue`, so the brain defers follow-ups exactly as before | `mod_arms.OrderQueue` |
| 1 · explore guard | `ExploreGuard` (on), `ExploreGuardRadius` 4 | `--no-explore-guard` | `mod_arms.ExploreGuard` |
| 1b · combat frame | `CombatFrames` (**0 = off**), `CombatFramePolls` 20 | `--combat-frames 1` turns it ON for a run | `mod_arms.CombatFrames` |
| 2 · moves capped to this turn's leg, trusted movement | `CapMovesToReach` (on) | `--no-cap-moves-to-reach` — also stops the seat advertising `moves_at_turn_start`, so the mirror returns to the full allowance | `mod_arms.CapMovesToReach` |
| 2 · queued paths cancelled at turn start | `CancelQueuedPaths` (on) | `--no-cancel-queued-paths` | `mod_arms.CancelQueuedPaths` |
| 3 · joint search reach lines | `joint_reach_lines` (on wherever the joint search runs) | live: `--without joint-reach-lines` (`live_without_joint_reach_lines` arm); bench/arena: `advanced_joint_tactics_geometric` seats the pre-§17 portfolio — measured to reproduce the old figure exactly (cavalry +398.0 on block 7,200,000) | `withheld` (live) / the arm name |
| 5 · strike preview | `StrikePreview` (on) | `--no-strike-preview` — `strike` events carry no prediction | `mod_arms.StrikePreview` |

Pillage translation, the tactical ledger's events and the export of ids,
roads, `queued_dest`, `embarked` and `attacks_remaining` are data, not
decisions, and stay on.

**What to read per arm.** `orders_queue.strikes_landed` and the arrival
ledger (step 1); `move_capped`, `move_no_reach`, `queued_paths`, `moves_short`
and the did-not-move share (step 2); the ledger's kills per loss, damage
dealt/taken and the hover share (all); `combat_frame` /
`combat_frame_timeout` and the ladder's `applied_pct` (1b). The bench pair for
step 3 is `battle_bench --a advanced_joint_tactics --b
advanced_joint_tactics_geometric` (cavalry cell, 300 seeds, block 7,200,000:
**+59.0 ± 17.1**, sign p = 0.0023), and `tactics_bench` seats both arms on the
arena regimes.

## 10. Step 4, first cut — arrive together (`arrival-waves`, opt-in, off)

Item 4 asked for an army plan with per-unit ETAs, a go/no-go on the muster
ring, a siege as ring → walls → capture, and wounded rotation. Reading the
controller before writing any of it: most of that already exists, behind
names this document had not connected to it. The pre-war ETA and go/no-go
are `WarPlan` (`war_package_status`: `staged_bodies`, `fourth_one_turn_away`
from `war_staging_route_for_unit`) and `campaign_staged_for_war` (three
bodies on the 3..=5 ring, a melee capturer, local strength ≥ 1.05); the
ring and the walls are the joint evaluator's own terms (a melee blow on a
city is worth +520 only at ≤ 40 hp with the wall down, `tactics.rs`
`strike_prior`; wall damage is priced at 1.35×); the ring is `rush_siege_step`
for a rush and `siege_role` / `siege_tracks_wall` / `siege_commitment` /
`siege_is_progress` for a campaign; wounded rotation is the per-unit
`withdraw_hp` recovery step and the group `Recover` posture. All of those
are live-bridge treatments already and all of them are in the screen below.

What was **not** there is the one mechanism the recorded evidence actually
names: reinforcements that reach an engaged front one at a time.
`wartime_reinforcement_step` walks the standing-still rear to the objective's
ring, and the turn a marcher comes inside `command_radius` of the front it
joins that clique and takes its orders — for an `Engage` / `Advance` front,
that is walking into the fight alone with whatever strength it brought. The
live ledger's shape for this is the 73 of 231 combat losses taken at a
>30-point strength gap.

**`arrival-waves`** (`AdvancedAi::arrival_waves`; `enable_` / `disable_`;
`PRODUCTION_OPT_INS` row, so `gene_screen` screens it and `victory_eval
--with arrival-waves` seats it): a unit that marched as a rear reinforcement
last turn and now stands in an engaging clique, out of contact (no enemy
within 2), **holds where it stands, fortified**, until `ARRIVAL_WAVE_SIZE`
(2) such arrivals stand within `ARRIVAL_WAVE_RADIUS` (6, the clique radius)
of each other, or it has waited `ARRIVAL_WAVE_PATIENCE_TURNS` (3). The whole
wave is then released in one turn. Decided once per force-group rebuild
(`plan_arrival_waves`, at the end of `rebuild_force_groups`), idempotent
within a turn (a release forgets the march, so the rebuild after an attack
cannot re-hold the unit), dropped the moment the unit is in contact or its
group stops engaging. A held unit still counts in its group — its strength is
in the front's `local_strength_ratio` and its presence in `readiness` — so a
front too weak to advance may clear the floor the turn the wave stands
beside it, and then the whole group goes.

- **Census** (`StrategyCensus`): `arrival_wave_holds` (units held, once per
  hold), `arrival_wave_releases` (units released as a wave of ≥ 2),
  `arrival_wave_lone` (released alone on patience). `civvis soak` prints
  `ARRIVAL_WAVES held= in_wave= alone=` when any fired. Measured on six 4p
  60×38 domination/score games with the native repair bundle plus the flag
  on seat 0: 0–27 held, 0–12 in a wave, 0–11 alone per game — it fires, and
  roughly half of the arrivals it holds do find company inside three turns.
- **Off everywhere by default**, including the bridge: the recorded rule for
  this layer is that standing-still postures are load-bearing and four
  earlier attempts to choreograph a siege each measured worse, so this one
  is priced before it is promoted. Its first price is the
  `domination,score` gene screen of 2026-08-19 (`docs/eval/`), where it is a
  gene beside the fifty-nine war and siege repairs it would choreograph.
- Tests: `arrival_waves_hold_a_lone_fresh_reinforcement_short_of_an_engaged_front`,
  `arrival_waves_release_a_pair_together`,
  `arrival_waves_release_a_lone_arrival_when_patience_runs_out`,
  `arrival_waves_drop_the_hold_once_in_contact`.

Still open from item 4 after this: nothing of it is bridge-enabled, and the
siege flags it would choreograph remain, as §3 says, unmeasured on the live
seat — the first live runs with the order queue (seven had run by 11:21Z on
2026-08-19, on another machine) have not been read.
