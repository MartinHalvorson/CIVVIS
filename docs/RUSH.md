# The ancient window

*What an early rush can reach in this engine, measured rather than assumed.*

Every prior war measurement in this repository is of a **late** war —
`docs/AI_GAPS.md` and [`civvis-wars-take-nothing`] both start from full-length
games and ask why 0.33 cities a game get taken. This document asks the prior
question: **is there an early window at all, what is standing in it, and what
does it cost to take a capital inside it?**

The instrument is `src/bin/rush_census.rs`. The treatment is `advanced_rush`.

---

## 1. The window is real, and much wider than Civ 6 intuition suggests

`rush_census --players 6 --maps 12 --turns 120` (74x46, seed 900000), stock
`advanced`:

| turn | capitals walled | `city_strength` | garrison | military/civ | `masonry` |
|---|---|---|---|---|---|
| 20 | **0.0%** | 14.1 | 1.4 | 1.9 | 0% |
| 30 | **0.0%** | 14.3 | 1.5 | 2.0 | 0% |
| 40 | **0.0%** | 16.0 | 0.7 | 2.7 | 0% |
| 50 | **0.0%** | 17.2 | 0.7 | 3.4 | **0%** |
| 60 | 0.0% | 20.1 | 1.0 | 4.5 | 7% |
| 80 | 8.3% | 24.1 | 3.2 | 6.3 | 43% |

Three facts, none of which were on record:

- **No capital anywhere carries a wall before turn 80.** Not one, across 12
  games. No empire holds `masonry` at turn 50. The walled-city problem that
  dominates this repo's siege design *does not exist inside this window*.
- **Capitals are, on average, empty.** A mean garrison strength of 0.7 means
  most capitals have no military unit standing on them at all.
- **`city_strength` at turn 50 is 17.2** — that is `max(strongest_built-10,
  garrison, 10) = 10`, plus the palace's 3, plus a few points of terrain.

### Why walls never arrive: the defender is not allowed to see it coming

`production_value` pays 320 for a wall building only when `threatened` is set,
and `threatened_city` counts hostile units within 6 tiles **while already at
war**. `walls` costs 80 production against an early city's handful per turn.

> **A declaration issued from an already-adjacent stack cannot be answered.**
> The same declaration issued at marching distance hands the victim ten turns
> of warning. This is the whole timing rule of the lane.

---

## 2. What it costs to take one: a Monte Carlo over the engine's own formulas

`damage`, `effective_strength`, `city_strength` and `city_take_damage` were
replicated exactly and played out 3000 times per row against the **measured**
turn-50 profile above (not against an assumed one — the assumed profile,
`city_strength` 23 with a warrior garrison, costs 50% more army):

| composition | production | P(capture), empty capital | P(capture), defender pulls its army home |
|---|---|---|---|
| 1x warrior | 40 | 0.0% | 0.0% |
| **2x warrior** | **80** | **100%**, 3 turns | 0.0% |
| 3x warrior | 120 | 100%, 2 turns | 31% |
| **4x warrior** | **160** | **100%**, 2 turns | **100%**, 2 turns |
| 3x heavy_chariot | 195 | 100% | 100% |
| 2x horseman | 160 | 100% | 100% |

Two warriors is enough on paper, and two is what shipped — but **not because
of this table**. The count was swept against 3 and 4 on 12 maps and 2 took the
most cities and killed the most empires; more importantly the readiness gate
does not rely on the count at all. `early_rush_stack_ready` asks the engine's
own `damage` curve whether the staged force can deliver the city's HP *before
it dies*, which is the question this table is really answering and the one a
head-count gets wrong the moment the units are horsemen rather than warriors.

**Oligarchy's +4 combat strength changes nothing this early** (100% either
way) — it is a lever for a *later* war.

### Ranged units are a trap, and this is why

- A land ranged unit attacking a city takes a flat **−17 strength**
  (`src/game.rs`, the `city_id` arm of the ranged attack).
- An ordinary ranged attack **cannot reduce a city below 1 HP** — only a
  Bombard-class `spec.siege` shot may deplete, and even that cannot capture.
