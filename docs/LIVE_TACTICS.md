# Unit movement and combat on the live seat: the pipeline, its leaks, the program

2026-08-19. Companion to `src/ai/advanced.rs` (the engine's native tactical
path) and `docs/CIV6_COMPUTER_CONTROL.md` (the order channel). This document
is about the *other* half of fighting a real Civilization VI game: what happens
to a tactical decision between `AdvancedAi` and the host board, where the value
leaks, and the ranked program that closes the leaks. Each step of the program
lands as its own PR and appends its measured result here; a step that is not
yet measured says so.

## 1. What the record says

The engine's native tactical path makes one unit's decision at a time. The live
seat does not get a separate combat controller.
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
3. **One CIVVIS turn on a clone of the mirror.** The per-unit picker attacks
   only from the tile the unit stands on; `route_step` prices every edge at 1
   and, beyond the first step, ignores units, ZOC and cliffs. No friendly
   pass-through, no swap — a friendly in the way is a hold.
4. **Translate and coalesce.** `coalesce_unit_paths` collapsed a unit's walk to
   its furthest hex and **deferred the first non-move and everything after it
   to the next turn** — every ATTACK, RANGE_ATTACK, FORTIFY or PILLAGE that
   followed a step. A move followed by an action therefore executed as a step:
   the unit walked into contact and stood there, unstruck, through the enemy's
   turn. Only the finishing volley (`live_finishing_candidates`) sent
   approach+blow as one `MOVE_TO` onto the defender, and only for proved kills
   on wounded units.
   `Action::Pillage`, Heal, Alert, swap and formation combine were not
   translated at all.
5. **Lua applies the list, sequentially, open-loop.** Every order is gated on
   `CanStartOperation`, but later orders never see earlier results. Any unit
   CIVVIS did not mention receives an explicit hold; only CIVVIS names a
   movement destination.
6. **The host rolls its own dice** and the enemy takes its turn.
7. **Next turn's export is the first feedback.** No unit-killed, unit-lost,
   combat or capture event exists; `move_refused` is written by the mod and
   read by nothing in Rust; the ladder records `applied_pct`.

## 3. The program, ranked

| # | step | what it removes | measured by |
|---|---|---|---|
| 1 | **Sequenced, closed-loop actuation** — per-unit order queues in the mod with tick-level readback, proved approach+blow as one host order, explicit holds for every unit the planner leaves in place, Pillage translated; later a mid-turn combat frame | one order per unit per turn; step-then-strike losing the strike; the host scattering held units | share of planned strikes landing the same turn; hover share; Hall-of-Fame exchange ratio |
| 2 | **Host-grounded planning board** — seat-turn-start movement, embarked and ZOC state, front-line terrain and roads every turn, the host's own reachable set, engine parity for pass-through/swap/stacking, refusals consumed as facts | invented MP; stale front; the 12.5 % of MOVE_TOs that never moved; "next tile refuses the unit" holds | arrival ledger (planned vs actual), no-op share |
| 4 | **Operational choreography** — army plan with per-unit ETAs, go/no-go on the muster ring, siege as ring → walls → capture, wounded rotation | piecemeal arrival; sieges walked away | first live capture; arrival spread |
| 5 | **Live tactical ledger + replay bench** — kill/loss/combat/capture events, hostile ids, predicted-vs-actual per order, Hall-of-Fame per run, offline replay of recorded frames | the reconstruction this document had to do by hand | the next report cites the ledger |

Everything is bridge-scoped: the native controller keeps its rating anchor,
and historical whole-game evidence did not justify carrying a more expensive
tactical controller in the native evaluator. Do not re-attempt the recorded
nulls: the adjacent-support term, `tile_defense_bonus` in the closed-form
reply, budget above the knee in the former line space, promoting bridge war
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
- Units CIVVIS does not mention are explicitly held, regardless of distance to
  an enemy. The host never chooses a route: the next movement must be a new
  CIVVIS decision from the next exported board.
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

The ledger's `engagement` section (2026-09-02, `engagement_section`) is the
doctrine scorecard, every KPI printed as numerator/denominator:
`initiative_share`, `army_kills_per_loss` (with `city_strikes` /
`city_strike_kills` kept apart), `killed_when_wounded_share`,
`wounded_exposed_share` / `wounded_healing_share`, `firepower_utilisation` /
`idle_healthy_share`, `focus` (targets, multi-hit, left low), `chip_share`,
`suicidal_attacks` and `cities_lost_undefended`. It is unit-vs-unit only and
reads frame-0 states. **The junk-row rule:** the mod emits `combat` rows in
which attacker and defender are both `district` with id −1 and both `_killed`
flags set — 65 of 446 rows in run `civvis-20260901T132005Z`, 776 across the
37 runs since 2026-08-29 — and a district attacker carries
`attacker_killed=true` whenever it is `gone`; the section drops the former
outright and every non-unit attacker from attacker-side statistics, and prints
both counts in its header. On run `civvis-20260901T132005Z` the older
`combat` line, which reads the flags unfiltered, prints kills/loss 0.24; the
`engagement` line prints 0.87. Across the 37 runs since 2026-08-29:
initiative 45 %, army kills/loss 1.30, killed when wounded 65 %, firepower
32 %, chip 31 %.

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

## 7. Retired engagement-window planner

The `joint-tactics` search and its reach-line companion were removed on
2026-08-25 after the 35,148-seat screen read a −0.104 pp win difference. The
native and live paths both retain their ordinary per-unit tactical behavior.

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
0. On a frame `applyOrders` makes no extra unmentioned-unit disposition and
writes no `turn` record. Units export `attacks_remaining`; under `moves_at_turn_start`
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
| 1b · combat frame | `CombatFrames` (**0 = off**), `CombatFramePolls` 20 | `--combat-frames 1` turns it ON for a run | `mod_arms.CombatFrames` |
| 6 · replan frames | `ReplanFrames` (**2**), same `CombatFramePolls` | `--replan-frames 0` — no mid-turn frame opens for revealed ground (nor for strikes unless `CombatFrames` says so), and the seat stops advertising `replan_frames` | `mod_arms.ReplanFrames` |
| 6 · tiles delta | `TileDelta` (on) | `--no-tile-delta` — revealed ground crosses only with the `TileExportEvery` sweep, as before | `mod_arms.TileDelta` |
| 6 · per-plot yields | `TileYields` (on) | `TileYields=false` in the mod config — the `tiles` record carries no `yl` and the board falls back to CIVVIS's own tile catalogue, which is short by every disaster's fertility | — |
| 2 · moves capped to this turn's leg, trusted movement | `CapMovesToReach` (on) | `--no-cap-moves-to-reach` — also stops the seat advertising `moves_at_turn_start`, so the mirror returns to the full allowance | `mod_arms.CapMovesToReach` |
| 2 · queued paths cancelled at turn start | `CancelQueuedPaths` (on) | `--no-cancel-queued-paths` | `mod_arms.CancelQueuedPaths` |
| 5 · strike preview | `StrikePreview` (on) | `--no-strike-preview` — `strike` events carry no prediction | `mod_arms.StrikePreview` |

Pillage translation, the tactical ledger's events and the export of ids,
roads, `queued_dest`, `embarked` and `attacks_remaining` are data, not
decisions, and stay on.

**What to read per arm.** `orders_queue.strikes_landed` and the arrival
ledger (step 1); `move_capped`, `move_no_reach`, `queued_paths`, `moves_short`
and the did-not-move share (step 2); the ledger's kills per loss, damage
dealt/taken and the hover share (all); `combat_frame` /
`combat_frame_timeout` and the ladder's `applied_pct` (1b).

## 10. Step 4, first cut — arrive together (`arrival-waves`, opt-in, off)

