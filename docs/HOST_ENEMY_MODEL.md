# The host enemy model

How the real Civilization VI AI fights, measured from recorded live games, for
the battle solver's modelled enemy. Fitted by `tools/fit_host_enemy.py`; the
tables live in `src/ai/advanced/battle_solver/host_enemy.rs`, whose test reads
this page back, so a refit has to change both. Gene `solver-plays-the-host`
(opt-in, acts only with `battle-solver` on) makes the solver's enemy play by
them.

## Why

`docs/BATTLE_SOLVER.md` names the enemy model as the solver's binding limit.
Assuming a greedy enemy that always swings read −100 on the bench; assuming
it acts half the time read +262; a horizon past two rounds read worse because
the model was wrong. Its stock enemy is still a guess: greedy on blow value,
standing still on half its turns, retreating at 25 hp, otherwise closing on
the nearest unit. Nobody had fitted it to the host.

## Data

Every non-continuation live run `~/civvis-civ6-runs/control/civvis-2026111*T*`
on 2026-11-12 at 16:30Z: 119 runs, `civvis-20261111T160039Z` to
`civvis-20261112T161344Z`, all Deity; 116 had an enemy in sight. The
directory keeps a rolling window, so a later extraction reads a different
set. Read from each run's `events.jsonl`.

- **A row** is one *enemy unit-turn*: a visible land fighter of a player at
  war with us (`rivals[]` and `minors[]` with `at_war`, plus `hostiles[]`,
  which holds the barbarians and Free Cities), standing at the end of our
  turn. Ships, aircraft, support, religious, civilian and embarked units are
  not rows. 30,869 rows; 17,669 had a unit of ours in reach.
- **The end of our turn** is the turn's last `state` frame, updated by the
  `host_move` and our own `combat` rows after it (our moves, wounds and
  kills).
- **The enemy's turn** is every `combat` row with `our_turn: false` labelled
  with that turn, before the next turn's first frame. Rows from a district
  (city strikes) and the mod's district-vs-district id −1 junk rows are not
  unit strikes. A unit is matched by `(player, id)`: host unit ids are unique
  per player only.
- **Struck** means its first strike that enemy turn hit a unit of ours (4,415
  of the 17,669 in-reach rows). 701 struck a city or district of ours and 275
  struck a third party instead; both count as "did not strike one of ours".
- **Seen next** is the unit's plot in the next turn's first frame, against
  the nearest unit of ours: *closer*, *hold* (same plot), *level* (another plot
  at the same gap) or *away*.

### What the solver can compute, and the approximations

Every feature is one the solver's own board (`model::Board`, `Sim`) also
computes, so the tables can be applied there:

- **Class**: melee (melee, anti-cavalry, recon, monk, Giant Death Robot),
  ranged, siege, cavalry (light and heavy). From the exported promotion class,
  else `data/units.json`.
- **Owner**: major, city-state (Free Cities included), barbarian.
- **Its own hit points**, in three bands.
- **Home**: in or beside a city of its own owner. The solver cannot tell one
  enemy's city from another's, so there it is any enemy city.
- **Edge**: the best attack-minus-defence over the units in reach, each side
  after its wound penalty (`dice::effective_strength`). ⚠ From base strengths
  only (`combat`, `ranged`, a siege gun's bombard −17 as
  `Game::ranged_pair_bases` has it): no terrain, fortification, flanking,
  promotion or difficulty bonus, which the solver's own strengths include.
  A blow does `30·e^(edge/25)`, so a melee trade's dealt/taken ratio is
  `e^(2·edge/25)`.
- **Targets**: units of ours in reach, one, two, three or more. A unit of
  ours in its city is not a target: the blow lands on the city.
- **Still**: one of them is in reach without moving (adjacent, or in range of
  a shooter).
- ⚠ **Reach is open ground.** Melee and cavalry reach `d ≤ max_moves`;
  a shooter `d ≤ range + max_moves − 1`; siege only from where it stands
  unless it carries Expert Crew. Terrain costs, rivers, zones of control and
  units in the way are not read, so reach is over-counted in rough ground;
  the solver floods real movement. The check on it: of 4,618 strikes on our
  units by units in sight, 4,356 (94%) hit a unit inside this reach. "Still"
  is exact either way, and it carries most of the signal.
