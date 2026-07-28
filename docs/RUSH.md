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

### Capability — 24 maps at the deployment shape (6p 74x46, seed 950000)

| | stock `advanced` | `advanced_rush` |
|---|---|---|
| first war between majors (median) | turn 92 | **turn 34** |
| maps with any capture | 13/24 | **23/24** |
| first capture (median) | turn 83 | **turn 56** |
| **maps with any elimination** | **0/24** | **12/24** |
| first elimination (median) | — | turn 64 |
| blows on cities by turn 60 | **0.0** (0 HP) | **17.9** (619 HP) |
| majors alive at the end | 6.00 | 5.33 |

> **The stock agent eliminates nobody in twenty-four games and lands no blow on
> any city in the first sixty turns.** The rush kills an empire in half of them.

⚠ **The literal turn-50 target is met only occasionally.** Majors alive at turn
50 is 5.92 against 6.00 — the median first elimination is turn **64**, not 50.
War opens at turn 34 and the first city falls around 56. The remaining cost is
the siege, not the march.

### What the siege executor and the horse were each worth

Cumulative, 12 maps at seed 900000, each row adding to the one above:

| | blows by t60 | captures | eliminations |
|---|---|---|---|
| lane only, force-group siege | 6.1 (192 HP) | 6/12 | 2/12 @ t102 |
| **+ dedicated siege executor** | **12.8** (432 HP) | **12/12** | 4/12 @ t70 |
| + persist past the window, finish the victim | 14.1 (471 HP) | 12/12 | **6/12** @ t70 |
| + `horseback_riding`, finish-capable gate | 15.3 (508 HP) | 12/12 @ **t45** | 4/12 @ **t49** |

The executor is the single largest step, and it is the one that stopped the
stack trading itself: a warrior against the measured capital takes 28 damage a
blow and dies on its fourth having dealt 134 of the 200 needed.

---

## 5a. ★★★ THE VERDICT ON WINS — early aggression is significantly harmful

⚠ **Read the map size on any `ai_eval` line.** It defaults to **24x16**, which
is not the shape this window was measured on. The first two runs below are on
that default; the third is at the deployment shape and is the one that counts.
See [[civvis-eval-defaults-are-not-the-deployment]] — the sign has flipped
between these two shapes before.

| | 24x16 (default) | 24x16, +fixes | **74x46, 6p (deployment)** |
|---|---|---|---|
| `advanced_rush` seat wins | 8.6% | 9.4% | **9.2%** |
| `advanced` seat wins | 24.7% | 23.9% | **24.2%** |
| paired map directions | 0 / 29 | 0 / 26 | **3 for / 21 against** |
| sign test | p=0.0000 | p=0.0000 | **p=0.0003** |
| gate | RETAIN | RETAIN | **RETAIN `advanced`** |

**The verdict survives the map correction.** It is not an artifact of a cramped
evaluator.

### The mechanism: it wins everything except the game

At the deployment shape the rush leads on **every** development statistic and
still loses two seats in three:

| | `advanced_rush` | `advanced` |
|---|---|---|
| terminal score | **506.3** | 456.8 |
| cities / pop / tech | **5.37 / 64.5 / 41.4** | 4.40 / 53.0 / 37.5 |
| military | **720.5** | 597.7 |
| **religious victories** | **2** | **43** |
| domination victories | **0** | **0** |

Two facts decide it:

1. **Religion is this engine's win condition**, and war destroys it — 43 → 2.
2. **Domination converts zero at deployment scale, for both agents**, even
   with twelve eliminations across twenty-four games. Conquest buys territory,
   score, population and tech. It does not buy the win.

> A bigger, richer, more advanced, more warlike empire that wins a third as
> often. Terminal score is at parity-to-favourable (52.8%, p=1.0000) while wins
> are 3-to-21 against — a clean demonstration, on a fresh axis, of this
> repository's standing law that **score is not win probability**.

This independently reproduces `docs/GENOME.md`'s league-genome result, which
lost 98 Elo by routing to domination while domination converted 0%. **Two
unrelated interventions, same mechanism.**

**So this ships as an eval-only entrant and a document, not as a default.**
`advanced` behaviour is unchanged: every path is behind `early_rush`.

## 6. ⚠ Measured and rejected

**Letting a rush ignore `relieving` and `Muster`.** The theory was that a stack
sized against one undefended capital should never stand still. Over the same 12
maps it made things **worse**: captures fell 9/12 → 6/12 and the median first
capture slipped turn 79 → 96. The two standing-still postures are load-bearing
even for a rush. Do not retry without a different mechanism.

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