> **REMOVED 2026-08-21.** Two screens priced the wave at −3.0 [−6.7, +0.6]
> (war, seeds 44M) and −35 wins/10k (6p native, seeds 52M) — never a
> measured help — and the operator's fix-or-remove directive took the code
> with the bottom of the ranking (PR #2235). The section below is the
> historical record of what it was.

Item 4 asked for an army plan with per-unit ETAs, a go/no-go on the muster
ring, a siege as ring → walls → capture, and wounded rotation. Reading the
controller before writing any of it: most of that already exists, behind
names this document had not connected to it. The pre-war ETA and go/no-go
are `WarPlan` (`war_package_status`: `staged_bodies`, `fourth_one_turn_away`
from `war_staging_route_for_unit`) and `campaign_staged_for_war` (three
bodies on the 3..=5 ring, a melee capturer, local strength ≥ 1.05); the
ring and walls have explicit campaign and siege terms; the ring is
`rush_siege_step` for a rush and `siege_role` / `siege_tracks_wall` /
`siege_commitment` / `siege_is_progress` for a campaign; wounded rotation is
the per-unit
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


## 11. Step 6 — the bridge talks more than once a turn, and a unit that sees something new thinks again

**What was wrong.** Two things, one shape. (1) One exchange per turn: the
board went out at turn start, the orders came back, and nothing the turn
then revealed — a coast, a rival border, a barbarian camp two hexes past a
scout's first step — reached the brain until the next turn; the combat frame
(§8) was the one exception, strike-only and off. (2) The map itself crossed
only with the `tiles` sweep, every `TileExportEvery` turns (25 by default,
4 on the ladder): `exportTiles` sends every revealed plot or nothing, so the
ground a unit uncovered on turn 26 was planned on from turn 29 at the
earliest, and on a plain `civ6_play` run from turn 50. Meanwhile
`coalesce_unit_paths` sends a unit's whole walk as one `MOVE_TO` to its
furthest hex: the host walks a scout three hexes into the fog whatever its
first step showed. Natively the engine does better than the bridge here — every
evaluator and the live decider run units serially
(`advance_unit_serial`), one sighted step at a time — except that the force
groups are re-formed only when an attack dirties them ("movement cannot
change the opposing force"), so a step that brings a hostile into view
changes nothing for the units still to move; and the parallel CLI path
(`civvis --jobs`, the only `WorkPool` installer) plans batched units up to
eight hexes on a clone that reveals nothing and replays them blind.

**Mechanism, four parts, each its own switch (§9).**

- **Tiles delta** (`CivvisTiles`, mod). `known[plot] = owner` remembers what
  has crossed; `exportTiles` between sweeps (and `CivvisTiles.sweep` on a
  frame) sends only plots revealed or re-owned since, as a `tiles` chunk
  stamped `delta: true` plus a `tiles_delta` summary (`turn`, `frame`,
  `plots`). The full sweep keeps its cadence and re-primes `known`. Rust
  merges chunks in stream order; a delta lands its plots but does **not**
  advance the snapshot's sweep turn (`Snapshot::merge_delta`), because the
  `improved` fold keeps events at or after the newest sweep and a delta
  carries none of the older plots' improvements. Cost: one `IsRevealed` and
  one `GetOwner` per plot per turn (≈3,400 calls on the 74×46 board) and a
  record only for the new plots — not the per-plot field walk that once took
  a turn to ten minutes.
- **Replan frames** (`CivvisFrames`, mod; `ReplanFrames = N`, default 2).
  `settleTurn`, once the opening orders and the per-unit queue have drained,
  calls `CivvisFrames.observe` — the tiles delta sweep (so the count IS the
  export) and a count of units with movement left — and `why()`: `strike`
  if a strike went out since the last board (combat or replan frames),
  `revealed` if ground was revealed and somebody can still move (replan
  frames only). `begin` emits `combat_frame` or `replan_frame` (with
  `reason`, `strikes`, `revealed`, `movers`), re-arms the handshake and the
  queue's tick budget (`CivvisQueue.ticks = 0`: a turn with N frames may
  hold up to (N+1)·`OrderQueueMaxTicks`), and re-exports the board stamped
  `frame = N`. The brain (`civ6_brain.py`) is frame-generic already: frame
  N for a served turn is served once more; `Decider.ask(turn)` reads the
  newest state for the turn, which is the frame; `orders.frame` rows ride at
  seq 10000·N; `ready` names the newest frame, which is safe because frame
  N+1 opens only after N's answer was consumed. A frame waits
  `CombatFramePolls` (20) with no stale answer and no fallback; on timeout
  the turn's remaining frames are abandoned by name (`combat_frame_timeout`
  carries the reason) and the turn ends as before. On the recorded run
  `live-head-rome-religious-actions-20260802T173404Z` every opening answer
  landed inside the first poll (median and max `polls` = 1 of 40), so 20 is
  twenty times the observed need. The seat advertises `replan_frames` and
  `tile_delta`.
- **The brain leaves an opening walk intact.** A replan frame operates on the
  state the host exports after its opening orders settle; it does not shorten a
  planned walk at the fog edge. If an action leaves movement or an attack for a
  unit, the next frame can spend that live state.

**What to read on the first ladder runs** (the climb now forwards
`--replan-frames` and `--combat-frames`; the latter was never forwarded, so
no ladder run has played the combat frame either): per turn, `replan_frame` /
`combat_frame` against `combat_frame_timeout` (a timeout rate above a few
percent says the poll budget or the brain's frame latency needs attention);
`tiles_delta.plots` per turn against the old sweep gap; turn wall time against a
`--replan-frames 0` arm — the frame is a second export and a second decide.
The decide is cheap: `civvis_orders --mirror <run> --turn 120` on the 20 MB,
241-turn journal of `live-head-rome-religious-actions-20260802T173404Z`
answers in 0.26 s wall (0.81 s at turn 60, cold), whole-journal re-reads
included; what a frame costs is the host's `exportState` and the relay, so
that is where to look if frames cost more than they return. The arm that withholds everything at
once is `--replan-frames 0 --no-tile-delta`.

Tests: `replan_frame_test.lua` (the delta sweep, its withhold and re-prime;
the revealed/movers/strike triggers; the queue re-arm; default-off
inertness), `a_tiles_delta_merges_new_ground_without_standing_for_a_sweep`,
`a_step_that_sights_a_hostile_dirties_the_force_groups`, and
`test_the_mid_turn_frames_reach_the_play_command`.

## 12. Step 7 — a unit spends every action it has in the turn

**The ask (2026-08-21).** A unit should be able to step and settle in the
same turn, step and step again, step and shoot. Use every action to the full.

**What the record could say.** Replays use `--assume-seat
order_queue,replan_frames,tile_delta` to present the capabilities today's
mod advertises, while `step_turn_actions_test.lua` drives the shipped
mod's `beginTurn`/`settleTurn`/`applyOrders` against a fake host
that walks, spends and reveals.

**What was wrong — three things, in the mod and on the bridge.**

1. **The turn was released on the tick the opening orders went out.**
   `settleTurn`'s apply tick returned true, and the tick handler requests
   `ACTION_ENDTURN` the moment it does. On that tick every unit holds its
   FIRST order only: the strike after the step, the found after the walk,
   the second step — all still on the per-unit queue — and no frame has
   been considered. Whenever the host took the request at once (nothing
   blocking: every unit busy walking), the turn ended under the queue
   (`queue_turn_over`) and the frame never opened: a unit stepped, stood,
   and kept the rest of its movement. The apply tick now returns false;
   the next tick drains the queue, opens a frame if one is wanted, and
   only then releases — the branch written for exactly that, which never
   got a tick. The same for a frame's answer and a stale answer. And that
   branch decided on the frame the first time the queue was EMPTY — which,
   for a unit whose whole order was one `MOVE_TO`, is while it is still
   walking: nothing revealed yet, no frame, the turn latched settled. The
   queue now **watches** every unit's opening walk (`CivvisQueue.watch`, a
   rows-less entry that settles like any queued order — arrival, no
   movement, the host's event, or the grace period — and is dropped), so
   the frame decision is taken on the landed board.
2. **`FOUND_CITY` founded where the settler stood.** The row carried no
   site; the mod runs every found before the settler's walk (so a settler
   already on its site founds at once) and re-queues a refused one behind
   the walk. Civilization VI refuses a found only where founding is
   illegal, and the hex one step short of a chosen site is legal far more
   often than not (`CITY_MIN_RANGE` 3) — so a planned "step, then settle"
   founded BEFORE the step, and a walk the host capped short founded on
   the capped hex once the settler arrived with movement to spare. The
   row now carries the site (`stamp_found_sites`: the walk's last hex, or
   where the unit stands; `found_sites=N` in the note) and the mod founds
   only with the settler on it, naming a miss `found_off_site` — not
   `found_refused`, which feeds the brain's permanent `blocked_city_sites`.
3. No way to replay an old journal as today's mod would present it —
   hence `--assume-seat` (replay only; the live brain reads the seat the
   mod advertised).

**What to read on the first live run with this mod.** `orders_queue`:
`queue_turn_over` should be ~0 (it was the signature of defect 1);
`strikes_landed`/`strikes_planned` should converge. `replan_frame` /
`combat_frame` per turn against `combat_frame_timeout`. `found` x/y against
the preceding `FOUND_CITY` row's x/y in `orders.sqlite` (they must match
now; before, a run where they differ by one hex is defect 2 caught in the
act), and `found_off_site` in `orders`/`orders_queue` refusals (expected
once per step-then-settle turn: the found that ran first).

Tests: `step_turn_actions_test.lua` (step → frame → second
step through `settleTurn`, with a synchronous and an asynchronous host —
the latter proves the turn is held until the walk lands; step → shot from
the queue before release;
step → settle on the site, the first-run found refused by name without a
`found_refused`; a capped walk does not settle short; a row without a site
keeps the old behaviour),
`a_found_city_row_carries_the_site_the_walk_ends_on`,
`assumed_seat_capabilities_are_named_and_checked`.

## 13. The arena is the gate (2026-08-26)

The joint-tactics search of §7 won 99.6 % of its arena fights and was removed
on a whole-game screen that resolves fourteen kills a game. That is the wrong
instrument for this subsystem, and it has now culled three tactical layers in
a row. `docs/DOCTRINE_ARENA.md` ("Captured engagements, healing, and genes in
the arena") records the instrument that replaces it as the gate for tactical
genes: `doctrine_arena --capture` takes the engagements real games produce —
the board at first contact, rivers, wounded units and promotions included —
into a file the arena plays back paired and role-swapped; `--heal` makes
recovery measurable (the arena never healed, so unit preservation was never
priceable); and a seat is `advanced+<gene>`, so a gene is priced here before
the whole-game screen prices what it is worth. The whole-game screen becomes a
no-harm check for tactical genes, not their gate.

## 14. Close as a body, and screen the shooters (2026-08-26, opt-in genes)

Two terms in `coordinated_tactical_step`'s tile score
(`src/ai/advanced/close_as_a_body.rs`), the mover the deployed controller
actually uses. Neither adds a stand — `arrival-waves` (−3.0 pp) and
`contact-posture` (−1.14) both did, and both are gone; here every unit spends
its movement every turn and what changes is the tile.

- **`close-as-a-body`**: on an `Advance`, no unit ends the turn more than the
  body's pace (the slowest member's movement) plus one tile closer to the
  objective than the force's anchor stood; recon exempt, units in contact
  exempt. Measured on the gate of §13: the curriculum at 40 seeds **+24.2 ±
  9.5 a seed** (t 2.55, 87/52), carried by **`central_position` +116.5 ± 30.3
  (t 3.84, 26/7, sign p 0.0013)** — the position whose doctrine it encodes —
  and `the_reserve` +81 ± 53 (10/3, p 0.09); `battle_bench` −5.6 ± 6.8 (null);
  the 68-war file null, and it fires on 17 of 408 seeds there, because a
  captured board starts at contact and the gene acts on the approach.