- **Not in the data**: 1,370 of the 6,313 unit-on-unit strikes on us (22%)
  came from units not in sight at the end of our turn. The solver does not
  model unseen enemies either (`docs/BATTLE_SOLVER.md`).

## Method

- **P(strike)**: binomial logistic on one-hot bands of the seven features,
  the first band of each the reference, ridge 1.0 on every weight but the
  intercept, Newton's method on the aggregated cells.
- **Target**: conditional logit over the units in reach, on the strikes that
  hit a unit in reach with two or more in reach (2,512 choices, 3.3 units
  each on average). Features: target hp/100, expected damage (capped at its
  hp)/100, the blow is expected to kill, the target is ranged or siege,
  distance, no move needed. Ridge 1.0.
- **Advance**: P(closer | struck nothing), by its own hp (33 or less, more)
  and where it stood (a unit of ours in reach that it declined; else the gap
  to our nearest unit), each cell shrunk toward its column's rate with a
  pseudo-count of 5. Keying by owner or class instead read no better on
  held-out games.
- **Held out by game**: five folds by a hash of the run name; every row is
  scored by a fit that never saw its game. Log-likelihoods are nats per row.

## Held-out log-likelihood

| model | rows | fitted | constant | stock rule |
|---|---|---|---|---|
| P(strike one of ours \| one in reach) | 17,669 | **−0.4725** | −0.5623 (train rate) | −0.6931 (flat 0.5); −0.6904 (0.5 when the greedy blow is worth > −12 hammers, else 0.02) |
| target, given a strike | 2,512 | **−0.5912** | −1.0876 (uniform) | −1.1203 (greedy pick, ε fitted on training games) |
| P(closes \| struck nothing) | 17,563 | **−0.6270** | −0.6339 (train rate) | −0.6931 (flat 0.5) |

Summed over the held-out rows the strike model gains 1,588 nats on the
constant and 3,899 on the stock 0.5; the target model 1,247 on uniform and
1,329 on the greedy pick. The strike model beats the constant on every fold
(−0.4619/−0.5511, −0.4718/−0.5615, −0.4594/−0.5507, −0.4875/−0.5702,
−0.4843/−0.5797), and so does the target model against uniform
(−0.560/−1.070, −0.555/−1.105, −0.577/−1.030, −0.678/−1.084,
−0.601/−1.138). The fitted target is the one struck 76% of the time on
held-out games; the greedy pick 37%, uniform 37%.
The advance table is a small gain: movement is mostly noise at this grain.

Calibration of the held-out strike probabilities:

| predicted band | rows | mean predicted | struck |
|---|---|---|---|
| 0.0–0.1 | 4,208 | 0.070 | 0.083 |
| 0.1–0.2 | 5,815 | 0.137 | 0.130 |
| 0.2–0.3 | 1,345 | 0.247 | 0.226 |
| 0.3–0.4 | 1,601 | 0.351 | 0.345 |
| 0.4–0.5 | 2,131 | 0.448 | 0.427 |
| 0.5–0.6 | 1,816 | 0.546 | 0.550 |
| 0.6–0.7 | 753 | 0.649 | 0.722 |

No unit-turn is predicted above 0.7: the host never strikes as surely as a
greedy model assumes.

## What the host does

- It strikes **a quarter** of the time it has a unit of ours in reach (4,415
  of 17,669), and **43%** when one is in reach without moving (3,324 of
  7,738) against 11% when it would have to move first (1,091 of 9,931).
  Whether it must move is the largest term (+1.79 log-odds).
- It strikes more when healthy, when it trades well, beside its own city, and
  when it is a ranged unit or a barbarian.
