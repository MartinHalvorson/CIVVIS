# What the search must become

`docs/AI_GAPS.md` names ten gaps and ranks them. `docs/EVAL.md` records what
happened when they were attacked. This file is the third thing: given
everything both of those now say, **what would a much stronger designer
actually build, and in what order.**

It is written to be falsifiable. Every mechanism below states the evidence it
rests on, the check that proves it is not inert, and what would refute it.

---

## 0. The one fact that organises everything

Sort every AI change this repository has measured into two piles.

**Won maps at adequate power — all of them are policy rollouts:**

| change | result |
|---|---|
| `strategic_r20` (2× reviews) | 15 map directions to 2, p=0.0023 |
| `strategic_h80` (2× horizon) | 21 to 5, p=0.0025 |
| `strategic_deep` (4×, promoted) | 56 to 17 over a pre-registered 300 maps |

**Measured null or negative — all of them consume a learned state value, or
change what the argmax ranges over:**

| change | result |
|---|---|
| `policy` tactical net | gain exactly 0.0 on 96.4% of candidates |
| `policy_wide` (representation fixed) | **Elo −313**, 1 map to 87 |
| `strategic` value-net blend | 20/20 identical maps, 127 identical reviews |
| `Doctrine` second axis | 0 switches in 16 reviews |
| `production_net` | 109/240 against 108/240 |
| `rotate_lanes` | null |

The pattern is not "learning is hard here". It is sharper than that:

> **A state-value function tells you how good a position is and says nothing
> about which action caused it. A rollout is counterfactual; a regression on
> outcomes is not.**

`policy_wide` is the proof. Fixing its blindness (action visibility 44.5% →
86.1%) made it *catastrophically worse*, and freezing exactly two features —
the contact terms, indices 29/30 — recovered it from −313 Elo to exactly 0.
Those two features are correlational: adjacency is high in won games because
strong empires press attacks. An argmax over sibling actions found them and
maximised them, marching units into fights they lost.

**The design rule that follows.** In any feature set consumed by an argmax
over actions, every feature must be one you would be content for the agent to
maximise. Causal terms (material, HP, fortification, city fabric) qualify.
Correlational ones do not. Visibility needed both kinds; ranking survives only
the first.

Everything below is an application of that rule.

---

## 1. The structural diagnosis

The macro search spends its entire budget on the **lowest-entropy variable in
the game.**

One review clones the whole `Game` seven times — the adaptive baseline plus
each enabled lane — and projects each forty rounds with every seat playing.
That is roughly 280 branch-rounds of full simulation, and it answers a
**seven-way categorical question**, asked about 1.5 to 10 times per seat per
game.

Meanwhile every decision that repeats — what each city builds, where each unit
moves, what to research — is a hand-written heuristic evaluated greedily.
There are between 10⁴ and 10⁵ of those in a game.

So the search is both **under-provisioned** (doubling it always wins) and
**aimed at the wrong variable**. Those are separable problems and they have
separable fixes.

Two further constraints bound any fix:

- **A shallow estimate is not rank-preserving with respect to the deep one.**
  This is the strongest constraint on the list, and three separate runs are
  three faces of it. `strategic_h80` doubles the depth of *every* branch and
  wins 21 map directions to 5. Stopping as soon as the branches separate
  (`adaptive_horizon`) scores 39.2%, Elo −76, sign p=0.0000. Concentrating the
  same budget on the branches that look best after a short chunk
  (`focused_deepening`) scores 49.2%, p=0.8318 — exactly nothing. A lane behind
  the adaptive baseline at depth 12 can be the best lane at depth 84, so *any*
  within-review pruning discards real signal.

  **This retires a whole family of ideas as predicted-null**: rotation,
  adaptive stopping, focused deepening, progressive widening, sequential
  halving — every scheme that re-allocates a fixed review budget using an
  early ranking. The only lever on this search that has ever worked is raising
  the total.
- **The horizon saturates.** A branch that reaches a decided game returns
  exactly 1.0 or 0.0, so once every branch resolves they agree *by
  construction*. Share of reviews in that state: 22% at horizon 40, 56% at 80,
  89% at 120. Pushing horizon past 80 buys agreement, not discrimination.
