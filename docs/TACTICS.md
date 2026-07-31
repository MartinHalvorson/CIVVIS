# Unit battling: the state of the art, what CIVVIS does, and what changed

2026-07-31. Companion to `src/ai/tactics.rs`, `src/skirmish.rs` and
`src/bin/battle_bench.rs`.

## 1. What the published state of the art is for this problem

A Civ turn is a **multi-action turn**: one player moves every unit before the
opponent moves any. If a side has `n` units with `b` choices each, the branching
factor for a single turn is `b^n`, not `b`. At six units and six options that is
already 46,656 turns to consider, and it grows exponentially in army size. This
is why ordinary game-tree search does badly here: alpha-beta and vanilla MCTS
spend their whole budget failing to get past the root.

The literature that owns this exact shape is small and consistent:

- **Portfolio Greedy Search** — Churchill & Buro, *Portfolio Greedy Search and
  Simulation for Large-Scale Combat in StarCraft*, CIG 2013. Do not search over
  actions; search over **assignments of scripts to units**, and hill-climb the
  joint assignment one unit at a time, re-evaluating with a playout. Scales to
  50-vs-50 fights where alpha-beta cannot start.
- **Online Evolutionary Planning** — Justesen, Mahlmann & Togelius, *Online
  Evolution for Multi-Action Adversarial Games*, EvoApplications 2016. Treat the
  whole turn as a **genome** — one action per unit — and evolve it against a
  state evaluation function. Evaluated on *Hero Academy*, a turn-based
  multi-action tactics game structurally identical to a Civ turn, this beat both
  MCTS and greedy search **by a large margin**. This is the closest published
  analogue to our problem that exists.
- **Portfolio Online Evolution** — Wang et al., AIIDE 2016. The combination:
  evolve over the space of script assignments rather than raw actions.
- Adjacent: **NaiveMCTS / combinatorial multi-armed bandits** (Ontañón) for
  multi-action move generation, and **Puppet Search** (Barriga, Stanescu & Buro)
  for searching over a handful of scripted choices instead of raw actions.

For comparison, the strongest *shipped* commercial approach in this genre —
Civilization V's `CvTacticalAI` — is not a search at all. It partitions the map
into **dominance zones**, assigns each zone a **posture**, and then runs an
ordered list of scripted **tactical moves** with prioritised target lists. It is
a coordination layer over heuristics, and its strength comes from deciding
things *per zone* rather than *per unit*.

The through-line of all of it: **the win is in deciding the units together, not
in evaluating any one of them better.**

## 2. What CIVVIS already had

Worth stating plainly, because it is better than a reading of `src/ai.rs` would
suggest, and because the real agent is in `src/ai/advanced.rs`:

For each candidate attack, `AdvancedAi` clones the game, applies the action
through the **real engine**, and reads the exact result
(`tactical_attack_value_owned`). It then extends the line with
`forcing_reply_penalty_owned`, a bounded **quiescence-style reply search** that
refreshes the enemy's forcing actions and prices a two-action focus-fire answer,
including one-step approach moves. Candidate evaluation is parallelised across a
persistent `WorkPool`, with only owned clones crossing the thread boundary.

As a **single-unit** evaluator that is genuinely close to the state of the art.
It is not a weak heuristic and it should not be described as one.

The weakness is one level up. The turn is assembled by
`advanced_units` walking the units in a **fixed class order** — ranged, then
siege, then melee — and letting each one commit **greedily and irreversibly**
before the next is considered. That commitment rule costs four things:

1. **The reply term is biased by position in the order.** Each unit prices the
   enemy's answer against a board where its own teammates have not moved yet.
   Ranged units act first and over-price their exposure — the screen that will
   stand in front of them does not exist yet — and melee acts last and
   under-prices it. The bias runs in exactly the wrong direction.
2. **No joint target assignment.** With four attackers and three defenders,
   every attacker picking its individually-best target is not the best *set* of
   attacks. The `focus_fire` gene is a flat bonus toward one shared tile, which
   trades spread-fire for overkill rather than solving either.
3. **The order is never questioned.** Soften-then-capture is usually right,
   which is why a fixed order works as well as it does, but it is wrong whenever
   a melee kill has to clear a tile or a firing lane first.