- Ranged units **exert no zone of control**, so they cannot help seal a siege
  ring.

Damage per production against a naked capital: swordsman 0.53, horseman 0.45,
heavy chariot 0.43, **archer 0.27**. An archer is half a melee unit at more
than half the cost, and it can neither capture nor besiege.

### The siege ring is two units, not six

A city heals **+20 HP/turn unless besieged**, and `district_under_siege`
requires *every* passable neighbour to be occupied **or covered by hostile
ZOC**. A ZOC unit covers its own ring tile plus both ring-neighbours — three of
six. So **two melee units placed three apart seal a six-neighbour city.** ZOC
does not cross rivers, so a river capital costs more.

---

## 3. Reach is the binding cost, not the army

| | tiles | march turns at 1.5 tiles/turn |
|---|---|---|
| nearest rival capital, median | 13 | 9 |
| p90 | 17 | 12 |
| minimum observed | 9 | 6 |

No seat in 66 had a rival capital inside 8 tiles. `RUSH_REACH = 16` covers
88% of seats.

⚠ **Tightening it is measured worse.** At `RUSH_REACH = 11` the median
separation of 13 leaves most seats with no legal victim at all: first war
slipped turn 34 → 51, blows by turn 60 fell 17.9 → 4.9, eliminations 12/24 →
9/24.

**The army is ~160 production; the march is 9–12 turns.** But the horse is what
paid: 4 movement against 2 halves the march *and* survives the siege, which is
why `advanced_rush` researches `horseback_riding` before anything else — and
why `rush_census` measuring **0% of empires holding it at turn 50** was the
most actionable number in this document.

---

## 4. What `advanced` cannot express

Two hard gates put the stock agent structurally outside this window:

- `assess` withholds `GrandStrategy::Conquest` until **turn 55**, except for
  five hardcoded civilizations (`Sumeria`, `Aztec`, `Nubia`, `Scythia`,
  `Byzantium`) which get turn 35.
- `advanced_war_declaration` carries a hard **`g.turn < 35`** floor and
  requires `my_power > target_power * 1.32 + 12.0` — an empire-wide comparison
  in which, at turn 40, the `+ 12` alone outweighs the ratio.

Measured consequence, stock agent over 12 games:

```
blows landed on cities by turn 60    : 0.0   (0 HP)
maps with any major eliminated       : 0/12
majors alive at turn 50              : 6.00 of 6
first capture between majors         : median turn 80
```

**The stock agent never touches a city in the first sixty turns.**

---

## 5. The lane, and what it moves

`advanced_rush` keeps every gate that is about the war and waives the two that
are calendar. It picks the nearest major whose capital is unwalled and within
`RUSH_REACH`, aims at the **capital**, raises the standing-army floor, researches
`horseback_riding`, and declares once the staged force can finish.

Critically, it **executes its own siege** (`rush_siege_step`). The general
force-group heuristics assemble the stack correctly and then will not put it on
the city: measured per siege rather than per civilization, the staging ring at
3–5 tiles reaches the full stack while the city's own ring never holds more
than two. Four attempts to make those heuristics besiege each measured worse
(§6). They are tuned for a field campaign between comparable armies; a rush is
not that.

### ★★★ Capability — it wipes a neighbour in 19 games of 24, by turn ~48

**24 maps at the deployment shape (6p 74x46), replicated on three disjoint
seed sets:**

| | stock `advanced` | `advanced_rush` |
|---|---|---|
| **maps with any empire eliminated** | **0/24, 0/24, 0/24** | **19/24, 19/24, 19/24** |
| first elimination (median turn) | — | **48, 48, 55** |
| blows on cities by turn 60 | **0.0** (0 HP) | 18.8 (640 HP) |
| majors alive at turn 50 | 6.00 | **5.54** |
| majors alive at the end | 6.00 | 4.83 |

> **The stock agent eliminates nobody, ever — 0 of 72 games across all three
> seed sets — and lands no blow on any city in the first sixty turns.** The
> rush wipes an empire in 79% of games, with the first kill landing at a median
> turn of 48.

