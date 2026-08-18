# Pricing the governor's routing and the settling asymmetry

_2026-08-18 · `97404c04` (PR #1955)_

## What was asked

`advanced_every_lane` measured −62 compact / −95 deployment, and its census
showed a uniformly ~30% smaller empire rather than a re-prioritisation. Two
routing questions follow that the composite cannot answer: is the governor bad
*everywhere* (including Recovery, the one lane where it decides production for
the shipped agent), and which half of the five-lane composite carries the
loss? A third question surfaced while wiring the arms: both settling gates in
`advanced.rs` read bare `plan.desired_cities` (3 until roughly turn 60 on
Online speed) while the cascade settles toward
`restore_target.max(plan.desired_cities)` = 4 — so for the whole opening the
agent refuses the fourth Settler its own cascade is trying to build.

## How it was measured

400 pairs per gate, compact and deployment profiles, one arm per question:

- `advanced_without_governor_recovery` (withholds `governor_in_recovery`,
  which is ON in production), seed 28000000.
- `advanced_governor_expansion_lane` (the Expansion half of the split
  composite; `enable_governor_every_lane` still sets both halves, pinned
  byte-identical by
  `splitting_the_every_lane_composite_leaves_it_exactly_as_it_was`),
  seed 30000000.
- `advanced_settlement_gap_target` (`settlement_target()` widens, never
  narrows; flag `settlement_gap_reads_city_target` default off), seed
  31000000.

Fires-checks pinned each flag and its dispatch source. The companion arm
`advanced_governor_victory_lanes` (seed 29000000) is registered and
pre-priced but had not run when this round closed.

⚠ The per-run interval lines were not preserved — the session recorded point
estimates and verdicts only (PR #1955 body is the primary record). The
verdicts below were called on those verdict lines, not reconstructed.

## What it measured

- **Recovery withhold** (seed 28000000): −7 compact / +6 deployment, both
  inconclusive → **RETAIN `advanced`**. A wash: the governor is *not*
  uniformly worse than the cascade, which localises the −95 to the growth
  lanes where an empire compounds.
- **Expansion half** (seed 30000000): −20 compact (direction significant,
  p=0.0089) / −16 deployment → **RETAIN**. The Expansion half is not the −95
  carrier; by subtraction the four victory lanes carry roughly −70 to −80.
  The composite's city deficit was a downstream symptom: districts hold at
  0.94× of control while traders (0.70×), gold (0.71×) and buildings (0.81×)
  pay for them.
- **Settling asymmetry repair** (seed 31000000): +2 compact / −9 deployment,
  both inconclusive → **RETAIN**. The census moves the predicted column but
  only barely (cities +0.09 compact, +0.02 deployment): the asymmetry is
  real and the repair is correct, but the gap it closes is too small to
  convert.

## What was decided

All three gates RETAIN: every new flag keeps its default
(`governor_in_recovery` on — today's routing; the two split-lane flags and
`settlement_gap_reads_city_target` off), and no shipped agent changes
behaviour. The arms keep their numbers so the decomposition can be finished:
`advanced_governor_victory_lanes` (seed 29000000) is the open end. The
settling-asymmetry defect is pinned by
`the_settling_gates_and_the_cascade_disagree_about_the_city_target` as a
recorded null — actuation repairs pay where valuation tunes do not, but this
actuation gap was measured too small to convert.