4. **Movement is blind to the attacks it enables.** A tile is scored on
   depth-to-target, adjacent support and incoming threat; nothing scores it for
   the shot it opens. This is the largest of the four by a wide margin — see §4.

## 3. What was built

`src/ai/tactics.rs` — a bounded **Portfolio Online Evolution** over the
engagement, behind `AdvancedAi::joint_tactics`, reachable as the
`advanced_joint_tactics` entrant.

- **Portfolio.** Each engaged unit gets a short list of candidate *lines* — an
  attack from where it stands, a step onto an adjacent tile followed by the
  attack that step opens, or the empty line that declines. Generated
  geometrically, so candidate generation costs no clones, and pruned to the best
  few by a closed-form damage prior.
- **Genome.** A line per unit plus a permutation giving the play order.
- **Fitness.** Clone once, play the whole turn, score the resulting position:
  material swing across the whole board, minus the **marginal** change in the
  enemy's best answer, minus the same `attack_threshold` toll the shipped agent
  charges itself, minus `FORTIFICATION_FORFEIT` for each unit the plan moved.
  Pricing the reply once against the finished turn is the direct repair for
  defect 1; the forfeit term is what makes defect 4 reachable.
- **Seeding.** Sequential greedy — the shipped construction — restarted from
  several orders, which is Portfolio Greedy Search's own remedy for
  order-dependence.

Three design rules earned by measurement, all recorded because getting them
wrong cost real iterations:

**Charge the shipped attack toll.** A first version dropped
`attack_threshold`, which made the treatment "plan jointly *and* be far more
aggressive". That conflates two changes, and the aggression alone is
composition-dependent. The search should change *which* attacks are made, not
how cheaply the agent is willing to be hit.

**Seed the real incumbent.** A first version seeded a *static* assignment —
every unit choosing against the untouched board — and called it greedy. It is
not: the shipped rule is sequential and adaptive, each unit choosing knowing
what its teammates already did. Over 400 melee scenarios the static seed spread
damage where the sequential rule concentrates it, and **lost 700 kills on
identical total damage** (2306 vs 3009 kills at 456,200 vs 456,220 damage). A
search that starts behind the incumbent cannot climb back on a small budget.

## 4. Approach moves, and the one term that made them work

Letting a unit **step onto a tile and then take the attack that step opens** is
where most of the value in this whole layer turned out to be. It is also
*actively harmful* until the evaluation can price what the step gave up.
Measured on `battle_bench`, paired material swing per scenario against the stock
agent:

| portfolio | melee-only | combined arms |
|---|---|---|
| attacks only, no stepping | +5.6 | +28.2 |
| stepping, no forfeit term | **−228.9** | +178.8 |
| stepping, only if the step does not thin the line | −44.7 | +119.0 |
| **stepping + fortification forfeit (shipped)** | **+21.4** | **+299.2** |

**The obvious diagnosis for that −228.9 is wrong, and it cost two attempts to
find out.** The natural story is that the unit broke formation, so the whole
enemy front answered it. An explicit adjacent-friendly-support term was built
and swept over weights 0/10/20/25/50 against three compositions on disjoint
seeds — and **could not be distinguished from zero at any weight**, with
combined arms in fact *highest* with it switched off entirely. The formation
hypothesis is refuted, not merely unhelpful; it is not worth re-attempting.

What the evaluation was actually missing is much simpler: **a unit that holds
its ground can dig in next turn, and one that stepped forward cannot.**
Fortification is a *future* action, so it is invisible to every term that prices
the position as it stands — material, damage, reply. Charging a flat
`FORTIFICATION_FORFEIT = 40` per unit that moved is one subtraction, and it
moves combined arms from +28.2 to +241.3 while taking melee-only from −228.9 to
neutral.

The weight is a plateau, not a knife edge. Swept out-of-sample at 700 seeds a
cell:

| forfeit weight | melee-only | combined arms |
|---|---|---|
| 20 | −19.5 | +228.1 |
| 30 | +0.5 | +219.4 |
| **40 (shipped)** | **+14.2** | **+223.3** |
| 50 | +15.4 | +195.7 |

Combined arms is flat across the whole range; melee crosses zero near 30. 40
sits comfortably inside both.

## 5. The instrument