- **`screen-the-shooters`**: a ranged or siege tile earns two screen weights
  when a melee friend stands beside it and nearer the enemy — the arena's own
  definition of screened, paid to the archer that stays behind the line.
  `battle_bench` stock army **+62.8 ± 19.9 a seed** (t 3.15, exchange 1.12 v
  0.90); curriculum −12.7 ± 13.5 (null); 68 wars null. Three forms were
  measured: both sides (+65.8 skirmish, −21.1 ± 14.8 curriculum), melee side
  only (+7.6 skirmish, −19.3 ± 12.1 curriculum), shooter side only (the one
  that ships). The melee half — a line stepping sideways to cover its
  archers — is what cost the curriculum, and it added nothing to the skirmish.

Both ship off; the whole-game screen is the no-harm check.
## 15. The fire plan (2026-08-26, opt-in gene `fire-plan`)

The removed joint search's measured value was mostly *ordering* — its static
seed lost 700 kills on identical total damage (§3 of TACTICS.md). `fire-plan`
(`src/ai/advanced/fire_plan.rs`) keeps that part without a clone: once a
turn, `legal_actions_within(UNITS)` names every strike the engine allows,
`ranged_strike_strengths` / `melee_exchange_strengths` price each at the
centre roll — the engine's own arithmetic, matchup, flanking, support,
terrain, river and fortification included — and kills are allocated
greedily, fewest shooters first, ranged before the melee finisher, with a
15 % margin over the centre roll where a spare shooter allows. The planned
shooters go first in the unit order and each is biased toward its planned
target in the attack scan; the exact, clone-verified attack decision is
unchanged. Off in both constructors, byte-identical when off.

Measured on the arena gate (§13): `battle_bench` stock six-unit army
−0.3 ± 13.1 (null; 84/160 seeds diverged), foot-heavy eight-unit army
+23.4 ± 19.2; the curriculum at 40 seeds **+20.6 ± 10.0 a seed** (t 2.05,
125/83, sign p 0.004), positive on nine of eleven boards and significant on
none alone; the 68-war captured file at 6 seeds +5.2 ± 5.9 with healing off
and **+14.0 ± 5.8** (t 2.42, 56/38) with healing on. The plan pays where
there are several shooters and a target worth finishing, and is inert in a
six-unit open-field trade. Whole-game screen pending, as a no-harm check.

## 16. The live seat can be told to play a gene, and the row says how the army fought (2026-08-26)

Three leaks in §2 and §3, closed together.

**A gene priced on the arena had no route to the live board.** `--with`
accepted only a *ledger-held live treatment* — a `Kind::Repair` or
`Kind::HostOnly` gene the batch rule had turned off — because that is a
restoration: the live universe set the flag and the ledger took it away, so
skipping the withholding restores it exactly. A `Kind::OptIn` gene was never
in that universe, so it could reach the live seat only by entering
`DEPLOYMENT_GENOME`, which is the whole-game screen's decision. Under §13's
policy that inverts the gate: the arena decides a tactical gene and the
screen is the no-harm check, and until the screen answers the gene cannot be
tried where it matters. `ledger_held_opt_in` / `forceable_treatments`
(`gene_ledger.rs`) name the other half, and `--with <opt-in>` now seats one —
an *addition*, recorded as such: `apply_gene_ledger_with_forced_live` puts it
in `applied.forced`, `deployment_treatments_with_forced_live` names it in the
arm's genome, and the deployment genome itself does not move. A
`Kind::HostOnly` gene is neither half: no screen row can hold one off, so every
live seat already plays it (`enable_live_bridge_universe` turns on every
`live()` gene), `--with` refuses it as already shipping, and `--without <tag>`
is the one lever that moves it — `python3 tools/genes.py list` prints such a
row `on(live)`, never `off`.

**The ladder row said nothing about the fighting.** It has carried
`applied_pct` since the bridge existed and nothing about the army, so every
claim about the live seat's exchange ratio (0.18 kills per loss, and the
Firaxis seats' 1.71) came from opening `HallofFame.sqlite` by hand or from a
code comment. `civ6_ladder.combat_totals` lifts the cheap half of the
tactical ledger — the half that needs only `events.jsonl` — onto the row:
`kills, losses, kills_per_loss, damage_dealt, damage_taken, cities_taken,
cities_lost, military_units_gone`, plus `forced` beside the `withheld` that
was already there. `None` on a run whose mod predates the tactical ledger,
which is a different statement from a seat that never fought.

**`city_occupation` was emitted and read by nothing.** The mod has written it
since the tactical ledger landed, so no report could say whether a war ended
in a capture — the question eleven declared wars and four sieges to 180-190
of 200 were waiting on. `city_occupations` counts a city taken from a rival
and one of ours lost (a city of ours retaken is neither), and it is now a
column on both the ledger report and the ladder row.

**`move_refused` was emitted and read by nothing**, and its own comment in
the mod said the point was that "the Rust side can feed a NAMED refusal back
so CIVVIS stops re-deriving the same impossible step". A failed `MOVE_TO` was
diagnosed by comparing two frames' positions — which reads the same for a
refusal, an exhausted allowance and an order the mod never issued. It is now
an `EVIDENCE_KIND`, and a `MOVE_TO` that did not move is failed as
`host_refused_impassable` / `host_refused_water` / `host_refused_move` when
the host said so, and `did_not_move` when it did not.

**What is still open from §3 after this:** the combat frame has still never
played (`CombatFrames` is 0 in all 261 ladder rows since 2026-08-19, and this
machine has held its live games since 2026-08-02); the swap verb is still
untranslated on the bridge, and `Action::Swap` — implemented, tested and
refused correctly in the engine since `do_swap` — is still chosen by no
controller (`docs/MOVEMENT.md` names the `swap-rotation` gene that would).

**Update (2026-09-02): the swap verb is translated.** `Action::Swap` crosses
as unit verb `SWAP` with `pos` = the partner's tile; the mod resolves
`UNITOPERATION_SWAP_UNITS` and requests it exactly as the shipped
`Civ6Common.lua:160-161` does (`CanStartOperation(unit, SWAP_UNITS, nil,
{PARAM_X, PARAM_Y})`, then `RequestOperation` with the same table), naming a
decline `cannot_swap`. The verdict is either half of the exchange on the next
frame — the unit on the partner's former tile or the partner on ours — else
`not_swapped`. What §18's `swap-rotation` decides can now reach the live seat.

**Update (2026-09-02): the air verbs are translated.** `Action::AirStrike`,
`Action::AirRebase` and `Action::AirPatrol` — legal in the engine since
`do_air_strike` / `do_air_rebase` / `do_air_patrol` and dropped by the bridge
as `unit_action_untranslated`, so no aircraft the live seat built ever flew —
cross as unit verbs `AIR_ATTACK` (`pos` = the target plot), `REBASE` (`pos` =
the new base plot) and `PATROL` (`pos` = the plot to fly to and intercept
from). The mod resolves `UNITOPERATION_AIR_ATTACK`, `UNITOPERATION_REBASE` and
`UNITOPERATION_DEPLOY` — there is no `AIR_PATROL` row in the shipped
`UnitOperations.xml`; a fighter's patrol is the shipped "Deploy" — and requests
each exactly as `WorldInput.lua:2077-2078` / `:2418-2419` / `:2486-2487` do
(`CanStartOperation(unit, OP, nil, {PARAM_X, PARAM_Y})`, then
`RequestOperation` with the same table), naming a decline `cannot_air_attack`
/ `cannot_rebase` / `cannot_patrol`. `AIR_ATTACK` passes `refuseWarStarter`
and `CivvisLedger.strike` like `RANGE_ATTACK`, so the preview and the combat
frame count follow it, and a refused one files `range_attack_refused` for the
decider's `blocked_strikes`. Verdicts: `AIR_ATTACK` like `RANGE_ATTACK`
(target harmed or a combat on the ledger, else `target_unharmed` / the host's
own reason); `REBASE` is the aircraft standing on the base plot next frame,
else `not_rebased`; `PATROL` is declared unverifiable — the frame carries no
patrol state. The own-unit export now carries `range` (`Unit:GetRange()`,
`UnitPanel.lua:2250`), the aircraft's operational reach from the plot it
stands on, which is its base; the mirror parses it (`StateUnit::range`) and
does not yet read it onto the board.

**Update (2026-09-02): the captured city's disposition is translated.**
`Action::KeepCity`, `Action::RazeCity` and `Action::LiberateCity` — mandatory
and exclusive on the board (`pending_city_capture_actions`: nothing else is
legal while a capture is unresolved) — were untranslated, and the mirror never
set `City::captured_from`, so the decision never even arose on the live board:
the mod lists `ENDTURN_BLOCKING_CONSIDER_RAZE_CITY` as a soft blocker, ends the
turn over it, and the host's default (keep) took every city. The export now
names the city the host is waiting on (`GetNextCapturedCity()`,
`Popups/RazeCity.lua:71`) with `captured_from` (`GetJustConqueredFrom`, `:86`)
and every own city's `original_owner` (`GetOriginalOwner`, `:85`); the mirror
maps both onto seats (`apply_city_capture`, both import paths), so the board
offers exactly the Keep / Raze / Liberate the shipped popup offers. The
decision crosses as order kind `city`, verb `KEEP` / `RAZE` / `LIBERATE`,
subject = the host city id, `pos` = the city plot; the mod requests the one
shipped command for all three, `CityCommandTypes.DESTROY` with the matching
`CityDestroyDirectives` flag in `PARAM_FLAGS` (`RazeCity.lua:15-55`;
`LIBERATE_FOUNDER`, since the engine liberates to the founder), gated by
`CanStartCommand` with the same table — a decline is `cannot_keep` /
`cannot_raze` / `cannot_liberate`. Verdicts: KEEP = still ours with the flag
clear next frame (`not_kept`); RAZE = gone or a citizen smaller next turn, or
the decision consumed on a same-turn frame (`not_razed`); LIBERATE = the
founder holds the plot (`liberated_to_another` / `not_liberated`).
## 17. Price it like the engine — and the null it measured (2026-08-26)