- Its target is the unit it damages most, the most wounded, and the nearest,
  above all one it hits **without moving** (+1.69). Expected kills add nothing
  beyond the damage. This is why "weakest" (27%) and "most damaged" (24%)
  each read as chance in `docs/BATTLE_SOLVER.md`: the host takes what is in
  front of it.
- It does not retreat when wounded. At 25 hp or less, of the unit-turns that
  struck nothing and were seen next, 281 stepped away, 253 closed, 547 held
  and 257 moved at the same gap. A unit that declines a strike closes about
  one turn in three (3,253 of 9,349) and otherwise holds.

## The tables

`host_enemy.rs` holds these numbers; its test
`the_tables_are_the_documents_tables` reads every row below. Printed by
`python3 tools/fit_host_enemy.py fit rows.jsonl --doc`.

| strike term | log-odds |
|---|---|
| intercept | -4.1300 |
| class=melee | +0.0000 |
| class=ranged | +0.4178 |
| class=siege | -0.0926 |
| class=cavalry | +0.0615 |
| owner=major | +0.0000 |
| owner=city_state | +0.1859 |
| owner=barbarian | +0.6539 |
| hp=hp<=33 | +0.0000 |
| hp=hp34-66 | +0.5406 |
| hp=hp67-100 | +0.9565 |
| home=away | +0.0000 |
| home=home | +0.3200 |
| edge=edge<-10 | +0.0000 |
| edge=edge-10..-1 | +0.3207 |
| edge=edge0..9 | +0.6191 |
| edge=edge>=10 | +0.8092 |
| targets=1 target | +0.0000 |
| targets=2 targets | +0.2081 |
| targets=3+ targets | +0.2849 |
| still=must move | +0.0000 |
| still=no move needed | +1.7895 |

| target term | weight |
|---|---|
| hp | -1.3986 |
| dealt | +2.3189 |
| kills | -0.0373 |
| shooter | +0.2853 |
| distance | -1.1584 |
| no move | +1.6865 |

P(closes on us | struck nothing), with the rows behind each cell:

| advance | declined | gap<=3 | gap4-5 | gap6-8 |
|---|---|---|---|---|
| hp<=33 | 0.1905 (1180) | 0.1948 (634) | 0.2208 (80) | 0.1725 (38) |
| hp>33 | 0.3708 (8169) | 0.3115 (5901) | 0.3640 (1052) | 0.2926 (509) |

The summary rate other code can price a unit's risk with today
(`host_enemy::strike_rate`): a visible at-war enemy unit with one of ours in
reach strikes one of ours this turn at

| owner | unit-turns in reach | struck one of ours | rate |
|---|---|---|---|
| major | 7821 | 1601 | 0.2047 |
| city_state | 1584 | 497 | 0.3138 |
| barbarian | 8264 | 2317 | 0.2804 |

## How the solver plays it

With `solver-plays-the-host` on, `model::Sim::act_as_host` replaces the
enemy's turn:

1. Each unit of ours it could strike this turn, from the stock enemy's stand
   (its own tile when in range or adjacent, else the nearest stand), with the
   solver's own strengths, which include terrain, fortification and support.
2. With one or more: it strikes with `strike_probability`, on the unit's own
   dice; the target is drawn with weights `exp(target_utility)`.
3. With none: a city of ours in reach is struck as the stock enemy would, at
   the stock 0.5. The fit has no city model.
4. A unit that struck nothing closes on us (the stock step toward the nearest
   unit of ours, or the city it holds) with `advance_probability`, and
   otherwise holds. There is no wounded retreat.

Off, the stock enemy plays exactly as before.

## Not verified

- Terrain-blind reach and base strengths (above). The solver applies the
  bands to its own, richer strengths, which read lower edges against units
  on hills, in woods or fortified.
- City strikes, wounded retreat and any unit we cannot see are not fitted.
- The fit is Deity and these two days' lanes (Gran Colombia domination,
  culture and religion seats); 47% of in-reach rows are barbarian.
- Whether the solver plays better live with it. The bench below is against
  this repository's own AI, and there it plays worse.

## Bench: a no-regression read, and it reads negative

