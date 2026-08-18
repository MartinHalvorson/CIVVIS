# The promotion gate stops charging every split map a coin flip's variance

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`ai_eval`'s promotion gate is a conjunction: anytime-valid betting evidence at
2.5% per direction, **and** a Wilson lower bound above parity. The Wilson half
treats each mirrored-map score as a Bernoulli observation and so charges it the
maximum variance `p(1 - p)`. But a mirrored A/B between close agents splits most
of its maps, and a split — scoring exactly 0.5 — is the observation that carries
no dispersion at all. The question: how much resolution does that assumption
cost, and is there a replacement that is narrower *and* still calibrated?

The second half of that question already had a recorded answer of "no". The test
`no_narrower_interval_here_is_also_calibrated` pinned it: a normal interval on
the sample variance and a percentile bootstrap were both ~2.2x narrower than
Wilson and **both undercovered**. That finding stands and is re-asserted below.
It was not the whole answer, because both alternatives estimate a dispersion
from observations that are mostly the same number, and an interval built on an
underestimated variance undercovers. An interval that never estimates the
dispersion has no such failure mode.

## How it was measured

Three ways, because the gate has to be judged on calibration, on power, and on
the runs it has actually seen.

1. **Calibration**, extending the existing pinned test: 400 replications of 120
   maps drawn on the shape these runs produce (10% break either way, 80%
   neutral), null true, coverage of parity counted for each interval.
2. **Power and error**, by Monte Carlo over the observed break rate (35%): the
   full gate — both conjuncts, `PROMOTION_MIN_MAPS`, the nonstationarity veto —
   run under the null and under fixed true effects, 4000 and 2000 trials.
3. **Real evaluator runs.** The three paired-map score vectors this repository
   recorded and filed as inconclusive, reconstructed from their pair outcomes;
   plus one fresh deployment-profile run, `advanced` against `advanced_v1`,
   24 pairs / 48 games at 6p 74x46, 6 city-states, Online, 150 turns, seed
   7700000.

The replacement is the same e-process the gate already trusts, inverted: retain
every candidate mean the run's own betting evidence cannot reject at 2.5% per
side. Coverage is then Ville's inequality — finite-sample, valid for any bounded
observation, and free of any variance estimate. One predictable adaptive stake
joins the fixed `BET_LAMBDAS` grid; Ville needs only that a stake be a function
of the maps already seen, so the mixture stays exactly as valid while spending
less of its capital on bets the run has already ruled out.

## What it measured

**Calibration** (400 replications, 120 maps, null true, nominal 95%):

| interval | covered | mean width |
|---|---:|---:|
| normal on the sample variance | 372/400 (93.0%) | 0.0793 |
| percentile bootstrap | 374/400 (93.5%) | 0.0790 |
| **betting (shipped)** | **397/400 (99.25%)** | **0.1197** |
| Wilson (retired) | 400/400 (100%) | 0.1760 |

The betting interval is **32% narrower than Wilson and still conservative**,
where both variance-estimating alternatives buy their width by undercovering.

**Whole-gate error under the null**, 4000 trials per cell, against a declared
budget of 2.5% per direction:

| maps | break rate | old gate | new gate |
|---:|---:|---:|---:|
| 20 | 35% / 60% | 0.00% / 0.07% | 0.00% / 0.07% |
| 36 | 35% / 60% | 0.03% / 0.25% | 0.38% / 0.53% |
| 50 | 35% / 60% | 0.10% / 0.22% | 0.38% / 0.50% |

Both gates are far inside the budget. The old one spends about a tenth of it.

**Power**, 2000 trials per cell, 35% break rate:

| true effect | 20 maps | 40 maps | 60 maps |
|---|---:|---:|---:|
| +40 Elo | 0.5% → 0.5% | 1.3% → **5.0%** | 2.9% → **10.2%** |
| +80 Elo | 4.0% → 4.0% | 15.3% → **31.7%** | 30.8% → **56.4%** |
| +120 Elo | 18.7% → 18.7% | 60.2% → **89.4%** | 87.4% → **99.5%** |
| +207 Elo | 22.9% → 22.9% | 70.0% → **96.9%** | 93.4% → **100.0%** |

⚠ **The gain is zero at exactly the 20-map floor** and arrives from 25 maps up.
At 20 maps only one prefix is monitored and the mixture still pays for bets the
run did not need, so the two gates promote the same runs. Deployment matrices
are decided at 40 maps and above, which is where the effect lands: at the +207
result this repository has actually recorded, 70.0% → 96.9%.

**On the recorded runs.** Lower bound, betting against Wilson:

| recorded run | maps | mean | betting low | Wilson low |
|---|---:|---:|---:|---:|
| advanced vs basic (6F, 13N, 1A) | 20 | 0.625 | 0.380 | 0.409 |
| deployment (6F, 29N, 1A) | 36 | 0.569 | **0.471** | 0.409 |
| strategic (8F, 16N, 1A) | 25 | 0.640 | **0.496** | 0.445 |

Their empirical variance sits 3.3x to 5.6x under the Bernoulli bound Wilson
charged. The 20-map row is the floor case above and is honestly worse.

**The fresh deployment run** — `advanced` 64.6% over 24 maps (11 sweeps, 9
neutral, 4 against), Elo-equivalent +104 — is **INCONCLUSIVE under both gates**:
betting CI 38.7%..93.1%, Wilson 44.7%..80.4%, anytime p ≤ 0.3755. A real run
that does not clear parity still does not clear it. That is the result to want
from a change that widens what can be resolved.

## What was decided

**Shipped.** `paired_inference` decides on the betting interval; the
maximum-variance Wilson interval stays computed and printed beside it so every
historical run remains comparable and the width the old rule charged stays
visible. The verdict logic, `PROMOTION_MIN_MAPS`, the per-direction budget, and
the nonstationarity veto are unchanged.

The pinned finding "there is no drop-in narrower interval here that is also
calibrated" is superseded and its test renamed to
`a_betting_interval_is_the_narrower_calibrated_one`, keeping both undercoverage
assertions that made the original point. Two tests were added: one holding the
new gate inside its declared null budget, one pinning the recorded-run lower
bounds including the floor case where the change does not help.

This is a measurement change and it promotes nothing on its own. It does not
make any treatment stronger; it makes a 40-map run able to resolve an edge that
40-map runs previously could not, which is the constraint that has been filing
+49 to +100 Elo-equivalent point estimates as inconclusive.
