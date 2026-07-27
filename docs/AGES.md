# Ages, Era Score and Dedications

What Gathering Storm's age system does in CIVVIS, what was checked against the
shipped game, and what the AI does with it.

Sources for every "shipped" claim below are the built rules database the game
leaves behind — `~/Library/Application Support/Sid Meier's Civilization VI/Cache/DebugGameplay.sqlite`
on this machine, with load order and every expansion overlay already resolved —
and the `en_US` text assets in the app bundle. Tables used: `CommemorationTypes`,
`CommemorationModifiers`, `Modifiers`, `ModifierArguments`, `RequirementSets`,
`RequirementSetRequirements`, `Requirements`, `Policies_XP1`, `GlobalParameters`,
`Eras`, `Eras_XP1`, `Moments`.

## The audit

### Correct before this change, and re-verified

- **Every Dedication's Era Score trigger and amount.** All twelve match their
  `COMMEMORATION_*_QUEST` modifier exactly: `PER_TECH_BOOST` 1, `PER_CIVIC_BOOST`
  1, `PER_DISTRICT_CONSTRUCTED` 1, `PER_CITY_RELIGION_CONVERSION` 2,
  `PER_CONTINENT_DISCOVERED` 3, `PER_NATURAL_WONDER_DISCOVERED` 3,
  `PER_NON_BARBARIAN_NAVAL_UNIT_KILLED` 1, `PER_TRADE_ROUTE_COMPLETED` 1,
  `PER_CORPS_KILLED` 1, `PER_ARMY_KILLED` 2, `PER_INDUSTRIAL_BUILDING_CONSTRUCTED`
  1, `PER_SPY_SUCCESSFUL_MISSION` 1, `PER_AERODROME_BUILDING_CONSTRUCTED` 1,
  `PER_GREAT_PERSON_EARNED` 1, `PER_ARTIFACT_EXTRACTED` 1,
  `PER_NON_BARBARIAN_UNIT_KILLED_BY_GDR` 1, and the two building shapes
  (`PER_CULTURE_BUILDING_CONSTRUCTED` / `PER_SCIENCE_BUILDING_CONSTRUCTED`), which
  are a building that *yields* that yield rather than one holding a slot — the
  English text says "Great Work Slot" but the modifier does not.
- **Era Score thresholds are self-correcting from the shipped parameters.**
  `THRESHOLD_SHIFT_PER_CITY` 3, `_PER_PAST_DARK_AGE` −10, `_PER_PAST_GOLDEN_AGE`
  +5; `_PER_ANARCHY`, `_PER_INCOMPLETE_ERA_TECH`/`_CIVIC`,
  `_PER_INCOMPLETE_OLD_TECH`/`_CIVIC` and `_PER_MISSING_AMENITY` all ship as 0,
  so there is nothing to model for them.
- **Loyalty per citizen by age.** Golden and Heroic 1.5, Dark 0.5, Normal 1.0.
- **Heroic Age.** A Dark Age that reaches the Golden threshold, three Dedications
  instead of one.
- **Dark Age policy cards** are Wildcard-slot cards offered only inside a Dark
  Age, unslotted when the age or era stops offering them.

### Fixed here

- **A Golden Age banks no Era Score.** Every quest modifier hangs off
  `PLAYER_ELIGIBLE_FOR_COMMEMORATION_QUEST`, a `REQUIREMENTSET_TEST_ANY` whose
  only two members are an *inverted* `REQUIREMENT_PLAYER_HAS_GOLDEN_AGE` and a
  `REQUIREMENT_PLAYER_ALWAYS_ALLOWED_COMMEMORATION_QUEST` that nothing in the
  shipped data grants. CIVVIS paid both halves at once, so a Golden Age handed
  out its bonus *and* financed its own successor. That is the single most
  consequential divergence in the system: it removed the trade-off the age
  economy is built on. The two halves are now exclusive.
- **Dark Age card era windows**, from `Policies_XP1.MinimumGameEra` /
  `MaximumGameEra`: Isolationism Classical–Industrial (was Classical–Renaissance),
  Monasticism Classical–Medieval (was Classical–Renaissance), Inquisition
  Classical–Renaissance (was Medieval–Industrial), Letters of Marque
  Renaissance–Modern (was Medieval–Industrial), Elite Forces
  Classical–Renaissance (was Renaissance–Modern). Twilight Valor and Robber
  Barons were already right.
