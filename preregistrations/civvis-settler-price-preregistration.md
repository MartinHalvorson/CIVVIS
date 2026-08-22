# Pre-registration — is there any headroom in what a settler is worth?

Written **before** the run, 2026-07-28. Branch `agent/martin-mbp/loop-lanes/…-2464`, PR #519.

## The measurement that motivates it

`expansion_funnel`, 48 seats over 12 full games (4p 60×38, 6 city-states, 500
turns, seed 2400000, shipped genome):

| bucket | share |
|---|---|
| already at target | 34.4% |
| settler already walking | 17.3% |
| expansion window closed | 14.3% |
| no city at pop 2 | 8.2% |
| **no reachable site** | **0.0% — zero seat-turns** |
| **lost the production argument** | **25.8% (3391 seat-turns)** |

Cities end at **4.02** against the agent's **own** planned target of **5.25**.
The map never runs out of room; the settler is out-bid, every time.

A settler is worth `920.0 + site_value * 4.0`. **920 is a hardcoded literal** —
not a gene, so `evolve` has never tuned it; not a doctrine lever, so the macro
search has never perturbed it.

## The treatment, and why it is an upper bound rather than a tuning

`advanced_settler_first` sets `settler_price = 100.0`, so a settler outbids
everything whenever the five gates permit one. **This is deliberately not a
shippable policy.** It is the oracle-ablation question this repository already
uses to decide whether a subsystem deserves work: grant it a cheating version
and see whether there is any headroom at all.

The gates still bind — one settler in flight, stop at the planned target, city
at pop 2, window open — so the treatment means *beeline to the city target*,
not *settlers forever*.

Testing on `advanced` rather than `strategic` because it is the agent whose
`production_value` this is, and it is fast enough to answer in one iteration.

## Prediction (mine, recorded before the run)

**I predict a null or a loss, and I am recording that against the fact that
four independent lines point the other way** (the funnel, the agent missing its
own target, `gene_leverage` measuring expansion *scrambling* as a help at 1.8
SE, and the oracle putting the leverage in the economy).

Reasons to expect it fails anyway: `settler_min_pop = 5` produced a **3.0 SE**
score-share gain that converted to **exactly zero** win improvement; raising the
`.min(6)` city ceiling was inert; and this loop's last two mechanism stories
were both wrong. A quarter of seat-turns being out-bid is evidence the constant
is *load-bearing*, not evidence it is *mis-set* — the same distinction that made
the assigned-Religion expansion bypass turn out to be correct play rather than a
bug, at a cost of 53 Elo to learn.

## Run

```
ai_eval advanced_settler_first advanced --players 4 --pairs 120 \
  --turns 500 --seed 2800000 --jobs 8
```

Log → `/Users/martin/settler-price-eval.log`.

## Decision rule, fixed now

- This is an **upper bound**, so the only two outcomes that matter are
  *headroom* and *no headroom*.
- **No headroom** (null or negative) closes the expansion-valuation axis
  outright: if a settler that always wins does not beat the shipped agent, no
  calibrated value of the constant will either. Record and move on — do **not**
  follow with 1.2×, 1.5×, 2× hoping for a middle that works.
- **Headroom** (gate PASS, or a clear positive direction at p<0.05) licenses
  exactly one follow-up: a calibrated `settler_price` chosen on a *disjoint*
  seed and confirmed on another. The upper bound itself never ships.
- Diagnostics to read **before** the win rate: cities per seat, settlers
  started, and military — the failure mode for this treatment is an empire that
  expands into an army it cannot afford.
