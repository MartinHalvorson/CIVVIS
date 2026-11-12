# The assault forecast

Genes `assault-forecast-is-read` and `siege-goes-on-the-forecast` (both
opt-in, off and byte-identical when off). Code in
`src/ai/advanced/assault_forecast.rs`; the train's hooks are in
`assess_siege` (`siege_train.rs`), the board's in `board_rows`
(`objective_board.rs`).

## Why

On 2026-11-12, over 62 live Deity games, the siege planner made 873 decisions:
795 `stage`, 55 `invest`, 22 `reduce`, 1 `take`. 631 of the `stage`
decisions read "nothing to open the walls". Both siege bills (`siege_bill`
and the campaign bill behind `siege_requirement`) are linear strength sums.
The Stage -> Invest gate is a conjunction of them, patched by a dozen genes.
None of it is a probability of taking the city.

## What it computes

`AdvancedAi::assault_forecast(g, pid, city, force)` returns:

- P(capture within k turns) for k = 1..12;
- the force's expected losses before the capture or the horizon;
- what the force is short of: one more `taker`, `breaker` or `bodies`, or
  `nothing`.

The model is the battle solver's Game-free board (`battle_solver::model`):
exact dice, walls routing blows by `city_take_damage`'s rule, the city's
strike, its heal unless besieged, garrison, zones of control and capture.
Every strength is read once from the engine. The board is every tile within 4
of the city. Both sides play Press for 12 turns, 64 rollouts, on common dice.

A force member, or a hostile in sight within 8 of the city, that stands off
the board walks in. It enters on the edge tile its cheapest route reaches, at
the end of the turn that route takes, using `Game::battle_step_cost` and a
reverse search from the edge. It is not dropped while it can arrive inside
the horizon.

"Short of" reruns the same dice with one hypothetical unit added. The unit is
spawned on a clone of the board so the engine reads its strengths:

- our best melee taker;
- our best siege gun;
- one more of the force's most common unit.

Each enters when the force's median member does. The kind that raises
P(within 12) most, by at least 5 points, is named.

Forecasts are cached for the turn by the city's tile and the signature of the
force and the hostiles (kind, tile, health). A forecast with the short-of
reruns costs 10 ms on average, 50 ms at most, on a CI runner.

### Two switches on the model, off for the solver

- `Board::march_on_objective`: our side walks to the city it is to take when
  no enemy soldier is left on the board. Without it, a unit more than three
  tiles from an ungarrisoned city never moved. In round 1, the forecast read 0
  for ungarrisoned cities the AI then took.
- `Board::melee_breach_share`: our melee strike a walled city only once its
  walls are at or under that share of their pool. The forecast sets it to the
  train's `MELEE_WALL_FRACTION` (0.2).

`extract::snapshot` and the search's test boards set both to off. Off,
`Sim::act` takes exactly the path it took before, so the solver's own play
is unchanged.

### The forecast's enemy acts every turn

The solver's enemy acts on half its turns (`Weights::enemy_aggression` 0.5),
which is tuned for choosing moves two rounds out. Over 12 turns, an enemy
that always acts gave the better Brier score at all three horizons, so the
forecast ships with 1.0 (table below).

## The two genes

`assault-forecast-is-read` computes the forecast for every siege the train
assesses, while the journal records Decision lines, and writes one line, e.g.:

```text
Assault forecast for Kraków: 71% by turn +8, 1.4 units lost, short of: breaker | P% by turn +1..+12: ...; 5 members on the board, 2 walking in (6 within 3 turns), 3 hostiles; walls 100/100, city 200/200; 64 rollouts, ...
```

It changes no decision. The test `the_reading_changes_no_decision` plays a
posed siege with the gene on and off and compares every unit, the city and
the siege record each turn.

`siege-goes-on-the-forecast` implies the reading. It adds:

- (a) **Go.** Stage moves to Invest when P(capture within `GO_HORIZON` = 10)
  is at least `GO_P` = 0.55, whatever the bill, the breaker hold and the
  damage budget say. The muster closes on the same reading. It is an OR
  beside `open-city-is-the-objective`'s go. The shortfall that asks cities for
  soldiers still reads the stock readiness, so production is unchanged.