### The campaign clock — where the turns actually go

Game-level firsts mislead, because the first war and the first capture need not
be the same campaign. Per campaign, split by when war opened:

| | early (declared < t50) | late (t50+) |
|---|---|---|
| wars | 41 | 72 |
| declared (median) | **turn 33** | turn 99 |
| took a city | **23 of 41** (median turn 43) | 16 of 72 |
| ...turns declaration → first city | **6** | 3 |
| **killed an empire** | **16 of 41** (median **turn 46**) | 12 of 72 (median turn 85) |
| ...turns declaration → the kill | **14** | 19 |

**An early rush that lands takes its first city 6 turns after declaring and
finishes the empire 14 turns after declaring.** The siege was never the
bottleneck; when the rush declares is.

### ★★ The two numbers that want opposite things

`RUSH_STACK` (what opens the war) and `RUSH_ARMY` (what the empire keeps
building) were one constant for too long, and it capped the whole lane:

| | opening stack 2 | opening stack 3 | **open at 2, build to 4** |
|---|---|---|---|
| declared (median) | turn 32 | turn 44 | **turn 33** |
| early wars that killed | 6/33 | 8/29 | **16/41** |
| early kill (median) | turn 47 | turn 56 | **turn 46** |
| maps with an elimination | 10/24 | 14/24 | **19/24** |

A bigger opening stack converts better and declares far too late; a smaller one
opens on time and cannot finish. Splitting them gets both: **the war starts the
turn it can, and the reinforcements walk into a siege already in progress.**

### What each step was worth

Cumulative, each row adding to the one above:

| | blows by t60 | captures | eliminations |
|---|---|---|---|
| lane only, force-group siege | 6.1 | 6/12 | 2/12 @ t102 |
| **+ dedicated siege executor** | **12.8** | **12/12** | 4/12 @ t70 |
| + persist past the window, finish the victim | 14.1 | 12/12 | 6/12 @ t70 |
| + `horseback_riding`, finish-capable gate | 15.3 | 12/12 @ t45 | 4/12 @ t49 |
| + per-turn plan cadence while rushing | 18.8 | 18/24 @ t46 | 10/24 @ t59 |
| **+ open at 2, build to 4** | 18.8 | **23/41 campaigns** | **19/24 @ t48** |

---

## 5a. ★★ THE VERDICT ON WINS — the cost fell to inconclusive

⚠ **Read the map size on any `ai_eval` line.** It defaults to **24x16**, which
is not the shape this window was measured on, and this repository has already
seen a sign flip between the two. Always pass `--width 74 --height 46
--players 6`.

At the deployment shape, as the lane got better at actually killing, its cost
in wins fell away:

| | early lane (3 for / 21 against) | **finished lane** |
|---|---|---|
| `advanced_rush` seat wins | 9.2% | **13.8%** |
| `advanced` seat wins | 24.2% | 19.6% |
| paired map directions | 3 / 21 | **4 for / 11 against** |
| sign test | **p=0.0003 SIGNIFICANT** | **p=0.1185 INCONCLUSIVE** |
| gate | RETAIN `advanced` | **INCONCLUSIVE** |
| terminal score | 52.8% (p=1.0000) | **59.1%, 33 for / 7 against, p=0.0000** |

**The early lane paid all of aggression's cost and collected none of its
payoff.** It diverted the empire out of the religious lane while eliminating a
rival in 2 games of 12. The finished lane eliminates one in 19 of 24, and the
win cost is no longer statistically distinguishable from parity.

It is still not *better*, and the direction still leans against it. What
changed is the mechanism:

| victory type | `advanced_rush` | `advanced` |
|---|---|---|
| religious | 6 | **35** |
| diplomatic | **18** | 12 |
| culture / science / domination | 4 / 3 / 2 | 0 / 0 / 0 |

Conquest still does not convert directly — **domination is 2 wins out of 33.**
What a successful rush buys is a much larger empire (score 504 vs 352, cities
5.3 vs 3.1, population 65 vs 38, military 717 vs 441) that then wins
*diplomatically*. Religion remains the single most reliable condition in this
engine and war still costs most of it.

