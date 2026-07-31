# What the search must become

> **Scope note (2026-07-30).** This is a research design and historical
> argument, not the live-agent description. Unless a row says otherwise, its
> search results were measured at four players on a 24×16 Standard map. The
> evolved genome and macro-search gains did not transfer to the measured
> six-player 74×46, six-city-state Online profile, no value net ships, and no
> searching agent is live-eligible in the exhibition. `docs/AI_GAPS.md` is the
> current status; `docs/EVAL.md` retains the chronological evidence and
> corrections.

`docs/AI_GAPS.md` ranks the current gaps. `docs/EVAL.md` records what happened
when they were attacked. This file is the third thing: given everything both
of those now say, **what would a much stronger designer actually build, and in
what order.**

It is written to be falsifiable. Every mechanism below states the evidence it
rests on, the check that proves it is not inert, and what would refute it.

---

## 0. The one fact that organises everything

Sort every AI change this repository has measured into two piles.

**Won maps at adequate power on the source benchmark — all of them are policy
rollouts:**

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

So on that source benchmark the search is both **under-provisioned** (each
tested doubling won) and **aimed at the wrong variable**. Those are separable
problems and they have separable fixes; neither claim has been established
across the rotating live profiles.

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
  early ranking. On this benchmark, the only lever on this search that worked
  was raising the total.
- **The horizon saturates.** A branch that reaches a decided game returns
  exactly 1.0 or 0.0, so once every branch resolves they agree *by
  construction*. Share of reviews in that state: 22% at horizon 40, 56% at 80,
  89% at 120. Pushing horizon past 80 buys agreement, not discrimination.
