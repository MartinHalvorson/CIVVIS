# Pre-registration: does a league-selected genome beat strategic_deep?

Written before the run. This tests MY conclusion against another agent's claim,
and I have an interest in the outcome, so the rule is fixed in advance.

## The two positions

**PR #421 (mine), measured:** on the statistic that tracks winning, not one of
the eight gene blocks is load-bearing. Randomising any block costs nothing
outside its interval. If that is right, swapping one genome for another cannot
win, whatever their league ratings say.

**docs/LEAGUE_GENOME_CHALLENGER.md (theirs):** `strategic_deep_league`
transfers the conservatively strongest untargeted league genome (`g20-21`,
rating 1790.8, lower-confidence 1730.1) into the promoted macro-search budget,
against the `advanced` anchor at 1702.7.

## Prediction

**I predict NULL** — `strategic_deep_league` will not beat `strategic_deep`
outside the interval. If it wins, my genome conclusion is wrong and
`docs/GENOME.md` needs retracting, not qualifying.

## The run

`ai_eval strategic_deep_league strategic_deep --players 4 --turns 500`, paired
and seat-mirrored on a fresh seed.

## Decision rule, fixed now

- **Their claim is supported** if map directions favour the league genome with
  sign p < 0.05.
- **My claim is supported** if the result is a null.
- A result against me is the more informative one and gets reported first.

## What I am NOT entitled to conclude either way

A null does not prove league ratings are meaningless — the league rates
genomes against each other in its own pool, and the transfer changes the
opponent. It would only show the transfer does not pay at this budget.