> **Standing caution.** Terminal score is 59.1% with 33 map directions for and
> 7 against — overwhelming — while wins are 4-to-11 against. Anyone tempted to
> select on score here should read that pair twice. It is the cleanest
> demonstration in this repository that **score is not win probability**.

**So this ships as an eval-only entrant, not as a default.** `advanced`
behaviour is unchanged: every path is behind `early_rush`.

## 6. ⚠ Measured and rejected

Five interventions were measured and are not in the shipped lane. They are
recorded here, and in place at each call site, so they are not retried.

**1. Letting a rush ignore `relieving` and `Muster`.** The theory was that a
stack sized against one undefended capital should never stand still. Captures
fell 9/12 → 6/12 and the median first capture slipped turn 79 → 96. Those
standing-still postures are load-bearing even for a rush.

**2. Pinning `focus_target` to the objective city.** The theory was that the
first defender met otherwise pulls the column off the capital. Blows on cities
by turn 60 fell 6.1 → 2.9 and first capture slipped turn 65 → 86. A rush that
walks past the defenders to stand on the ring dies on the ring.

**3. `RUSH_REACH` 16 → 11**, to shorten the march. First war slipped turn
34 → 51, blows fell 17.9 → 4.9, eliminations 12/24 → 9/24. The median capital
separation is 13, so a shorter reach leaves most seats with **no legal victim
at all**.

**4. Requiring a genuinely weaker victim** — `power <= mine * 0.85` instead of
`* 1.15 + 5`. Early wars fell 33 → 14, the median declaration slipped turn
32 → 47, and kills slipped turn 47 → 70. Early empires are *all* near-parity
because they all field one or two units, so a superiority test does not select
weak victims — it postpones the rush out of its own window. What makes the rush
work is the staged stack against an unwalled capital, which is what
`early_rush_stack_ready` checks directly.

**5. Editing `production_value` to raise the army — zero effect, twice
measured byte-identical.** `advanced_production` only runs for `Recovery` or an
assigned victory target. Every other plan, **including `Conquest`**, takes
production through `BasicAi::cities` in `src/ai.rs`, whose army target is
`mil_per_city * n_cities` with `mil_per_city` defaulting to 1.0. This is the
mirror image of the standing "read the right file" warning about
`src/ai/advanced.rs`: for *production* on a Conquest plan, the real code is in
`src/ai.rs`.

### ⚠ A measurement trap worth more than any of them

"Melee adjacent to a rival capital = **0.03 per civilization**" read as *the
column never reaches the city*, and three interventions were aimed at that
non-problem. It was a **mean over six seats when only about one in six is
rushing** — it divided one real siege by the five empires not conducting one.
The per-siege **maximum** told the truth immediately: the staging ring at 3–5
tiles reached the full stack while the city's own ring never held more than
two, which is a completely different fault with a completely different fix.

> **For anything only a minority of seats do at a time, report the maximum or
> the per-event figure. Never the mean.**

---

## 7. A note on target selection: the roster barely differentiates

Worth recording because it is easy to assume otherwise:

- `Game::leader_trait()` has **zero callers**. Every trait
  (`aggressive_military`, `expansionist`, …) is inert.
- **92 of 105 civilizations have an ability that appears nowhere** in `src/` or
  the modifier data. The 13 live ones are Rome, Egypt, Greece, China, Sumeria,
  Aztec, Nubia, Scythia, Byzantium, Zulu, Portugal, Norway, Maori.
- Only 9 civilizations have a unique unit.

So a victim should be chosen by **state** — unwalled, near, weak — and not by
nameplate, which is what `early_rush_victim` does. The civilizations that
genuinely resist an ancient rush are the short list with an early defensive
unique (Greece's hoplite 28/65, Aztec's eagle_warrior 28/65 at no tech cost)
or a live combat ability (Scythia's `killer_of_cyrus`, Norway's `knarr`).

---

