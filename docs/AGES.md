# Ages, Era Score and Dedications

What Gathering Storm's age system does in CIVVIS, what was checked against the
shipped game, and what the AI does with it.

Sources for every "shipped" claim below are the resolved Gathering Storm rules
and the matching English Civilopedia data. Tables used: `CommemorationTypes`,
`CommemorationModifiers`, `Modifiers`, `ModifierArguments`, `RequirementSets`,
`RequirementSetRequirements`, `Requirements`, `Policies_XP1`, `GlobalParameters`,
`Eras`, `Eras_XP1`, `Moments`.

## The audit

### Final result (2026-07-29)

The original diagnosis was right — Era Score supply was starved — but the first
repair was incomplete. A second row-by-row audit of all 143 positive-score rows
in Gathering Storm's 162-row `Moments` catalogue found and closed the remaining
families. It also found three independent structural bugs:

- world era used the leading civilization instead of the field median, then the
  first median repair used the lower side of an even field; the shipped trigger
  is reached when **at least half** the living majors enter the next individual
  era;
- the 40-turn minimum existed, but the ten-turn warning and 60-turn maximum did
  not; all three are now speed-scaled and serialized;
- stock Gathering Storm thresholds were using Dramatic Ages overrides. Stock is
  base 14/28, Ancient shift -3, +1 per city, -5 per past Dark Age and +5 per
  past Golden/Heroic Age. The +3-per-city/-10-per-Dark values belong to the
  optional Dramatic Ages mode.

The score hooks now cover every positive-score moment: exploration and contacts;
city sites, growth and transfers; research,
governments and Governors; districts, buildings, Wonders, improvements and
railroads; units, formations, promotions and underdog kills; religion; Great
People and patronage; city-state Envoys and levies; trade posts; wars,
Emergencies and liberation; disasters and Power; archaeology, parks and Rock
Bands; space projects and the Diplomatic Victory resolution. First-in-world,
ordinary, repeatable, obsolete-era and replacement-versus-stacking behavior is
tested separately. Taj Mahal applies once to each qualifying Historic Moment,
never to Dedication score or to a sum of unrelated +1 moments. CIVVIS recruits
and expends named Great People immediately, so each recruited General or Admiral
can oversee one future offensive in the matching land or sea domain; this
preserves each person's first-victory Moment without inventing a passive map
unit that the engine does not otherwise model.

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
The shipped game defines 162 Historic Moments, 143 of which carry positive Era
Score. Thresholds are calibrated to that supply, so nobody cleared them. The
first repair added the highest-frequency families below; the final audit above
completed every other family the engine can emit, at its exact
`Moments.EraScore` value:

| moment | value | frequency |
|---|---|---|
| `TECH_RESEARCHED_IN_ERA_FIRST` / `_IN_WORLD` | +1 / +2 | once per era per civ |
| `CIVIC_CULTURVATED_IN_ERA_FIRST` / `_IN_WORLD` | +1 / +2 | once per era per civ |
| `CITY_BUILT_ON_DESERT` / `_SNOW` / `_TUNDRA` | +1 each | per city, stacking |
| `CITY_BUILT_NEAR_FLOODABLE_RIVER` / `_VOLCANO` / `_OTHER_CIV_CITY` | +1 each | per city, stacking |
| `CITY_BUILT_NEAR_NATURAL_WONDER` | +3 | per city, stacking |
| `CITY_BUILT_NEW_CONTINENT` | +2 | first settlement on each new continent |
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

The complete shipped pacing rule is now modeled: at least half the living majors
entering the next individual era arms a ten-turn warning; it cannot expire
before turn 40 of the current world era, and an untriggered era arms the same
warning in time to end at the turn-60 maximum. A transition advances exactly
one era, so a late research grant cannot skip age judgments.

### What the fixes did, and the answer to the question that started this

The completion census used 24 deterministic six-seat, 500-turn maps at seed
990000 (336 age transitions). It is a diagnostic sample, not a balance gate:

| era | dark | normal | golden | heroic | n |
|---|---|---|---|---|---|
| 1 | 30% | 25% | **46%** | 0% | 138 |
| 2 | 65% | 32% | 3% | 0% | 60 |
| 3 | 69% | 12% | **19%** | 0% | 48 |
| 4 | 54% | 42% | 0% | 4% | 48 |
| 5 | 46% | 50% | 0% | 4% | 24 |
| 6 | 17% | 75% | 0% | 8% | 12 |
| 7 | 17% | 83% | 0% | 0% | 6 |
| **all** | **46%** | **31%** | **22%** | **1%** | **336** |

Golden/Heroic ages are now 23.2% of transitions, rather than the original 1.2%,
and the opening era is no longer an automatic Dark Age. Era pacing is bounded
exactly as shipped: mean 53.0 turns, p10 40, median 57, p90 60.

Dark misses remain real (median six points short), but they are no longer caused
by an impossible score supply or a runaway civilization advancing the world.
The small completion sample does **not** reproduce the old claim that barely
missing Normal halves win rate: within two points, just-Dark seats won 5.9%
and just-Normal seats 9.4%, with only 17 and 32 observations. That earlier
causal claim is withdrawn pending a new pre-registered large run.

### Remaining scope boundaries

- Ten Gathering Storm Dark Age policy cards remain outside this Era Score
  repair: Decentralization, Samoderzhaviye, Soft Targets, Despotic Paternalism,
  Collectivism, Rogue State, Flower Power, Cyber Warfare, Automated Workforce
  and Disinformation Campaign. They require unrelated engine effects, not Era
  Score hooks.

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