**The mirror bench reads negative at both horizons:** pooled battle_bench
−84 and doctrine_arena −137 at horizon 2, −90 and −81 at horizon 4. **Only
live games against the host can judge the gene.**

The opponent on `battle_bench` and `doctrine_arena` is this repository's own
AI, not the host. The evidence for the model is the held-out likelihood
above; this bench can only say how a solver that assumes the host's habits
fares against an opponent that does not have them.

`src/ai/advanced/battle_solver/host_enemy_bench.rs`, run on the PR's CI
(`cargo-test`, 4 threads, run 43259725481, head b5a426ae70 merged with main):
seat A is the live tactical stack + `battle-solver` + `solver-plays-the-host`,
seat B the same without the gene, both at the horizon named. Columns: paired
material swing A less B (mean ± se), seeds better/worse/tied, kills/losses of
each seat, seeds where play diverged, elapsed seconds.

```
solver-plays-the-host bench: A = live tactical stack + battle-solver + solver-plays-the-host, B = the same without solver-plays-the-host; paired material swing A less B, mean +/- se, seeds better/worse/tied; 96 skirmish seeds, 48 position seeds, seed block 9800000, 4 threads
h2 control on/on                       +0.00 +/-    0.00    0/  0/  2  kills/losses fitted    2/2    stock    2/2     diverged   0/2        3s
h2 control off/off                     +0.00 +/-    0.00    0/  0/  2  kills/losses fitted    3/3    stock    3/3     diverged   0/2        4s
h2 battle_bench combined arms         -75.21 +/-   24.45   31/ 62/  3  kills/losses fitted  293/347  stock  347/293   diverged  96/96      55s
h2 battle_bench ranged heavy          -46.56 +/-   20.86   32/ 55/  9  kills/losses fitted  260/292  stock  292/260   diverged  96/96      99s
h2 battle_bench melee only            -39.48 +/-    8.53   11/ 38/ 47  kills/losses fitted   47/76   stock   76/47    diverged  96/96     148s
h2 battle_bench medieval mounted     -176.46 +/-   63.42   32/ 54/ 10  kills/losses fitted  432/468  stock  468/432   diverged  96/96     191s
h2 battle_bench pooled                -84.43 +/-   18.05  106/209/ 69  kills/losses fitted 1032/1183 stock 1183/1032  diverged 384/384    191s
h2 doctrine central_position         -116.25 +/-   32.05   19/ 27/  2  kills/losses fitted  179/221  stock  221/179   diverged  46/48     210s
h2 doctrine the_defile                -95.83 +/-   29.64   15/ 30/  3  kills/losses fitted   75/110  stock  110/75    diverged  45/48     226s
h2 doctrine the_ridge                -127.29 +/-   46.63   15/ 32/  1  kills/losses fitted  225/273  stock  273/225   diverged  47/48     246s
h2 doctrine hammer_and_anvil         -171.88 +/-   37.54   12/ 35/  1  kills/losses fitted  194/253  stock  253/194   diverged  47/48     265s
h2 doctrine the_relief               -172.50 +/-   52.45   15/ 32/  1  kills/losses fitted  185/238  stock  238/185   diverged  47/48     298s
h2 doctrine pooled                   -136.75 +/-   18.11   76/156/  8  kills/losses fitted  858/1095 stock 1095/858   diverged 232/240    298s
h4 control on/on                       +0.00 +/-    0.00    0/  0/  2  kills/losses fitted    7/7    stock    7/7     diverged   0/2      301s
h4 control off/off                     +0.00 +/-    0.00    0/  0/  2  kills/losses fitted    7/7    stock    7/7     diverged   0/2      304s
h4 battle_bench combined arms         -70.31 +/-   20.67   34/ 52/ 10  kills/losses fitted  244/298  stock  298/244   diverged  96/96     370s
h4 battle_bench ranged heavy          -70.83 +/-   22.26   35/ 53/  8  kills/losses fitted  272/329  stock  329/272   diverged  96/96     427s
h4 battle_bench melee only            -12.50 +/-    6.89   11/ 20/ 65  kills/losses fitted   25/36   stock   36/25    diverged  96/96     493s
h4 battle_bench medieval mounted     -206.67 +/-   59.06   30/ 59/  7  kills/losses fitted  397/439  stock  439/397   diverged  96/96     546s
h4 battle_bench pooled                -90.08 +/-   17.02  110/184/ 90  kills/losses fitted  938/1102 stock 1102/938   diverged 384/384    546s
h4 doctrine central_position         -158.54 +/-   35.89   12/ 36/  0  kills/losses fitted  152/208  stock  208/152   diverged  48/48     568s
h4 doctrine the_defile                -73.33 +/-   32.76   17/ 27/  4  kills/losses fitted   86/113  stock  113/86    diverged  44/48     586s
h4 doctrine the_ridge                -102.08 +/-   50.16   20/ 28/  0  kills/losses fitted  217/248  stock  248/217   diverged  48/48     612s
h4 doctrine hammer_and_anvil          -57.29 +/-   38.71   20/ 26/  2  kills/losses fitted  200/219  stock  219/200   diverged  47/48     642s
h4 doctrine the_relief                -14.58 +/-   56.47   25/ 23/  0  kills/losses fitted  235/240  stock  240/235   diverged  48/48     678s
h4 doctrine pooled                    -81.17 +/-   19.64   94/140/  6  kills/losses fitted  890/1028 stock 1028/890   diverged 235/240    678s
```