- **Half of reviews never reach the rollouts.** Three priors answer first, and
  `urgent_counter` alone takes a third of them. In a duel the religious prior
  answers *all* of them, which is why every published duel number for
  `strategic` measures a forced lane rather than search.

---

## 2. Mechanisms

Ordered by expected value per unit of risk. Each is stated as a mechanism, the
evidence it rests on, its fires-check, and what would refute it.

### M1 — Scale the commitment threshold to the candidate set

**Mechanism.** `choose_rollout_target` commits when the best lane clears the
adaptive baseline by a constant `TARGET_COMMITMENT_MARGIN = 0.01`. Replace the
constant with a threshold derived from the review's own observed dispersion —
`α · (max − min)` over the projected branches, floored — so it is scale-free.

**Why.** A review is a *maximum over the surviving lanes*, and a maximum over
one draw clears a fixed margin far less often than a maximum over six. So the
constant is not a neutral parameter: it encodes an assumption about how many
branches are being compared, and any change to the candidate set silently
moves the effective decision threshold.

This was measured, expensively. A version of focused deepening that pruned to
one lane by rank scored 46.2% over 120 mirrored maps while **terminal score was
a dead heat** (49.4%, 56 map directions to 57, p=1.0000) — it built an equally
good empire and stopped committing: 58.4% of player-turns uncommitted against
the control's 44.9%, religious commitment 24.5% against 29.0%, dominant-plan
religious seats 70 against 127. Religious is the lane that converts, at 29.9%
of its seats.

It also supplies a mechanism for a null nobody had explained: `rotate_lanes`
cuts seven branches to about three through the same argmax, and would have
inherited exactly this bias.

**A second reason to suspect the constant, and its refutation — read both.**
The repository records a median branch spread of 0.0045 at horizon 40, less
than half the margin, which reads as a search that cannot clear its own
threshold. Measured directly over real mid-game reviews at the full horizon,
calling `rollout()` the way the search does, it is not:

| config | median spread | max | share above the 0.01 margin |
|---|---|---|---|
| 4p 24×16, cold branches | 0.0311 | 0.73 | 61% |
| 4p 24×16, warm branches | 0.0622 | 0.75 | 78% |
| 3p 20×14, cold branches | 0.0491 | 0.62 | 94% |
| 3p 20×14, warm branches | 0.0850 | 0.72 | 94% |

Both numbers are right about different populations. A branch that reaches a
decided game returns exactly 1.0 or 0.0, and 22% of reviews are in that state
at horizon 40, so the 0.0045 figure — whose reported *maximum*, 0.0145, is
below the median above — describes the **undecided** subset. Over undecided
reviews the gate is nearly unreachable; over all reviews it is cleared 61–94%
of the time.

So the gate binds on a minority, and moving it does a minority-sized thing.
`strategic_m002` measured exactly that: uncommitted player-turns 47.3% → 41.9%,
switches 2.33 → 2.47, paired score flat at 50.0% over 60 maps. **M1 is
therefore demoted** — the doctrine axis needed its margin lowered because its
spread really was 0.0044 across the *whole* population, and the lane axis is
not in that position.

The part that survives is the coupling: a review is a maximum over the
surviving lanes, so any change to the candidate set moves the effective
threshold, whatever the spread is.

**Fires-check.** Commitment rate and lane distribution must not shift when the
candidate set changes; the plan-commitment table is the instrument, not the win
rate.

**Refuted by.** A margin sweep in which commitment rate moves but paired score
does not. That would say routing is not the binding constraint, which is a
result worth having.

**Note the scope.** The pruning that exposed this is retired (see the
rank-preservation constraint above) — but the margin defect it exposed is not.
It applies to `rotate_lanes`, to any ruleset that disables victory conditions
and so shrinks the enabled-lane set, and to a duel, where the enabled set is
the same but three priors answer first. The threshold moving with the
candidate count is a standing property of the decision rule, independent of
whatever produced the count.

### M2 — Make the counterfactual an exact simulation of the decision ★ LANDED

