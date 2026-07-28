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

Two warriors is enough on paper and four is the honest number, because the
lane declares three tiles out and cannot rule out the victim recalling its
field army. That is `RUSH_STACK`.

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
88% of seats and refuses the marches that cannot arrive before the window
shuts.

**The army is ~160 production. The march is 9–12 turns.** Any design effort
spent making the stack cheaper is spent on the wrong term.

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

## 5. The lane, and what it actually moved

`advanced_rush` keeps every gate that is about the war and waives the two that
are calendar. It picks the nearest major whose capital is unwalled and within
`RUSH_REACH`, aims at the **capital** specifically, raises the standing-army
floor to `RUSH_STACK` melee, and declares once that many takers are staged.

| | stock | `advanced_rush` |
|---|---|---|
| melee units at turn 50 | 2.5 | **3.7** |
| majors at war at turn 40 | 0% | **11%** |
| majors at war at turn 50 | 0% | **33%** |
| blows on cities by turn 60 | **0.0** | **4.6** (126 HP) |
| maps with a capture | 8/12 | 9/12 |
| maps with an elimination | 0/12 | 1/12 (turn 87) |

> **The lane fires, marches, declares and attacks. It does not yet kill.**

---

## 5a. ★★★ THE VERDICT ON WINS — early aggression is significantly harmful

`ai_eval advanced_rush advanced --players 6 --pairs 60 --turns 500`, twice, at
seed 3100000. The second run includes the `domain_objective` and mid-war
persistence fixes below:

| | run 1 | run 2 (with fixes) |
|---|---|---|
| `advanced_rush` seat wins | **8.6%** | **9.4%** |
| `advanced` seat wins | **24.7%** | **23.9%** |
| paired map directions | 0 for / 29 against | 0 for / 26 against |
| sign test | **p=0.0000** | **p=0.0000** |
| anytime evidence crossed | map 20 | map 23 |
| gate | RETAIN `advanced` | RETAIN `advanced` |

**Zero map directions in favour, across 120 paired mirrored maps.**

### The mechanism is lane diversion, not lost wars

The rush ends up *ahead* on every development statistic and loses anyway:

| | `advanced_rush` | `advanced` |
|---|---|---|
| terminal score | 110.1 | 108.0 |
| cities / pop / tech | 1.73 / 11.3 / 9.2 | 1.61 / 10.1 / 9.2 |
| military | 168.8 | 148.9 |
| **religious victories** | **25** | **79** |
| domination victories | 6 | 3 |

> **80 of `advanced`'s 89 wins are religious.** The rush trades a religious
> victory for a domination attempt, and domination is the lane this engine
> converts worst.

This independently reproduces `docs/GENOME.md`'s league-genome result, which
lost 98 Elo with the same diagnostic — routing to domination far more often
while domination converted 0%. Two unrelated interventions, same mechanism.

**The cost is paid up front and the payoff never arrives.** The rush buys all
of aggression's expense — diverted production, diverted civics, war weariness,
a lane it cannot finish — while eliminating a rival in only 2 of 12 games, and
never before turn 87. A rush that actually killed by turn 50 might price out
differently; this one does not get to find out.

### ★ The remaining gap: the stack never closes the last three tiles

The decisive diagnostic. Melee standing **adjacent** to a rival capital, per
civilization:

| turn | staged (≤5 tiles) | **adjacent (≤1 tile)** |
|---|---|---|
| 40 | 1.00 | **0.03** |
| 50 | 1.00 | **0.03** |
| 60 | 1.04 | **0.01** |

The column marches to the staging ring at 3–5 tiles, declares, and stops. Two
causes were found and fixed, and **adjacency did not move for either**:

1. **`domain_objective` ranked `threatened_city` above `target_city`**, and
   `threatened_city` is empire-global — so the turn the victim's counter-raid
   pressured any city of ours, the whole column re-aimed homeward. The rush now
   keeps its objective. (blows 4.6 → 5.6, first capture median 79 → 62)
2. **The lane switched itself off mid-war.** `early_rush_victim` filters on
   `military_power(victim) <= my_power * 1.15 + 5`, which is a test for
   *opening* a war; once the victim mobilised it failed, dropping the army
   floor and handing the objective back. Now waived while already at war.
   (blows 5.6 → 6.1, 156 → 192 HP — but captures 7/12 → 6/12, i.e. **noise at
   12 maps**; this change is kept on mechanism, not on evidence.)

192 HP by turn 60 is now within sight of the 200 a capital holds — but spread
across a whole game, against 20 HP/turn of regeneration, and delivered from
three tiles away.

> **A third cause remains unidentified.** Something keeps melee off the ring
> tiles even with the objective set to the city and the campaign live. Until
> adjacency moves off 0.03, no amount of extra army or damage will convert:
> only a melee unit standing on a ring tile can seal the siege or land the
> capturing blow.

**Do not spend further effort on stack size, unit choice, or tech order.** The
Monte Carlo already says four warriors suffice; the measurement says they never
arrive.

---

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
