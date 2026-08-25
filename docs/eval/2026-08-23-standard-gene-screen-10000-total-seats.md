# Standard gene screen — 10,000-total-seat calibrated results

_2026-08-23 · source `5afee5666f0850484e6b7a63c91c671e668475f5` · release binary SHA-256 `704907b03bc5c68378ba501ca3bb81254cbbe68f79e49574947222979e0cac86`_

## Scope

This is a result-only record of the completed current-standard screen. It does not update `docs/gene_ledger.json`, runtime defaults, or any game rule. The screen used six majors on a 74×46 Continents map with nine city-states, Online speed through turn 250, all six victory lanes, shuffled civilizations, the best-genome baseline, and an all-seats independent draw.

## Games, wins, and seats

The headline counts are completed games, their recorded winners, and player seats. Each six-player game contributes six seats and exactly one winner.

| quantity | calculation | result |
|---|---:|---:|
| completed games | — | 1,667 |
| recorded wins | one winner per completed game | 1,667 |
| player seats | 1,667 games × 6 players | 10,002 |
| chance wins over the completed run | 10,002 seats × 1/6 | 1,667 |
| requested reporting basis | — | 10,000 total player seats |
| chance expectation at that basis | 10,000 seats × 1/6 | 1,666.667 wins (about 1,667) |
| on-arm seat range across genes | independently drawn | 4,825–7,608 |
| off-arm seat range across genes | independently drawn | 2,394–5,177 |
| excluded game-seat rows | — | 0 |

The completed sample is 1.000200× the 10,000-total-seat reporting basis, so normalized excess totals use the factor 10,000 / 10,002 = 0.9998000400. The on/off arms are independently drawn rather than mechanically balanced, so their actual seat counts remain visible in every table row.

On−off win Δpp and win z remain the canonical gene statistics. The raw win columns below are a count check: on wins plus off wins equals 1,667 for every gene, one recorded winner per completed game. The excess columns use `(on wins − on seats / 6)` and scale that unrounded quantity by 10,000 / 10,002 for the 10,000-total-seat display. This changes only the reporting scale; it does not add synthetic games or seats.

## Statistics