The controls read exactly zero with the gene on both sides and off both
sides. Against this AI the fitted enemy is worse at both horizons: pooled
battle_bench −84.4 ± 18.1 at horizon 2 and −90.1 ± 17.0 at horizon 4,
doctrine_arena −136.8 ± 18.1 and −81.2 ± 19.6. The stock enemy's 0.5 was
tuned on this bench against this AI (`docs/BATTLE_SOLVER.md`: a greedy enemy
read −100, half-aggression +262); the fitted enemy strikes on about a quarter
of its chances and is fitted to the host. This bench cannot tell "the host
model is wrong" from "the host model describes the host and not this AI".
Only live games against the host can judge the gene, and none has been
played with it.

Run it with `cargo test --lib host_enemy_bench -- --ignored --nocapture`
(`CIVVIS_HOST_BENCH_SEEDS` sets the skirmish seeds per row; positions get
half).

## Rival chairs that fight like the host: `--rival-host-tactics`

Every tactical gene is priced in `gene_screen` paired games whose rival
chairs are this repository's own AI, and the bench above shows how differently
that AI fights. The harness switch makes a simulated opponent fight at these
rates instead (`src/host_tactics.rs`); `--chair-tactics-census` reads how the
chairs fight without changing anything.

**The census** reads a chair's turns with the definitions of this page:
a row is a chair's land fighter at war with a measured seat and on a tile
that seat sees, at the start of the chair's turn; reach is open ground; the
strike is read from the game's action log; "closer" compares where it ended
the turn with where the measured seat's units stood.
`python3 tools/fit_host_enemy.py rates rows.jsonl` prints the same rows for
the live host.

**The switch** governs a chair's land fighters within 8 tiles of a measured
seat's unit, once per unit per turn, with draws hashed from the game seed.
The chair's AI may neither strike a measured seat's unit with a governed unit
nor move it that turn; the harness plays the host's turn for it:

- **Strike.** With a measured seat's unit in reach, it strikes with
  `strike_probability` on this page's features, read on open-ground reach as
  the fit read them, divided by the share of the chair's open-ground reach so
  far that was exact (the engine's own movement flood and line of sight).
  Open ground counts targets that rough ground or a zone of control puts out
  of reach, and the fit's rates carry that; dividing by the share is what lets
  the census read the fitted rate back. The target is drawn on the fitted
  weights among the units in exact reach, struck from its own tile when it
  can.
- **Close, step away or hold.** A unit that struck nothing steps toward the
  measured seat with `advance_probability` (a shooter stops at its range),
  divided by the share of units in the same column of the advance table that
  had a tile to close to (one already beside us cannot, and the fit's rate
  counts those too); else it steps away at the live majors' rate (24% at
  25 hp or less, 13% above); else it holds. There is no other retreat.