> **Promoted 2026-07-26 and on by default.** Pre-registered 500 maps at seed
> 132000: 55.3%, Wilson 50.9%–59.6%, 87 map directions to 34, sign p=0.0000,
> e=6.6e4 crossing at map 209, Elo-equivalent **+37**, `promotion gate: PASS`.
> Three disjoint seed sets total 860 maps, 140–58, p=5.2e-09.
>
> **The measured mechanism is spread, not the force-group story argued below.**
> Projecting from the plan in force roughly *doubles* the spread between branch
> values — 0.031 → 0.062 at four players, 0.049 → 0.085 at three — so the
> counterfactual discriminates about twice as well between lanes. A cold
> branch's first act is to re-plan, which partially washes out the very
> difference the branch exists to measure. The lane shift went the opposite way
> from the force-group prediction: the promoted agent takes *fewer* domination
> seats (32 against 51), not more.

**Mechanism.** Project each branch from the planner **in force**, not from a
newly constructed one. Clone the agent and apply the branch's decision through
the same `retarget` / `adapt` / `reweight` calls the real agent makes after a
review.

**Why.** `AdvancedAi` carries state a forty-round rollout cannot re-derive: the
strategic plan, settler and builder assignments, `major_war_since`, a
`peace_until` cooldown, and the force groups that make a campaign coherent.
Handing every branch a fresh agent answers *"what happens if I restart my
planner and commit to this lane"* while standing in for *"what happens if I
commit to this lane from here."*

The distortion is not symmetric. A branch reaching for the army finds an empty
force-group table, which costs the militarised lanes most — and domination
already converts worst. A branch projected from an empire mid-campaign forgets
it is at war and forgets it is inside a peace cooldown, so it will re-declare a
war the real agent legally cannot.

**Cost.** Nothing. One clone of a small struct where there was one construction
of it.

**Fires-check.** Move only the flag, on one agent, in one position, and assert
the projected values differ.

### M3 — Model rivals as the empires they visibly are

**Mechanism.** Rivals are projected as blank-slate adaptive `AdvancedAi`.
Instead, infer each rival's lane from *public* state and seat it targeted.

**Why.** The counterfactual currently asks what happens against opponents who
have just forgotten their plans. That systematically understates how far along
a rival's victory line is, which is precisely the failure the `urgent_counter`
prior exists to patch — and that prior now eats a third of all reviews. A
search that could see the threat itself would need the prior less, and raising
the share of reviews that reach the rollouts is worth more than improving any
single review.

**The hard part, stated honestly.** `victory_progress` is 0 for every lane in
the early game, where most reviews land, so it cannot carry the inference
alone. It needs an early-game signal from district mix, religion, army size and
envoys — and that is a heuristic, which is the class of "clever" change that
has measured null twice here. Build it only with the fires-check below, and
expect it to be the riskiest item on this list.

**Fires-check.** The inferred lane must be non-adaptive for a substantial share
of rival-seatings by mid-game, *and* must change at least one branch value in
an ordinary position. Both, or it is inert.

### M4 — Search a decision that repeats, with a *sealed* rollout

**Mechanism.** Choose city production by projecting each candidate build — but
project **one city's** yields, queue and growth with the rest of the world held
fixed, for a horizon that outlasts the payoff (60–100 turns), and judge the
branch by that city's own discounted yield stream.

**Why.** This is the highest-frequency decision in the game and the one that
compounds most. `ProductionSearchAi` already attacked it and lost 9 map
directions to 21 — but read what it actually did: it cloned the **whole game**,
which forced a horizon capped at 40, and then judged a **city-level decision by
an empire-level terminal value** (score share). Substituting the trained net
changed nothing, because the net's inputs *are* the 25 empire aggregates that
score share summarises. Its own module note reaches the right conclusion:
"a production search worth building needs a terminal value that sees something
score share does not."

A sealed rollout supplies both missing halves at once. It is roughly four
orders of magnitude cheaper than a full-game clone, so it can afford a horizon
that outlasts a wonder; and it produces a **city-level** terminal value that is
causally downstream of the decision, which is exactly the "evaluator matched to
the decision" property that makes the lane search work.

**Fires-check.** Branch values must separate — if every candidate returns the
same number, the horizon still does not outlast the build and the search is
blind, which is the defect that killed `PolicyAi` on 96% of its candidates.

