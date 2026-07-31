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
before the next is considered. That commitment rule costs three things:

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

## 3. What was built

`src/ai/tactics.rs` — a bounded **Portfolio Online Evolution** over the
engagement, behind `AdvancedAi::joint_tactics`, reachable as the
`advanced_joint_tactics` entrant.

- **Portfolio.** Each engaged unit gets a short list of candidate *lines* — an
  attack it can make from where it stands, or the empty line that declines.
  Generated geometrically, so candidate generation costs no clones, and pruned
  to the best few by a closed-form damage prior.
- **Genome.** A line per unit plus a permutation giving the play order.
- **Fitness.** Clone once, play the whole turn, score the resulting position:
  material swing across the whole board, minus the **marginal** change in the
  enemy's best answer, minus the same `attack_threshold` toll the shipped agent
  charges itself. Pricing the reply once against the finished turn is the direct
  repair for defect 1.
- **Seeding.** Sequential greedy — the shipped construction — restarted from
  several orders, which is Portfolio Greedy Search's own remedy for
  order-dependence.

Two design rules earned by measurement, both recorded because getting them wrong
cost real iterations:

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

## 4. Approach moves: built, measured, not shipped

The obvious fourth defect — movement being blind to the attacks it enables —
turns out not to be reachable this way. Adding "step onto a tile, then take the
attack that step opens" to the portfolio, at 400 matched scenarios per cell:

| approach moves | melee-only army | combined arms |
|---|---|---|
| none (**shipped**) | **+12.8** | **+16.1** |
| only if the step does not thin the line | −44.7 | +119.0 |
| unrestricted | −228.9 | +178.8 |

Paired material swing per scenario against the stock agent; positive is better.

Approach moves are worth a great deal with ranged support and **ruinous without
it**. A melee unit that leaves the line to land a blow is answered by the whole
enemy front, and this module cannot see that: its evaluation prices material and
a one-step damage estimate and has **no positional term at all**. The
formation-preserving filter recovers two thirds of the loss and still does not
clear zero.

So the module does what it can evaluate — which attacks to make, and in what
order — and leaves where to stand to the tuned movement layer. **Re-opening this
needs a positional evaluation first, not a wider portfolio.**

This is also a caution about the earlier, larger-looking numbers: an unrestricted
portfolio measured +178.8 on combined arms, roughly eleven times the shipped
variant. That number was real and it was not a gain worth taking, because the
same mechanism produced −228.9 one composition over.

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
seats swapped. `advanced_joint_tactics` against `advanced`.

| cell | seeds | exchange ratio (ours vs theirs) | paired material swing | sign p |
|---|---|---|---|---|
| combined arms, block 900k | 1500 | **1.094** vs 0.914 | **+29.9 ± 4.1** | <0.0001 |
| combined arms, block 5M (out of sample) | 1500 | **1.087** vs 0.920 | **+28.2 ± 4.4** | <0.0001 |
| melee only, block 2M | 1500 | **1.045** vs 0.957 | **+7.4 ± 2.0** | 0.0158 |
| melee only, block 7M (out of sample) | 1500 | **1.023** vs 0.978 | **+5.6 ± 1.8** | 0.0116 |
| control (`advanced` vs `advanced`) | 60 | 1.000 vs 1.000 | 0.00 ± 0.00 | 1.0000 |

The effect replicates on disjoint seed blocks it was not chosen on. Both
out-of-sample point estimates are *smaller* than their in-sample counterparts
(+28.2 against +29.9; +5.6 against +7.4), which is what
`civvis-effect-sizes-are-winners-curse` predicts and a reason to quote the
out-of-sample figures.

### Cost

`battle_bench --cost`, 6 players, 74×46, 9 city-states, 250 turns, interleaved
so both fleets meet the same contention:

| fleet | ms a game-turn | ratio |
|---|---|---|
| all `advanced` | 51.68 | 1× |
| **one joint-tactics seat among five** | **53.82** | **1.04×** |

Measured in the configuration that would ship — one seat among five — for the
reason `docs/EVAL.md` records: measuring only an all-treated fleet once read 29×
for something that costs 6.4× seated. For scale, a `StrategicAi` seat is 6.4×;
this is 1.04×.

### Fires-check at deployment scale

Over 965 treated seat-turns in full six-player games, the search planned on **91
turns (9.4%)**, reaching **277 unit decisions**. It is not a no-op in
deployment, so the whole-game result below is not "the layer never ran".

⚠ Getting this number required fixing a real instrumentation bug first: the
blanket `impl<T: Ai + ?Sized> Ai for Box<T>` in `src/ai.rs` forwards each trait
method explicitly, and a newly added one silently fell back to the default
`None`. The first reading was a confident **0 fires**, contradicted by the run
being 1.99× slower than its control. **A census that reports zero while the
clock says otherwise is a broken census, not a null.**

### Whole game — INCONCLUSIVE

`ai_eval`, deployment profile: 6 players, 74×46, 9 city-states, 250 turns,
mirrored maps with seats swapped.

| maps | paired score | Elo-equivalent | sign p | verdict |
|---|---|---|---|---|
| 120 | 47.5% (38.8–56.4) | −17 (−79..+45) | 0.3915 | INCONCLUSIVE |
| **300** | **51.3% (45.7–56.9)** | **+9 (−30..+49)** | 0.4657 | **INCONCLUSIVE** |

Only 92 of 300 maps broke at all. The confidence interval admits everything from
a small loss to a small gain; this does not show the change helps, and it does
not show it hurts.

The terminal-score diagnostic — explicitly *not* a promotion input — favours the
treatment in both runs, 67/53 at 120 maps and **173/127 at 300 (p=0.0093)**.
Consistent direction, but wins are what the gate reads.

## 7. What this means, stated plainly

- **The change does what it claims.** It fights better, significantly and
  reproducibly, on disjoint seeds and both army compositions, at 1.04× cost.
- **That does not convert into measured wins.** At 300 maps the whole-game
  result is inconclusive.
- **That is the expected outcome, not a surprise.** `docs/ORACLE.md` already
  bounded military capability as not being on this simulator's critical path:
  `taker`, `modernity` and `attrition` — free positioning, free unit quality,
  free healing — all measured null, while `treasury` went 62–0. A tactical layer
  cannot beat a grant that makes the same subsystem simply not fail.

So it ships **off by default**, registered as `advanced_joint_tactics`, with the
null recorded — the same disposition as `advanced_relief_scoped` and
`advanced_lane_reachable`. It is there to be re-measured when the constraint
moves, rather than re-derived from scratch.

**If someone picks this up**, the ranked list is:

1. **Do not spend more on tactical quality expecting wins.** The oracle bounds
   it and this result is consistent with that bound. The battle benchmark can
   keep improving while the win rate does not.
2. **A positional evaluation is the one thing that would unlock the rest.**
   Approach moves are worth +178.8 on combined arms and −228.9 on melee, and the
   difference is entirely "is this a good tile to stand on", which nothing in
   this module can currently answer. That is a well-posed, self-contained
   problem with a measured payoff on both sides of it.
3. **`battle_bench` is reusable for any tactical change**, at roughly 800× the
   events per unit of compute that whole games give. Run the control first,
   every time.