- **Sky and Stars** opens in the Information era, not the Atomic
  (`COMMEMORATION_AERONAUTICAL.MinimumGameEra`).

### The measurement that reordered everything

Before any of this, `age_census` over 60 six-seat 500-turn games (seed 900000,
1668 age transitions, 360 seats):

| era | dark | normal | golden | heroic | n |
|---|---|---|---|---|---|
| 1 | **98%** | 2% | 0% | 0% | 360 |
| 2 | 55% | 43% | 0% | 1% | 360 |
| 3 | 87% | 12% | 0% | 1% | 282 |
| 4 | 72% | 28% | 0% | 0% | 204 |
| **all** | **79%** | 20% | **0.4%** | 0.8% | 1668 |

**Six Golden Ages in 1668 transitions, and not one seat in 360 avoided a Dark
Age.** The age system was not a strategic axis; it was a near-permanent Loyalty
penalty that every civilization paid equally. The misses were not close-run
either — a Dark Age fell a median 6 Era Score short of Normal, and a Normal Age
a median 12 short of Golden.

Two causes, both fidelity gaps, both fixed here.

**1. The Era Score supply was starved.** CIVVIS awarded Era Score at 14 sites.
The shipped game defines 162 Historic Moments. Thresholds are calibrated to the
shipped supply, so nobody cleared them — even after
`THRESHOLD_SHIFT_PER_PAST_DARK_AGE` −10 compounds and floors the threshold at 6.
Added here, at their exact `Moments.EraScore` values:

| moment | value | frequency |
|---|---|---|
| `TECH_RESEARCHED_IN_ERA_FIRST` / `_IN_WORLD` | +1 / +2 | once per era per civ |
| `CIVIC_CULTURVATED_IN_ERA_FIRST` / `_IN_WORLD` | +1 / +2 | once per era per civ |
| `CITY_BUILT_ON_DESERT` / `_SNOW` / `_TUNDRA` | +1 each | per city, stacking |
| `CITY_BUILT_NEAR_FLOODABLE_RIVER` / `_VOLCANO` / `_NATURAL_WONDER` / `_OTHER_CIV_CITY` | +1 each | per city, stacking |
| `CITY_BUILT_NEW_CONTINENT` | +2 | per city |
| `PLAYER_MET_MAJOR` | +1 | per rival |
| `PLAYER_MET_ALL_MAJORS` / `_FIRST_IN_WORLD` | +3 / +5 | once |
| `GOODY_HUT_TRIGGERED` | +1 | per village |
| `CITY_SIZE_{SMALL,MEDIUM,LARGE,EXTRA_LARGE}_FIRST` / `_IN_WORLD` | +1 / +2 | at pop 10/15/20/25 |
| `GOVERNMENT_ENACTED_TIER_N_FIRST` / `_IN_WORLD` | +2 / +3 | four tiers |

**2. An era could last one turn.** The pacing table read
`mean 41.9  p10 1  median 43  p90 65`. `era_from_progress` tracked the single
most advanced civilization with no floor, so a leader opening two eras in
consecutive turns handed the whole table an age it had no turns to bank in — a
guaranteed Dark Age for everyone. Shipped `Eras_XP1.GameEraMinimumTurns` is 40
for every era; that floor is now enforced, speed-scaled, via
`Game::world_era_since`.

`GameEraMaximumTurns` (60) is deliberately **not** modelled: forcing the world
era past what anybody has researched reaches into wonder eligibility, Dark Age
card windows and unit obsolescence far further than the evidence here justifies.

### What the fixes did, and the answer to the question that started this

Same instrument, same seed, same shape — 60 six-seat 500-turn games at seed
900000, so the two runs are directly comparable:

| | before | after |
|---|---|---|
| dark | 79% | **64%** |
| normal | 20% | **33%** |
| golden | 0.4% | **2%** |
| heroic | 0.8% | **1.3%** |
| era 1 dark | 98% | **36%** |
| turns between transitions | mean 41.9, **p10 1** | mean 47.7, **p10 40** |

Golden Ages went from 6 in the run to 27, and the pacing floor holds exactly.