Two opt-in genes (`src/ai/advanced/engine_pricing.rs`) replace the
controller's two hand-written estimates of a fight with the engine's own
arithmetic: `exchange-is-the-engines` routes `exchange_score`'s defended
branch through `melee_exchange_strengths` / `ranged_strike_strengths` +
`expected_damage`, and `defend-where-you-stand` prices the defender on the
candidate tile with that tile's own defence, in `projected_counter_damage`
and in `coordinated_tactical_step`'s inline threat term. Both are strictly
more accurate than what they replace. Both measure **null**: `battle_bench`
+10.9 ± 11.9 and +5.4 ± 16.2, the doctrine curriculum +8.1 ± 10.3 and
+2.1 ± 6.4, the 68-war captured file −10.0 ± 6.7 and −13.9 ± 8.8 with
healing on — and 79 and 140 of 160 skirmish seeds diverged, so the arms
fired.

Recorded here because it belongs beside §7's null and §15's positive: the
tactical layer's accuracy is not obviously its constraint. `docs/AI_GAPS.md`
carries the hypothesis (`attack_threshold` was calibrated against the biased
estimate, so exactness without re-fitting the toll is two changes at once)
and the finding that the engine has no Great General to model.
## 18. Swap rotation, and what it says about why (2026-08-26)

`Action::Swap` has been legal, tested and correctly refused in the engine
since `do_swap`, and no controller had ever chosen one. `swap-rotation`
(`src/ai/advanced/swap_rotation.rs`, opt-in, off) is its first use: a unit
in contact and at or below `withdraw_hp` trades places with an adjacent
melee friend that is 25 hp healthier and further from the enemy, ahead of
the recovery step that would otherwise walk it away and take the tile with
it.

Positive on every instrument and significant on none: curriculum
+15.6 ± 11.3 with healing on and +14.4 ± 9.6 with it off, the 68-war file
+7.5 ± 6.0, `battle_bench` +20.2 ± 14.9 with the exchange ratio at 1.044
against 0.958. `the_reserve` — the position built to charge for a line
handled piecemeal — carries the curriculum at +115.

**The heal-off reading is the finding.** The gene was written expecting to
need healing, and it does not: the wounded unit never has to come back. What
pays is that the line does not open when it leaves. That is a different
claim from "rotate to heal", and it is the one the numbers support.

## 19. The arena can pose a siege, and the siege is an arrival problem (2026-08-27)

`docs/DOCTRINE_ARENA.md` ("The arena can pose a siege") records the
instrument: a position may state a city with hit points and a wall pool, a
captured `Engagement` carries the city the fight was over, and the ledger
counts what changed hands.

The first assault position, `the_storming` — a 200-hit-point city behind 100
points of wall against a siege train worth three times the garrison —
reproduces §1's live record in thirty-four turns: over forty assaults the
deployed controller took the city **three times**, and it **loses the
position** to `basic` at −200.8 ± 102.5 a seed (sign p 0.024).

**The cause is not the walls.** The besieger's arrival spread on that board
is **10.91 turns**, the worst in the arena against 1.2–8.1 everywhere else,
with contact at 70 %, the lowest anywhere. The siege train is fed to the
garrison a unit at a time — §10's finding, on the position that punishes it
hardest. `close-as-a-body` is the gene shaped for it and reads the right sign
(+37.5 ± 30.3) while firing on only 6 of 40 seeds, because it acts on an
`Advance` posture out of contact and this board reaches contact quickly. A
gene that paced a *siege train* specifically — hold the melee until the
catapults are in range and the wall is going down — is the next thing this
board can price, and it could not be priced at all before it existed.

## 20. The evacuation lands (2026-09-01)

Measured on the 32 live ledger runs of 2026-08-30..09-01 that reached turn
100 (`~/.cache/civvis/ledger/runs`, `tools/live_ledger.py pull`), reading
the `combat`, `state`, `order_verified`/`order_failed` and `host_move`
events:

| our combat deaths, 32 runs | count |
|---|---:|
| total, per run | 461, 14.4 |
| killed by barbarians | 408 |
| victim at 50 HP or less when the blow landed | 352 |
| victim already hit on an earlier turn, left in reach | 334 |
| victim hit twice or more on the death turn | 119 |
| ranged units killed by a melee attacker | 135 |
| scouts | 80 |
| victims with a `MOVE_TO` on the death turn that never executed | 200+ |
| victims with **no host move at all** on the turn before they died | 383 |

The decider was not the failure. The recovery (`BasicAi::healing_step`,
`retreat_step`) chose the evacuation and issued the `MOVE_TO`; the host
accepted it (`CanStartOperation` true, `RequestOperation` returned) and the
unit did not move. The order queue watched the leg for `grace` ticks and
then dropped the watch in silence, and the Rust side found the standstill
a turn later as `did_not_move` — 5,873 of them across the 32 runs, the
largest single refusal — by which time the unit was usually dead.

⚠ `live-move-refusal-break` was **on** for every one of these games: every
`Kind::HostOnly` gene ships on the live seat (`enable_live_bridge_universe`)
and the ledger never withholds one. It keys on a unit standing on the same
plot for `MOVE_REFUSAL_STRIKES` = 2 *consecutive turns* and then bars that one
step for 8 turns (`BasicAi::judge_move_refusals`). It never sees the same-turn
case, and a wounded unit rarely has two turns. `HostMoveRefusals` in
`civvis_orders.rs` is the same shape a turn later. Neither reads the mod's
`move_refused`, which is emitted only when `operate()` returns false — the
silent no-op (`operate()` true, nothing moved) reached no instrument at all.

Three mechanisms, each behind its own switch:

1. **The mod names and answers the no-op** (`CivvisBoard.moveNoop`,
   `fallbackStep`, `classifyNoop`; `MoveFallback` in the mod config, on by
   default, recorded in `mod_arms`). A `MOVE_TO` records the plot and
   movement it left from. A watched leg whose unit is still on that plot with
   its movement intact when the watch runs out is probed on the host —
   `cannot_start`, `no_path`, `beyond_turn`, `occupied`, `hostile_on_plot`,
   `zoc`, `hostile_adjacent`, `no_moves`, `unknown` — emitted as `move_noop`,
   and in the same pass the nearest legal neighbour toward the wanted plot
   is sent instead (`move_fallback`), once per unit per turn. A leg the host
   refuses outright takes the same fallback at once. The Rust verdict reads
   `move_noop` and names it `host_noop_<why>` instead of `did_not_move`.
2. **The gene `wounded-out-of-reach`** (opt-in, off; `advanced/wounded_out_of_reach.rs`)
   withdraws ahead of the recovery on the roll-top total of everything that
   reaches the tile (the recovery reads the mean), remembers a raider in the
   fog through `barbarian_reach`, and keeps a shooter or scout out of a
   raider's reach unless a melee unit stands beside it. It stands down when
   one attack would kill the last thing in reach.
3. **The row says whether the evacuation happened**
   (`civ6_tactics_ledger.evacuation_section`, lifted onto
   `combat_totals`): `deaths_wounded_at_turn_start`,
   `deaths_after_unexecuted_move`, `move_noop`, `move_fallback`. Read these
   beside `kills_per_loss` on the next runs; the first two are the numbers
   above and are computable on every run already on the ledger.

What to read first: `move_noop_reasons` on the first runs with
`MoveFallback` on. The reason distribution decides the next repair —
`occupied` and `zoc` are pathing the mirror could model; `unknown` is a
host behaviour still to be found.

## 21. The battle planner: the force's turn as one decision (2026-09-02, opt-in gene `battle-planner`)

Every step above leaves the unit-at-a-time decision in place: §15 orders the
loop, §18 rotates one pair, and each unit still prices its own attack from
where it stands. `battle-planner` (`src/ai/advanced/battle_planner.rs`,
opt-in, off) plans the turn jointly, in three parts, each on the engine's own
arithmetic rather than a copy of it:

1. **A danger field.** `danger(tile, unit)` is what every visible hostile that
   can reach and strike the tile next turn (`Game::attack_reach`) would do to
   our unit standing there unfortified — `melee_exchange_strengths` /
   `ranged_strike_strengths` and `expected_damage`, read on one speculative
   probe with the unit relocated onto the tile — plus the strike of every
   walled city or Encampment within two. One engine fact fell out of writing
   the test: stepping into a tile beside an enemy ends a move here (zone of
   control), so a melee unit two tiles off cannot close and strike in one turn
   and the field correctly reads zero there.
2. **A kill plan.** Every legal blow each unit could make, from its tile or
   after a move (never a siege unit that moved; a siege unit within range of
   an enemy city is left to the ladder), priced with the exact pair; a beam
   search (width 32, ≤12 shooters, ≤8 targets) over the ordered sequence,
   kills counted at 1.15× the hit points, ranged before the melee finisher,
   return damage and every end tile's danger charged on
   `tactical_attack_result_in`'s scale, a penalty for a target left at 1–30
   with a finisher to spare, and three vetoes (a return that kills the
   attacker, a striker under 50 hp not finishing from safety, melee into a
   walled city — cities are not targets here). The sequence is replayed on
   ONE clone through `tactical_attack_result_in` and refused blows dropped
   before it lands. It replaces `prioritize_immediate_kills` and `fire-plan`
   at their seam; the ladder leaves the planned units alone.
3. **A heal rotation.** A unit under 50 hp, or where the danger exceeds its
   hit points minus 20, steps to the nearest reachable tile with no danger
   (a district, friendly ground and an `adjacent_heal` neighbour preferred),
   fortifies, and stays out of the kill plan until 80 hp; a fresh friend
   behind it takes an `Action::Swap` instead where one is legal. On a board
   that does not heal only the lethal reading fires and nothing is
   remembered, for the reason `healing_step` gives.

