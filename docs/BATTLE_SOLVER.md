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

Gene `solver-plays-the-host` (opt-in) replaces that guess with the host's own
habits, fitted to 30,869 enemy unit-turns from 116 live Deity games and scored
on held-out games (`docs/HOST_ENEMY_MODEL.md`, `battle_solver/host_enemy.rs`).
The host struck one of ours on a quarter of the turns it had one in reach (43%
when it need not move, 11% when it must), took the unit in front of it rather
than the best trade, and did not retreat when wounded. The fitted target is
the one struck on 76% of held-out strikes, against 37% for the greedy pick.
**On this page's mirror bench the gene reads negative at both horizons:**
against this AI (not the host) the solver with the fitted enemy does worse
than with the stock one, pooled battle_bench −84 and doctrine_arena −137 at
horizon 2, −90 and −81 at horizon 4. The stock enemy was tuned against that
opponent. **Only live games against the host can judge the gene**; none has
been played with it, and it is not armed.

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

### Whole games: no measurable change

Paired `gene_screen` games on 2026-11-09: Deity Online, firaxis-faithful rivals,
the live genome (619 treatments) with and without `battle-solver`, one seat of
each lane in the rotation. Delta is solver minus control, with z in brackets.

| seat | pairs | cities lost | cities taken | score |
|---|---|---|---|---|
| Gran Colombia domination, first batch | 10 | −2.90 (−3.1) | −0.30 (−0.6) | +73.6 (+2.1) |
| Gran Colombia domination, second batch | 16 | +0.94 (+1.1) | +0.44 (+2.0) | −21.8 (−0.8) |
| Greece culture | 12 | −0.17 (−0.1) | +0.17 (+1.5) | +25.5 (+0.6) |
| Khmer religious | 12 | +1.42 (+1.6) | 0.00 | −96.1 (−2.4) |
| **pooled** | **50** | **+0.02 (0.0)** | **+0.12 (+0.9)** | **−9.2 (−0.5)** |

No seat won a game in either arm. The first batch looked like a clear gain and
the next 40 pairs did not repeat it, so it should not be quoted alone. The one
pooled line past z 1.5 is cities taken by turn 100 (+0.14, z 2.0, 5 up and
0 down), one of eleven lines read, so it is a lead and not a result. Khmer's
score loss is the line to recheck before arming on that seat.

`docs/TACTICS.md` records the same shape for one-turn joint tactics: a large
bench gain and an inconclusive whole-game read. The seat records final counts
only, so these runs cannot say whether the in-game kill ratio moved.

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

## What a 10× would take

The solver computes the fight it is shown. Four things limit how much that is
worth, in the order they bind:

1. **Being in the fight with comparable units.** Of 765 live Lv 7 military
   losses (2026-11-04 census), 580 died to one hit, and the healthy units among
   them were mostly archers and spearmen (15–25 strength) beside Line Infantry
   and Machine Guns (55–70). Damage is exponential in the strength gap, so no
   ordering of attacks recovers a 30-strength deficit.
2. **Knowing what the enemy will do.** The tuning's largest swing came from the
   assumed enemy (−100 to +262), and depth made things worse because the model
   is wrong. A model fitted to the host's recorded moves is what makes a horizon
   beyond two rounds pay. The strike census already holds the data.
3. **Owning the sieges.** Cities decide the domination lane and the solver hands
   them to `siege_train`. Solver-run sieges did not help on the replays, so this
   needs siege-specific scoring first.
4. **The army it is given.** Ranged-heavy armies gained the most (2.60 against
   0.39), melee-only the least (1.14 against 0.88), so production decides how
   much the tactics can return.

A game-level kill and loss count on the seat record was the instrument that was
missing: without it a whole-game null cannot be split into "the tactics did not
change the exchange" and "the exchange did not change the game". It exists
since 2026-11-12. The engine counts each unit where it removes it in combat
(`Game::combat_ledger`, `src/game/combat_ledger.rs`): units killed, units
lost and who struck them (a major, a city-state or the barbarians), and
civilians taken and lost. `gene_screen` writes the seat's ledger on every row
(`units_killed`, `units_lost`, `units_lost_to_majors`,
`units_lost_to_city_states`, `units_lost_to_barbarians`, `civilians_lost`) and
at every `--trace` mark, and `--analyze` prints their on−off Δ beside
`cities_taken` and `cities_lost`. The row's `last_orders` map says which step
last ordered each of the seat's military units that died — the battle
planner, the solver, the rotation, the coordinated step and some forty others
— and whether it ended its last turn within two tiles of a visible hostile
(`src/ai/advanced/last_orders.rs`). Nothing reads either to decide anything.