- (b) **Hold.** An invested train whose forecast is at or above `HOLD_P` = 0.4
  does not take the strength, damage-budget or breaker abort.
- (c) **Stand down.** `HOPELESS_STREAK` = 3 gathered assessments in a row
  stand the siege down for 10 turns. An assessment counts when three in four
  members are on the board within 3 turns, P(within 12) is under
  `HOPELESS_P` = 0.1, and no one more taker, breaker or body lifts it by 5
  points. While it is down, the train does not muster for the city and the
  Objective Board leaves its Siege row out. The stand-down is kept by the
  city's tile.
- (d) **Rank.** Siege rows on the Objective Board are worth value x P / mean
  turn of capture. The cities are sorted on that before the rows are built,
  because each Siege row depends on the one before it.

The no-one-unit-helps clause in (c) is not in the brief. It comes from the
paired evidence below: as first specified, the stand-down cost cities in both
posed sets. `GO_P`, `HOLD_P`, `HOPELESS_P` and `GO_HORIZON` are the brief's
starting values; the evidence gave no reason to move them.

## How the evidence was produced

All of it ran on this PR's CI (`cargo-test` job, 4 cores). The driver is
`src/ai/advanced/assault_forecast/calibration.rs`, ignored by default:

```text
cargo test --profile ci --lib -- --ignored assault_forecast_calibration --nocapture
```

Each seed poses one siege on open ground (hills and forest sprinkled near the
city):

- our capital at (2, 8) and player 1's city at (18, 8);
- walls 0/100/200/400, a quarter of them already damaged;
- city health 200, or 90-199 three times in ten;
- the city's owner one era behind, level or ahead of ours (its defence
  follows its best unit), with a garrison 65% of the time and 0-2 field
  defenders;
- our force of 2-9 of our era's melee, shooters and guns, between 2 and 10
  tiles out;
- turn 60 + 35 x era, at war.

Our seat is the live genome seated as `siege_replay` seats it: Domination,
aimed at player 1, the live-bridge universe, then
`apply_gene_ledger_with_forced_live` with the operator's arms of 2026-11-12
(`live_force_on_20261112.txt`, a copy of `~/.civvis-live-force-on`). Player 1
is the stock agent. The forecast is read before our first move. The real AI
then plays the world on the engine.

These are a fixed, repeatable problem set for this codebase's AI on both
sides. They do not forecast live play.

## Calibration: 1,200 posed sieges, 12 turns each

Forecast against whether the stock train took the city (no seed ended in
peace).

| P(capture within 12) | sieges | mean predicted | realized | stock train invested |
|---|---|---|---|---|
| 0.0-0.2 | 655 | 0.01 | 0.04 | 0.58 |
| 0.2-0.4 | 26 | 0.30 | 0.12 | 0.85 |
| 0.4-0.6 | 18 | 0.51 | 0.39 | 0.89 |
| 0.6-0.8 | 28 | 0.71 | 0.14 | 0.86 |
| 0.8-1.0 | 473 | 0.99 | 0.44 | 0.96 |

Brier score within 12: 0.252. The base rate (0.20) for every siege scores 0.162.

| P(capture within 10) | sieges | mean predicted | realized | stock train invested |
|---|---|---|---|---|
| 0.0-0.2 | 684 | 0.01 | 0.03 | 0.59 |
| 0.2-0.4 | 25 | 0.29 | 0.08 | 0.96 |
| 0.4-0.6 | 16 | 0.52 | 0.12 | 0.75 |
| 0.6-0.8 | 26 | 0.72 | 0.27 | 0.88 |
| 0.8-1.0 | 449 | 0.99 | 0.41 | 0.97 |

Brier score within 10: 0.245. The base rate (0.18) scores 0.149.

| P(capture within 5) | sieges | mean predicted | realized | stock train invested |
|---|---|---|---|---|
| 0.0-0.2 | 869 | 0.00 | 0.01 | 0.66 |
| 0.2-0.4 | 20 | 0.30 | 0.00 | 0.85 |
| 0.4-0.6 | 24 | 0.49 | 0.04 | 0.96 |
| 0.6-0.8 | 16 | 0.69 | 0.06 | 1.00 |
| 0.8-1.0 | 271 | 0.99 | 0.38 | 0.99 |