**The gate (§13), read in order.** `battle_bench` control `advanced` v
`advanced`, 60 seeds: **+0.00 ± 0.00, 0 diverging**. Treatment, 200 seeds ×
2 seatings each: the default combined band **+476.2 ± 25.8 a seed (t 18.4;
182 better / 17 worse / 1 tied)**, exchange ratio **2.52 against 0.40**;
four archers and two warriors **+338.7 ± 21.3 (t 15.9; 173/22/5)**, 1.98
against 0.50; four warriors and two spearmen **+99.5 ± 11.8 (t 8.4;
141/33/26)**, 1.49 against 0.67. Doctrine control, 12 seeds: +0.0 with no
divergence on any of the twelve boards. Treatment, 40 seeds × 2 role swaps,
the rows: **the_reserve +368.2 ± 57.4 (t 6.41, 33/3, fires 39/40)**,
**central_position +194.2 ± 30.7 (t 6.32, 33/6, 40/40)**,
**hammer_and_anvil +161.2 ± 49.2 (t 3.27, sign p 0.029, 25/11, 40/40)**,
**the_storming +422.5 ± 77.6 (t 5.44, sign p 0.0002, 26/5, 40/40)**; every
board positive, the pooled row +278.3 ± 17.6 at **1.31 kills per loss
against 0.76**. Cities taken stay +1/−1 each way: the assault is not solved,
the garrison's defenders are killed more cheaply.

**Where the profile moved.** `focus` — the share of a side's blows on the
unit it had already hurt — is the column the plan claims and the one that
moved on every board: the_reserve 99 %/94 % against 93 %/78 %,
central_position 100 %/85 % against 96 %/60 %, hammer_and_anvil 77 %/100 %
against 66 %/98 %, the_storming 97 %/96 % against 91 %/81 %. On the_storming
the besieger's `salvag.` fell **54 % → 41 %** and its arrival spread
**7.69 → 5.90**, with `absent` up 1 % → 10 % — wounded units pulled out of the
ring rather than ground down in it, which is §19's diagnosis answered on the
symptom and not the cause. On the boards it was winning the winner's
salvageable share rose (the_reserve 40 → 43 %), the expected shape.

**What this does not say.** The fires probe (6 games, 36 seats, gene on in
8) reads `~`; the whole-game screen at scale is the no-harm check that
follows, and §7 and §17 record how little a tactical swing of this size has
moved a win rate before. Movement of the units that are not striking is
still `coordinated_tactical_step`'s; a `positions_plan` is the follow-up the
module is shaped for, and pricing the city assault jointly is the other.

## 22. The positions plan: where the units that are not striking stand (2026-09-02, opt-in gene `battle-planner-2`)

§21 left one thing to `coordinated_tactical_step`: the movement of every
unit the kill plan did not spend and the rotation did not pull out. That
mover prices one unit's next tile at a time against the group's *anchor* —
progress, cohesion, threat, spacing, a screen term — and the arena's own
finding about it (§14) is that it arrives over twice the span `basic` does
and leaves its shooters unscreened. `battle-planner-2` (version two of the
family; one version plays, `enable_battle_planner_2` turns version one off)
replaces that step for the members of an advancing or engaging force with a
plan laid against the *enemy* and the *objective*:

1. **Slots, not scores.** Front slots (Vanguard and Mobile) stand at
   distance one from the nearest visible contacts, on our side of them,
   the best ground first — `tile_defense_bonus` plus five for a river between
   the slot and the enemy it faces; with no enemy within three of any member
   they stand at the front's depth from the objective instead. Shooter slots
   stand at attack range from the three most finishable contacts, in line of
   sight, with a front slot between them and the enemy where one exists;
   siege slots in range of the objective city behind a front slot, furthest
   standoff first; support slots beside the most front slots, behind the
   line. There are as many slots of a kind as units of that role. Cohesion is
   a property of the layout alone: there is no scored adjacency or support
   term, which `docs/TACTICS.md` §4 and §7 swept and refuted.
2. **A minimum-cost assignment.** Units go to slots by the Hungarian method
   on route turns (`route_distance` at the unit's movement; one turn for any
   tile it reaches now) plus the danger field at the slot past the unit's
   spare hit points (`hp − 30`) at a twentieth of a turn per point. A slot the
   field says would kill the unit is never taken; a unit with no slot it can
   take keeps today's step. A unit under 60 hit points, where the board
   heals, takes a heal slot — a zero-danger tile, a district or friendly
   ground preferred — and enters the rotation's recovery.
3. **Reservations and order.** Every assigned tile is reserved; each unit's
   end tile this turn is the reachable tile nearest its slot that no one else
   holds and that would not kill it; moves are issued front to rear so a rear
   unit can enter the tile a front unit vacates, two units standing on each
   other's slots trade places by `Action::Swap`, and a second pass walks on
   anyone the first left short once the occupant has gone.
4. **Pace on the approach only.** With no enemy within two of any member,
   no unit ends more than the slowest member's pace plus one closer to the
   objective than that member stood — `close-as-a-body`'s slack, applied as
   a floor on the end tile rather than a penalty in a score. In contact
   nothing is paced.

Everything the plan does not place — a unit with no group, a scout, a
garrison, a member of a holding or mustering force, a unit whose every slot
was lethal or unreachable — plays the ladder exactly as before, so behaviour
changes only where a slot plan exists. One "Military/Decision" line per
force per turn says how many slots were filled, units placed and units paced;
`StrategyCensus` counts the same three.

**The gate (§13), read as version against version.** `battle_bench`
control `advanced` v `advanced`, 60 seeds: **+0.00 ± 0.00, 60 tied, no
divergence**. Treatment `advanced+battle-planner-2` v
`advanced+battle-planner`, 200 seeds × 2 seatings a cell: the default band
(warrior warrior spearman archer archer horseman) **+147.1 ± 30.5 a seed
(t 4.82, p < 0.0001; 122 better / 77 worse / 1 tied)**, exchange ratio
**1.23 against 0.82**, units lost 1,265 against 1,551, material destroyed
78,955 against 64,245; four warriors and two spearmen **+32.0 ± 18.1
(t 1.76, p 0.078; 102/92/6)**, 1.08 against 0.93; four archers and two
warriors **−9.4 ± 29.3 (t −0.32; 97/99/4)**, 0.95 against 1.05 — a null.
Fires: 200/200, 200/200 and 199/200 seeds diverged.

`doctrine_arena`, 12 seeds of control: +0.0 with no divergence on any of
the twelve boards. Treatment, 40 seeds × 2 role swaps, the rows —
**central_position +182.5 ± 37.9 (t 4.82, sign p 0.0005, 30/8, fires
40/40)**, **the_storming +285.0 ± 98.2 (t 2.90, sign p 0.014, 24/9,
40/40)**, **the_breakthrough +104.0 ± 43.8 (t 2.38, 24/13, 40/40)**,
the_ridge +165.0 ± 94.4 (t 1.75, 23/14), hammer_and_anvil +104.8 ± 64.9
(t 1.61, 23/16), the_river_line +100.5 ± 74.8 (t 1.34, 22/18),
oblique_order +62.5 ± 51.5 (t 1.21, 23/12), the_golden_bridge +26.5 ± 29.3,
double_envelopment +17.0 ± 10.8, the_defile +9.0 ± 13.8 (39/40),
lake_trasimene −22.8 ± 74.2 (t −0.31, 15/22), the_reserve −44.0 ± 67.2
(t −0.65, 17/18, 38/40). Pooled **+82.5 ± 18.1 (t 4.57, 236/153)**, kills
per loss **1.09 against 0.92**, cities taken **+8/−0**: as the besieger on
the_storming version two takes the city in eight of forty seeds where
version one takes it in none.

**Where the profile moved.** The columns the plan claims are `arrival`,
`foot`, `absent` and `screen`, and they moved the way the design says:
the_storming's besieger arrives in **2.87 turns' spread against 8.43**
(`foot` the same), screens **57 % against 33 %** of its shooter-turns, and
loses 2 % of the force to `absent` against 5 %; the_defile's column
**3.06 against 4.13**, screen **74 % against 55 %**, absent 8 % against 16 %;
hammer_and_anvil's hammer 1.63 against 2.07 and screen 44 % against 31 %;
central_position's interior body 3.67 against 5.10, screen 48 % against
38 %; the_reserve's near reserve 2.35 against 2.74 and screen 43 % against
33 %, the far reserve 2.71 against 4.03. `salvag.` — the share of a side's
losses already at or under 30 hit points the turn before — went the other
way for the besieger on the_storming, **69 % against 49 %**, and on the
column at the_defile fell 60 % to 40 %: on a board that does not heal a
unit that holds its slot is ground down in it, and a unit that is pulled
out never comes back, so the number reads as the slot layout holding
rather than as a leak (the heal slot exists only where the board heals).

**What it took to get there, in order.** The first cut placed every
member the kill plan had not spent, and read **−129 ± 30** on four archers
and two warriors (**80/114/6**), **−243 ± 81** on the_ridge and **−29 ± 18**
pooled: material destroyed fell, not material lost, because an archer the
beam had *declined* — a net-negative shot from inside the foot's reach —
was marked ordered and never reached the ladder's own exact-clone pricing,
which version one gives it and which takes the shot more often than the
plan does. So a unit with a blow to offer is the ladder's, and the ranged
cell came back to −69 ± 29 with the curriculum at +76 ± 17 pooled and every
board non-negative. The second cut found the rest of it in the shooter
slots themselves: with four archers and two front slots, two shooters stood
unscreened at range two — inside the foot's reach, unable to fire this turn
— so a shooter no front slot covers now stands one tile back and steps in
to fire next turn, and the cell reads −9 ± 29 with the default and melee
cells unmoved.