- **Not made host-like:** what the chairs build and how many soldiers they
  field, which cities they target, when they declare war, their units more
  than 8 tiles from the measured seat, their strikes on cities and districts
  (allowed, from where the unit stands) and their cities' own strikes, other
  players, and fog: the switch governs every unit in range, seen or not, and
  the census reads only what the measured seat sees. The measured seat's own
  agent is untouched.

Both shares are counted on the units the measured seat sees (the rows the fit
and the census read) and shrunk toward values pooled over the lane recipe
(`STRIKE_SHARE_PRIOR`, `CLOSE_SHARE_PRIOR`), because a chair meets only a
handful of such turns a game.

### The first reading (CI, 2026-11-12)

The lane recipe (`--players 4 --width 60 --height 38 --city-states 6 --map
pangaea --difficulty deity --rivals firaxis-faithful --handicap rivals
--rival-chairs 3 --rival-rung -2 --rival-pace --target-mix domination`), seeds
63100000-63100007, the measured seat on the deployment genome, run by
`src/bin/gene_screen/host_tactics_tests.rs` on the PR's CI (cargo-test run
43338413481, head c1931ab761 on main). "live majors" is the host's rows for
major civilizations only, which is what a chair is. The second block prices
the same simulated rows with the fitted tables.

```
row                                           live host      live majors     chairs, switch off      chairs, switch on
strike | in reach                       0.250 ( 17669)  0.205 (  7821)        0.318 (   107)        0.194 (   155)
strike | in reach without moving        0.430 (  7738)  0.344 (  3179)        0.500 (    46)        0.310 (    87)
strike | must move first                0.110 (  9931)  0.109 (  4642)        0.180 (    61)        0.044 (    68)
target: nearest                         0.879 (  2512)  0.856 (  1048)        0.773 (    22)        0.893 (    28)
target: nearest (chance)                0.486 (  2512)  0.440 (  1048)        0.387 (    22)        0.359 (    28)
target: weakest                         0.537 (  2512)  0.545 (  1048)        0.591 (    22)        0.500 (    28)
target: weakest (chance)                0.504 (  2512)  0.508 (  1048)        0.560 (    22)        0.335 (    28)
target: most damaged                    0.503 (  2512)  0.523 (  1048)        0.409 (    22)        0.321 (    28)
target: most damaged (chance)           0.484 (  2512)  0.485 (  1048)        0.484 (    22)        0.469 (    28)
closes | struck nothing                 0.328 ( 17563)  0.339 (  8598)        0.290 (   176)        0.304 (   184)
closes | declined                       0.348 (  9349)  0.351 (  4238)        0.209 (    67)        0.287 (   122)
closes | nothing in reach               0.306 (  8214)  0.327 (  4360)        0.339 (   109)        0.339 (    62)
steps away | hp<=25, struck nothing     0.213 (  1309)  0.239 (   457)        0.286 (     7)        0.200 (     5)
holds | hp<=25, struck nothing          0.405 (  1309)  0.394 (   457)        0.714 (     7)        0.800 (     5)
steps away | hp>25, struck nothing      0.121 ( 16254)  0.128 (  8141)        0.142 (   169)        0.106 (   179)

the same rows against what the fitted tables give the simulated units themselves (realized / fitted):
strike | in reach                                                            0.318 /  0.220     0.194 /  0.263
strike | in reach without moving                                             0.500 /  0.384     0.310 /  0.399
strike | must move first                                                     0.180 /  0.096     0.044 /  0.088
closes | struck nothing                                                      0.290 /  0.322     0.304 /  0.340
closes | declined                                                            0.209 /  0.352     0.287 /  0.353
closes | nothing in reach                                                    0.339 /  0.304     0.339 /  0.315

What the switch did (count, governed unit-turns):
  gate: closed                 272 of 766
  gate: could close, declined      36 of 123
  gate: could close, gap4-5       5 of 6
  gate: could close, gap6-8       6 of 6
  gate: could close, gap<=3      20 of 57
  gate: exact reach, in reach without moving      82 of 84
  gate: exact reach, must move first      31 of 72
  gate: held                   395 of 766
  gate: refusals                 0 of 766
  gate: stepped away            56 of 766
  gate: strikes drawn           38 of 766
  gate: strikes failed           4 of 766
  gate: strikes landed          34 of 766
  gate: valves                   0 of 766

Paired: the measured seat with defend-force-holds-the-city off and on (sums over the worlds):
  switch off: gene off killed  282 lost   70 (kill:loss 4.03) cities lost  15 taken   0 | gene on killed  306 lost  186 (kill:loss 1.65) cities lost  19 taken   4
    seed 63100000: off 45/8 lost 0 city | on 33/33 lost 0 city
    seed 63100001: off 21/13 lost 2 city | on 38/21 lost 2 city
    seed 63100002: off 65/27 lost 0 city | on 114/24 lost 0 city
    seed 63100003: off 39/0 lost 0 city | on 13/33 lost 2 city
    seed 63100004: off 42/10 lost 4 city | on 33/47 lost 3 city
    seed 63100005: off 32/4 lost 0 city | on 38/1 lost 1 city
    seed 63100006: off 8/6 lost 8 city | on 10/23 lost 10 city
    seed 63100007: off 30/2 lost 1 city | on 27/4 lost 1 city
  switch on : gene off killed  288 lost   67 (kill:loss 4.30) cities lost  10 taken   0 | gene on killed  330 lost  181 (kill:loss 1.82) cities lost  17 taken   2
    seed 63100000: off 45/8 lost 0 city | on 34/24 lost 0 city
    seed 63100001: off 21/9 lost 0 city | on 42/27 lost 3 city
    seed 63100002: off 70/26 lost 0 city | on 130/31 lost 0 city
    seed 63100003: off 39/0 lost 0 city | on 13/27 lost 0 city
    seed 63100004: off 43/12 lost 1 city | on 36/40 lost 1 city
    seed 63100005: off 32/4 lost 0 city | on 38/1 lost 1 city
    seed 63100006: off 8/6 lost 8 city | on 10/27 lost 11 city
    seed 63100007: off 30/2 lost 1 city | on 27/4 lost 1 city
```

