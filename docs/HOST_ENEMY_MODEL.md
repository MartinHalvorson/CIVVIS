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
- Whether the solver plays better live with it. The bench below is a
  no-regression read only.

## Bench: a no-regression read

The opponent on `battle_bench` and `doctrine_arena` is this repository's own
AI, not the host, so these rows say only that the fitted enemy does not break
the solver. The evidence for the model is the held-out likelihood above.

`src/ai/advanced/battle_solver/host_enemy_bench.rs` (`cargo test --lib
host_enemy_bench -- --ignored --nocapture`). See the PR for the run.

## Reproduce

```bash
python3 tools/fit_host_enemy.py extract --out rows.jsonl     # ~30 s, one process
python3 tools/fit_host_enemy.py fit rows.jsonl --json fit.json --doc --rust
```
