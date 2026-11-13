# The assault forecast

**Verdict** (1,200 posed sieges with both seats kept solvent; #5207, CI
runs 43303003805 and 43315093796).

- **Low readings are right.** 655 sieges read under 0.2, predicted 0.01,
  and 0.09 were taken.
- **High readings run 0.18 hot.**
  - 473 sieges read 0.8 or more, predicted 0.99, and 0.81 were taken.
  - By wall pool, the high bin was taken 0.85 with no walls, 0.83 at 100,
    0.79 at 200 and 0.70 at 400.
- **It beats the base rate.** Brier within 12 turns is 0.136, against
  0.238 for giving every siege the base rate.
- **The gap is the train's, not the model's.** Of the 92 high readings not
  taken:
  - 65 are execution: the train did not do what the forecast's press does;
  - 3 are the model: the owner raised something the forecast does not see;
  - 24 are undetermined.

  See [Why high readings fail](#why-high-readings-fail-5207).
- **The go gene does not close it.** On the high readings it gained
  +0.006 +/- 0.008 captures (paired, same sieges).

**Superseded: #5195's first numbers.** #5195 first reported 0.44 taken for
the high bin and a Brier of 0.252, worse than the base rate's 0.162. Those
seats were not funded. When a seat's gold runs out, the engine disbands
one unit per 10 Gold of deficit each turn (`settle_gold_budget`), and a
pop-1 capital cannot pay for up to nine soldiers. Force members lost
outside combat came to 2.11 a siege; with solvent seats, 0.00. Do not quote
0.44.

## For callers

These entry points are all `pub(crate)` on `AdvancedAi`, in
`src/ai/advanced/assault_forecast.rs`. They read with both genes off, are
cached for the turn, and change no decision.

- `assault_forecast(g, pid, city, force) -> Option<AssaultForecast>`: at war
  only. `None` means the city is ours, gone, or not at war with us.
- `assault_forecast_at_peace(g, pid, city, force) -> Option<AssaultForecast>`:
  the same reading as if the war with the city's owner (a major or a
  city-state) began this turn. The owner's units in sight are hostiles and
  its borders open. `None` only means the city is ours or gone. At war it
  equals `assault_forecast`.
- `assault_forecast_recruiting(g, pid, city, force, recruits) -> Option<AssaultForecast>`:
  the at-peace reading with `Recruit { kind, count, turns }` added. Each
  recruit arrives at the board's edge nearest our cities `turns` from now.
  A Battering Ram or Siege Tower among them stands beside the city when its
  walk in from that edge ends.
- `assault_corps_for(g, pid, city, pool, want_p, horizon) -> Option<AssaultCorps>`:
  the smallest greedy subset of `pool` whose reading reaches `want_p` within
  `horizon` turns (1-12), at war or as if at war. `pool` can hold soldiers
  and support units, up to 20 soldiers. `None` means the whole pool falls
  short, or no city reads.
- `assault_corps_best(g, pid, city, pool, want_p, horizon) -> Option<AssaultCorps>`:
  from the same cached search, the corps that reaches, or else the best the
  pool reached, for the caller to log. `None` only when no city reads.
- `AssaultForecast::calibrated_within(turns, walls) -> f32`, **provisional**:
  a conservative chance for turning a reading into an expected value.
  - Readings under 0.2 pass through; low readings were right.
  - Above 0.2 the reading is pulled down linearly to the high bin's realized
    rate for the city's wall pool (table below), and is never raised.
  - #5207 found the gap is the train's execution, not the model. The
    discount stays until the train realizes what the forecast reads.

A Battering Ram or Siege Tower in the force or among the recruits works as
the engine works it (`Game::siege_support_effects`). From the turn after it
stands beside the city, our melee and anti-cavalry hits do full damage to
the walls (ram) or pass them (tower). That holds only where
`Game::city_allows_siege_support` lets it: a ram against Ancient Walls, a
tower against Ancient or Medieval Walls.

**Cost.**

- A reading with the short-of reruns takes 10-11 ms on average, 50-100 ms
  at most, on the 36x22 posed map on a CI runner.
- Reading at peace, or with recruits, adds one copy of the board.
- The corps search makes up to about n²/2 quick readings of 24 rollouts each
  (n is at most 22), plus a full reading near the target. Expect up to a
  few hundred ms per city, once a turn for each (city, pool, chance,
  horizon).

**Warning: a raw high reading is not a probability.**