**What this does not say.** The fires probe (`gene_screen --games 6 --genes battle-planner,battle-planner-2
--start-seed 97400100`, `--analyze` → `docs/gene_screens/fires/battle-planner-2.json`)
has version two on in 6 of 36 seats and reads win +20.0 pp at z +1.30 with
share +5.8 pp at z +2.8 — proof the gene fires, and at six games nothing
more (the run resolves ±44.8 pp on win). The whole-game screen at scale is the
no-harm check; §7 and §17 record how little a tactical swing has moved a win
rate before. Pricing the city assault jointly remains the follow-up.

## 23. The siege train and the anvil: a force whose objective is a city (2026-09-02, opt-in genes `siege-train`, `anvil`)

§19 ended on the gene the siege board could price and did not have: one that
paced a siege train — held the melee until the guns were in range and the
wall was going down. The only ring seal in the tree, `rush_siege_step`, is
gated on `plan.rush`; the group mover's role spacing puts shooters at their
range and melee at one; nothing decided *when* the train closed, *which*
tiles sealed the ring, *what* each arm shot, or *who* walked in. The live
record has the same shape at two hundred turns: eleven wars, four sieges to
180–190 of 200, no capture. `siege-train` and `anvil`
(`src/ai/advanced/siege_train.rs`, both opt-in, off, byte-identical when
off) are the two doctrines of a force whose objective is a city.

**`siege-train`** is a state machine per objective city — keyed by the city,
because force groups are rebuilt every turn and a group's id is its lowest
unit — with five stages and one "Military/Decision" line a turn:

- *Stage.* The train gathers on the 3–5 ring, never inside the City Center's
  own strike reach, until the strength standing there meets the bill: the
  defenders within six, the city's strength and its walls at ten a hundred
  points, times 1.25. A unit inside the reach steps back out; one far off
  marches to the ring; relievers that come out are fought on the exact
  forward model. On an arena the gate is arrival alone, for the reason the
  posture ladder gives (no reinforcement is coming).
- *Invest.* Every unit gets a post for the turn, drawn once at assessment:
  melee take ring tiles in `rush_siege_step`'s spread-first order (a zone of
  control covers a ring tile and both its ring neighbours, so two units
  three apart seal what two side by side do not), the taker first; guns and
  shooters keep a tile they can already shoot the city from, else take one
  at their range behind a ring post and away from hostiles. The ring is
  sealed when every passable neighbour is held or covered — the test
  `Game::city_under_siege` applies, and the condition under which the city
  stops healing twenty a turn. The test fixture proves it the engine's way:
  the city at 150 does not heal at its owner's end of turn with two
  warriors three apart on the ring, and heals to 170 when one is removed.
- *Reduce.* Guns shoot the city — walls first by `city_take_damage`'s own
  routing, then the garrison — unless a reliever within three of the city
  can be killed with a 1.15 margin. Shooters kill a reliever if they can,
  shoot units while the wall stands, and turn on the city once it is down.
  Melee on the ring hold it and fortify: a swing at a wall above a fifth of
  its pool lands fifteen percent on the wall and one point on the city and
  costs a return blow, so it is refused unless a ram or tower stands by.
- *Take.* One melee-capable unit — adjacent, or one move from the ring, with
  the most movement — is the taker and is reserved: excluded from every
  other blow and move, published through `reserved_units` for a joint
  planner to read. When the city's hit points are within its expected blow
  (the engine's melee arithmetic against `city_strength`, routed through the
  wall pool as the engine routes it) it attacks, and the attack that reduces
  the city is the capture.
- *Hold.* The ladder's own `occupation_garrison_target` seats one unit; the
  rest release to a group whose objective has moved on.

Two engine facts fell out of writing it. A unit's route to a ring tile runs
through the ring when that is shortest, and a unit entering the city's zone
of control there is stopped on the wrong tile — the three-warrior fixture
clumped on three adjacent tiles and left one side open — so the approach is
by explicit steps that never cross a ring tile other than the goal, with one
sideways step allowed before the unit has moved. And a goal that re-ranks as
the unit walks makes it walk out and back within the turn: posts are drawn
once a turn, for the whole train.

**`anvil`**, for `plan.threatened_city`: the land group nearest it holds the
city as a formation in place of the relief hold point — a shooter on the
City Center (the garrison bonus and the city strike), melee on the two or
three adjacent tiles that face the enemy with the best `tile_defense_bonus`,
everyone else within two so the city strike joins their fight, never an
empty ring while a hostile stands within six. A unit under 50 hp rotates
into the city to heal by `Action::Swap` with the fresh unit standing there,
which takes its tile — executed the moment the posts are drawn, before any
unit has spent its movement fortifying, which is the order the first fixture
got wrong. The formation engages relievers only when the exchange favours it:
a shot has no return; a melee blow is taken when the engine's pair says it
deals more than it takes and leaves the unit standing.

**The gate (§13), read in order.** `doctrine_arena` control, 12 seeds:
`+0.0`, 0/12 diverging on all thirteen boards. `the_storming`,
`advanced+siege-train` against `advanced`, 40 seeds × 2 roles: **+474.2 ±
67.1 (t 7.07, sign p < 0.0001, 33/5, fires 40/40)**, **1.57 kills per loss
against 0.64**, and **cities +29/−1 against +1/−29** — the city taken in
twenty-nine of forty assaults. Besieger arrival 8.86 → 6.08, `screen` 38 %
→ 51 %, the besieger's swing per seed −291 → −54. On top of the kill plan
(`advanced+battle-planner+siege-train` against `advanced+battle-planner`):
**+284.5 ± 94.7 (t 3.00, sign p 0.0135, 24/9)**, 1.38 against 0.72, **cities
+10/−2 against +2/−10**, arrival 7.80 → 3.74. `the_relief` (the new board,
`docs/DOCTRINE_ARENA.md`), `advanced+anvil` against `advanced`: **+64.8 ±
33.0 (t 1.96, sign p 0.26, 23/15)** — the right sign, not a reading yet.
`the_storming`'s garrison with the anvil: **+175.8 ± 57.1 (t 3.08, sign p
0.0106, 18/5)**, 1.13 against 0.88, `ground` 16 % against 11 %, `screen`
28 % against 10 %; the garrison lost the city twice in forty against once,
one event at this n. `battle_bench` no-harm, `advanced+siege-train+anvil`
against `advanced`, 100 seeds × 2: **+132.5 ± 33.3 (t 3.98, p 0.0001;
62/37/1)**, exchange 1.21 against 0.83.

**What this does not say.** The fires probes (six games each; the gene on in
11 and 9 of 36 seats) read `~`, and the whole-game screen at scale is the
no-harm check, for the reason §7 and §17 give. `battle-planner` does not yet
read `reserved_units`, so on a board where both are on the plan can spend
the taker's attack before the taker's turn; the hook is published for that
change. The bill is read from visible defenders only, and a coastal city's
water side cannot be sealed by land units — both are the engine's facts,
not the doctrine's choices.

## 24. The Objective Board: what the army is for, ranked, and the forces raised against it (2026-09-02, opt-in gene `objective-board`)

Every step above plans *how* a force fights. What a force is *for* is still
`rebuild_force_groups`: every field unit clustered by proximity (a clique of
`command_radius` six), anchored on its medoid, aimed by `domain_objective` at
one empire-wide objective — the threatened city, else the target city, else
the nearest enemy — and given a posture from a ladder of ratios. The groups
are rebuilt every turn and after every strike, a group's id is its lowest
unit, nothing is ever *asked* of the army, and two cities under pressure at
once produce one `threatened_city` and one relief: the argmax flips between
them and the second city gets nothing. §1's record has that shape — the
relief column that holds at its centroid, forty cities lost on the King rung,
a siege fed to a garrison a unit at a time.

`objective-board` (`src/ai/advanced/objective_board.rs`, opt-in, off,
byte-identical when off; runs for every major seat, `victory_planning` or
not) replaces the clustering and the ladder with a board written once a turn:

1. **Rows.** `Defend` a city whose pressure reaches `BASTION_PRESSURE`
   (0.45), `Relieve` it from beyond six, `Siege` the plan's target city and
   the campaign's cities in order, `Destroy` a hostile force in the field not
   already covered by a Defend or a Siege, `ClearCamp` a camp within nine of
   a city before turn 100, `Escort` a settler or builder outside our borders,
   `Deter` the strongest bordering major while our power is under 0.8 of
   theirs, `Recon` an unexplored sector no scout holds. Each carries a
   **value in hammers** (a unit its cost at its hit points; a city the
   replacement production of its districts and buildings, the lane's own
   district at 1.5, plus 20 a citizen; a settler its cost plus 200; a camp
   120 plus its guard; Deter 0.3 of the contact city), a **requirement**
   (`ForceNeed`: strength, melee, ranged, siege, bodies — Siege the campaign
   bill × 1.25 with a melee taker and siege while walls stand; Defend the
   hostile strength within six × 1.2 less the city's own; ClearCamp the guard
   × 1.5; Escort one melee, plus a ranged unit when a hostile is remembered
   within six; Destroy the body × 1.5) and a **deadline** where one can be
   named (Defend: turns until the city falls at the damage it has been
   taking, never under two; Escort: turns until a known raider is in reach;
   Destroy: turns until our nearest unit can reach it).
2. **Rank.** Value over deadline (ten turns where there is none), under two
   hard rules: a Defend whose deadline is inside the relief time of the
   nearest force outranks every offensive row, and no row ranks above one it
   depends on — Relieve after its Defend, the campaign's second city after
   its first.
