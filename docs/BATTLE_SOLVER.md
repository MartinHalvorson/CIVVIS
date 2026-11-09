# The battle solver

Gene `battle-solver` (opt-in). Code in `src/ai/advanced/battle_solver/`, with
the engine reads in `src/game/battle_board.rs`.

`battle_planner` plans one turn and prices it against one enemy reply. It
cannot see that a position it likes loses the fight two turns later, or that
walking away now wins it. The solver plays each local fight forward to its end
and picks the turn by how the battle comes out.

## What it does each turn

1. **Finds the fights** (`extract::theaters`). Our land units within
   `ENGAGE_RADIUS` of a visible hostile, joined where they share one.
2. **Reads each fight once from the engine** (`extract::snapshot`): strength
   tables (`melee_pair_bases`, `ranged_pair_bases`, `city_pair_bases`), the
   exact first-turn reach (`approach_arrivals`), line of sight, ground, rivers,
   heal rates. From here on nothing touches `Game`.
3. **Names the objective** (`extract::choose_objective`): keep a city under the
   enemy's reach, take the plan's city or one we stand beside, otherwise destroy
   what is here.
4. **Searches** (`search::solve`). A joint turn is one line per unit: where it
   ends and what it strikes. Each posture the objective allows (`Press`, `Hold`,
   `Withdraw`) gets a seeded turn improved one unit at a time on rollouts, then
   the postures' best turns play a final on more rollouts. Every candidate is
   scored by playing the battle out for `horizon` rounds on common dice.
5. **Plays the turn** (`apply_solution`): shots, then blows, then moves. When a
   roll surprises the plan (a target dies early, or survives a kill the plan
   counted on) the rest of the fight is re-solved on the real position.
6. **Writes why** to the journal: objective, posture, kill chances, expected
   losses, and each posture's score.

## Exact dice

`dice.rs` carries a blow's whole distribution through a volley, wound penalty
included, so a kill is a probability and not a margin. Against 4,669 live Civ VI
strikes (2026-11-08/09, `strike.preview` joined to the `combat` that resolved
it) the host's dealt/preview ratio has mean 0.997 and range 0.80–1.20: the
engine's roll is the host's roll.

The wound penalty is why this matters: at 70 hp, two blows centred on 30 kill
**68%** of the time, while a mean-damage reading says the kill cannot happen.

## How it is held to the engine

`extract_tests.rs` asserts on real positions that the model's melee and ranged
strengths equal `melee_exchange_strengths` and `ranged_strike_strengths`, that
its movement flood equals `approach_arrivals`, and that the engine accepts every
line the search plans.

## What the enemy is modelled as

Greedy on blow value: it strikes whatever trades well and otherwise closes on
the nearest unit or the objective. `Weights::enemy_aggression` makes it swing
less often. Measured over 115 Deity games (2026-11-08/09): the host AI strikes
far less reliably than a greedy model, and among the units it could hit it picks
neither the weakest (27%) nor the one it damages most (24%) more often than
chance. The model is deliberately the more dangerous opponent.

## Evidence

Pending the engine benches; see the PR.

## Not done yet

- Sieges in progress stay with `siege_train`: the solver leaves its reserved
  units, active members and staging members alone.
- Ships, aircraft and embarked units are not modelled.
- Unseen enemies are not modelled; the forecast is conditional on what is in
  sight.
