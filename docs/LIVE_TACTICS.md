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