**Is it ever good to intentionally take a Dark Age? No.** The threshold gives a
natural experiment: among seats that finished an era within a few Era Score of
the Normal line, which side they fell on turns on one Eureka or one barbarian
camp, and is close to independent of how strong the seat is.

| margin | just dark, win% | just normal, win% | dark n | normal n |
|---|---|---|---|---|
| within 2 | **9.7%** | **20.5%** | 103 | 127 |
| within 4 | 10.9% | 20.8% | 175 | 178 |
| within 6 | 10.1% | 20.7% | 267 | 222 |

**Falling just short of Normal roughly halves a seat's win rate** (base rate at
six seats is 16.7%), and the effect is flat across all three bandwidths, which is
what separates a real discontinuity from an artefact of where the window was
drawn.

The Heroic escape hatch does not rescue it. What follows each age:

| after | dark | normal | golden | heroic | n |
|---|---|---|---|---|---|
| dark | **73%** | 24% | 0% | **3%** | 745 |
| normal | 70% | 29% | 2% | 0% | 438 |
| golden | **92%** | 8% | 0% | 0% | 26 |
| heroic | 73% | 27% | 0% | 0% | 15 |

A Dark Age converts to Heroic **3% of the time** and to another Dark Age 73% of
the time. `THRESHOLD_SHIFT_PER_PAST_DARK_AGE` −10, the Dark Age cards and
Heroic's three Dedications together do not pay for the Loyalty penalty and the
lost age. The 92% after a Golden Age is not a bug — it is the Golden Age banking
nothing, which is exactly the oscillation the mechanic is designed to produce.

**Caveat on the discontinuity.** The buckets are seats, not transitions, and a
seat with several age transitions can land in both. That biases toward finding
nothing, so it does not manufacture the gap, but it does mean the numbers are a
strong reading rather than a clean estimate.

### Known gaps, not closed here

- **Ten of the seventeen shipped Dark Age cards are absent**: Decentralization,
  Samoderzhaviye, Soft Targets, Despotic Paternalism, Collectivism, Rogue State,
  Flower Power, Cyber Warfare, Automated Workforce, Disinformation Campaign.
  Each needs a new engine effect at a new read site, not just a data row.
  Decentralization (Classical–Renaissance, +4 Loyalty in cities of 6 or less
  population, −15% Gold above that) is the one that most changes Dark Age play,
  because it directly answers the Dark Age's own Loyalty penalty.
- **Which civilizations set the world era.** CIVVIS takes the maximum over
  seats; the shipped game weighs the field. A runaway leader therefore drags
  every threshold up (they scale +3 per era) while the laggards' capacity to
  bank does not follow. The minimum-turn floor blunts this but does not answer
  it. **This is the most likely cause of the residual 64% Dark rate** and is the
  first thing to try next; it is a one-line change to `era_from_progress` and
  needs its own measurement, because moving the world era moves wonder
  eligibility, Dark Age card windows and unit obsolescence with it.
- **Historic Moments, the rest of them.** The shipped game defines 162 and this
  change closes the highest-frequency families. Still absent and worth having:
  the six `DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_*` moments (+3 each), the
  `FORMATION_*_FIRST` ladder (+1/+2), `UNIT_CREATED_FIRST_*` (+2 to +5),
  `WAR_DECLARED_USING_CASUS_BELLI` (+2), `NATIONAL_PARK_CREATED` (+3),
  `WORLD_CIRCUMNAVIGATED` (+3/+5), and the `PROJECT_FOUNDED_*` set. Each is a
  hook at an event the engine already resolves.

## What the AI does

Both tiers used to take `available_dedications(pid).into_iter().next()` — the
first key of a `BTreeMap`, so **every civilization in every game dedicated
alphabetically.** In the Classical era that is Exodus of the Evangelists, taken
by civilizations that have not founded a religion and never will.

`ai::choose_dedications` now ranks the offer by
`Game::projected_dedication_score`: what each Dedication *would have paid* over
the era that just ended, computed from `Player::last_era_triggers`, a tally the
engine keeps of every trigger firing whether or not that trigger was dedicated.

One number ranks both halves of the choice, because a Dedication's two halves
name the same activity — Free Inquiry counts your Eurekas and then makes Eurekas
worth more; To Arms counts your Corps kills and then makes military units
cheaper. Which half is *live* changes what the number means, and the engine
settles that: in a Golden or Heroic Age nothing is banked, so the tally is read
as "which lane am I in"; in a Normal or Dark Age it is read literally, as the
score that buys the next age.