3. **Task forces.** Kept on the controller across turns with an id that
   survives the death of any member. Allocation walks the rows in rank order
   taking the best contribution per travel turn until the row is met — a
   unit's strength toward the unmet need and a body toward an unmet count,
   times an arrival factor of one inside the deadline and 0.7 a turn late
   after it. A served row is never stripped below its need by a lower one; a
   unit stays in its force unless the gain is at least 25 % or its row is
   done; an urgent Defend may pull anyone. The leftovers are the **Reserve**
   at the Deter row's tile, else the frontier city nearest the strongest met
   rival, else the capital. Sea units form their own forces for a coastal
   Siege, an embarked Escort and a naval Destroy; air units stay out.
4. **Integration.** `force_groups` is built *from* the forces — one
   `ForceGroup` per force, `objective` the row's tile, `anchor` where a
   standing force stands, `posture` from the row's doctrine: Defend and
   Relieve hold the city and engage the threat on contact; Siege follows the
   siege train's stage when that gene is on (Muster while staging, Advance
   to invest, Engage to reduce and take) and otherwise musters on the far
   side of the city, advances and engages on contact; Destroy engages at an
   exchange of 1.5 and holds defensive ground below it; ClearCamp and Recon
   advance; Escort and Deter hold. So `battle_planner.rs`, `siege_train.rs`
   and the per-unit ladder read `force_groups` unchanged. On an arena — no
   reinforcement coming, nothing to hold — the army is one force per domain
   aimed at the top-ranked row, the shipped layer's own one-group doctrine
   with the board choosing the objective; `central_position` is the board
   that insisted (below).
5. **The record.** One "Military/Strategy" line a turn — `Board: Defend
   Aquileia (value 2,400, need 180, force #3 of 6, deadline 4) · Siege Seoul
   (value 1,120, need 260, force #5 of 4, staging) · …` — with the rows by
   kind, the forces, the units that changed row and the rows left short;
   `StrategyCensus` counts the same four; `AdvancedAi::requisitions()`
   publishes the shortfall per row (kind, units short, by which turn, at
   which city) for a production consumer — the next change; nothing reads it
   yet.

Ten deterministic tests hold the claims: the gene ships off and is
registered; off, the board is never written and the shipped group is built;
two pressured cities produce two Defend rows and both are served; a force is
not stripped below its need by a lower row; a task force's id survives the
death of its lowest member and the reserve fills its gap; a Siege row's
requirement is the campaign bill × 1.25 with a taker and siege while the
walls stand; a Defend inside relief time outranks a Siege worth far more and
pulls the far warrior home; a camp within nine is a row before turn 100 and
not after; a settler outside the borders is an Escort row and a requisition;
on `central_position` the army is one force and nothing stands still.

**The gate (§13), read in order.** Every arena cell below is on the ci
binaries built from this branch, with `battle-planner-2` on both sides of
the captured cells so the board is read on top of the shipped planner.

- `doctrine_arena --capture --games 24` took **599 engagements** from 24
  whole games; the host was loaded, so the cells were run on the **first 60**
  of them (same JSON shape, a prefix of the file) — a sample of the
  distribution, not the curriculum. Control on those 60 boards, `advanced`
  against itself, 12 seeds × 2 roles: **+0.0, 0/12 diverging on every board,
  healing off and on**.
- Captured file, `advanced+objective-board+battle-planner-2` against
  `advanced+battle-planner-2`, 60 boards × 40 seeds × 2 roles, healing off:
  **+14.7 ± 2.7 a seed (t 5.40, sign p 0.0004, 767 better / 633 worse)**,
  **1.03 kills per loss against 0.97**, cities **+119/−117 against
  +117/−119**. Healing on: **+7.4 ± 2.7 (t 2.73, sign p 0.0355, 764/683)**,
  1.02 against 0.98, cities +405/−433 against +433/−405.
- Curriculum, `advanced+objective-board` against `advanced`, 13 boards × 40
  seeds × 2 roles (control 12 seeds: +0.0, 0/12 on all thirteen): pooled
  **−0.2 ± 13.3 (181/174)**, 1.00 kills per loss each way. The rows, which
  are what a curriculum is read on: **hammer_and_anvil +137.8 ± 47.4 (t 2.91,
  sign p 0.0167, 25/10)**, the_ridge +111.5 ± 65.8 (t 1.70, 20/10),
  oblique_order +43.8 ± 50.7, double_envelopment +21.0 ± 13.6 (t 1.55),
  central_position +12.5 ± 29.8 (18/18), the_golden_bridge +8.5 ± 23.4,
  the_relief −7.8 ± 5.5 (fires 16/40), the_storming −15.0 ± 15.0 (fires
  2/40 — a Siege row aims the force where the shipped objective already
  did), the_defile −24.0 ± 12.9, the_reserve −39.0 ± 66.8, the_river_line
  −65.5 ± 71.2, lake_trasimene −92.5 ± 77.4, the_breakthrough −93.8 ± 50.5
  (t −1.86, 19/20). Nothing negative is a reading at this n; two boards are
  positive readings.
- `battle_bench`, `advanced+objective-board` against `advanced`, 100 seeds ×
  2 seatings (control 60 seeds: +0.00, 60 tied): **+18.7 ± 22.0 (t 0.85,
  p 0.40; 52/40/8)**, exchange ratio 1.047 against 0.955 — the no-harm
  reading, a null.
- Fires (`gene_screen --games 6 --genes objective-board --start-seed
  97600100`, `--analyze` → `docs/gene_screens/fires/objective-board.json`):
  on in 7 of 36 seats, win +14.8 pp ± 15.8 (z +0.93), **share +4.92 pp
  (z +3.28)** — proof the gene fires, and at six games nothing more.
- The whole-game no-harm read (`gene_screen --games 24 --difficulty emperor
  --genes objective-board,battle-planner-2 --p-on 0.75 --start-seed
  97600200`, 144 seats): `objective-board` on in 106 seats against 38 off,
  win **17.9 % against 13.2 %, +4.8 pp ± 7.1 (z +0.67)**, share **+1.69 pp
  (z +1.84)**, `~`; `battle-planner-2` beside it +9.2 pp ± 6.4 (z +1.48).
  Not negative; at 24 games not a reading either, for the reason §7 and §17
  give.

**What it took to get there, in order — the arena's corrections.** The
first cut held a Defend force at its city on every board, arena included,
and read +9.1 ± 3.0 on the captured file at **86 cities taken against 136**:
the garrison never left home. The shipped ladder's arena exception applies —
nothing stands still where nothing is coming — so a Defend on an arena
engages the threat (and in a game the group is sent at the nearest besieger
on contact, as `domain_objective` sends a relief). That cut read −1.5 ± 2.8
with the captures still 86 against 123, and on the curriculum
**central_position −184.2 ± 48.7 (t −3.79)** and **the_breakthrough −143.8 ±
39.0**: two hostile bodies made two Destroy rows, the army split between
them by contribution per travel turn, and each half engaged a whole body.
Merging only the forces under their bill made it worse (−262.2 ± 50.3),
because the army then marched past the nearer body to the one worth more:
a Destroy row had no deadline, so a farther body of the same value ranked
level with the near one and a bigger body ranked above it. Two changes
answered both: **a Destroy row falls due when the nearest unit of ours can
reach it** — the nearer body ranks first at equal value and a farther one
outranks it only by being worth proportionally more, which is the central
position's own rule — and **on an arena the army is one force per domain,
aimed at the top row**, the shipped one-group doctrine with the board
choosing the objective. central_position came back to +12.5 ± 29.8 (18/18)
and the captured file to the numbers above.

**What this does not say.** The captured file is 60 of 599 engagements
and a prefix of the file, not a random draw; the curriculum is read on its
rows and two of thirteen are readings. The board's whole-game machinery —
persistent forces, hysteresis, the Reserve, the requisitions — is what an
arena cannot price: a fixed army over sixteen turns has nothing to raise and
nowhere to send a shortfall, so the arena prices the ranking and the posture
map, and the whole-game screen at scale is where the rest is read.
`requisitions()` has no production consumer yet; the Deter row has no
requirement and is the Reserve's anchor only; a Relieve row's need is the
Defend's less what stands within six, read once a turn; sea forces exist
for the three sea rows and take the shipped sea mover unchanged. The
`victory_planning` gate does not apply: the board runs for every major seat.

## 25. Requisitions: the board's shortfall reaches production and the treasury (2026-09-02, opt-in gene `requisitions`)

§24 left the board's shortfall published and unread: `requisitions()` said
what every row still lacked, and nothing built it. Meanwhile the army was
sized by a headcount — `city_count`, doubled at war — every military unit
was `best_military`'s strongest producible unit of a role whatever it cost,
and the city-defence purchases ran their own detectors (`besieged_city_item`
for a bleeding city, `border_parity_*` for the strongest bordering major),
each with its own reserve and its own unit, none of them knowing what the
field was short of.

`requisitions` (`src/ai/advanced/requisitions.rs`, opt-in, off,
byte-identical when off, inert without `objective-board`) makes the board
the single source:

1. **Production.** Ahead of every economic reserve item in
   `advanced_production`, an idle city starts the unit a requisition asks
   of it — the row's nearest city, or the nearest idle city that can build
   the kind when that one is busy or cannot. The kind follows the unmet
   need: siege while the row lacks siege, ranged while it lacks ranged,
   melee while it lacks melee; a bare strength shortfall asks a shooter for
   a city (Defend, Relieve, Deter — §23's anvil seats a shooter on the
   centre) and balances melee against ranged in the field, cavalry for a
   Destroy falling due within three turns, a ship for a sea-only row. Within
   the kind the unit is the best **worth per hammer** — e^(0.08·strength),
   the two-sided exchange the damage formula gives ten points of strength,
   over the cost — so a Swordsman outbids a Warrior and never the other way.
   Units already queued for the kind are credited to the rows in rank
   order, so a shortfall of two does not start six.
2. **Gold.** Right behind the emergency defence, the highest-ranked open
   requisition the treasury covers above the reserve is bought at its city,
   one a turn.
