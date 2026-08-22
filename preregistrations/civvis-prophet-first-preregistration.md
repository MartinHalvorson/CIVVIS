# Pre-registration — test the finite Prophet race before the opportunistic war

Written **before** the run, 2026-07-28. Branch `agent/martin-mbp/loop-lanes/…-2464`, PR #519.

## The treatment

`advanced_prophet_first` sets `prophet_before_opportunism = true`. In
`assess()`, the arm that fires Conquest on a bare power ratio
(`turn >= 55 && cities >= 2 && my_power > weakest_rival * 1.80 + 20.0`) is
tested **after** `religious_opening_viable` instead of before it. `at_war`
keeps its priority either way, so a war already running is unaffected — only
the *opportunity* to start one is deferred.

With the flag off the cascade is arithmetically identical to the shipped
`at_war || A || B`, so this ships zero behaviour change until the entrant is
selected.

## Why the motivation is now much stronger than when this was built

It was built on PR #366's oracle result, which I then showed does not reproduce
on the shipped genome — so I deliberately did **not** evaluate it.

The `strategic_religion_expand` run has since supplied a *causal* replacement.
Shifting the search's commitment **away** from religion (36.1% → 29.3% of turns,
164 → 123 dominant seats) and toward domination (10.1% → 13.8%) measured
**−53 Elo**, sign p=0.0014, on 120 mirrored maps. So on this engine:

> moving commitment from religion to opportunistic conquest is worth about
> −53 Elo, measured, not inferred.

`prophet_before_opportunism` moves it the other way, by the same kind of
mechanism, at the layer where the choice is actually made. That direction has
never been tested.

## Fires-check (already run, strict form)

`the_prophet_reorder_fires_in_ordinary_games` asserts, over three full games in
the turn-40..130 window where the two arms can collide:

- `preempted > 0` — the stock cascade chooses Conquest where the reorder chooses
  Religion, so the arms genuinely collide;
- `differed == preempted` — the reorder changes **exactly** that one thing and
  nothing else.

Both pass. This is a fires-check on the *decision*, not on a bucket that could
only move one way — the mistake made twice already this loop.

## Prediction (mine, recorded before the run)

**I predict a small positive, and I hold it loosely.** My predictions this loop
have been wrong four times: the commitment-timing curve shape, the direction of
the projection bias, "null" for a result that came in at −53 Elo, and the
expansion diagnosis. The base rate for mechanisms reasoned out in advance in
this repository is poor.

What supports a positive: the causal −53 Elo result points this way; religion is
the lane that converts (103 of 120 `advanced` wins in the
`refuse_unreachable_lanes` eval were religious); domination converts 3–8%; and
the deferred arm is an *opportunity*, not an obligation — the war is still there
ten turns later, while a Prophet slot is not.

What would make it lose: the power-ratio window is also when a neighbour is
weakest, so deferring may forfeit the only cheap conquest of the game; and
`religious_opening_viable` already self-limits to the best contenders, so seats
that lose the prophet race may now have wasted the war window too.

## Run

```
ai_eval advanced_prophet_first advanced --players 4 --pairs 120 \
  --turns 500 --seed 3100000 --jobs 8
```

Log → `/Users/martin/prophet-first-eval.log`.

## Decision rule, fixed now

- **PASS** requires the unmodified gate: map directions FOR > AGAINST at sign
  p < 0.05 with the Wilson lower bound clearing parity.
- **Anything else ships the flag off with the result recorded**, on the
  `advanced_lane_reachable` precedent. No seed re-rolls, no margin tuning.
- A gate PASS earns **one** confirmation at a disjoint seed before any claim of
  a promotion, and a confirmation on `strategic` before it is claimed for the
  agent the exhibition runs.
- Read first, before the win rate: the plan-commitment table (religious and
  domination shares) and the victory-type counts. If commitment did not move
  toward religion, the treatment did something other than what it says.
