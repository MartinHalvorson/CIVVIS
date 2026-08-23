# Standard gene screen — 10,000-total-seat calibrated results

_2026-08-22 · source b3ad9f00d56992b738cd5397ceac4cbb5c22e39b · release binary SHA-256 abbac3af9b1d24fc4ff8dfc6c38bbb5864ced29e9d0449401128733645c27f05_

## Scope

This is a result-only record of the completed standard screen.  It does not
update docs/gene_ledger.json, runtime defaults, or any game rule.  The screen
used six majors on a 74×46 Continents map with nine city-states, Online speed
through turn 250, all six victory lanes, shuffled civilizations, the
best-genome baseline, and an all-seats foldover.

## Games, wins, and seats

The headline counts are completed games, their recorded winners, and player
seats.  Each six-player game contributes six seats and exactly one winner.

| quantity | calculation | result |
|---|---:|---:|
| completed games | — | 7,874 |
| recorded wins | one winner per completed game | 7,874 |
| player seats | 7,874 games × 6 players | 47,244 |
| chance wins over the completed run | 47,244 seats × 1/6 | 7,874 |
| requested reporting basis | — | 10,000 total player seats |
| chance expectation at that basis | 10,000 seats × 1/6 | 1,666.667 wins (about 1,667) |
| per-gene split in the completed run | — | 23,622 treated seats + 23,622 control seats |
| per-gene split at the reporting basis | 10,000 total seats ÷ 2 | 5,000 treated seats + 5,000 control seats |
| excluded game-seat rows | — | 0 |

The completed sample is 4.724400× the 10,000-total-seat reporting basis, so
normalized totals use the factor 10,000 / 47,244 = 0.2116670900.  The
treatment/control split is balanced for every gene.

On−off win Δpp and win z remain the canonical treatment-versus-control
statistics.  The raw win columns below are a count check: treated wins plus
control wins equals 7,874 for every gene, one recorded winner per completed
game.  The excess columns use round((win_on − 1/6) × N_treated), where
N_treated is 23,622 for the completed data and 5,000 for the
10,000-total-seat display.  This changes only the reporting scale; it does not
add synthetic games or seats.

## Statistics