Ties — including the all-zero tie of a civilization whose first age arrives
before it has done anything the table counts — fall back to alphabetical order,
so the choice only moves where there is evidence to move it.

### And it lost. The alphabetical default ships.

`ai_eval advanced advanced_measured_dedication`, 120 mirrored maps, 240 games,
4 players, stock turn budget, seed 940000:

| | measured | alphabetical |
|---|---|---|
| game-win share | 41.2% | **58.8%** |
| map directions | 10 | **31** |
| sign test | | **p=0.0015** |
| Elo-equivalent | **−61** (CI −124..+1) | |
| anytime-valid e-process | not crossed | **6.04e2, crossed at map 51** |
| terminal score (diagnostic) | 46.3% | 53.7%, p=0.0000 |

The alphabetical agent had more of everything — cities 2.53 vs 2.20, population
16.0 vs 14.2, techs 14.0 vs 13.1, faith 267 vs 211 — and converted 113 religious
victories to 82.

**Why, and it is not subtle in hindsight.** In the Classical era alphabetical
order leads with Exodus of the Evangelists, whose Golden-Age half feeds
missionaries, apostles and Great Prophet points. Religion is the lane that
converts in this engine. The arbitrary default was accidentally feeding the
strongest lane, and ranking by measured Era Score routed civilizations *away*
from it toward whatever they happened to have been doing.

**The general lesson is one this repository keeps relearning.** Era Score is the
right objective in a Normal or Dark Age — it literally buys the next age. In a
Golden or Heroic Age it is only a **correlate** of what the Golden half is worth,
and an argmax over a correlate optimises the correlate. Unifying both halves
under one number was the error; it is the same shape as the state-value net that
went to −313 Elo.

### The repair, and it passed: `Banking`

The diagnosis named its own fix. Rank by the projection **only in a Normal or
Dark Age**, where Era Score is the literal objective, and leave the Golden and
Heroic choice exactly where `Alphabetical` puts it. No new signal — the same
signal withdrawn from the half of the decision it never governed.

| | seed 960000, 120 maps | **seed 970000, 300 maps (pre-registered)** |
|---|---|---|
| game-win share | 56.2% | **57.7%** |
| map directions | 26 to 11 | **67 to 21** |
| sign p | 0.0201 | **0.0000** |
| Elo | +44 (CI −19..+106) | **+54 (CI +14..+93)** |
| Wilson | 47.3%–64.8% | **52.0%–63.1%** |
| e-process | 9.87, not crossed | **5.72e4, crossed at map 112** |
| gate | INCONCLUSIVE | **PASS** |

**Pooled over both disjoint seeds: 420 maps, 93 map directions to 32.**
`Measured` had scored 41.2% on a third disjoint seed, so the same signal is worth
−61 Elo applied to both halves and +54 applied to one.

The mechanism is legible in the diagnostics. Banking keeps Exodus of the
Evangelists in the Golden Age — faith 285.9 vs 230.9 on the confirmation run,
414.6 vs 188.3 on the first — so the religion engine that made the arbitrary
default strong survives, while ranking where the number is causal builds a better
empire underneath it: cities 2.57 vs 2.22, population 16.1 vs 13.8, science 19.3
vs 16.4, culture 25.4 vs 21.1.

**The transferable rule.** Before ranking on a number, ask which decisions it is
the objective for and which it merely correlates with, and *withdraw it from the
second set*. A signal that is causal for half a decision and correlational for
the other half will lose if you apply it to both — and win if you don't.

`Weights::dedication_choice` selects the arm. `Banking` is the default because it
passed its gate. `Alphabetical` remains the frozen control for reproducing any
age number published before 2026-07-27, and `Measured` is retained as
`advanced_measured_dedication` so the negative result stays reproducible too.

## Measuring

`age_census` is the instrument. It never changes a decision and no agent can
name it.

```text
cargo run --release --bin age_census -- --players 4 --maps 24 --turns 500
```

It reports the age distribution per era, how far short of each threshold the
misses fell, how often the Heroic route was taken, win rate and mean final rank
by the ages a seat held, and — the diagnostic the chooser exists for — what
share of choices took the best-projected Dedication and how much Era Score the
rest left on the table.