- **Half of reviews never reach the rollouts — and the search decides far less
  than half the lanes.** Three priors answer first. In a duel the religious
  prior answers *all* of them, which is why every published duel number for
  `strategic` measures a forced lane rather than search.

  Audited directly (`search_probe --priors`, 200 four-player positions at
  turn ~60), the split is worse than "half":

  ```
  who decides the lane, over 200 sampled reviews:
    priors    answered   99 reviews and named a lane in   99 (100%)
    rollouts  answered  101 reviews and named a lane in   33 (33%)
    -> the priors make 3.0x as many lane decisions as the search does
  ```

  A prior always names a lane. The rollouts name one only when a lane clears
  the adaptive baseline by the commitment margin, and two times in three none
  does. **So the search this document is about picks roughly a quarter of the
  lanes this agent plays**, and one predicate —
  `viable_religious_commitment`, 92 of the 99 prior-answered reviews — picks
  half of them on its own, disagreeing with the projection 85% of the time.

  That reframes every result in section 0. Depth (`strategic_h80`, 21–5),
  frequency (`strategic_r20`, 15–2) and branch fidelity (#413, +37 Elo) are
  all improvements to the quarter. None of them touches the half.

  It does **not** follow that the priors are wrong. `viable_religious_commitment`
  guards an irreversible global race that a forty-round projection scored by
  score share provably cannot price, and religion is the lane that converts
  best in this simulator.

  **It was then tested, and removing it is null.** `strategic_noprophet`, 240
  mirrored maps at seed 160000: 49.6%, Elo −3, 12 map directions to 14, sign
  p=0.8450. Both pre-registered predictions fired — search exposure 57% → 63%
  with `irreversible-religion` priors 211 → 0, religious commitment 30.3% →
  26.8% — so the treatment did exactly what it was built to do and it changed
  nothing.

  **Why a 3× decision share converts to a 0× strength share.** `adaptive` is
  not a lane. Returning `None` hands the turn back to `AdvancedAi`'s own
  victory planner, which frequently picks the same lane the prior named — so
  85% of reviews changed *label* while religious victories moved 171 to 164.
  The prior is largely **redundant with** the behaviour underneath it, not
  additional to it. A disagreement rate is an upper bound on behavioural
  impact, and a very loose one.

  **The consequence for this document's ranking is larger than the result.**
  Three treatments have now moved lane decisions by very different amounts:

  | treatment | lane decisions changed | strength |
  |---|---|---|
  | `strategic_noprophet` | ~42% of all reviews | **0** (p=0.8450) |
  | `focused_deepening` rank-pruned | commitment 44.9% → 58.4% uncommitted | **0** (p=0.8318 repaired) |
  | warm branches (#413) | 1 review in 4 | **+37 Elo** |

  Changing *more* lane decisions is uncorrelated with strength; the one that
  won changed the fewest. **Lane routing is not the lever**, and the macro
  search — whose only output is a lane — is closer to exhausted than sections
  0 and 2 imply. What is left there is compute, which works and has a measured
  ceiling. Effort belongs on M4: a decision that repeats.

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

> **⚠ Do not read a commitment-rate change as a strength signal in either
> direction.** Two treatments that lost — adaptive stopping and rank-pruned
> focused deepening — both reduced it, and that coincidence was written up here
> as if it were a law. `search_probe` refutes it directly. Screened at 57
> paired positions against the stock agent:
>
> | knob | eval outcome | would-commit | flips toward adaptive |
> |---|---|---|---|
> | `--horizon 80` (`strategic_h80`) | **won 21–5, p=0.0025** | 28% → **12%** | 12 of 15 |
> | `--rotate` (`rotate_lanes`) | **null** | 28% → **7%** | 12 of 12 |
> | `--cold` (reverting the promotion) | **lost 34–87** | 28% → 21% | 9 of 14 |
>
> The knob that *won* cuts commitment further than the knob that measured
> null, and the knob that lost cuts it least. Commitment rate is a diagnostic
> that a treatment is doing something, not evidence about what.

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
> **The mechanism is decision change, not dispersion, and not the force-group
> story argued below.** Two mechanisms were proposed for this promotion and
> both were wrong. The force-group argument predicted *more* domination; the
> promoted agent takes less. The dispersion argument — that warm branches
> roughly double the branch spread — rested on an **unpaired** measurement
> comparing arms that played different games, and does not survive a paired
> one: flipping the flag on one agent at one position, over 57 positions,
> gives cold spread higher on 17 and lower on 20, sign p=0.7428.
>
> What survives is measured and modest: the two configurations **decide
> differently on 14 of 57 positions — one review in four** — with the spread
> distribution unchanged. Fidelity moves the search's answer rather than
> sharpening its resolution, and those different answers win 87 mirrored map
> directions to 34. Which lane benefits is still unexplained: the flip
> direction is 5 toward a lane against 9 toward adaptive, p=0.4240.
>
> **The general lesson is about measurement, not about search.** An unpaired
> comparison of two agents is a comparison of two *trajectories*, because each
> agent steers itself into different positions. `search_probe` exists because
> of this; its first run refuted a claim its author had merged an hour earlier.

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

> ### ⚠ M4 IS RETIRED — measured, before it was built
>
> The fires-check was run first (`search_probe --production`, 71 city
> decisions, four players) and **the premise fails**:
>
> - the evaluator separates **3.7 of 5.0** candidates; only **4%** of
>   decisions score every candidate alike. The search is **not blind**.
> - raising the horizon ceiling from 40 to **200** — long enough for any
>   build in the game to land and compound — leaves the chosen item unchanged
>   on **54 of 56** decisions (**96%**). The ranking does not depend on the
>   window.
>
> So the design above is aimed at a defect that is not there. Cheapening the
> rollout to afford a payoff-length horizon cannot help a ranking that is
> already horizon-stable, and **the whole family — sealed per-city rollouts,
> frozen rivals, payoff-length windows — is predicted-null.**
>
> What is left is the *objective*, not the window. **Score share is not win
> probability.** The lane search works because its branches reach decided
> games and return exactly 1.0/0.0 — 22% of reviews at horizon 40, 56% at 80.
> A production rollout from mid-game essentially never decides, so it ranks
> entirely by a proxy, and the hand-written governor's sequencing beats that
> proxy. That is why `production_net` changed nothing: it swapped one function
> of the 25 aggregates for another, when the problem is that no function of
> them is win probability.
>
> **The surviving route is M6 → M5**, not a better online search: continue
> branches to a real result offline, label them with the outcome, and distil.
> A full continuation per candidate is roughly seventy times the cost of a
> game, which is affordable as a labelling job and not as an agent.
>
> **Measured too, and it is thinner than it sounds.** `search_probe --outcome`
> continues *every* candidate of a city decision to a real result at the stock
> 500-turn budget — the label M6 would produce — over 51 decisions:
>
> ```
> candidates continued per decision              5.0
> decisions where the label DISCRIMINATES        14 of 51 (27%)
> ...of those, proxy pick == outcome pick         3 of 14 (21%)
> ...of those, the proxy's pick WON its game      6 of 14 (43%)
> ```
>
> On **73% of decisions every candidate leads to the same outcome**, so the
> label carries no signal at all and the seventy-fold cost buys nothing. Where
> it does discriminate the proxy is near chance (43%), so there is real
> headroom — but it exists on about a quarter of decisions.
>
> **And 27% is an upper bound, not an estimate.** The engine is deterministic,
> so each continuation is a *single sample*, and a build that "wins" may win
> for reasons entirely unrelated to it. Chaotic divergence and causal effect
> are indistinguishable in this design, and determinism means the label cannot
> be denoised by repeating it.
>
> **What would make M6 viable is therefore replication across opponents, not
> more decisions.** Continue each candidate against several distinct rival
> policies and label with the *win rate* rather than one outcome.
>
> **Measured, and it puts a price on M6 rather than a plan.** Replicating over
> five distinct opponent policies (850 full games, 12 minutes): 28% of
> candidates changed outcome when only the opponents changed — so replication
> does denoise — and the median win-rate spread *between* candidates is
> **0.20** against a per-candidate standard error of **0.224 at five
> replicas**. The signal is under its own noise floor. Resolving it needs
> about **100 replicas per candidate**, twenty times that run: four hours for
> 34 decisions, ~1200 hours for a 10,000-decision corpus.
>
> M6 is therefore a **compute project, not a coding project**, and what it
> would buy is a ~0.20 win-rate edge on the ~25% of decisions where the label
> discriminates at all. Worth stating before anyone starts it.

### The genome axis, measured to exhaustion

The GA was run for the first time in this repository's history and produced a
champion worth **+49 Elo** over the hand-written defaults every agent had been
silently playing (`docs/EVAL.md`, 2026-07-27). The obvious next question is how
much more is there. Four runs of `search_probe --selection`, at increasing
power, say: **not much, and not findable by ranking.**

Every genome in a 24-member population is measured, none is selected, and the
win indicator and the selection value come from the same games on common seeds
— so there is no winner's curse, unlike any comparison of generation champions.

| games/genome | spread across genomes | SE per genome | spread ÷ SE |
|---|---|---|---|
| 48 | 0.028 | 0.065 | 0.43 |
| 300 | 0.018 | 0.027 | 0.67 |
| 480 | 0.025 | 0.020 | 1.25 |
| **800** | **0.017** | **0.016** | **1.06** |

**The spread between candidate genomes never clearly exceeds the error on
measuring it.** The best estimate of the true spread is ~0.013 win rate, which
needs about **5,000 games per candidate** to rank reliably — 24 × 5,000 per
generation. `evolve` spends 8 by default.

Split by mutation operator on the same games and seeds:

| operator | true spread |
|---|---|
| independent per-gene noise, ±25% on a third of genes | **0.013** |
| coordinated movement along a `Doctrine` axis | **~0.000** |

**⚠ This reverses a claim made earlier in this document's history.** From two
marginal numbers taken in different runs I asserted that coordinated
perturbations move strength about six times more than random ones, and built a
mutation-operator proposal on the ratio. Measured properly — same games, same
seeds, both arms — the coordinated arm is *narrower*. The claim is withdrawn.

### The promotion test measures against the wrong null

`evolve::sprt_confirm` tests `H0 = 1/players` against `H1 = max(0.40, 1.6/players)`
on a table `make_table` builds as **candidate + one frozen-default anchor +
champions**. The shipped champion is +49 Elo over the default, so the anchor is
the weakest seat and the other three split more than their nominal share.

Measured — the champion played against its own table
(`genome_gate --calibrate`):

```
86/240 = 0.358   (nominal 1/players = 0.250)      seed 9000
83/240 = 0.346                                     seed 9100
```

**Parity for a candidate merely *equal* to the champion is 0.35, not 0.25**, and
that sits far closer to H1 than to H0. A sequential test with those bounds
accepts equals. The two seeds agree within a standard error of 0.031, so this
is a property of the table rather than a sample.

The effect is not theoretical. The same searcher, same budget, same draws,
only the null changed:

| null | acceptances | candidates | rate |
|---|---|---|---|
| nominal 0.250 | 4 | 15 | **27%** |
| measured 0.358 | **0** | **121** | **0%** |

It also explains a pattern in the 25-generation run recorded above: candidates
that *won* their SPRT and were then vetoed by the holdout, twice. **The holdout
was catching what the null let through.**

Three repairs are possible and none is obviously right — calibrate `p0` per
configuration, drop the anchor from the SPRT table at the cost of the
intransitivity it exists to prevent, or keep the null and rely on the holdout,
which demonstrably works. That call belongs to whoever owns `src/evolve.rs`.

**Nothing here disturbs the shipped champion.** Its SPRT promotion was marginal
under the corrected null (22–32 = 0.407 against 0.358), but it was validated
independently by a pre-registered `ai_eval` against the hand-written defaults
over 1300 mirrored maps — 54.6%, Wilson 51.9–57.3%, gate PASS — a paired
comparison that never used the SPRT. That separation is the whole reason the
result survives its own promotion mechanism turning out to be permissive.

### How much is left on the genome axis

With the null corrected, **242 random mutations of the shipped champion, across
two mutation radii, produced no candidate worth +10 percentage points
(~+72 Elo)**, at ~60 games per candidate against a 200-game cap — the test
rejects fast, so these are not marginal calls.

| operator | candidates | acceptances |
|---|---|---|
| ±25% on 34% of genes | 121 | **0** |
| ±60% on 70% of genes | 121 | **0** |

The two radii differ by more than 2× in step size and 2× in genes touched, and
neither finds anything. That matters because a single radius finding nothing
has two readings — exhausted neighbourhood, or a step too small to leave it —
and the wide arm removes the second.

Set beside the spread measurements (typical mutation worth ~0.013 win rate,
needing ~5,000 games to resolve), the reading is:

> **The shipped champion sits in a neighbourhood that single-parent random
> mutation does not improve, at any scale this hardware can resolve.**

That bounds *large* improvements, not small ones, and it bounds *this operator*,
not the genome. Crossover, restarts elsewhere in the bounds, and structured
parameterisations each search differently — though the one structured operator
measured here came out **narrower** than random, not wider.

It also refutes the reallocation hypothesis that motivated the searcher.
`evolve` spends 23:1 on ranking versus gating, and the ranking cannot resolve
typical candidates — but testing 121 candidates instead of two found nothing.
**Reallocating a budget does not help when the thing being searched has nothing
at that scale to find.**

### What follows: select at the gate, not in the ranking

Most mutations are strength-neutral at any resolution that is affordable. Yet
the GA did produce +49 Elo, through a 200-game SPRT against the incumbent. Both
facts fit one model:

> **The GA is not hill-climbing. It is filtered random search — most candidates
> are indistinguishable, rare ones are large, and the gate is what finds them.**

That explains an empirical result from #457 that had no mechanism attached: a
measured 16-game control showed no agreement gain over the shipped 8, so the
budget was retained. The reason is that the ranking is unresolvable at *any*
affordable budget — 8 and 16 and 96 and 800 are all far below 5,000, so none of
them ranks better than another.

**Design consequence.** Per-genome fitness games buy a ranking that cannot
resolve typical differences; SPRT games buy the filter that actually decides.
More candidates through a cheap screen, gate unchanged, searches more of the
space for the same cost. This is the opposite of the direction argued earlier
here, and it is the direction the measurement supports.

**Design consequence for the objective.** Whether the breeding statistic is
monotone in strength remains open and is not cheaply answerable — at 800 games
the correlation still reads `NO VERDICT`. That no longer blocks anything: used
as a **control variate** rather than a substitute, the continuous statistic is
unbiased for the win rate whatever its correlation, with variance `(1 − r²)`
times the plain estimator's. A statistic of unknown monotonicity is safe there
and nowhere else.

### The criterion that generalises all of this

> **Search pays on a decision whose effect exceeds the outcome noise floor
> _at the sample size the search can afford_.**
>
> The italicised clause is the whole of it, and it took a measurement to find.
> A rollout search deciding one build can afford a handful of samples per
> candidate; a genetic algorithm tuning a policy parameter averages over
> hundreds of games. They face the same noise and resolve completely different
> effect sizes.
>
> | mechanism | effect | samples it can afford | resolves? |
> |---|---|---|---|
> | lane search | **1.0** (branch reaches a decided game, returns 1.0/0.0) | 1 | **yes** — the one component that has ever won |
> | production search | 0.20 win rate | ~5 | no — floor is 0.224 |
> | policy genome (GA) | 0.08 win rate | hundreds | **yes** — floor at n=120 is 0.046 |
>
> Measured for the genome axis (`search_probe --genome`, 24 positions, the four
> `Doctrine` perturbations, 5 opponent replicas each): mean win rates
> **0.375 / 0.367 / 0.325 / 0.292** for consolidate / incumbent / expand /
> militarize — an **0.083 spread, about 65 Elo**, at 1.8 standard errors on
> 120 games apiece. Per *position* that is under the noise floor (median
> spread 0.200 against an SE of 0.224); in *aggregate* it is not, and a GA only
> ever needs the aggregate.
>
> **So the same number that closes per-decision search opens policy-level
> optimisation.** That is not a contradiction — it is the sample-size clause
> doing its work, and it is why sections 0–2 kept finding null after null while
> the untouched 40-scalar genome sat there.
>
This is measurable **in advance**, for any decision, by
> `search_probe --outcome --replicas K`, and for the policy axis by
> `search_probe --genome`. Do that before building search for a new decision
> type. It is the same discipline that retired the sealed-rollout family and
> the commitment-margin hypothesis, applied one level up: not "does the
> treatment fire" but "is there anything here to find".
>
> **⚠ Do not extend this arithmetic to the GA — I did, and it is wrong.** The
> Bernoulli standard error `sqrt(0.25/games)` describes a *win rate*. `evolve`
> does not select on wins: since #457 its selection statistic is
> `50·P·score_share + 12·P·combat_share`, a continuous quantity, and every
> genome in a generation is scored on the **same map seeds and seats**, so the
> comparison available to it is *paired*. A binomial SE describes neither. The
> shipped eight-game budget was retained on a measured 16-game control showing
> no agreement gain, and `src/bin/evolve_probe.rs` measures the real noise floor
> of the real statistic — use it rather than the table I originally put here.
>
> The clause still holds where the outcome genuinely is a Bernoulli draw, which
> is every per-decision search in section 2: those label branches by win or
> loss and cannot pair across candidates.
>
> **The standing implication.** `elo::builtin_ai` resolves every strategic
> agent through `load_champion("evolved").unwrap_or_default()`; `evolved/` is
> gitignored and **no `best.json` exists on this machine or in a fresh clone**.
> So the 40 evolved weights have never been evolved, and every result in this
> document was measured on top of an untuned policy. Shipping a champion
> genome is the one lever this repository built machinery for — a GA with an
> anchor, a holdout and a promotion gate — and never pulled.

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

> **Rewritten 2026-07-27 on measurement.** The order below this box was written
> when the macro search was the only subject and none of its items had numbers.
> Since then M1 and M4 have been retired, M2 has landed, an axis the document
> did not contain at all produced the largest measured gain, and the
> through-line sentence turned out to be wrong. The original is kept beneath so
> the path is visible.

**The state, as measured:**

| axis | status |
|---|---|
| macro search — lane routing | **closed.** Three treatments moved lane decisions by 42%, by a commitment collapse, and by 1-in-4; only the smallest won. Decision share is uncorrelated with strength. |
| macro search — within-review allocation | **closed.** A shallow estimate is not rank-preserving w.r.t. the deep one, so any pruning discards signal. Retires rotation, adaptive stopping, focused deepening, progressive widening and sequential halving together. |
| macro search — the priors | **closed.** The predicate deciding half of all reviews disagrees with the search 85% of the time, and removing it is null: `adaptive` is not a lane, it delegates to a planner that picks the same lane anyway. |
| macro search — branch fidelity (M2) | **★ +37 Elo, shipped.** |
| production search | **closed.** Not blind (3.7/5.0 distinct values), horizon-stable to 200 (96% same pick). It sees the difference, keeps seeing it once every payoff has landed, and still loses to the scripted governor. |
| learned value on empire aggregates | **closed.** No function of the 25 aggregates is win probability. |
| outcome labelling (M6→M5) | **priced.** ~1200 h for a corpus, and 73% of decisions carry no signal at all. A compute project, not a coding one. |
| **the genome** | **★ +49 Elo, shipped — then bounded.** 242 mutations across two radii found nothing worth +72 Elo more. |
| structural behaviour | **open, and where the evidence points.** Untouched by any of the above. |

**What actually paid, and it is not what this document originally proposed.**
Two changes shipped. Neither was a new mechanism:

1. **The counterfactual was simulating the wrong thing.** Every branch of the
   macro search was handed a *freshly constructed* planner, so it answered
   "what if I restart and commit to this lane" while standing in for "what if I
   commit from here". Cost: one struct clone. Worth +37 Elo.
2. **The repository was loading its fallback.** `load_champion(…).unwrap_or_default()`
   plus a gitignored `evolved/` plus no artifact anywhere meant every agent
   played `Weights::default()`. Running the GA that had been built for this and
   never completed produced +49 Elo. Cost: one evening of compute.

Both were found by *checking something that looked fine*. Every mechanism this
document proposed from first principles — allocation, margin calibration,
sealed rollouts, structured mutation — measured null or reversed.

**So the sequencing advice that survives is procedural, not architectural:**

- **Measure the artifact before the algorithm.** Ask what the process actually
  loads, at the path it actually runs from. Two of the largest findings here
  were a silent fallback and a mis-specified null, neither visible in code
  review.
- **Screen before you build.** `search_probe` has retired four mechanisms for
  minutes of compute each, against ~40 minutes per `ai_eval`. A treatment that
  cannot move the numbers a decision is made from cannot move a win rate.
- **Check the null when a result looks good.** A 27% acceptance rate against a
  5% alpha was the tell that `evolve`'s SPRT tests parity at `1/players` on a
  table whose true parity is 0.35.
- **Structural before parametric.** The parametric axis is bounded above; the
  structural one has never been systematically measured. `src/oracle.rs`
  (#366) is the right instrument for ranking structural gaps, and the noise
  floor for it is in this document.

**The through-line sentence this document used to end on — "the rollout is the
only evaluator in this codebase that has ever been right" — was wrong.** It is
the only evaluator that was right *inside the macro search*, which is the only
place the document was looking. The single largest gain came from an axis it
did not mention.

---

### The original sequencing, kept for the path

**M2 (landed) → M6 → M5** — M4 retired on measurement (see its entry), with M3 attempted only alongside its
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