- **The numbers.** Readings of 0.8 or more were taken 0.81 of the time
  against 0.99 predicted, on 1,200 posed sieges with solvent seats (#5207).
  The forecast's Brier within 12 turns is 0.136, against 0.238 for the base
  rate.
- **Where the gap comes from.** Of the 92 high readings not taken, 65 are
  the train's execution, 3 the model's, and 24 undetermined.
- **What to use.** Use `calibrated_within` for an expected value.
- **The superseded figure.** The 0.44 #5195 first reported came from
  unfunded seats.

### The high bin by wall pool (what `calibrated_within` reads)

From the 1,200 mixed posed sieges with solvent seats: #5207, CI run
43303003805 on c908fea4c6.

Before each seat's turn its treasury is topped up to seven turns of its
deficit plus 20 Gold. Neither the engine's bankruptcy (one unit disbanded
per 10 Gold short) nor the agent's own `bankruptcy-exits-the-drain-2`
disbands anything: force members lost outside combat fell from 2.11 a siege
to 0.00. A test holds the constants to these lines.

| walls | read >= 0.8 | taken within 12 | rate |
|---|---|---|---|
| 0 | 165 | 141 | 0.85 |
| 100 | 113 | 94 | 0.83 |
| 200 | 109 | 86 | 0.79 |
| 400 | 86 | 60 | 0.70 |

The same sieges with unfunded seats, as #5195 ran them: 99 / 165 (0.60),
47 / 113 (0.42), 39 / 109 (0.36), 22 / 86 (0.26).

## Why high readings fail (#5207)

**Answer.**

- The model is not where the gap is. The city's owner raised something the
  forecast does not see before the press would have taken the city in 3 of
  the 92 misses. All three were Warriors the city finished on turn 11. None
  was bought.
- The train is. In 65 of the 92 it did not do what the press does: it
  struck late, stayed in Stage, arrived piecemeal, left the ring open, or
  kept its shooters out of range of standing walls.
- The other 24 are undetermined: 6 where our units died while the walls
  stood, and 18 that fit no class.
- The high bin stays at 0.81 realized against 0.99 predicted. That gap of
  0.18 misses the 0.15 target. It is the train's to close, and
  `calibrated_within` keeps discounting until it does.

**Data.** The 1,200 mixed posed sieges with solvent seats: CI run
43315093796 on 868cdae043. The forecast is the merged one, unchanged by
#5207. 473 read 0.8 or more within 12 turns, 381 were taken, and 92 were
not.

Each miss gets the first cause that fits, in the order of the table.

- A raise (a new soldier, or walls rising) counts only if it came by the
  press's capture turn: the first turn the forecast takes the city in half
  its rollouts.
- The last column counts the sieges where the train fell back from Invest or
  later to Stage at some point.

| first cause | all | walls 0 | 100 | 200 | 400 | fell back to Stage |
|---|---|---|---|---|---|---|
| peace: the war ended | 0 | 0 | 0 | 0 | 0 | 0 |
| the train never left Stage, or left it on turn 4 or later | 8 | 0 | 1 | 4 | 3 | 3 |
| the first blow came 3+ turns after the press made its first | 19 | 6 | 6 | 2 | 5 | 14 |
| the force arrived piecemeal | 8 | 4 | 2 | 2 | 0 | 6 |
| the city fell low and no taker took it | 6 | 4 | 0 | 1 | 1 | 0 |
| our units died before the walls opened | 6 | 0 | 3 | 1 | 2 | 6 |
| the owner raised soldiers or walls before the press's capture turn | 3 | 0 | 0 | 0 | 3 | 2 |
| the city healed as fast as we hurt it | 11 | 8 | 1 | 1 | 1 | 8 |
| the walls never opened with shooters in range | 13 | 0 | 1 | 5 | 7 | 9 |
| time ran out with the city under 25% | 0 | 0 | 0 | 0 | 0 | 0 |
| other | 18 | 2 | 5 | 7 | 4 | 4 |
| taken / read high | 381 / 473 | 141 / 165 | 94 / 113 | 86 / 109 | 60 / 86 | |

**Model vs execution.**

- Model: 3 of 92 (3%), the owner's raise.
- Execution: 65 of 92 (71%). This counts Stage, the late first blow,
  piecemeal arrival, the missing taker, walls held with shooters out of
  range, and healing. Healing counts as execution because the model heals
  the city by the same rule (20 a turn unless the ring is sealed). The
  difference is the real train's ring and its rate of damage.
