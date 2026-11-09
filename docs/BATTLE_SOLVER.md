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
the nearest unit or the objective, and on any round it may simply stand
(`Weights::enemy_aggression`, 0.5). Measured over 115 Deity games (2026-11-08/09): the host AI strikes
far less reliably than a greedy model, and among the units it could hit it picks
neither the weakest (27%) nor the one it damages most (24%) more often than
chance.

## Evidence

Measured on `battle_bench` and `doctrine_arena`, seat A
`advanced+battle-planner-2+slots-survive-the-roll+safest-stand+shared-danger+ranged-before-melee+ranged-finish-on-the-mean+ranged-hp-reserve+anvil-spares-the-one-blow`
plus `battle-solver`, seat B the same genome without it: the tactical genes the
live seat runs. Control (`A` v `A`) reads exactly 0.00 with no divergence. Each
row is 300 seeds on a block fresh to the tuning (9,700,000), both seat orders:

| army | exchange ratio (solver / live stack) | paired swing | seeds won/lost |
|---|---|---|---|
| ranged heavy | 2.60 / 0.39 | +356.6 ± 17.3 | 261 / 33 |
| combined arms | 1.76 / 0.57 | +306.6 ± 21.5 | 237 / 60 |
| with siege | 1.66 / 0.60 | +302.1 ± 25.7 | 227 / 73 |
| medieval mounted | 1.39 / 0.72 | +809.8 ± 47.9 | 254 / 44 |
| cavalry | 1.30 / 0.77 | +133.2 ± 24.4 | 185 / 112 |
| melee only | 1.14 / 0.88 | +114.3 ± 9.2 | 228 / 61 |

`doctrine_arena`, 13 posed positions, 24 seeds, both roles: positive on all
thirteen (+280.8 ± 20.1 pooled, 246/59), 1.47 kills per loss against 0.68, and
cities at parity (+11/−12). Scoring only material took units and walked past
cities (+1/−6); scoring progress toward the objective city and closing on it
fixed that.

Six recorded live sieges replayed on `siege_replay` (4 seeds × 40 turns each)
read neutral: captures 6/24 live, 5/24 with the solver leaving sieges to the
train. Running the sieges through the solver instead did not help (6/24, more
units lost), so sieges stay with the train.

**Read these as a mirror measurement.** Both seats are this codebase's AI; the
live opponent is the host's AI, which attacks far less reliably than this
stack. A gain here is a necessary condition for a gain live, not a measurement
of it, and `docs/TACTICS.md` §6–§7 records that a large gain on this bench has
not moved whole-game win rate before.

What the tuning found: the assumed enemy matters most. Modelling a greedy,
always-swinging enemy read **−100** on combined arms and **−88** on melee;
assuming it acts half the time read +262 and held melee. A longer horizon made
things worse (two rounds +216, six +104): model error compounds, so depth is
only worth buying with a better enemy model. Raising the budget from 2,400 to
6,000 rollouts changed nothing, and valuing a unit for staying alive
(`alive_floor` 0.6) was the single largest lift.

## Not done yet

- Sieges in progress stay with `siege_train`: the solver leaves its reserved
  units, active members and staging members alone.
- Ships, aircraft and embarked units are not modelled.
- Unseen enemies are not modelled; the forecast is conditional on what is in
  sight.