Brier score within 5: 0.153. The base rate (0.09) scores 0.083.

Units lost within 12 turns: predicted 1.98, realized 2.64 a siege. Realized
counts the posed force only.

| walls | sieges | mean P(12) | realized by 12 |
|---|---|---|---|
| 0 | 294 | 0.60 | 0.37 |
| 100 | 288 | 0.43 | 0.21 |
| 200 | 310 | 0.37 | 0.15 |
| 400 | 308 | 0.30 | 0.09 |

| stock train reached | sieges | mean P(12) | realized by 12 |
|---|---|---|---|
| stage only | 304 | 0.08 | 0.00 |
| invest or further | 896 | 0.54 | 0.27 |

Short of (sieges, of them taken within 12): bodies 30 (3), breaker 194 (11),
nothing 852 (208), taker 124 (22).

The same 1,200 sieges under each forecast setting:

| forecast settings | Brier within 12 | within 10 | within 5 | mean P(12) | sieges over 0.8, taken |
|---|---|---|---|---|---|
| melee wait for the breach, enemy always acts (shipped) | 0.252 | 0.245 | 0.153 | 0.42 | 473, 0.44 |
| melee wait for the breach, enemy acts three turns in four | 0.257 | 0.252 | 0.159 | 0.44 | 479, 0.43 |
| melee wait for the breach, enemy acts half its turns | 0.273 | 0.266 | 0.169 | 0.46 | 494, 0.43 |
| melee chip any wall, enemy always acts | 0.259 | 0.252 | 0.174 | 0.43 | 473, 0.44 |
| melee chip any wall, enemy acts half its turns | 0.276 | 0.268 | 0.188 | 0.46 | 484, 0.42 |

**What it says:**

- The forecast is nearly all-or-nothing: 1,128 of the 1,200 readings are
  under 0.2 or over 0.8.
- Under 0.2 it is right: 0.01 predicted, 0.04 realized.
- Over 0.8 it is overconfident: 0.99 predicted, 0.44 realized. The gap grows
  with the walls: realized 0.37 at no walls, 0.09 at 400, while the forecast
  holds near 1.
- The bottom bin alone does not carry the Brier score: every setting scores
  worse than the base rate at every horizon.
- The model under-counts losses: 1.98 predicted against 2.64.

Read it as "this force could take this city" rather than "this train will".
The model plays both sides on Press, every turn. The train stages, musters
and holds its taker, and the city's owner builds defenders the model does
not see.

Sieges the stock train never invested (304) read 0.08 and none was taken.
Where it invested (896) the forecast read 0.54 against 0.27 realized.

## Paired: `siege-goes-on-the-forecast` on and off

400 posed sieges, 30 turns each. Each seed is the calibration's siege plus a
second city of player 1's about eight tiles beyond it (walls 0/0/100/200, a
garrison half the time), so a siege stood down has somewhere else to go. A
war that ends in peace takes nothing. Peace ended the war in 209 of the 400
pairs in at least one arm.

Final code (commit 2587b46aac, the driver as committed):

| arm | cities taken | pairs ahead / behind off | invested | force units lost (vs off) | turns to first capture | go orders | held aborts | stand-downs | zero-rate row turns |
|---|---|---|---|---|---|---|---|---|---|
| off | 132 | | 302 | 3.00 | 11.5 (132) | 0 | 0 | 0 | 0 |
| on | 134 | +11 / -9 | 326 | 3.05 (+0.04 +/- 0.04) | 11.6 (134) | 209 | 194 | 177 | 5759 |

The 219 pairs no arm made peace in:

| arm | cities taken | pairs ahead / behind off | invested | force units lost (vs off) | turns to first capture | go orders | held aborts | stand-downs |
|---|---|---|---|---|---|---|---|---|
| off | 131 | | 186 | 1.96 | 11.5 (131) | 0 | 0 | 0 |
| on | 133 | +10 / -8 | 191 | 2.09 (+0.12 +/- 0.06) | 11.5 (133) | 114 | 111 | 59 |

- Two cities more out of 132: +11 / -9 pairs, within noise.
- Units lost: +0.04 +/- 0.04 a siege (+0.12 +/- 0.06 where nobody made peace).
- No faster captures (11.6 against 11.5 turns to the first).
- Per siege, the gene issued 0.5 go orders, held 0.5 aborts and stood down
  0.4 times.

