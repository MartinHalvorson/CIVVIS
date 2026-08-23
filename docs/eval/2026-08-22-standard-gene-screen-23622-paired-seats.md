# Standard gene screen — 10,000-seat calibrated results

_2026-08-22 · source `b3ad9f00d56992b738cd5397ceac4cbb5c22e39b` · release binary SHA-256 `abbac3af9b1d24fc4ff8dfc6c38bbb5864ced29e9d0449401128733645c27f05`_

## Scope

This is a result-only record of the completed standard screen.  It does not
update `docs/gene_ledger.json`, runtime defaults, or any game rule.  The screen
used six majors on a 74×46 Continents map with nine city-states, Online speed
through turn 250, all six victory lanes, shuffled civilizations, the
best-genome baseline, and an all-seats foldover.

## Why the reporting unit is seats

The ranking's expected **1,667** wins is the six-player chance expectation for
**10,000 on-arm seats**, not 10,000 raw games: `10,000 × 1/6 = 1,666.667`.
Each all-seats foldover map pair contributes six matched seat comparisons per
gene.  One comparison contains one on-arm seat and its matched off-arm seat,
so every gene receives the same number of observations in both arms.

| quantity | calculation | result |
|---|---:|---:|
| complete map pairs | — | 3,937 |
| raw games | 3,937 × 2 | 7,874 |
| game-seat rows, both arms | 7,874 × 6 | 47,244 |
| matched seat comparisons per gene | 3,937 × 6 | 23,622 |
| on-arm seats per gene | — | 23,622 |
| off-arm seats per gene | — | 23,622 |
| requested reporting basis | — | 10,000 paired seats |
| chance expectation on that basis | 10,000 × 1/6 | 1,666.667 wins (about 1,667) |
| nearest whole-map-pair boundary | 1,667 × 6 | 10,002 paired seats |

All 23,622 comparisons are complete; no game-seat rows were excluded.  The run
therefore completed 2.362200× the requested 10,000-seat reporting basis.  The
operational totals below use the factor `10,000 / 23,622 = 0.4233341800`.

`on−off win Δpp` and `win z` remain the canonical paired-analysis statistics.
For the two excess columns, the calculation is
`round((win_on − 1/6) × N)`, using the unrounded on-arm win rate.  This
normalizes the observed rate to the requested number of seats; it does not add
synthetic games or seats.

## Statistics