## 8. 2026-07-29 preregistration: the game that ships removes the rush's largest cost

The result above is not a deployment result. It used the deployment's old
six-player rectangle but otherwise inherited `ai_eval` defaults: Standard
speed, Pangaea, fixed civilizations, and all six victory conditions. The live
supervisor now plays Online and enables only Science, Culture, and Domination.
It varies player count, world type, and topology between games; its currently
running cell is eight players on 84x54 Continents/Planet with 12 city-states.

That difference is mechanistically important rather than cosmetic. In the
all-victory result, `advanced_rush` gave up 29 Religious wins (6 versus 35),
while gaining every Culture, Science, and Domination win in the comparison
(4/3/2 versus 0/0/0). Religious Victory is disabled in the live game. The
fixed hypothesis is therefore:

> On the live three-victory game, the completed rush converts its already
> measured territorial gain into more wins because the victory path it paid
> for that gain by abandoning is unavailable to either arm.

This is a direct policy A/B, not a score proxy. The first screen uses fresh
maps and the exact cell currently served by the managed spectator:

```text
ai_eval advanced_rush advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 60 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9960000 --jobs 6
```

`ai_eval` did not accept the last five profile dimensions when this was
written. They are added before the run, defaulting to its historical Pangaea,
flat, polar, fixed-roster, all-victories behaviour. The output must print the
resolved profile so an omitted flag cannot silently recreate the old test.

### Fixed decision rule

The 60-map screen advances only if all of these hold:

- paired win score is at least 55%;
- favorable map directions outnumber adverse directions;
- paired terminal-score share is at least 52%; and
- the existing promotion gate does not retain `advanced`.

This screen cannot promote a default. Passing earns a disjoint, predeclared
confirmation at seed 9961000 with at least 240 maps and the same exact profile.
That confirmation must pass the repository's unchanged win promotion gate;
terminal score remains diagnostic. A policy change would be conditional on the
enabled victory set, so the prior all-victory result is not overwritten by a
different game. It would then need a separately fixed sample across the
supervisor's varying player-count, map, and topology distribution before being
called an exhibition-wide gain.

If the screen fails, there is no threshold or seed retry. The next permissible
hypothesis is a selective rush whose eligibility is fixed from pre-war state
and whose direct treatment effect is developed and held out by map. No selector
may be fitted to these 60 outcome maps and then evaluated on the same maps.

### Result: refuted on the live cell

The exact screen completed all 60 maps (120 mirrored games) at seed 9960000.
It failed every advancement term:

- paired win score was **47.1%** (95% Wilson CI 35.0%..59.5%, Elo-equivalent
  -20, CI -107..+67), below 55%;
- map directions were **6 rush-favoured, 42 neutral, 12 advanced-favoured**
  (two-sided sign p=0.2379), so favourable did not outnumber adverse;
- paired terminal-score share was **50.4%**, below 52% (31 maps favoured the
  rush and 29 favoured advanced, p=0.8974); and
- the unchanged promotion gate was **INCONCLUSIVE**, retaining `advanced`.

The direct outcome also runs opposite the mechanism proposed above:
`advanced_rush` won 8 games and `advanced` won 15. Every rush victory was
Science; advanced won 12 Science and 3 Culture games. Removing Religious
Victory therefore did not expose a hidden conversion of the rush's land into
live-profile wins. Advanced still won more Science games itself.

The economy diagnostic explains why score could not be used as a substitute
for winning. Rush seats finished with more cities (6.05 versus 5.35),
population (75.4 versus 68.9), military units (20.0 versus 16.3), and nominal
score (571.7 versus 566.0), yet they had fewer civics (5.9 versus 6.8), fewer
tourists (26.2 versus 29.0), and fewer actual victories. The rush changes the
shape of the empire without improving victory routing. There is no disjoint
confirmation and no seed or threshold retry.

#### Evaluation throughput discovered by the screen

The first attempt also exposed a non-gameplay bottleneck on Planet. A
frequency-21 globe has 4,412 tiles, whose complete exact-distance table is
about 39 MB, but `Sphere` stopped its shared cache at 512 rows. Once early
games filled those rows, later recon and tactical movement repeatedly ran A*
for the same endpoints. A read-only stack sample localized the tail to those
exact distance calls.