On these boards the gene does what it says and moves the result by nothing
measurable.

Against the same paired sieges the forecast scores better than the base rate,
with or without the gene:

- off arm: Brier within 12 is 0.196 against 0.235;
- on arm: 0.201 against 0.232;
- over 0.8 the realized rate is 0.69 and 0.67.

These are the two-city worlds over 30 turns, read where no arm made peace.
Peace selects the wars that were going well, so this is not the clean
calibration; the 1,200-siege table above is.

### How the stand-down was tuned (WIP commits, CI variant switch)

These ran on WIP commits of this PR through a test-only switch that has since
been removed.

**Round 3** (commit 063b0ca69f): one enemy city, the forecast's enemy acting
half its turns, the stand-down as first specified.

| arm | cities taken | gained / lost vs off | force units lost (vs off) | stand-downs |
|---|---|---|---|---|
| off | 132 | | 2.87 | 0 |
| on | 119 | +8 / -21 | 3.03 (+0.16 +/- 0.05) | 427 |
| on, no stand-down | 135 | +9 / -6 | 2.87 (-0.01 +/- 0.03) | 0 |
| on, stand-down needs a stall | 123 | +7 / -16 | 3.00 (+0.13 +/- 0.05) | 359 |
| on, enemy always acts | 114 | +7 / -25 | 3.05 (+0.18 +/- 0.05) | 438 |
| on, enemy always acts, no stand-down | 131 | +5 / -6 | 2.90 (+0.03 +/- 0.03) | 0 |
| on, melee chips any wall | 120 | +8 / -20 | 3.02 (+0.15 +/- 0.05) | 402 |

**Round 4** (commit c3c5af864c): the two-city worlds above, the forecast
as shipped (enemy acts every turn).

| arm | cities taken | pairs ahead / behind off | force units lost (vs off) | stand-downs |
|---|---|---|---|---|
| off | 132 | | 3.00 | 0 |
| on, stand-down as first specified | 123 | +15 / -24 | 3.07 (+0.07 +/- 0.05) | 499 |
| on, no stand-down | 130 | +5 / -7 | 3.02 (+0.02 +/- 0.03) | 0 |
| on, stand-down only when no one unit helps (shipped) | 134 | +11 / -9 | 3.05 (+0.04 +/- 0.04) | 177 |
| on, stand-down under 2%, six readings running | 128 | +9 / -13 | 3.10 (+0.10 +/- 0.04) | 328 |
| on, both of the last two | 133 | +8 / -7 | 3.04 (+0.04 +/- 0.04) | 76 |
| on, no stand-down, no row ranking | 130 | +5 / -7 | 3.02 (+0.02 +/- 0.03) | 0 |
| on, forecast enemy acts half its turns | 127 | +13 / -18 | 3.06 (+0.06 +/- 0.05) | 481 |

**What the rounds say:**

- As first specified, the stand-down cost cities in both rounds (round 3:
  132 to 119; round 4: 132 to 123). In round 3, every city the off arm took
  and the on arm did not was taken after turn 12, outside what a 12-turn
  forecast can see.
- Restricted to sieges no one more unit would rescue, the loss goes away
  (134 against 132). That is a null, not a gain.
- Go and hold without any stand-down are within noise of off: +3 cities in
  round 3, -2 in round 4, units lost -0.01 and +0.02.
- The Siege-row ranking changed no outcome: "no stand-down" and "no
  stand-down, no row ranking" are identical row for row. These worlds have
  one or two rows and a single front.

## What this does not show

- **Live play.** Both seats are this codebase's AI, on posed boards. The
  reading gene exists to calibrate on live games. Its journal lines can be
  joined to captures from why.log the same way.
- **Whole worlds.** None were run here. The coordinating session runs paired
  `gene_screen` worlds from this branch.
- **The stand-down's purpose.** Retargeting the army to a better city needs
  several fronts. The two-city worlds are the smallest version of that, and
  the evidence there is a null.
- **What the forecast cannot see:** units the owner builds or brings from
  out of sight, ships and aircraft, Battering Rams and Siege Towers, and
  promotions earned in the fight.