- Undetermined: 24 of 92 (26%), units dead before the breach and other.
  The model over-counts losses overall (1.98 a siege predicted, 0.61
  realized). Deaths before the breach are therefore more likely the train
  exposing units than the model missing a threat. They are not counted
  either way.

The train fell back to Stage in 52 of the 92 misses.

City health plus walls at the end of each turn, mean. Model is the
forecast's press; real is the train.

| sieges | n | | t1 | t2 | t3 | t4 | t6 | t8 | t12 |
|---|---|---|---|---|---|---|---|---|---|
| read high, taken | 381 | model | 271 | 216 | 156 | 99 | 30 | 6 | 0 |
| | | real | 276 | 227 | 173 | 123 | 61 | 23 | 0 |
| read high, not taken | 92 | model | 337 | 313 | 271 | 224 | 132 | 64 | 3 |
| | | real | 342 | 336 | 330 | 317 | 293 | 279 | 236 |

In the misses the real train is behind from turn 1. By turn 3 it has taken
12 off the city against the press's 66.

**What did not close it.**

- **The owner's defenders, put into the forecast (WIP rounds 7-9, then
  taken out).**
  - The model: the owner buys the strongest land soldier its treasury
    covers on its first turn, then builds its best melee unit each time the
    city's production finishes one.
  - No siege of the 1,200 planned one. 100 Gold bought nothing at the
    engine's prices, and no city finishes its best melee unit within 12
    turns.
  - What the owner did raise: a soldier in 478 of the 1,200 sieges, none
    paid for in Gold. In the 27 misses whose cause round 7 gave as the owner's soldiers, every one
    was a Warrior finished on turn 11. Round 7 had read any raise as the
    cause, at any turn. Counting only raises by the press's capture turn
    leaves 3.
- **`siege-goes-on-the-forecast`, the existing execution gene.** It played
  the same 1,200 solvent sieges for 12 turns, paired seed by seed:

| sieges | n | taken off | taken on | on - off (+/- se) | fell back to Stage off / on | force lost off / on |
|---|---|---|---|---|---|---|
| all | 1200 | 0.39 | 0.38 | -0.006 +/- 0.004 | 381 / 380 | 0.61 / 0.61 |
| read >= 0.8 | 473 | 0.81 | 0.81 | +0.006 +/- 0.008 | 53 / 46 | 0.26 / 0.27 |
| read >= 0.8, walls 0 | 165 | 0.85 | 0.85 | +0.000 +/- 0.009 | 15 / 12 | 0.25 / 0.24 |
| read >= 0.8, walls 100 | 113 | 0.83 | 0.84 | +0.009 +/- 0.020 | 11 / 12 | 0.35 / 0.35 |
| read >= 0.8, walls 200 | 109 | 0.79 | 0.79 | +0.000 +/- 0.018 | 13 / 11 | 0.22 / 0.25 |
| read >= 0.8, walls 400 | 86 | 0.70 | 0.72 | +0.023 +/- 0.023 | 14 / 11 | 0.22 / 0.27 |
| read < 0.8 | 727 | 0.12 | 0.10 | -0.014 +/- 0.005 | 328 / 334 | 0.84 / 0.83 |

With the gene: 292 go orders, 239 holds (an abort vetoed) and 236
stand-downs.

- On the high readings it does nothing measurable. It trims the fall-backs
  (53 to 46) but not the misses.
- On the low readings it costs 0.014 (+/- 0.005) of captures. Its
  stand-downs act on those readings; they were not measured separately.

**The tempo family.** 400 sieges with the city open or behind 100 walls,
the force 6-12 tiles out, and the owner raising walls to 200 on turn 8-12.

- The press's first dent and the train's come on the same turn on average:
  5.4 against 5.2.
- The high bin realized 0.73 against 0.99 predicted.
- Of its 52 misses, the largest classes are healing (17, 13 of them fell
  back to Stage), Stage (15) and piecemeal arrival (8). The late first blow
  accounts for 4.
- Tempo in the sense of a slow march to the walls is not the gap; what the
  train does once there is.

**Objective switches and guard holds on the walk** (CI run 43320853600 on
b90df60568, the same sieges). The walk is counted up to the first blow, or
all 12 turns when none came.

- *Target switches*: turns on which the campaign's `target_city` changed.
- *Held*: force members more than two tiles out that ended our turn no
  nearer, in unit-turns. This includes members holding a staging post by
  design.
