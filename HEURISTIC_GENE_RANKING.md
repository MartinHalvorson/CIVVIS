# Heuristic Gene Ranking

This is a read-only snapshot of the current advanced-AI gene ledger. It is a
ranking aid, not a live-game decision: defaults remain evaluator-gated and a
single Civ6 game is not promotion evidence.

Snapshot: 2026-08-21. Source: [the gene ledger](docs/gene_ledger.json).
The ledger currently contains 70 genes: 12 `helps` (on by default), 11 `hurts`
(off), and 47 unresolved (off).

## How to read the table

Rows are ranked by native-regime win-rate delta, not by the default decision.
The numerical columns are percentage points (`pp`) from each gene's native
screen; the ledger records the authoritative verdict, default, and any
alternative-regime evidence. A gene is `helps` when its deciding regime reaches
`z >= 2` on win rate without contradictory share evidence, or conversely on
share without contradictory win evidence; `hurts` is the mirror rule.

The leading unresolved candidate, `camp-party`, is deliberately still off:
its native win `z` is +1.993, just below the promotion threshold. See the
[screen sources](docs/gene_screens/) and ledger for sample counts and per-regime
results.

| Rank | Gene | Verdict | Default | Win Δ (pp) | Win z | Share Δ (pp) | Share z |
| ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | `garrison-under-fire` | helps | on | +1.368 | +3.280 | +0.230 | +3.307 |
| 2 | `barbarian-scouts-are-scouts` | helps | on | +1.101 | +2.556 | +0.156 | +2.246 |
| 3 | `siege-tracks-wall` | helps | on | +1.011 | +2.398 | +0.044 | +0.621 |
| 4 | `war-reinforcement` | helps | on | +0.982 | +2.277 | +0.119 | +1.709 |
| 5 | `founder-temple` | helps | on | +0.967 | +2.136 | +0.019 | +0.490 |
| 6 | `camp-party` | unresolved | off | +0.863 | +1.993 | +0.100 | +1.434 |
| 7 | `whole-turn-backtrack-guard` | unresolved | off | +0.773 | +1.782 | +0.022 | +0.307 |
| 8 | `blind-objective-strength` | unresolved | off | +0.729 | +1.709 | +0.076 | +1.120 |
| 9 | `inquisition-on-threat` | unresolved | off | +0.700 | +1.529 | -0.025 | -0.640 |
| 10 | `score-horizon` | unresolved | off | +0.699 | +1.599 | +0.067 | +0.981 |
| 11 | `wide-map-capacity` | helps | on | +0.699 | +1.629 | +0.326 | +4.649 |
| 12 | `amenity-project-preemption` | unresolved | off | +0.669 | +1.539 | +0.102 | +1.482 |
| 13 | `loyalty-rate-alarm` | helps | on | +0.654 | +1.546 | +0.470 | +6.930 |
| 14 | `siege-role` | unresolved | off | +0.595 | +1.402 | -0.069 | -0.992 |
| 15 | `bounded-recovery` | helps | on | +0.580 | +1.319 | +0.166 | +2.414 |
| 16 | `recon-replacement` | unresolved | off | +0.580 | +1.360 | -0.112 | -1.594 |
| 17 | `slot-kind-tiebreak` | unresolved | off | +0.521 | +1.217 | -0.043 | -0.625 |
| 18 | `religion-sues-peace` | unresolved | off | +0.491 | +1.162 | +0.068 | +0.959 |
| 19 | `idle-faith-patronage` | helps | on | +0.467 | +2.276 | +0.062 | +3.959 |
| 20 | `buildings-before-projects` | helps | on | +0.461 | +1.072 | +0.232 | +3.374 |
| 21 | `one-launch-pad` | unresolved | off | +0.461 | +1.081 | +0.037 | +0.533 |
| 22 | `strategic-wonders` | unresolved | off | +0.416 | +0.977 | +0.044 | +0.630 |
| 23 | `strike-opening` | unresolved | off | +0.402 | +0.946 | +0.106 | +1.540 |
| 24 | `wonder-ring-settle-value` | unresolved | off | +0.312 | +0.711 | +0.003 | +0.048 |
| 25 | `suzerain-cards` | unresolved | off | +0.283 | +0.650 | -0.009 | -0.122 |
| 26 | `recorded-tactical-step` | helps | on | +0.268 | +0.621 | +0.160 | +2.346 |
| 27 | `step-and-reassess` | unresolved | off | +0.200 | +0.277 | +0.005 | +0.064 |
| 28 | `blind-objective-units` | unresolved | off | +0.119 | +0.279 | +0.020 | +0.284 |
| 29 | `stranded-settler-discount` | unresolved | off | +0.104 | +0.242 | +0.057 | +0.813 |
| 30 | `endgame-war-runway` | unresolved | off | +0.074 | +0.173 | -0.019 | -0.267 |
| 31 | `war-patience` | unresolved | off | +0.059 | +0.136 | -0.051 | -0.733 |
| 32 | `recon-flight` | unresolved | off | +0.030 | +0.069 | +0.050 | +0.720 |
| 33 | `escort-unstick` | hurts | off | -0.000 | -0.000 | -0.031 | -0.450 |
| 34 | `peacetime-deterrence` | unresolved | off | -0.000 | -0.000 | +0.074 | +1.114 |
| 35 | `housing-cards` | unresolved | off | -0.089 | -0.215 | -0.005 | -0.074 |
| 36 | `settler-guard-holds` | unresolved | off | -0.089 | -0.215 | +0.003 | +0.049 |
| 37 | `theology-for-founders` | unresolved | off | -0.100 | -0.213 | -0.028 | -0.733 |
| 38 | `barbarian-walls-one-tier` | unresolved | off | -0.119 | -0.277 | +0.039 | +0.564 |
| 39 | `civilian-rescue` | unresolved | off | -0.119 | -0.278 | +0.115 | +1.661 |
| 40 | `arrival-waves` | unresolved | off | -0.149 | -0.344 | -0.102 | -1.426 |
| 41 | `ranged-line-of-sight` | unresolved | off | -0.149 | -0.355 | -0.016 | -0.235 |
| 42 | `settler-target-hysteresis` | unresolved | off | -0.164 | -0.382 | -0.039 | -0.559 |
| 43 | `district-coverage` | unresolved | off | -0.208 | -0.492 | -0.059 | -0.850 |
| 44 | `relief-targets-the-siege` | unresolved | off | -0.223 | -0.514 | +0.001 | +0.019 |
| 45 | `amenity-district-path` | unresolved | off | -0.238 | -0.557 | -0.015 | -0.205 |
| 46 | `naval-recon` | unresolved | off | -0.253 | -0.585 | -0.096 | -1.369 |
| 47 | `housing-buildings` | unresolved | off | -0.283 | -0.655 | -0.120 | -1.743 |
| 48 | `come-ashore` | unresolved | off | -0.342 | -0.786 | +0.095 | +1.384 |
| 49 | `joint-tactics` | unresolved | off | -0.357 | -0.841 | +0.040 | +0.590 |
| 50 | `idle-walkers-close-the-pipeline` | unresolved | off | -0.372 | -0.864 | -0.034 | -0.485 |
| 51 | `camp-reach` | unresolved | off | -0.387 | -0.902 | -0.045 | -0.648 |
| 52 | `housing-districts` | unresolved | off | -0.387 | -0.892 | -0.061 | -0.881 |
| 53 | `settler-site-agreement` | unresolved | off | -0.416 | -0.975 | -0.007 | -0.107 |
| 54 | `housing-research` | hurts | off | -0.521 | -1.210 | -0.185 | -2.631 |
| 55 | `siege-muster` | helps | on | -0.521 | -1.208 | +0.216 | +3.133 |
| 56 | `holy-lane-parity` | unresolved | off | -0.533 | -1.193 | -0.050 | -1.341 |
| 57 | `home-defense` | unresolved | off | -0.595 | -1.384 | -0.021 | -0.297 |
| 58 | `army-target-weighs-enemy` | unresolved | off | -0.669 | -1.511 | -0.028 | -0.394 |
| 59 | `muster-at-command-radius` | unresolved | off | -0.669 | -1.543 | -0.099 | -1.369 |
| 60 | `siege-commitment` | unresolved | off | -0.803 | -1.869 | +0.018 | +0.267 |
| 61 | `apostle-promotion-by-role` | unresolved | off | -0.833 | -1.950 | -0.103 | -1.459 |
| 62 | `wonder-prereq-reach` | hurts | off | -0.907 | -2.085 | -0.025 | -0.357 |
| 63 | `loyalty-policy-defence` | hurts | off | -1.071 | -2.472 | -0.105 | -1.495 |
| 64 | `garrison-walls` | hurts | off | -1.086 | -2.529 | -0.130 | -1.864 |
| 65 | `governor-every-lane` | hurts | off | -1.116 | -2.516 | -3.377 | -51.501 |
| 66 | `siege-is-progress` | hurts | off | -1.279 | -3.023 | -0.093 | -1.297 |
| 67 | `campus-every-city` | hurts | off | -1.874 | -4.391 | -0.403 | -5.719 |
| 68 | `stacked-escort` | hurts | off | -2.082 | -4.814 | +0.034 | +0.486 |
| 69 | `settler-stack-discipline` | hurts | off | -2.320 | -5.476 | -0.309 | -4.430 |
| 70 | `war-economy` | hurts | off | -3.838 | -8.920 | -0.735 | -10.686 |