Whole-game win rate is the right acceptance test and the wrong measuring device
here. `docs/ORACLE.md` puts a 100-cell paired run's resolving power at roughly a
70/30 split, and a six-player game produces on the order of fourteen unit kills.
A change that fights meaningfully better can pass through it without a trace.

`src/skirmish.rs` + `src/bin/battle_bench.rs` measure combat directly: two
identical armies, the same map, **the seats swapped**, and a count of what each
agent destroyed and lost. The swap makes terrain and seat advantage cancel
exactly within the pair.

**The control is the load-bearing part.** `--a advanced --b advanced` reports a
paired mean of **exactly** 0.00 with 0 of 60 seeds diverging. Nothing this
harness reports is its own noise. The report also carries a **fires-check** —
seeds on which the two agents' play diverged at all — because a null from a
treatment that never fired says nothing about the game.

## 6. Results

### Combat, where the change acts

`battle_bench`, 24 turns, 28×20, armies of six, each seed played twice with
seats swapped. `advanced_joint_tactics` against `advanced`, **2000 seeds a
cell, on seed blocks fresh to the final configuration** — disjoint from every
block used to choose the forfeit weight or the search budget.

| composition | exchange ratio (ours vs theirs) | paired material swing | sign p |
|---|---|---|---|
| combined arms | **1.792** vs 0.558 | **+299.2 ± 7.4** | <0.0001 |
| ranged heavy | **2.041** vs 0.490 | **+271.4 ± 7.5** | <0.0001 |
| with siege | **1.259** vs 0.795 | **+121.2 ± 7.6** | <0.0001 |
| melee only | **1.023** vs 0.978 | **+21.4 ± 4.5** | 0.0035 |
| control (`advanced` vs `advanced`) | 1.000 vs 1.000 | 0.00 ± 0.00 | 1.0000 |

**Positive and significant on all four compositions**, including the melee-only
stress case that the intermediate versions lost outright. The size tracks how
much the composition rewards positioning: an all-ranged line more than doubles
its exchange ratio, a melee scrum barely moves.

For scale against the first version of this work, which planned only attacks
already available and did not step: combined arms **+28.2 → +299.2**, melee
**+5.6 → +21.4**.

⚠ **The sign test in this harness had an overflow bug** that reported
`p = 1.0000` on a 1122-to-317 split. `2^n` is `inf` past n≈1023, the binomial
coefficients overflow with it, `inf / inf` is NaN, and Rust's `NaN.min(1.0)`
returns **1.0** — a perfectly confident null on overwhelming evidence. Large n
now uses a normal approximation. Every number above was recomputed after the fix.

### Search budget is binding

Unlike the macro search, where doubling the budget is the one reproducible win
and quadrupling does nothing, combat quality here keeps climbing across the
whole range measured (700 scenarios a cell):

| pop / gen / lines | combined arms | melee | seated cost |
|---|---|---|---|
| 12 / 6 / 6 | +236.9 | +6.2 | 1.13× |
| **20 / 10 / 10 (shipped)** | **+279.1** | **+17.6** | **1.29×** |
| 32 / 20 / 16 | +300.0 | +17.6 | 2.19× |

20/10/10 is the knee — most of the available gain at a cost still far under a
searching macro seat. Raising it further is a live option if the compute is
ever worth spending, which on the whole-game evidence below it currently is not.

### Cost

`battle_bench --cost`, 6 players, 74×46, 9 city-states, 250 turns, interleaved
so both fleets meet the same contention:

| fleet | ms a game-turn | ratio |
|---|---|---|
| all `advanced` | 29.75 | 1× |
| **one joint-tactics seat among five** | **43.65** | **1.47×** |

Measured in the configuration that would ship — one seat among five — for the
reason `docs/EVAL.md` records: measuring only an all-treated fleet once read 29×
for something that costs 6.4× seated. For scale, a `StrategicAi` seat is 6.4×;
this is 1.47×.

### Fires-check at deployment scale

Over 949 treated seat-turns in full six-player games, the search planned on
**243 turns (25.6%)**, reaching **885 unit decisions**. (The attack-only variant
managed 9.4% and 277; admitting approach moves nearly tripled the layer's
footprint, because a unit no longer has to already be in contact for the search
to have anything to decide.) It is emphatically not a no-op in deployment.