3. **Routing.** `border_parity_purchase` and the `border-parity-2` idle
   block stand down (the Deter row now raises a requisition for the parity
   gap at the contact city, at most three bodies), `border_parity_production`'s
   severe-deficit pre-emption takes its city and unit from that requisition,
   and `emergency_city_defense_purchase` takes a bleeding city's unit from
   its Defend row; the wall answer is untouched.
4. **Composition.** While the land military is under the board's summed
   need — bodies in the land forces plus bodies requisitioned —
   `production_value`'s `desired_military` is that need, capped at four a
   city.

**Reading.** Fires (`gene_screen --games 6 --jobs 3 --genes
objective-board,requisitions --p-on 0.75 --start-seed 97700000`,
`docs/gene_screens/fires/requisitions.json`): on in 25 of 36 seats, win
+10.9 pp ± 13.1, share +0.43 pp — fires. Whole-game no-harm (`--games 24
--jobs 4 --difficulty emperor --genes objective-board,requisitions --p-on
0.75 --start-seed 97700100`, 144 seats,
`docs/gene_screens/requisitions-noharm.json`): requisitions on 115 / off 29,
win **17.4 % v 13.8 %, +3.6 pp ± 8.0 (z +0.44)**, share +0.01 pp; techs at
the end +3.6 ± 2.8; compute cost +7.3 ± 3.2 %. Not negative, not a reading
at this n. objective-board on the same seats +5.3 pp ± 6.0.

**What this does not say.** A requisition's count is bodies at the board's
average body, so a strength shortfall of one Warrior asks one unit even
when the unit built is three times a Warrior; the pipeline credit reads the
head of each queue only; the Deter requisition is the board's, so
`border-parity-3`'s local-staging detector is untouched; Recon rows are
not served (the exploration governor buys scouts); the purchase and the
production pass read the board as assessed this turn (production assesses
it first), the Gold pass one turn behind.

## 26. War policy via the board: feasibility, the declaration, the peace term (2026-09-02, opt-in gene `war-policy-via-board`)

The board of §24 writes what every objective *costs* — a Siege row's
`ForceNeed` is the bill for that city — and the strategic layer still
decided the wars on empire totals: `assess` ranked rivals by
`rival_value_with_culture` (distance, power, score, victory pressure) with
no test that the army could ever take a city of theirs; the elective
declaration asked `my_power > target_power × 1.32 + 12` (or the campaign's
bill, or the domination ratio) and a staged stack of three; and the peace
chain sued the moment `my_power < theirs × 0.62` — "outmatched" — whatever
the front looked like, so a defensive war whose threatened city was held
by a served force was begged out of on the empire total while the field
was even.

`war-policy-via-board` (`src/ai/advanced/war_policy.rs`, opt-in, off,
byte-identical when off; reads the board when `objective-board` is on and
the board's own `siege_requirement` either way):

1. **Feasibility.** A rival whose nearest city's Siege requirement is over
   the whole roster's strength (`campaign_field_army`) is not a target:
   `assess` drops it from the candidates it ranks and from the campaign's
   own choice. A rival whose clock is short (`urgent_victory_threat`) is
   untouched — denial is not elective. "Not a target" (Detail) names the
   city and the two numbers.
2. **Declaration.** An elective or campaign war opens only when the
   strength on the objective city's 3–5 ring (`staged_campaign_units`)
   meets that city's Siege requirement, no other major war is being
   fought, and every Defend row is served (no Defend requisition open). The
   `close_enough` and `staged` gates stand; the "Holding off war" line
   carries the board's reason.
3. **Peace.** The `0.62` term is replaced: peace is sued for only when no
   Siege row against that rival is feasible (the board's Siege rows against
   them, else their nearest city) **and** either the tide ledger reads net
   negative over its window — §24's `one_war` exchange, kept here per rival
   at war, gene or no gene — or an urgent Defend row has gone unserved for
   three turns. A defensive war with a served Defend row and an even tide
   is fought, not begged. Every other peace term stands; the tribute the
   `0.62` rout licensed is not claimed by this term (white peace).

**Reading.** Fires (`gene_screen --games 6 --jobs 3 --genes
objective-board,war-policy-via-board --p-on 0.75 --start-seed 97700250`,
`docs/gene_screens/fires/war-policy-via-board.json`): on in 25 of 36 seats,
win −15.3 pp ± 12.1, share −2.35 pp ± 1.16 (z −2.02) — fires, and reads
against at six games. Whole-game no-harm (`--games 24 --jobs 4 --difficulty
emperor --genes objective-board,war-policy-via-board --p-on 0.75
--start-seed 97700200`, 144 seats,
`docs/gene_screens/war-policy-via-board-noharm.json`): on 114 / off 30, win
**14.9 % v 23.3 %, −8.4 pp ± 7.1 (z −1.19)**, share **−0.84 pp ± 1.18
(z −0.71)**, `~`; objective-board on the same seats **+15.8 pp ± 4.6
(z +3.43)**, share +2.24 pp (z +2.17). Not significant, and the sign is
against on both probes: the suspicion is the feasibility bar — a Siege
requirement is the campaign bill (defenders + city + walls, × 1.5) × 1.25,
and the campaign itself plans on three quarters of its bill, so a roster
the campaign would have marched with is refused a target here, and an
army that never gets a target never gets the score a war brings. The
ledger prices it at scale; the natural repair is to read feasibility at
the campaign's own planning fraction.

**What this does not say.** The tide is the war ledger's unit and city
losses, so a war with no exchange reads even; the unserved-Defend clock
reads the board as it stood at the last unit pass; the declaration's ring
strength counts `staged_campaign_units` (3–5 out, no enemy territory), so
a stack inside three is not staged; a city-state target is gated like a
major.

## 27. The battle planner reads the wider machinery: the siege's taker, the host's price, the previews asked for (2026-09-02, opt-in gene `battle-planner-3`)

§21–§22 planned a force's turn from the board alone; §23 published the
siege's taker through `reserved_units` for a joint planner to leave alone,
and the planner did not read it; #3026 gave the board the host's own
`SimulateAttackInto` reading (`Game::host_preview`, a `preview` order
answered at issue time) and nothing consumed it. `battle-planner-3`
(`src/ai/advanced/battle_planner.rs`, opt-in, version three of the family:
`enable_battle_planner_3` turns one and two off, `battle_planner_on()`
covers all three, `MAX_VERSIONS` is 3) is version two plus three readings:

1. **The taker is the siege's.** A unit `siege_train.rs` has reserved
   (`unit_is_reserved`) is skipped by the kill plan, the heal rotation and
   the positions plan. The Take blow is the siege train's own step — the
   planner's targets are units, never cities — so the exception the brief
   names falls out: the planner never spends the unit that walks in.
2. **The host's price beats the closed form.** Where `Game::host_preview`
   holds a reading for a `(unit, target tile, ranged)` pair, the candidate
   carries the host's damage both ways in place of `expected_damage`'s
   centre roll — from every stand of the pair, since the reading is made
   of the defender's tile and health and the attacker's health and
   promotions, none of which a move changes (a river crossing is the one
   term it misses) — and a blow the host says kills the attacker is vetoed
   outright, whatever the kill is worth. Native boards hold no previews,
   so the closed form stands there.
3. **Asking for the price.** Before the search, the top 24
   (`MAX_WANTED_PREVIEWS`) candidate pairs by closed-form damage from the
   unit's own tile that have no reading yet are published through
   `AdvancedAi::wanted_previews()`; `civvis_orders` turns them into
   `preview` orders (`kind: "preview"`, `ATTACK`/`RANGE_ATTACK`, the target
   plot) ahead of the frame's strikes, and the answers reach
   `Game::host_previews` on the turn's next frame, where the plan reads
   them. Sixteen lines in `decide`, after `take_turn`.

**Reading** (the tactical gate, `docs/DOCTRINE_ARENA.md`; ci binaries of
this branch; every number is `advanced+battle-planner-3+siege-train` less
`advanced+battle-planner-2+siege-train`, so what the arena prices is rule 1
alone — rules 2 and 3 are live-only):

- `battle_bench` control `--a advanced --b advanced --games 60`: +0.00,
  0/60 diverging.
- `battle_bench --games 200` (×2 seatings): **−11.30 ± 4.56 a seed (t −2.48,
  p 0.013; 9 better / 18 worse / 173 tied, fires 38/200)**; exchange 0.985
  v 1.016; units lost 1,694 v 1,668. On the open field the siege train
  reserves a taker for a city that is not there to take, and the unit it
  holds back is a blow the plan does not strike.
- `doctrine_arena --position the_storming` control `--seeds 12`: +0.0,
  0/12 diverging. `--seeds 40` (×2 roles): **−2.8 ± 42.5 (t −0.06, 4/6,
  fires 18/40)**; kills per loss 0.98 v 1.02; **cities +16/−11 v +11/−16**
  — the city taken five more times in eighty assaults, the reading rule 1
  exists for; besieger arrival 2.82 v 2.85, screen 58 % v 56 %.
- Fires (`gene_screen --games 6 --jobs 3 --genes
  battle-planner,battle-planner-2,battle-planner-3 --p-on 0.75 --start-seed
  97700300`, `docs/gene_screens/fires/battle-planner-3.json`): version three
  on in 6 of 36 seats, win −20.0 pp ± 17.9 (z −1.12), share −0.95 pp — fires,
  and nothing more at six games.

**What this does not say.** The bench and the arena cannot see rules 2 and
3: the live seat is where a host reading exists, and the live arm
(`~/.civvis-live-force-on`) is where they are read. The host prices a pair
from the unit's tile; the plan applies that price to the same pair after a
move. A `preview` order is a question the mod answers with an event and
`UNVERIFIABLE_ORDER_KINDS` already lists it, so the verdicts do not chase
it. The wanted list is the last plan's; a replan frame within the turn
reads the answers, the first frame of a turn asks.