**Before** (the chairs as they are), the chairs struck one of ours on 32% of
the turns they had one in reach against 21% for the live majors: 50% against
34% without moving and 18% against 11% when they had to move first.
**After** (the switch on): 19.4% in reach (live majors 20.5%), 31.0% without
moving (34.4%), 4.4% when they must move first (10.9%; the fitted tables give
these rows 8.8%), closing 30.4% (33.9%), stepping away 10.6% above 25 hp
(12.8%), and the target the nearest 89% (86%), the weakest 50% (55%), the most
damaged 32% (52%, against 47% by chance in this sample). The strike rows are
small (155 rows in reach, 68 that must move); "must move first" is the one row
still well under the host. The switch played 34 strikes over 766 governed
unit-turns with no refusals reaching the real game and no valve trip.

`defend-force-holds-the-city` off and on for the measured seat, the same
eight worlds: with the chairs as they are, killed/lost 282/70 (4.03) against
306/186 (1.65) and 15 against 19 cities lost; with the switch on, 288/67
(4.30) against 330/181 (1.82) and 10 against 17. The gene loses more units
against both oppositions in these eight worlds; the lane runs the 60 seeds.

The refusals are the engine's (`Game::host_gate`, read in `Game::apply`), set
only for a governed chair's own turn; with no seat wrapped the field is never
set and a game is byte for byte what it was. A unit refused 12 times in one
turn has its moves and attacks spent, so an AI that retries a refused order
still ends its turn.

## Reproduce

```bash
python3 tools/fit_host_enemy.py extract --out rows.jsonl     # ~30 s, one process
python3 tools/fit_host_enemy.py fit rows.jsonl --json fit.json --doc --rust
```