**Refuted by.** Separation that exists but does not survive contact with the
empire — a city optimised in isolation starving the empire of settlers or
military. Watch the plan-commitment and unit-count diagnostics, not the yield.

### M5 — Distil the search into a policy prior, never into a value

**Mechanism.** Log (position, the *action the search chose*) and train a
classifier to predict that choice. Use it to order and prune candidates, not to
replace the search.

**Why.** This is the one learned route the evidence permits. The target is the
search's counterfactual choice, not an outcome correlate, so it cannot be
farmed the way `policy_wide` farmed contact. It is also the only mechanism that
converts compute into *frequency*: a prior that ranks candidates well lets the
same budget cover more decisions.

**Fires-check.** The prior must reproduce the search's choice out of sample
well above the base rate, *and* pruning by it must leave the search's decision
unchanged on a held-out set of positions. The second check is the load-bearing
one.

### M6 — Generate data containing the bad version of the action

**Mechanism.** In a dedicated data-generation mode — never in the league — let
ε of reviews commit to a *randomly chosen* lane, play the game out, and label
it.

**Why.** Action-conditioned value learned from logged expert play inherits the
same confound as state value. Over 480 expert decisions: 25.2 move candidates
each, at most 3.96% can ever appear as a taken action, and the expert raises
contact on 36% of turns against a 15% base rate. The corrective examples —
the same action in *unfavourable* positions — are exactly the moves a competent
agent declines and therefore never logs. **A bigger expert corpus cannot fix
this.** Only exploration that takes the losing move and records the loss, or
search that plays the candidate out, produces them.

**Refuted by.** Nothing cheap. This is infrastructure; its payoff is M5's
training set.

### M7 — Hunt the simulator's seams before optimising harder against it

**Mechanism.** Run an agent whose objective is bare score-per-turn with no lane,
over several hundred games, and report which engine rules it used
disproportionately relative to the scripted fleet.

**Why.** A strong optimiser farms the simplifications. The risk is not that the
agent gets worse; it is that it gets *better at CIVVIS and no better at Civ 6*,
and nothing in an in-house Elo pool can tell the two apart. This is cheap
insurance that becomes mandatory the moment M5 works.

---

## 3. What not to build

Each of these is falsified here, with the run that did it.

- **A wider input for a value net consumed by an argmax.** Representation was
  fixed and the agent got worse: `policy_wide` scored 14.2% against `advanced`,
  1 map to 87. The blindness had been suppressing a bad policy, not hiding a
  good one.
- **Better calibration.** The net reached grouped-holdout BCE 0.4058 against a
  0.5636 constant baseline and changed no lane choice on 20 of 20 maps, with
  127 identical reviews on both sides.
- **More horizon.** 89% of reviews are saturated at 120.
- **Measured per-stage information as rating weights.** It scores below the
  constant it replaces: how *predictable* a stage is, is not how much evidence
  it carries.
- **Interrupts that postpone the periodic search.** Changed the review count by
  one, 95 against 96, and lost a map.
- **Any conclusion from a 20-map run.** Two conclusions from that size inverted
  at 120 maps *in opposite directions*. Minimum useful size is about 100 maps,
  which is one `--jobs 12` run.

---

## 4. Sequencing

**M2 (landed) → M4 → M6 → M5**, with M3 attempted only alongside its
fires-check and M7 landed before M5 ships anything. **M1 is demoted out of the
sequence** — measurement showed its premise held only for a minority of
reviews, and the treatment moved the commitment rate without moving the score.

Note what is *not* on the list: any further attempt to spend a review's budget
more cleverly. That was the obvious first idea, it was measured twice from
opposite directions, and both times it lost or drew. Depth has to be bought,
not re-allocated.

M2 first because it is free and strictly increases fidelity. M1 second because
it is a precondition for evaluating *any* later change to the candidate set —
without it, every such change is confounded by a moving decision threshold. M4
third because it is the first mechanism that attacks the frequency problem
rather than the depth problem. M6 and M5 are the learned route, in that order,
because M5's training set is M6's output.

The through-line is one sentence: **the rollout is the only evaluator in this
codebase that has ever been right, so the work is to make it faithful (M2, M3),
to make its decisions well-calibrated (M1), to point it at decisions that
repeat (M4), and only then to amortise it (M6, M5).**
