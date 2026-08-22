# Pre-registration — repairing the assigned-Religion expansion bypass

Written **before** the run, 2026-07-28. Branch `agent/martin-mbp/loop-lanes/…-2464`, PR #519.

## The defect

`AdvancedAi::assess` (`advanced.rs:1807`) sends an assigned-Religion seat that
has no religion yet straight to `GrandStrategy::Religion`, skipping the "can
this lane still afford to expand first?" test at `:1809` that **every other**
assigned target reaches.

Measured end-to-end (`commit_curve`, 40 maps, 4p 60×38, shipped genome): a seat
committed to Religion finishes on **1.68 cities against an adaptive seat's
4.10**.

`StrategicAi::branch_agent` builds every branch with `retarget`, so the macro
search projects each religion branch with an empire that has stopped growing.

## The treatment

`strategic_religion_expand` — `set_religion_may_expand(true)`, which applies the
repair to the **acting agent and every projected branch together**. Those must
move together: on the branches alone the search ranks a game the agent will not
play; on the actor alone the search keeps projecting the old behaviour.

## Screen already run (`lane_projection`, 24 positions at turn 40)

- religion branch value moved on **20 of 24**
- **10 up / 10 down**, mean −0.0037 against a branch spread of 0.060
- **argmax lane changed on 6 of 24 — one review in four**
- 3 toward Religion, 3 away — **no directional preference**

Not inert, and the decision moves. This is the same signature `continue_from_plan`
showed (14 of 57, one in four, no directional story) before it measured +37 Elo.

## Prediction (mine, recorded before the run)

**I predict NULL**, and I am recording that against the temptation to read the
`continue_from_plan` resemblance as evidence.

Reasons: (1) the same repair applied to a *committed* agent end-to-end was a
null on wins (4 helped / 7 hurt, p=0.5488); (2) it recovers only 0.44 of a
2.42-city deficit, so most of what cripples a targeted seat is untouched; (3)
my previous two predictions this loop were both refuted, and the base rate for
mechanisms proposed from first principles in this repository is poor —
`docs/SUPERHUMAN.md` records that every one of them measured null or reversed,
while both shipped gains came from checking something that looked fine.

The `continue_from_plan` resemblance is a **pattern match to a prior success**,
which is exactly the reasoning this repository has been burned by. It justifies
spending one eval; it is not evidence of a gain.

## Run

```
ai_eval strategic_religion_expand strategic --players 4 --pairs 120 \
  --turns 500 --seed 2600000 --jobs 8
```

240 games. Log → `/Users/martin/religion-expand-eval.log`.

## Decision rule, fixed now

- **PASS** requires the unmodified promotion gate: map directions FOR > AGAINST
  at sign p < 0.05, and the Wilson lower bound clearing parity.
- **INCONCLUSIVE or worse → the flag ships off with the null recorded**, on the
  `advanced_lane_reachable` precedent. No re-running at a new seed to find a
  better number.
- A promising-but-short result gets **one** confirmation at a disjoint seed,
  pre-registered separately, not a promotion.

## What refutes the whole line

A null here says the projection defect, though it changes one review in four,
does not change strength — which would make the `1.68 vs 4.10` city finding a
fact about targeted play with no consequence for the search. That is a real
possibility and the honest outcome to report if it happens.