Removing the obsolete row-count ceiling leaves the existing 64 MB byte budget
in charge: this stock globe may cache every row, while larger globes remain
bounded. Distance values and AI choices are unchanged. The identical first
six maps fell from about 77 minutes to **132 seconds** even while contending
with the obsolete run; the full screen then completed in **1,223 seconds**.
The evaluator now prints a deterministic progress line after each six-map
chunk so another long run cannot look hung.

---

## 9. 2026-07-29 preregistration: only march at a capital a land army can reach

The failed screen above is not used to fit a threshold or classify a map. It
did expose a code-level mismatch between the old evidence and the live world:
the rush was developed on Pangaea/flat, while the screen was
Continents/Planet. `early_rush_victim` calls `wdist` and accepts a capital at
most 16 graph steps away, but graph distance crosses water. It never proves
that an ancient land unit can route to the victim. A nearby capital on another
continent can therefore trigger Horseback Riding, a four-melee army, and a
Conquest plan even though none of those units can arrive.

The fixed hypothesis is:

> The rush is valuable only when its initial army has a real land route to the
> staging ring. Refusing geometrically close but route-disconnected targets
> preserves the measured same-continent capability without paying its research
> and production cost on an overseas war that cannot open in the ancient
> window.

The treatment is named `advanced_rush_connected`. On the first turn at which
the agent can inspect all founded major capitals, before it has opened a war,
it freezes the set of rivals reachable by one of its current land,
melee-capable units. A rival is eligible when that unit is already within three
steps of the capital or `route_step(unit, capital, 3)` returns a step. The
three-step range is the existing staging distance, not a fitted reach
threshold. The route uses current terrain, traversal technology, and border
access. This set is never recomputed after the treatment can affect research,
production, diplomacy, or movement. Once an eligible rush opens, the existing
lane finishes it even if the route later changes.

Every other rush condition remains unchanged: the 16-tile geometric ceiling,
unwalled capital, victim-power guard, two-unit finish-capable opening stack,
four-unit production floor, dedicated siege execution, research order, and
window close. On a connected Pangaea fixture the new entrant must make the
same plan as `advanced_rush`; on a split-continent fixture it must remain
ordinary `advanced`. `ai_eval` must report both the fraction of treatment
seat-games that ever carried a rush plan and the fraction of observed
player-turns spent rushing, so a near-control result cannot masquerade as a
successful selector.

### Fresh development screen

PR #544 merged as `1da0a9b` before any development outcome was read, and the
implementation began in PR #557. The fixed 120-map development screen is:

```text
ai_eval advanced_rush_connected advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 120 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9970000 --jobs 6
```

None of seeds 9960000..9960059 may be used for implementation choices,
thresholds, or selector labels. The development screen advances only if all
of these predeclared conditions hold:

- 10% to 80% of treatment seat-games ever carry a rush plan;
- paired win score is at least 52%;
- favourable map directions outnumber adverse directions;
- paired terminal-score share is at least 50%; and
- the unchanged promotion gate does not retain `advanced`.

This is a mechanism/development gate and cannot promote the default. Coverage
below 10% means the treatment is too close to control to evaluate; coverage
above 80% means route connectivity did not meaningfully select.

### Disjoint holdout

Passing every development term earns one unchanged 240-map holdout at seed
9971000 with the same profile and selector. The holdout must keep coverage
inside 10%..80%, have more favourable than adverse directions, retain at least
50% terminal-score share, and produce **PROMOTE** from the repository's
unchanged win gate. Only then may the connected selector become the default
for this exact cell; a separate predeclared sample across the supervisor's
player-count, map, and topology distribution is still required for an
exhibition-wide claim.

If development or holdout fails, there is no route threshold, staging range,
coverage band, or seed retry. The ancient-rush line is retired on this live
cell, and the next military work returns to mid-game victory routing rather
than fitting another selector to these outcomes.