| gene | treated wins / 23,622 seats | control wins / 23,622 seats | on−off win Δpp | win z | treated excess @ 23,622 seats | treated excess @ 10,000 total seats |
|---|---:|---:|---:|---:|---:|---:|
| governor-victory-lanes | 3,378 / 23,622 | 4,496 / 23,622 | -4.73 | -15.37 | -559 | -118 |
| governor-every-lane | 3,384 / 23,622 | 4,490 / 23,622 | -4.68 | -15.12 | -553 | -117 |
| war-economy | 4,215 / 23,622 | 3,659 / 23,622 | +2.35 | +7.50 | +278 | +59 |
| air-surge | 4,191 / 23,622 | 3,683 / 23,622 | +2.15 | +6.99 | +254 | +54 |
| great-person-housing | 4,158 / 23,622 | 3,716 / 23,622 | +1.87 | +5.93 | +221 | +47 |
| wide-map-capacity | 4,151 / 23,622 | 3,723 / 23,622 | +1.81 | +5.84 | +214 | +45 |
| buildings-before-projects | 4,082 / 23,622 | 3,792 / 23,622 | +1.23 | +3.95 | +145 | +31 |
| contact-posture | 3,808 / 23,622 | 4,066 / 23,622 | -1.09 | -3.55 | -129 | -27 |
| raid-pillage-prizes | 4,062 / 23,622 | 3,812 / 23,622 | +1.06 | +3.42 | +125 | +26 |
| opportunistic-war | 4,053 / 23,622 | 3,821 / 23,622 | +0.98 | +3.14 | +116 | +25 |
| district-lookahead-settle | 3,840 / 23,622 | 4,034 / 23,622 | -0.82 | -2.65 | -97 | -21 |
| idle-faith-patronage | 4,028 / 23,622 | 3,846 / 23,622 | +0.77 | +2.50 | +91 | +19 |
| loyalty-rate-alarm | 4,027 / 23,622 | 3,847 / 23,622 | +0.76 | +2.42 | +90 | +19 |
| escort-unstick | 4,022 / 23,622 | 3,852 / 23,622 | +0.72 | +2.29 | +85 | +18 |
| housing-districts | 3,850 / 23,622 | 4,024 / 23,622 | -0.74 | -2.35 | -87 | -18 |
| amenity-project-preemption | 3,857 / 23,622 | 4,017 / 23,622 | -0.68 | -2.17 | -80 | -17 |
| war-reinforcement | 4,017 / 23,622 | 3,857 / 23,622 | +0.68 | +2.17 | +80 | +17 |
| guru-heals-the-corps | 3,868 / 23,622 | 4,006 / 23,622 | -0.58 | -1.83 | -69 | -15 |
| recon-replacement | 4,007 / 23,622 | 3,867 / 23,622 | +0.59 | +1.90 | +70 | +15 |
| bounded-recovery | 4,005 / 23,622 | 3,869 / 23,622 | +0.58 | +1.84 | +68 | +14 |
| garrison-under-fire | 3,873 / 23,622 | 4,001 / 23,622 | -0.54 | -1.75 | -64 | -14 |
| governor-expansion-lane | 3,872 / 23,622 | 4,002 / 23,622 | -0.55 | -1.76 | -65 | -14 |
| settle-sooner | 4,003 / 23,622 | 3,871 / 23,622 | +0.56 | +1.79 | +66 | +14 |
| war-patience | 3,872 / 23,622 | 4,002 / 23,622 | -0.55 | -1.77 | -65 | -14 |
| chain-tech-lookahead | 3,879 / 23,622 | 3,995 / 23,622 | -0.49 | -1.55 | -58 | -12 |
| culture-building-debt | 3,994 / 23,622 | 3,880 / 23,622 | +0.48 | +1.52 | +57 | +12 |
| district-coverage | 3,885 / 23,622 | 3,989 / 23,622 | -0.44 | -1.44 | -52 | -11 |
| settler-site-agreement | 3,883 / 23,622 | 3,991 / 23,622 | -0.46 | -1.47 | -54 | -11 |
| theology-for-founders | 3,990 / 23,622 | 3,884 / 23,622 | +0.45 | +1.43 | +53 | +11 |
| wonder-ring-settle-value | 3,988 / 23,622 | 3,886 / 23,622 | +0.43 | +1.37 | +51 | +11 |
| holy-lane-parity | 3,983 / 23,622 | 3,891 / 23,622 | +0.39 | +1.23 | +46 | +10 |
| naval-recon | 3,889 / 23,622 | 3,985 / 23,622 | -0.41 | -1.31 | -48 | -10 |
| peacetime-deterrence | 3,986 / 23,622 | 3,888 / 23,622 | +0.41 | +1.32 | +49 | +10 |
| research-grants-first | 3,889 / 23,622 | 3,985 / 23,622 | -0.41 | -1.32 | -48 | -10 |
| army-target-weighs-enemy | 3,980 / 23,622 | 3,894 / 23,622 | +0.36 | +1.15 | +43 | +9 |
| campus-adjacency-threshold | 3,895 / 23,622 | 3,979 / 23,622 | -0.36 | -1.15 | -42 | -9 |
| holy-site-where-the-threat-is | 3,893 / 23,622 | 3,981 / 23,622 | -0.37 | -1.21 | -44 | -9 |
| home-defense | 3,894 / 23,622 | 3,980 / 23,622 | -0.36 | -1.16 | -43 | -9 |
| housing-research | 3,896 / 23,622 | 3,978 / 23,622 | -0.35 | -1.10 | -41 | -9 |
| recorded-tactical-step | 3,979 / 23,622 | 3,895 / 23,622 | +0.36 | +1.14 | +42 | +9 |
| religion-sues-peace | 3,895 / 23,622 | 3,979 / 23,622 | -0.36 | -1.14 | -42 | -9 |
| research-floor-holds | 3,893 / 23,622 | 3,981 / 23,622 | -0.37 | -1.19 | -44 | -9 |
| score-horizon | 3,979 / 23,622 | 3,895 / 23,622 | +0.36 | +1.14 | +42 | +9 |
| settler-target-hysteresis | 3,894 / 23,622 | 3,980 / 23,622 | -0.36 | -1.16 | -43 | -9 |
| settler-threat-detour | 3,980 / 23,622 | 3,894 / 23,622 | +0.36 | +1.16 | +43 | +9 |
| amenity-district-path | 3,977 / 23,622 | 3,897 / 23,622 | +0.34 | +1.07 | +40 | +8 |
| apostle-promotion-by-role | 3,975 / 23,622 | 3,899 / 23,622 | +0.32 | +1.02 | +38 | +8 |
| barbarian-ranged-answer | 3,976 / 23,622 | 3,898 / 23,622 | +0.33 | +1.05 | +39 | +8 |
| camp-party | 3,901 / 23,622 | 3,973 / 23,622 | -0.30 | -0.96 | -36 | -8 |
| endgame-war-runway | 3,898 / 23,622 | 3,976 / 23,622 | -0.33 | -1.06 | -39 | -8 |
| spread-campaign-persists | 3,900 / 23,622 | 3,974 / 23,622 | -0.31 | -0.99 | -37 | -8 |
| culture-coverage | 3,904 / 23,622 | 3,970 / 23,622 | -0.28 | -0.91 | -33 | -7 |
| lane-congress-favor | 3,904 / 23,622 | 3,970 / 23,622 | -0.28 | -0.90 | -33 | -7 |
| barbarian-scouts-are-scouts | 3,966 / 23,622 | 3,908 / 23,622 | +0.25 | +0.79 | +29 | +6 |
| congress-counter-votes | 3,907 / 23,622 | 3,967 / 23,622 | -0.25 | -0.83 | -30 | -6 |
| fifteenth-citizen | 3,909 / 23,622 | 3,965 / 23,622 | -0.24 | -0.76 | -28 | -6 |
| priced-tile-purchase | 3,910 / 23,622 | 3,964 / 23,622 | -0.23 | -0.74 | -27 | -6 |
| religious-units-heal-first | 3,965 / 23,622 | 3,909 / 23,622 | +0.24 | +0.77 | +28 | +6 |
| strategic-wonders | 3,964 / 23,622 | 3,910 / 23,622 | +0.23 | +0.73 | +27 | +6 |
| barbarian-bargain | 3,961 / 23,622 | 3,913 / 23,622 | +0.20 | +0.65 | +24 | +5 |
| barbarian-hunt | 3,961 / 23,622 | 3,913 / 23,622 | +0.20 | +0.65 | +24 | +5 |
| builder-barbarian-safety | 3,914 / 23,622 | 3,960 / 23,622 | -0.19 | -0.62 | -23 | -5 |
| lane-space-race | 3,962 / 23,622 | 3,912 / 23,622 | +0.21 | +0.67 | +25 | +5 |
| one-launch-pad | 3,912 / 23,622 | 3,962 / 23,622 | -0.21 | -0.68 | -25 | -5 |
| science-payback-horizon | 3,912 / 23,622 | 3,962 / 23,622 | -0.21 | -0.68 | -25 | -5 |
| builder-worked-tile-priority | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.54 | -20 | -4 |
| condemn-under-congress | 3,919 / 23,622 | 3,955 / 23,622 | -0.15 | -0.49 | -18 | -4 |
| congress-banks-decided | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.55 | -20 | -4 |
| enhancer-for-the-corps | 3,955 / 23,622 | 3,919 / 23,622 | +0.15 | +0.48 | +18 | +4 |
| founder-temple | 3,954 / 23,622 | 3,920 / 23,622 | +0.14 | +0.46 | +17 | +4 |
| lane-congress-ballot | 3,956 / 23,622 | 3,918 / 23,622 | +0.16 | +0.51 | +19 | +4 |
| lane-culture-spending | 3,956 / 23,622 | 3,918 / 23,622 | +0.16 | +0.51 | +19 | +4 |
| one-shot-recovery | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.55 | -20 | -4 |
| power-the-laboratory | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.55 | -20 | -4 |
| religious-defence-scales | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.54 | -20 | -4 |
| science-multiplier-payoff | 3,919 / 23,622 | 3,955 / 23,622 | -0.15 | -0.49 | -18 | -4 |
| settler-guard-holds | 3,918 / 23,622 | 3,956 / 23,622 | -0.16 | -0.51 | -19 | -4 |
| siege-tracks-wall | 3,955 / 23,622 | 3,919 / 23,622 | +0.15 | +0.49 | +18 | +4 |
| stranded-settler-discount | 3,917 / 23,622 | 3,957 / 23,622 | -0.17 | -0.54 | -20 | -4 |
| envoy-infrastructure | 3,923 / 23,622 | 3,951 / 23,622 | -0.12 | -0.38 | -14 | -3 |
| relief-targets-the-siege | 3,951 / 23,622 | 3,923 / 23,622 | +0.12 | +0.38 | +14 | +3 |
| research-tier-premium | 3,923 / 23,622 | 3,951 / 23,622 | -0.12 | -0.38 | -14 | -3 |
| siege-commitment | 3,951 / 23,622 | 3,923 / 23,622 | +0.12 | +0.38 | +14 | +3 |
| siege-is-progress | 3,923 / 23,622 | 3,951 / 23,622 | -0.12 | -0.38 | -14 | -3 |
| whole-turn-backtrack-guard | 3,952 / 23,622 | 3,922 / 23,622 | +0.13 | +0.40 | +15 | +3 |
| campus-finishes-first | 3,946 / 23,622 | 3,928 / 23,622 | +0.08 | +0.25 | +9 | +2 |
| come-ashore | 3,948 / 23,622 | 3,926 / 23,622 | +0.09 | +0.29 | +11 | +2 |
| strike-opening | 3,947 / 23,622 | 3,927 / 23,622 | +0.08 | +0.27 | +10 | +2 |
| barbarian-capture-priority | 3,940 / 23,622 | 3,934 / 23,622 | +0.03 | +0.08 | +3 | +1 |
| civilian-rescue | 3,934 / 23,622 | 3,940 / 23,622 | -0.03 | -0.08 | -3 | -1 |
| early-contact-window | 3,941 / 23,622 | 3,933 / 23,622 | +0.03 | +0.11 | +4 | +1 |
| inquisition-on-threat | 3,943 / 23,622 | 3,931 / 23,622 | +0.05 | +0.16 | +6 | +1 |
| lane-great-people | 3,930 / 23,622 | 3,944 / 23,622 | -0.06 | -0.19 | -7 | -1 |
| blind-objective-strength | 3,936 / 23,622 | 3,938 / 23,622 | -0.01 | -0.03 | -1 | +0 |
| blind-objective-units | 3,936 / 23,622 | 3,938 / 23,622 | -0.01 | -0.03 | -1 | +0 |
| competition-victory-points | 3,936 / 23,622 | 3,938 / 23,622 | -0.01 | -0.03 | -1 | +0 |
| district-building-chain | 3,935 / 23,622 | 3,939 / 23,622 | -0.02 | -0.05 | -2 | +0 |
| lane-policy-deck | 3,937 / 23,622 | 3,937 / 23,622 | +0.00 | +0.00 | +0 | +0 |
| slot-kind-tiebreak | 3,935 / 23,622 | 3,939 / 23,622 | -0.02 | -0.05 | -2 | +0 |

## Provenance

The batch began at seed 141,000,000 and completed through 141,003,936.  The
canonical analyzer output has 99 screened genes and a family-wise threshold of
`|z| ≥ 3.478`.  It was finalized from the 7,874 complete games; no game-seat
rows were excluded before the treatment/control contrast or the seat-normalized
operational counts were calculated.