- *Hold lines*: the train's own journal lines, grouped by the guard that
  wrote them.

The mix has one city of the owner's, so its campaign has nothing to switch
to. A two-city family (400 of the mix's seeds with a second city of the
owner's about eight tiles beyond the first) gives it one. That family's
high bin realized 0.79 against 0.99 predicted: 111 of 141 taken.

| sieges | n | target switches (sieges with any) | held | hold lines | arrival spread | first blow turn (never) | muster close to first blow (n) |
|---|---|---|---|---|---|---|---|
| mix, read high, taken | 381 | 0.01 (2) | 3.3 | mustered train 14, short of its post 737 | 2.4 | 2.2 (0) | 1.0 (10) |
| mix, read high, not taken | 92 | 0.00 (0) | 10.3 | mustered train 21, short of its post 149 | 3.5 | 3.9 (7) | 1.1 (13) |
| of them, never left Stage | 8 | 0.00 (0) | 25.8 | mustered train 5, short of its post 6 | 3.1 | 4.7 (2) | 3.0 (2) |
| of them, the late first blow | 19 | 0.00 (0) | 19.1 | mustered train 13, short of its post 41 | 4.0 | 7.2 (5) | 3.0 (2) |
| two cities, read high, taken | 111 | 0.02 (2) | 3.8 | mustered train 14, short of its post 197 | 2.6 | 2.3 (0) | 0.9 (7) |
| two cities, read high, not taken | 30 | 0.27 (6) | 19.6 | fog hostile 2, mustered train 20, short of its post 74 | 3.4 | 4.1 (7) | 1.8 (6) |
| of them, never left Stage | 8 | 0.88 (5) | 46.9 | fog hostile 2, mustered train 3, short of its post 37 | 3.2 | 6.5 (6) | - (0) |
| of them, the late first blow | 6 | 0.17 (1) | 19.0 | mustered train 15, short of its post 9 | 3.3 | 6.6 (1) | 9.0 (1) |

- **Switching** explains little.
  - It never happens with one city.
  - With two, 6 of the 30 misses switched against 2 of the 111 taken. Five
    of the six are among the 8 misses that never left Stage, where the
    campaign spent 3.2 turns a siege on another target or none.
- **The guards** that hold a Settler (the fog hostile, closed borders,
  water and dry road holds on the train) wrote 2 lines across all the
  misses.
  - "Stops short of its post" is as common in the taken sieges as in the
    missed: 1.9 against 1.6 a siege in the mix.
  - The over-represented line is the muster's ("the mustered train holds"):
    0.23 a miss against 0.04 a taken siege in the mix, and 0.67 against
    0.13 with two cities.
- **What separates misses from takes** is members not walking. In the mix,
  10.3 held unit-turns a miss against 3.3 a taken siege. With two cities,
  19.6 against 3.8. The holds sit in the Stage and late-blow classes. Most
  of these unit-turns have no hold line: about 10 held unit-turns a miss
  against about 2 lines. The Stage march's gun-danger checks, for one,
  write none.
- The tempo family agrees: no switches, 13.4 held unit-turns a miss against
  4.6 a taken siege, and hold lines of muster 8 and post 126 against 2 and
  717.

The posed walks are short: the force stands 2-12 tiles out on open ground.
A live walk of 6-11 turns gives switching and guards more turns to act
than these do.

**Where a train fix would start.** Execution misses have no single
dominant class: the late first blow is 19, walls held with shooters out of
range 13, healing with the ring open 11. What cuts across them is the
fall-back to Stage (52 of 92) and the damage rate. By turn 4 the train
has done a fifth of the press's damage (25 against 113), and by turn 12 a
third (106 against 334). The worked examples show three patterns:

- Shooters and guns not in range of the city on most turns (shooters 0/0
  fired).
- Melee beside a walled city not striking while the walls stand.
- A knight spending its attacks on field units while the city heals.

On these posed sieges, keeping the objective while its forecast holds would
reach at most the two-city misses that switched (6 of 30, 5 of them in
Stage). The guard line that separates misses from takes is the muster's
hold, not a safe-step guard.

**Two candidate fixes (not built).** No execution class dominates, so
#5207 adds no gene. Both of these are follow-ups.

1. **The muster hold yields to a train the forecast reads high.**
   - Motivation: "the mustered train holds" is the one hold line the misses
     over-represent: 0.23 a miss against 0.04 a taken siege (0.67 against
     0.13 with two cities).
   - Meanwhile the train has done 25 of damage by turn 4, against the
     forecast's 113.
2. **The gun-danger and post checks write a journal line when they hold a
   member.**
   - Motivation: members more than two tiles out that end our turn no
     nearer come to 10.3 unit-turns a miss against 3.3 a taken siege.
   - Most of those write no line, so no guard can be named. The Stage
     march's gun-danger checks are one of the checks that write none.
   - Once the lines exist, the guard to make yield can be measured instead
     of guessed.

The driver prints two worked examples per cause, turn by turn: `cargo test
--profile ci --lib -- --ignored assault_forecast_calibration --nocapture`.

## The genes

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

**Solvent seats (#5207, CI run 43315093796): the current read.** Forecast
against whether the stock train took the city (no seed ended in peace).

| P(capture within 12) | sieges | mean predicted | realized | stock train invested |
|---|---|---|---|---|
| 0.0-0.2 | 655 | 0.01 | 0.09 | 0.72 |
| 0.2-0.4 | 26 | 0.30 | 0.38 | 1.00 |
| 0.4-0.6 | 18 | 0.51 | 0.67 | 1.00 |
| 0.6-0.8 | 28 | 0.71 | 0.29 | 0.96 |
| 0.8-1.0 | 473 | 0.99 | 0.81 | 1.00 |

Brier score within 12: 0.136. The base rate (0.39) for every siege scores
0.238.

**Superseded: unfunded seats, as #5195 first ran them.** Everything below
in this section is that run. Its misses are mostly units the bankrupt seat
disbanded (see the verdict).

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
- **Whole worlds.** None were run for the sections above; the last section
  is the whole-world read (PR #5211).
- **The stand-down's purpose.** Retargeting the army to a better city needs
  several fronts. The two-city worlds are the smallest version of that, and
  the evidence there is a null.
- **What the forecast cannot see:** units the owner builds or brings from
  out of sight, ships and aircraft, a Battering Ram or Siege Tower killed on
  its way, and promotions earned in the fight. Rams and towers themselves
  are modelled since #5212 (above, "For callers"); the calibration tables
  predate that.

## Whole worlds: hopeless sieges and the stand-down (PR #5211, 2026-11-12)

This experiment answers two questions, on whole worlds rather than posed sieges. Do hopeless sieges kill their units? Does standing them down save those units without costing cities?

The driver is `assault_forecast/world_tests.rs` (ignored by default). It runs #5196's two world shapes:
- 4 players on a 44x28 map, Online speed, 200 turns, 2 city-states.
- Seat 0 plays Domination without the handicap. It carries `deploy/live-force-on.txt` plus the live tactical genes.
- 24 paired seeds per shape.

It plays four arms through a test-only switch (`assault_forecast::variant`):
- **A**: the forecast is read only (`assault-forecast-is-read`). It changes no decision.
- **B**: `siege-goes-on-the-forecast` as merged.
- **C**: B, standing down after one hopeless reading, without the "no one more unit lifts it" clause.
- **D**: B's stand-down alone.

In the tables below, rows B to D are paired against A: mean and standard error over the seeds.

**Immortal with King yields** (rivals: Domination, Domination, Science)

| arm | units lost | cities lost | cities taken | stand-downs | `siege_doctrine` deaths by the siege's reading (<0.1 / 0.1-0.4 / >0.4 / none) |
| --- | --- | --- | --- | --- | --- |
| A | 25.75 a world | 1.29 | 0.96 | 0 | 138 / 0 / 30 / 101 |
| B | +1.71 ± 1.29 | +0.00 ± 0.15 | -0.17 ± 0.17 | 48 | 108 / 2 / 32 / 117 |
| C | +2.58 ± 2.28 | +0.46 ± 0.26 | +0.00 ± 0.21 | 61 | 103 / 2 / 11 / 161 |
| D | +0.46 ± 1.10 | -0.04 ± 0.13 | -0.21 ± 0.13 | 50 | 106 / 1 / 28 / 110 |

**Emperor** (rivals: Domination, Science, Culture)

| arm | units lost | cities lost | cities taken | stand-downs |
| --- | --- | --- | --- | --- |
| A | 15.79 a world | 0.75 | 0.46 | 0 |
| B | +0.96 ± 1.40 | +0.08 ± 0.06 | +0.00 ± 0.00 | 20 |
| C | +0.29 ± 1.36 | +0.00 ± 0.06 | +0.08 ± 0.06 | 36 |
| D | +0.21 ± 1.04 | +0.08 ± 0.06 | +0.00 ± 0.00 | 21 |

**Calibration on whole worlds.** Arm A, every assessment's P(capture within 10). A city counts as taken if it became ours within the next 10 turns.

| P within 10 | Immortal: taken / read | Emperor: taken / read |
| --- | --- | --- |
| 0.0-0.1 | 1 / 1238 | 0 / 1061 |
| 0.1-0.4 | 0 / 8 | 0 / 3 |
| 0.4-0.6 | 2 / 10 | 0 / 4 |
| 0.6-0.9 | 1 / 15 | 0 / 3 |
| 0.9-1.0 | 59 / 146 | 7 / 40 |

**Conclusion.**
- Hopeless sieges kill their units. On the Immortal shape, 138 of the 168 deaths last ordered by `siege_doctrine` that had a reading were in sieges read under 0.1. The train stands in Stage on 95% (Immortal) to 99% (Emperor) of its siege-turns.
- Standing those sieges down cuts that bin's deaths by about 30 over 24 worlds (B and D). It does not cut units lost: the same units then die under rotation, healing and the battle planner. The aggressive variant (C) costs cities.
- No change to this gene's constants follows.
- On whole worlds the forecast is reliable when low and overconfident when high, as on the posed sieges.

## Declarations: `declaration-needs-a-capture` (PR #5236, 2026-11-13)

**The question.** Our seat declares wars on majors that take nothing. Which
openers declare them, what did the forecast say, and does a forecast gate
on the declaration stop the ones that cannot pay?

**The census.** 108 live Deity games of 2026-11-12/13 (70 Gran Colombia
Domination, 4 players Pangaea; 38 Rome Score, 6 players Continents, turn
limit 170). A war is ours when a `war` event of `source: civvis` names a
major at peace on the state before and at war within two turns; segments of
one game (`-contN`) are chained, a later segment replacing the turns it
replays. The opener is the seat's own `why.log` line on the declaration
turn. Outcomes within 30 turns: cities of the target that became ours
(by tile), our cities lost (`city_occupation`) and to whom, and units lost to
and killed of the target (`combat` events).

| lane | opener | wars | took a city | cities lost to the target | to others | units lost / killed (target) |
|---|---|---|---|---|---|---|
| Domination | faith war (`faith-war-before-the-flip` family) | 99 | 5 | 3 | 19 | 211 / 132 |
| Domination | science denial | 32 | 0 | 13 | 6 | 128 / 56 |
| Domination | faith strike | 20 | 1 | 0 | 13 | 109 / 68 |
| Domination | main path (staged 5, culture counter 5, religion counter 1) | 11 | 0 | 0 | 4 | 46 / 8 |
| Domination | religious interception | 4 | 1 | 0 | 0 | 27 / 37 |
| Domination | no opener line | 2 | 2 | 0 | 0 | 20 / 23 |
| Score | faith strike | 20 | 0 | 3 | 6 | 43 / 19 |
| Score | science denial 2, open-city front 2, launcher war 1 | 5 | 0 | 0 | 0 | 15 / 8 |
| Score | no opener line | 2 | 1 | 1 | 0 | 13 / 35 |

- **The wars that take nothing are faith wars.** 143 of 195; their point
  is the spreaders. They hold the conversion: our cities on the target's
  faith fell 0.66 (Domination, 106 wars) and 1.13 (Score, 15) in the 15
  turns after one, and rose 1.46 (123) and 1.42 (78) after a faith war the
  seat wanted and held ("Holding the faith war / heretic strike", no war on
  that rival within 10 turns). They lost 6 cities to their target. Exempt.
- **The cities lost to the declared target come from the science denial
  wars**: 13 of the 20. None took a city, and the racer won the Science
  victory in 17 of the 31 decided games anyway.
- **Most cities lost within 30 turns of our declaration go to someone else**
  (49 of 69): a rival already at war with us, or one that declared later.
- **What the forecast said.** The reader (`assault-forecast-is-read`) is in
  the Score games only. The three gated wars it saw read 0% (science
  denial), 6% (the open-city front; 98% two turns later, and it took
  nothing) and 0% (launcher war). 14 of the 15 wars it saw read 0% on the
  first reading.
- **What a gate on the 48 non-faith declarations would have cost:** no
  capture (none took a city); it would have spared 13 cities and 189 units
  lost to the targets, against 72 of theirs killed.