| gene | on−off win Δpp | win z | on-arm excess @ 23,622 paired seats | on-arm excess @ 10,000 paired seats |
|---|---:|---:|---:|---:|
| `governor-victory-lanes` | -4.73 | -15.37 | -559 | -237 |
| `governor-every-lane` | -4.68 | -15.12 | -553 | -234 |
| `war-economy` | +2.35 | +7.50 | +278 | +118 |
| `air-surge` | +2.15 | +6.99 | +254 | +108 |
| `great-person-housing` | +1.87 | +5.93 | +221 | +94 |
| `wide-map-capacity` | +1.81 | +5.84 | +214 | +91 |
| `buildings-before-projects` | +1.23 | +3.95 | +145 | +61 |
| `contact-posture` | -1.09 | -3.55 | -129 | -55 |
| `raid-pillage-prizes` | +1.06 | +3.42 | +125 | +53 |
| `opportunistic-war` | +0.98 | +3.14 | +116 | +49 |
| `district-lookahead-settle` | -0.82 | -2.65 | -97 | -41 |
| `idle-faith-patronage` | +0.77 | +2.50 | +91 | +39 |
| `loyalty-rate-alarm` | +0.76 | +2.42 | +90 | +38 |
| `housing-districts` | -0.74 | -2.35 | -87 | -37 |
| `escort-unstick` | +0.72 | +2.29 | +85 | +36 |
| `amenity-project-preemption` | -0.68 | -2.17 | -80 | -34 |
| `war-reinforcement` | +0.68 | +2.17 | +80 | +34 |
| `recon-replacement` | +0.59 | +1.90 | +70 | +30 |
| `bounded-recovery` | +0.58 | +1.84 | +68 | +29 |
| `guru-heals-the-corps` | -0.58 | -1.83 | -69 | -29 |
| `governor-expansion-lane` | -0.55 | -1.76 | -65 | -28 |
| `settle-sooner` | +0.56 | +1.79 | +66 | +28 |
| `war-patience` | -0.55 | -1.77 | -65 | -28 |
| `garrison-under-fire` | -0.54 | -1.75 | -64 | -27 |
| `chain-tech-lookahead` | -0.49 | -1.55 | -58 | -25 |
| `culture-building-debt` | +0.48 | +1.52 | +57 | +24 |
| `settler-site-agreement` | -0.46 | -1.47 | -54 | -23 |
| `district-coverage` | -0.44 | -1.44 | -52 | -22 |
| `theology-for-founders` | +0.45 | +1.43 | +53 | +22 |
| `wonder-ring-settle-value` | +0.43 | +1.37 | +51 | +22 |
| `peacetime-deterrence` | +0.41 | +1.32 | +49 | +21 |
| `naval-recon` | -0.41 | -1.31 | -48 | -20 |
| `research-grants-first` | -0.41 | -1.32 | -48 | -20 |
| `holy-lane-parity` | +0.39 | +1.23 | +46 | +19 |
| `holy-site-where-the-threat-is` | -0.37 | -1.21 | -44 | -19 |
| `research-floor-holds` | -0.37 | -1.19 | -44 | -19 |
| `army-target-weighs-enemy` | +0.36 | +1.15 | +43 | +18 |
| `campus-adjacency-threshold` | -0.36 | -1.15 | -42 | -18 |
| `home-defense` | -0.36 | -1.16 | -43 | -18 |
| `recorded-tactical-step` | +0.36 | +1.14 | +42 | +18 |
| `religion-sues-peace` | -0.36 | -1.14 | -42 | -18 |
| `score-horizon` | +0.36 | +1.14 | +42 | +18 |
| `settler-target-hysteresis` | -0.36 | -1.16 | -43 | -18 |
| `settler-threat-detour` | +0.36 | +1.16 | +43 | +18 |
| `amenity-district-path` | +0.34 | +1.07 | +40 | +17 |
| `barbarian-ranged-answer` | +0.33 | +1.05 | +39 | +17 |
| `endgame-war-runway` | -0.33 | -1.06 | -39 | -17 |
| `housing-research` | -0.35 | -1.10 | -41 | -17 |
| `apostle-promotion-by-role` | +0.32 | +1.02 | +38 | +16 |
| `spread-campaign-persists` | -0.31 | -0.99 | -37 | -16 |
| `camp-party` | -0.30 | -0.96 | -36 | -15 |
| `culture-coverage` | -0.28 | -0.91 | -33 | -14 |
| `lane-congress-favor` | -0.28 | -0.90 | -33 | -14 |
| `congress-counter-votes` | -0.25 | -0.83 | -30 | -13 |
| `barbarian-scouts-are-scouts` | +0.25 | +0.79 | +29 | +12 |
| `fifteenth-citizen` | -0.24 | -0.76 | -28 | -12 |
| `religious-units-heal-first` | +0.24 | +0.77 | +28 | +12 |
| `lane-space-race` | +0.21 | +0.67 | +25 | +11 |
| `one-launch-pad` | -0.21 | -0.68 | -25 | -11 |
| `priced-tile-purchase` | -0.23 | -0.74 | -27 | -11 |
| `science-payback-horizon` | -0.21 | -0.68 | -25 | -11 |
| `strategic-wonders` | +0.23 | +0.73 | +27 | +11 |
| `barbarian-bargain` | +0.20 | +0.65 | +24 | +10 |
| `barbarian-hunt` | +0.20 | +0.65 | +24 | +10 |
| `builder-barbarian-safety` | -0.19 | -0.62 | -23 | -10 |
| `builder-worked-tile-priority` | -0.17 | -0.54 | -20 | -8 |
| `condemn-under-congress` | -0.15 | -0.49 | -18 | -8 |
| `congress-banks-decided` | -0.17 | -0.55 | -20 | -8 |
| `enhancer-for-the-corps` | +0.15 | +0.48 | +18 | +8 |
| `lane-congress-ballot` | +0.16 | +0.51 | +19 | +8 |
| `lane-culture-spending` | +0.16 | +0.51 | +19 | +8 |
| `one-shot-recovery` | -0.17 | -0.55 | -20 | -8 |
| `power-the-laboratory` | -0.17 | -0.55 | -20 | -8 |
| `religious-defence-scales` | -0.17 | -0.54 | -20 | -8 |
| `science-multiplier-payoff` | -0.15 | -0.49 | -18 | -8 |
| `settler-guard-holds` | -0.16 | -0.51 | -19 | -8 |
| `siege-tracks-wall` | +0.15 | +0.49 | +18 | +8 |
| `stranded-settler-discount` | -0.17 | -0.54 | -20 | -8 |
| `founder-temple` | +0.14 | +0.46 | +17 | +7 |
| `envoy-infrastructure` | -0.12 | -0.38 | -14 | -6 |
| `relief-targets-the-siege` | +0.12 | +0.38 | +14 | +6 |
| `research-tier-premium` | -0.12 | -0.38 | -14 | -6 |
| `siege-commitment` | +0.12 | +0.38 | +14 | +6 |
| `siege-is-progress` | -0.12 | -0.38 | -14 | -6 |
| `whole-turn-backtrack-guard` | +0.13 | +0.40 | +15 | +6 |
| `come-ashore` | +0.09 | +0.29 | +11 | +5 |
| `campus-finishes-first` | +0.08 | +0.25 | +9 | +4 |
| `strike-opening` | +0.08 | +0.27 | +10 | +4 |
| `inquisition-on-threat` | +0.05 | +0.16 | +6 | +3 |
| `lane-great-people` | -0.06 | -0.19 | -7 | -3 |
| `early-contact-window` | +0.03 | +0.11 | +4 | +2 |
| `barbarian-capture-priority` | +0.03 | +0.08 | +3 | +1 |
| `civilian-rescue` | -0.03 | -0.08 | -3 | -1 |
| `district-building-chain` | -0.02 | -0.05 | -2 | -1 |
| `slot-kind-tiebreak` | -0.02 | -0.05 | -2 | -1 |
| `blind-objective-strength` | -0.01 | -0.03 | -1 | +0 |
| `blind-objective-units` | -0.01 | -0.03 | -1 | +0 |
| `competition-victory-points` | -0.01 | -0.03 | -1 | +0 |
| `lane-policy-deck` | +0.00 | +0.00 | +0 | +0 |

## Provenance

The batch began at seed 141,000,000 and completed through 141,003,936.  The
canonical analyzer output has 99 screened genes and a family-wise threshold of
`|z| ≥ 3.478`.  It was finalized from the exact complete map pairs only;
interrupted rows would have been excluded before either the paired contrast or
the seat-normalized operational counts were calculated.