⚠ Getting this number required fixing a real instrumentation bug first: the
blanket `impl<T: Ai + ?Sized> Ai for Box<T>` in `src/ai.rs` forwards each trait
method explicitly, and a newly added one silently fell back to the default
`None`. The first reading was a confident **0 fires**, contradicted by the run
being 1.99× slower than its control. **A census that reports zero while the
clock says otherwise is a broken census, not a null.**

### Whole game — INCONCLUSIVE

`ai_eval`, deployment profile: 6 players, 74×46, 9 city-states, 250 turns,
mirrored maps with seats swapped.

| version | maps | paired score | Elo-equivalent | sign p | verdict |
|---|---|---|---|---|---|
| attacks only | 120 | 47.5% (38.8–56.4) | −17 (−79..+45) | 0.3915 | INCONCLUSIVE |
| attacks only | 300 | 51.3% (45.7–56.9) | +9 (−30..+49) | 0.4657 | INCONCLUSIVE |
| **+ approach moves** | **300** | **51.3% (45.7–56.9)** | **+9 (−30..+49)** | 0.4841 | **INCONCLUSIVE** |

**An 8.5× larger combat effect produced no change in win rate at all** — the
same 51.3% and the same +9 Elo. That is the single most informative number in
this document, and it is a much stronger version of the oracle's finding than a
single null: the tactical dial was turned an order of magnitude and the win rate
did not notice.

Only 100 of 300 maps broke at all. The confidence interval admits everything
from a small loss to a small gain; this does not show the change helps, and it
does not show it hurts.

The terminal-score diagnostic — explicitly *not* a promotion input — favours the
treatment in every run: 67/53, then 173/127 (p=0.0093), then 165/135 (p=0.0939).
Consistent direction, but wins are what the gate reads.

## 7. What this means, stated plainly

- **The change does what it claims, decisively.** It fights better on all four
  army compositions, significantly, on seed blocks fresh to the final
  configuration, at 1.47× the cost of a stock seat among five. Combined arms
  nearly doubles its exchange ratio, 1.792 against 0.558.
- **It does not convert into measured wins, and the failure is emphatic.** Two
  300-map runs at 6p/74×46 both land on 51.3% and Elo +9. The second had **8.5×
  the combat effect of the first** and moved the win rate not at all.
- **That is the expected outcome.** `docs/ORACLE.md` already bounded military
  capability as off this simulator's critical path: `taker`, `modernity` and
  `attrition` — free positioning, free unit quality, free healing — all null,
  while `treasury` went 62–0. What this adds is a *dose–response* check on that
  bound, which a single null cannot give: turning the tactical dial an order of
  magnitude produced no response at all.

So it ships **off by default**, registered as `advanced_joint_tactics`, with the
null recorded — the same disposition as `advanced_relief_scoped` and
`advanced_lane_reachable`. It is there to be re-measured when the constraint
moves, rather than re-derived from scratch.

**If someone picks this up**, the ranked list is:

1. **Do not spend more on tactical quality expecting wins.** This is now the
   strongest evidence on record for that: an 8.5× swing in measured combat
   quality, twice, at 300 maps each, with no movement in win rate. The battle
   benchmark can keep improving while the win rate does not, and the search
   budget table shows it *would* keep improving if compute were spent on it.
2. **The interesting question is why fighting better does not win.** Wars are
   rare (~1.4 a game) and take almost nothing (`civvis-wars-take-nothing`), so a
   layer that fights 80% better on the turns it fires may simply not fire on
   enough of the game to matter. Whether the constraint is *how well the army
   fights* or *how often and to what end it is used* is now the well-posed
   question, and the second half of it is untouched by this work.
3. **`battle_bench` is reusable for any tactical change**, at roughly three
   orders of magnitude more combat events per unit of compute than whole games.
   Run the control first, every time, and sweep at least a ranged-heavy and a
   melee-only composition — this work had a treatment that measured +178.8 on
   one and −228.9 on the other.
4. **The formation hypothesis is refuted, not open.** Do not re-attempt an
   adjacent-support term for approach moves; it was swept and is
   indistinguishable from zero. The term that mattered was forfeited
   fortification, and the general shape of that lesson — *the missing term was a
   future option the unit gave up, not a property of the position it reached* —
   is the one worth carrying to the next evaluator.