| gene | on wins / on seats | off wins / off seats | on−off win Δpp | win z | on-arm excess @ actual on seats | on-arm excess @ 10,000 total seats |
|---|---:|---:|---:|---:|---:|---:|
| `engine-faith-price` | 890 / 4,989 | 777 / 5,013 | +2.34 | +3.13 | +58.5 | +58.5 |
| `air-surge` | 1,306 / 7,578 | 361 / 2,424 | +2.34 | +2.78 | +43.0 | +43.0 |
| `great-person-housing` | 1,293 / 7,511 | 374 / 2,491 | +2.20 | +2.64 | +41.2 | +41.2 |
| `recon-replacement` | 1,291 / 7,511 | 376 / 2,491 | +2.09 | +2.52 | +39.2 | +39.2 |
| `war-economy` | 1,295 / 7,547 | 372 / 2,455 | +2.01 | +2.35 | +37.2 | +37.2 |
| `bounded-recovery` | 1,278 / 7,447 | 389 / 2,555 | +1.94 | +2.34 | +36.8 | +36.8 |
| `builder-worked-tile-priority` | 880 / 5,033 | 787 / 4,969 | +1.65 | +2.19 | +41.2 | +41.2 |
| `wide-map-capacity` | 1,278 / 7,475 | 389 / 2,527 | +1.70 | +2.07 | +32.2 | +32.2 |
| `idle-faith-patronage` | 1,278 / 7,485 | 389 / 2,517 | +1.62 | +1.92 | +30.5 | +30.5 |
| `garrison-under-fire` | 861 / 4,968 | 806 / 5,034 | +1.32 | +1.77 | +33.0 | +33.0 |
| `opportunistic-war` | 1,281 / 7,522 | 386 / 2,480 | +1.47 | +1.72 | +27.3 | +27.3 |
| `barbarian-scouts-are-scouts` | 1,286 / 7,562 | 381 / 2,440 | +1.39 | +1.62 | +25.7 | +25.7 |
| `religious-defence-scales` | 863 / 4,997 | 804 / 5,005 | +1.21 | +1.60 | +30.2 | +30.2 |
| `apostle-promotion-by-role` | 1,272 / 7,489 | 395 / 2,513 | +1.27 | +1.46 | +23.8 | +23.8 |
| `raid-pillage-prizes` | 1,288 / 7,608 | 379 / 2,394 | +1.10 | +1.27 | +20.0 | +20.0 |
| `tactical-strategy` | 850 / 4,959 | 817 / 5,043 | +0.94 | +1.27 | +23.5 | +23.5 |
| `lane-policy-deck` | 850 / 4,969 | 817 / 5,033 | +0.87 | +1.19 | +21.8 | +21.8 |
| `barbarian-ranged-answer` | 1,271 / 7,520 | 396 / 2,482 | +0.95 | +1.13 | +17.7 | +17.7 |
| `buildings-before-projects` | 1,252 / 7,406 | 415 / 2,596 | +0.92 | +1.10 | +17.7 | +17.7 |
| `research-grants-first` | 851 / 4,983 | 816 / 5,019 | +0.82 | +1.09 | +20.5 | +20.5 |
| `spread-campaign-persists` | 862 / 5,060 | 805 / 4,942 | +0.75 | +1.02 | +18.7 | +18.7 |
| `war-reinforcement` | 1,266 / 7,500 | 401 / 2,502 | +0.85 | +0.99 | +16.0 | +16.0 |
| `settle-sooner` | 1,259 / 7,461 | 408 / 2,541 | +0.82 | +0.96 | +15.5 | +15.5 |
| `lane-culture-spending` | 861 / 5,058 | 806 / 4,944 | +0.72 | +0.95 | +18.0 | +18.0 |
| `come-ashore` | 1,265 / 7,502 | 402 / 2,500 | +0.78 | +0.93 | +14.7 | +14.7 |
| `religious-units-heal-first` | 834 / 4,906 | 833 / 5,096 | +0.65 | +0.89 | +16.3 | +16.3 |
| `enhancer-for-the-corps` | 855 / 5,033 | 812 / 4,969 | +0.65 | +0.86 | +16.2 | +16.2 |
| `guru-heals-the-corps` | 851 / 5,009 | 816 / 4,993 | +0.65 | +0.86 | +16.2 | +16.2 |
| `priced-tile-purchase` | 846 / 4,987 | 821 / 5,015 | +0.59 | +0.79 | +14.8 | +14.8 |
| `congress-counter-votes` | 858 / 5,062 | 809 / 4,940 | +0.57 | +0.77 | +14.3 | +14.3 |
| `settler-target-hysteresis` | 851 / 5,018 | 816 / 4,984 | +0.59 | +0.77 | +14.7 | +14.7 |
| `wonder-ring-settle-value` | 1,256 / 7,464 | 411 / 2,538 | +0.63 | +0.76 | +12.0 | +12.0 |
| `settler-site-agreement` | 846 / 4,997 | 821 / 5,005 | +0.53 | +0.73 | +13.2 | +13.2 |
| `district-coverage` | 864 / 5,106 | 803 / 4,896 | +0.52 | +0.69 | +13.0 | +13.0 |
| `fortify-idle-units` | 859 / 5,082 | 808 / 4,920 | +0.48 | +0.65 | +12.0 | +12.0 |
| `religion-sues-peace` | 838 / 4,956 | 829 / 5,046 | +0.48 | +0.64 | +12.0 | +12.0 |
| `settler-guard-holds` | 853 / 5,046 | 814 / 4,956 | +0.48 | +0.64 | +12.0 | +12.0 |
| `governor-expansion-lane` | 839 / 4,966 | 828 / 5,036 | +0.45 | +0.60 | +11.3 | +11.3 |
| `campus-adjacency-threshold` | 836 / 4,954 | 831 / 5,048 | +0.41 | +0.56 | +10.3 | +10.3 |
| `relief-targets-the-siege` | 1,259 / 7,501 | 408 / 2,501 | +0.47 | +0.55 | +8.8 | +8.8 |
| `maintenance-aware-deck` | 831 / 4,929 | 836 / 5,073 | +0.38 | +0.52 | +9.5 | +9.5 |
| `competition-victory-points` | 830 / 4,924 | 837 / 5,078 | +0.37 | +0.50 | +9.3 | +9.3 |
| `one-launch-pad` | 837 / 4,972 | 830 / 5,030 | +0.33 | +0.45 | +8.3 | +8.3 |
| `fifteenth-citizen` | 829 / 4,928 | 838 / 5,074 | +0.31 | +0.40 | +7.7 | +7.7 |
| `builder-barbarian-safety` | 841 / 5,001 | 826 / 5,001 | +0.30 | +0.40 | +7.5 | +7.5 |
| `power-the-laboratory` | 831 / 4,941 | 836 / 5,061 | +0.30 | +0.40 | +7.5 | +7.5 |
| `lane-congress-favor` | 837 / 4,979 | 830 / 5,023 | +0.29 | +0.39 | +7.2 | +7.2 |
| `unit-cost-efficiency` | 834 / 4,965 | 833 / 5,037 | +0.26 | +0.35 | +6.5 | +6.5 |
| `coupled-expansion` | 831 / 4,950 | 836 / 5,052 | +0.24 | +0.32 | +6.0 | +6.0 |
| `housing-districts` | 829 / 4,941 | 838 / 5,061 | +0.22 | +0.29 | +5.5 | +5.5 |
| `whole-turn-backtrack-guard` | 1,248 / 7,461 | 419 / 2,541 | +0.24 | +0.28 | +4.5 | +4.5 |
| `lane-congress-ballot` | 833 / 4,972 | 834 / 5,030 | +0.17 | +0.23 | +4.3 | +4.3 |
| `civilian-rescue` | 836 / 4,991 | 831 / 5,011 | +0.17 | +0.22 | +4.2 | +4.2 |
| `recorded-tactical-step` | 1,256 / 7,516 | 411 / 2,486 | +0.18 | +0.21 | +3.3 | +3.3 |
| `theology-for-founders` | 835 / 4,987 | 832 / 5,015 | +0.15 | +0.20 | +3.8 | +3.8 |
| `holy-site-where-the-threat-is` | 829 / 4,954 | 838 / 5,048 | +0.13 | +0.18 | +3.3 | +3.3 |
| `blind-objective-strength` | 842 / 5,035 | 825 / 4,967 | +0.11 | +0.15 | +2.8 | +2.8 |
| `loyalty-rate-alarm` | 1,265 / 7,577 | 402 / 2,425 | +0.12 | +0.14 | +2.2 | +2.2 |
| `settler-threat-detour` | 1,257 / 7,530 | 410 / 2,472 | +0.11 | +0.13 | +2.0 | +2.0 |
| `camp-party` | 839 / 5,020 | 828 / 4,982 | +0.09 | +0.12 | +2.3 | +2.3 |
| `price-the-suzerainty` | 843 / 5,046 | 824 / 4,956 | +0.08 | +0.11 | +2.0 | +2.0 |
| `settlement-gap-target` | 841 / 5,037 | 826 / 4,965 | +0.06 | +0.08 | +1.5 | +1.5 |
| `research-tier-premium` | 834 / 4,996 | 833 / 5,006 | +0.05 | +0.07 | +1.3 | +1.3 |
| `strategic-wonders` | 843 / 5,051 | 824 / 4,951 | +0.05 | +0.06 | +1.2 | +1.2 |
| `stranded-settler-discount` | 833 / 4,993 | 834 / 5,009 | +0.03 | +0.04 | +0.8 | +0.8 |
| `culture-coverage` | 827 / 4,963 | 840 / 5,039 | -0.01 | -0.01 | -0.2 | -0.2 |
| `escort-unstick` | 1,256 / 7,539 | 411 / 2,463 | -0.03 | -0.03 | -0.5 | -0.5 |
| `amenity-project-preemption` | 841 / 5,051 | 826 / 4,951 | -0.03 | -0.04 | -0.8 | -0.8 |
| `slot-kind-tiebreak` | 824 / 4,950 | 843 / 5,052 | -0.04 | -0.05 | -1.0 | -1.0 |
| `culture-building-debt` | 1,249 / 7,501 | 418 / 2,501 | -0.06 | -0.07 | -1.2 | -1.2 |
| `housing-research` | 843 / 5,069 | 824 / 4,933 | -0.07 | -0.10 | -1.8 | -1.8 |
| `contact-posture` | 840 / 5,052 | 827 / 4,950 | -0.08 | -0.11 | -2.0 | -2.0 |
| `peacetime-deterrence` | 1,240 / 7,453 | 427 / 2,549 | -0.11 | -0.13 | -2.2 | -2.2 |
| `strike-opening` | 1,239 / 7,454 | 428 / 2,548 | -0.18 | -0.20 | -3.3 | -3.3 |
| `promote-when-wounded` | 841 / 5,069 | 826 / 4,933 | -0.15 | -0.20 | -3.8 | -3.8 |
| `amenity-district-path` | 1,245 / 7,493 | 422 / 2,509 | -0.20 | -0.24 | -3.8 | -3.8 |
| `science-multiplier-payoff` | 833 / 5,027 | 834 / 4,975 | -0.19 | -0.26 | -4.8 | -4.8 |
| `unit-objective-memory` | 823 / 4,968 | 844 / 5,034 | -0.20 | -0.26 | -5.0 | -5.0 |
| `inquisition-on-threat` | 1,240 / 7,468 | 427 / 2,534 | -0.25 | -0.29 | -4.7 | -4.7 |
| `barbarian-bargain` | 1,243 / 7,487 | 424 / 2,515 | -0.26 | -0.30 | -4.8 | -4.8 |
| `congress-banks-decided` | 839 / 5,069 | 828 / 4,933 | -0.23 | -0.31 | -5.8 | -5.8 |
| `district-lookahead-settle` | 822 / 4,968 | 845 / 5,034 | -0.24 | -0.32 | -6.0 | -6.0 |
| `condemn-under-congress` | 832 / 5,032 | 835 / 4,970 | -0.27 | -0.35 | -6.7 | -6.7 |
| `siege-commitment` | 831 / 5,026 | 836 / 4,976 | -0.27 | -0.36 | -6.7 | -6.7 |
| `holy-lane-parity` | 1,234 / 7,441 | 433 / 2,561 | -0.32 | -0.38 | -6.2 | -6.2 |
| `one-shot-recovery` | 827 / 5,009 | 840 / 4,993 | -0.31 | -0.42 | -7.8 | -7.8 |
| `lane-great-people` | 845 / 5,125 | 822 / 4,877 | -0.37 | -0.49 | -9.2 | -9.2 |
| `lane-space-race` | 813 / 4,934 | 854 / 5,068 | -0.37 | -0.51 | -9.3 | -9.3 |
| `campus-finishes-first` | 817 / 4,959 | 850 / 5,043 | -0.38 | -0.51 | -9.5 | -9.5 |
| `barbarian-capture-priority` | 818 / 4,968 | 849 / 5,034 | -0.40 | -0.54 | -10.0 | -10.0 |
| `endgame-war-runway` | 814 / 4,964 | 853 / 5,038 | -0.53 | -0.71 | -13.3 | -13.3 |
| `naval-recon` | 806 / 4,923 | 861 / 5,079 | -0.58 | -0.78 | -14.5 | -14.5 |
| `army-target-weighs-enemy` | 1,230 / 7,458 | 437 / 2,544 | -0.69 | -0.79 | -13.0 | -13.0 |
| `builder-reward-survey` | 825 / 5,050 | 842 / 4,952 | -0.67 | -0.90 | -16.7 | -16.7 |
| `home-defense` | 820 / 5,023 | 847 / 4,979 | -0.69 | -0.90 | -17.2 | -17.2 |
| `founder-temple` | 1,250 / 7,588 | 417 / 2,414 | -0.80 | -0.92 | -14.7 | -14.7 |
| `science-payback-horizon` | 806 / 4,941 | 861 / 5,061 | -0.70 | -0.94 | -17.5 | -17.5 |
| `barbarian-hunt` | 816 / 5,019 | 851 / 4,983 | -0.82 | -1.09 | -20.5 | -20.5 |
| `chain-tech-lookahead` | 804 / 4,975 | 863 / 5,027 | -1.01 | -1.35 | -25.2 | -25.2 |
| `coordinated-finish` | 802 / 4,966 | 865 / 5,036 | -1.03 | -1.36 | -25.7 | -25.7 |
| `siege-is-progress` | 817 / 5,059 | 850 / 4,943 | -1.05 | -1.42 | -26.2 | -26.2 |
| `research-floor-holds` | 808 / 5,006 | 859 / 4,996 | -1.05 | -1.42 | -26.3 | -26.3 |
| `envoy-infrastructure` | 805 / 5,004 | 862 / 4,998 | -1.16 | -1.58 | -29.0 | -29.0 |
| `pantheon-board` | 789 / 4,922 | 878 / 5,080 | -1.25 | -1.68 | -31.3 | -31.3 |
| `siege-tracks-wall` | 800 / 5,001 | 867 / 5,001 | -1.34 | -1.81 | -33.5 | -33.5 |
| `early-contact-window` | 809 / 5,057 | 858 / 4,945 | -1.35 | -1.84 | -33.8 | -33.8 |
| `score-horizon` | 1,228 / 7,552 | 439 / 2,450 | -1.66 | -1.87 | -30.7 | -30.7 |
| `blind-objective-units` | 799 / 5,012 | 868 / 4,990 | -1.45 | -1.96 | -36.3 | -36.3 |
| `naval-production-policy` | 762 / 4,825 | 905 / 5,177 | -1.69 | -2.23 | -42.2 | -42.2 |
| `district-building-chain` | 773 / 4,903 | 894 / 5,099 | -1.77 | -2.33 | -44.2 | -44.2 |
| `war-patience` | 783 / 5,080 | 884 / 4,922 | -2.55 | -3.41 | -63.7 | -63.7 |
| `governor-victory-lanes` | 758 / 5,041 | 909 / 4,961 | -3.29 | -4.35 | -82.2 | -82.2 |
| `governor-every-lane` | 727 / 5,013 | 940 / 4,989 | -4.34 | -5.91 | -108.5 | -108.5 |

## Provenance

The batch began at seed 168,000,000 and completed through 168,001,666. The canonical analyzer output has 113 screened genes and a family-wise threshold of `|z| ≥ 3.513`. It was finalized from 1,667 complete games / 10,002 player seats; no game-seat rows were excluded before the contrasts or the seat-normalized operational counts were calculated.
