# Pre-registration: replicate the isolated promotion null

Written before the run.

## What is being replicated
`search_dose --only STOCK --maps 100 --seed 4200000` measured stock 40/40
against the promoted 20/80 at **0.4900 ± 0.0188 (−0.5 SE)**, both arms
constructed in-process so genome and value-net status are identical and only
the budget differs. At that SE the promoted +45 Elo (~0.065) would have shown
at **3.5 SE**.

That is **one** adequately powered sample. The same three-way convergence this
loop documented — causal signal costs replication — applies to my own result.

## The run
`search_dose --only STOCK --maps 120 --players 4 --turns 500 --seed 4500000`
Disjoint seed, same construction.

## Decision rule, fixed now
- **Replicates** if the edge again sits inside its interval and pooling the two
  keeps the promoted 0.065 outside ~2 SE. Then the claim stands: at this
  genome and this power, the 4× budget is not distinguishable from stock.
- **Fails to replicate** if the second run shows the promoted effect. Then the
  first was a fluke, and the memory note and PR #482 need retracting — I have
  already told PR #481 that the 4× is unverified, so a retraction would have to
  go there too.
- Anything between is reported as between.

## Prior
I expect it to replicate, which is exactly why it needs running by someone
with that expectation. The result argues for reclaiming compute across every
batch job in the repository, and a claim with that consequence should not rest
on a single seed.
