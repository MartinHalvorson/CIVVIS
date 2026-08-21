# The heuristic gene ranking

Every screenable heuristic gene with a native measurement on the Advanced controller, ranked most beneficial to least by **wins added per 10,000 six-player games** at its measured on-rate. Screenable genes awaiting their first native measurement are listed separately below without a rank. Each gene's row comes from the **latest native screen that measured it** (the column *Source*), so a gene added after the whole-genome screen appears from its own screen; the *War* column is the same figure from the latest `domination,score` screen; *Default* is the deployment ledger's verdict (`docs/gene_ledger.json`, native regime governing, war filling in). Every screen is a foldover against the best-genome baseline with shuffled civs and every major seat carrying its own genome (errors clustered by game pair), so a gene's on/off columns cover the same maps. `docs/GENE_SCREEN.md` documents the instrument.

**Reading the table.** A six-player seat wins 1-in-6 by chance (1-in-4 in a four-player war screen), so *Wins ±10k* is how many wins above or below chance a seat carrying the gene collects per 10,000 games; the whole-genome screen resolves ±1.1 pp (≈ ±110 wins per 10,000) at 80% power and a single-gene 6,000-seat-pair screen ±1.3 pp — differences inside that band are noise, not nulls. `z` is the paired on−off win contrast; `share z` the score-share contrast, which resolves an edge at a fraction of the games a win count needs.

Regenerate with `python3 tools/heuristic_gene_ranking.py --write` after every screen enters the ledger; `tools/test_heuristic_gene_ranking.py` fails when this file is older than the ledger's sources.

| Rank | Wins ±10k | Gene | Default | Description | Win rate (on) | Win rate (off) | Games (on/off) | z | share z | War ±10k | Source |
|---:|---:|---|---|---|---:|---:|---:|---:|---:|---:|---|
| 1 | +81 | `recon-replacement` | **on** (helps) | Rebuild the recon arm when it is gone and there is ground left to chart. | 17.48% | 15.85% | 15,000/15,000 | +4.09 | +0.34 | +72 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 2 | +73 | `loyalty-rate-alarm` | **on** (helps) | Rank loyalty emergencies by turns-to-flip instead of by level. | 17.40% | 15.93% | 15,000/15,000 | +3.71 | +9.20 | +111 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 3 | +61 | `barbarian-scouts-are-scouts` | **on** (helps) | Stop pricing a Firaxis barbarian scout as a threat. | 17.27% | 16.06% | 15,000/15,000 | +3.02 | +1.22 | +96 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 4 | +53 | `camp-party` | **on** (helps) | The peacetime camp party. | 17.19% | 16.14% | 15,000/15,000 | +2.69 | -0.36 | +10 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 5 | +48 | `founder-temple` | **on** (helps) | A founder outside the Religion lane still builds its Shrine and Temple. | 17.15% | 16.18% | 6,000/6,000 | +2.14 | +0.49 | -27 | `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` |
| 6 | +39 | `bounded-recovery` | **on** (helps) | Stop the defensive-war posture from becoming permanent. | 17.06% | 16.27% | 15,000/15,000 | +1.97 | +4.52 | -5 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 7 | +39 | `housing-research` | off (hurts) | Aim research at the housing ceiling when the empire is paying it. | 17.06% | 16.27% | 15,000/15,000 | +2.00 | -0.88 | -68 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 8 | +39 | `peacetime-deterrence` | **on** (helps) | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | 17.05% | 16.28% | 15,000/15,000 | +1.90 | +2.74 | +84 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 9 | +36 | `come-ashore` | off (unresolved) | Keep the land army out of the water. | 17.03% | 16.31% | 15,000/15,000 | +1.80 | +1.40 | +27 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 10 | +35 | `inquisition-on-threat` | off (hurts) | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | 17.02% | 16.32% | 6,000/6,000 | +1.53 | -0.64 | -55 | `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` |
| 11 | +29 | `religion-sues-peace` | off (unresolved) | A Religion strategy offers peace to unblock its spread lane. | 16.96% | 16.37% | 15,000/15,000 | +1.53 | +0.88 | +14 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 12 | +29 | `wide-map-capacity` | **on** (helps) | Price the city ceiling off uncontested land. | 16.96% | 16.37% | 15,000/15,000 | +1.49 | +6.60 | +780 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 13 | +26 | `buildings-before-projects` | **on** (helps) | A district project waits behind the science and production buildings the city can already build. | 16.93% | 16.41% | 15,000/15,000 | +1.33 | +2.12 | -21 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 14 | +24 | `one-launch-pad` | off (unresolved) | Give the 3,000-point first-pad rung to one city at a time. | 16.91% | 16.43% | 15,000/15,000 | +1.22 | +0.90 | -14 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 15 | +23 | `idle-faith-patronage` | **on** (helps) | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | 16.90% | 16.43% | 6,000/6,000 | +2.28 | +3.96 | +27 | `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` |
| 16 | +23 | `settler-site-agreement` | off (hurts) | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | 16.89% | 16.44% | 15,000/15,000 | +1.15 | +1.56 | -50 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 17 | +23 | `whole-turn-backtrack-guard` | off (unresolved) | Refuse a step onto any tile this unit has already stood on this turn. | 16.89% | 16.44% | 15,000/15,000 | +1.14 | +0.57 | +43 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 18 | +21 | `siege-tracks-wall` | off (unresolved) | Size the siege train by the wall it has to breach. | 16.87% | 16.46% | 15,000/15,000 | +1.06 | +0.78 | -34 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 19 | +21 | `stranded-settler-discount` | **on** (helps) | Stop a Settler that has stopped walking from holding the expansion gate shut. | 16.87% | 16.46% | 15,000/15,000 | +1.04 | +2.30 | +26 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 20 | +21 | `strategic-wonders` | off (unresolved) | Build the wonders the chosen victory actually needs. | 16.87% | 16.46% | 15,000/15,000 | +1.04 | +1.72 | -12 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 21 | +21 | `strike-opening` | off (unresolved) | Let movement credit the attack a tile opens. | 16.87% | 16.46% | 15,000/15,000 | +1.05 | -0.13 | -10 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 22 | +20 | `slot-kind-tiebreak` | **on** (helps) | Break a production cost tie by which great-work slots can be filled. | 16.87% | 16.47% | 15,000/15,000 | +1.02 | +2.19 | -12 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 23 | +20 | `war-patience` | off (hurts) | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | 16.87% | 16.47% | 15,000/15,000 | +0.98 | -3.43 | +22 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 24 | +18 | `amenity-district-path` | off (unresolved) | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | 16.85% | 16.49% | 15,000/15,000 | +0.91 | +1.48 | -24 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 25 | +17 | `blind-objective-strength` | **on** (helps) | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | 16.83% | 16.50% | 15,000/15,000 | +0.84 | -1.23 | +75 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 26 | +17 | `garrison-under-fire` | **on** (helps) | A city losing hitpoints is besieged, whatever the fog says. | 16.84% | 16.49% | 15,000/15,000 | +0.87 | +2.93 | +26 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 27 | +14 | `siege-is-progress` | **on** (helps) | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | 16.81% | 16.53% | 15,000/15,000 | +0.70 | +2.79 | -43 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 28 | +13 | `settler-guard-holds` | off (unresolved) | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | 16.79% | 16.54% | 15,000/15,000 | +0.64 | -1.17 | -17 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 29 | +12 | `apostle-promotion-by-role` | off (unresolved) | Promote an Apostle for the job the empire has rather than for the largest number on the card. | 16.79% | 16.55% | 15,000/15,000 | +0.61 | +0.07 | -43 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 30 | +8 | `war-economy` | **on** (helps) | Send an adaptive Conquest plan through the war production path. | 16.75% | 16.59% | 15,000/15,000 | +0.40 | +5.12 | +106 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 31 | +7 | `escort-unstick` | off (unresolved) | Release an escort that is not walking its settler. | 16.73% | 16.60% | 15,000/15,000 | +0.34 | +0.16 | -36 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 32 | +6 | `relief-targets-the-siege` | off (unresolved) | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | 16.73% | 16.61% | 15,000/15,000 | +0.30 | +1.20 | +2 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 33 | +4 | `blind-objective-units` | off (unresolved) | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | 16.71% | 16.63% | 15,000/15,000 | +0.20 | +0.13 | -44 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 34 | +4 | `home-defense` | off (unresolved) | Let a raider standing in our own territory claim a unit before the offensive does. | 16.71% | 16.63% | 15,000/15,000 | +0.20 | -1.27 | +39 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 35 | +3 | `siege-commitment` | off (unresolved) | Keep a live campaign pointed at its chosen city. | 16.69% | 16.64% | 15,000/15,000 | +0.13 | +0.92 | -2 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 36 | +1 | `settler-target-hysteresis` | off (unresolved) | Keep a settler target dropped for danger out of the next picks for a few turns. | 16.67% | 16.66% | 15,000/15,000 | +0.03 | -0.03 | -5 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 37 | -2 | `recorded-tactical-step` | off (unresolved) | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | 16.65% | 16.69% | 15,000/15,000 | -0.10 | +0.50 | -9 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 38 | -3 | `score-horizon` | **on** (helps) | Skip a space race or a bomb that cannot finish before the turn limit. | 16.63% | 16.70% | 15,000/15,000 | -0.17 | +0.27 | +94 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 39 | -4 | `amenity-project-preemption` | **on** (helps) | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the polic… | 16.63% | 16.71% | 15,000/15,000 | -0.20 | +2.53 | +9 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 40 | -4 | `army-target-weighs-enemy` | **on** (helps) | Let the army target account for the enemy it has to beat. | 16.63% | 16.71% | 15,000/15,000 | -0.20 | +2.62 | -3 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 41 | -4 | `civilian-rescue` | off (unresolved) | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | 16.63% | 16.71% | 15,000/15,000 | -0.20 | +0.92 | +27 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 42 | -4 | `joint-tactics` | **on** (helps) | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | 16.63% | 16.71% | 15,000/15,000 | -0.20 | +1.24 | +26 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 43 | -5 | `theology-for-founders` | off (unresolved) | A founder researches Theology next. | 16.62% | 16.72% | 6,000/6,000 | -0.21 | -0.73 | -48 | `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` |
| 44 | -5 | `war-reinforcement` | off (unresolved) | March rear units to the campaign objective while the war is on. | 16.61% | 16.72% | 15,000/15,000 | -0.27 | -0.04 | -56 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 45 | -7 | `wonder-ring-settle-value` | off (unresolved) | Price a revealed natural wonder's ring into the settle scorer. | 16.60% | 16.73% | 15,000/15,000 | -0.33 | +1.08 | +7 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 46 | -8 | `governor-every-lane` | off (hurts) | Run the strategic governor under every lane. | 16.59% | 16.75% | 15,000/15,000 | -0.40 | -48.66 | -262 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 47 | -9 | `district-coverage` | off (unresolved) | Rank district families by how much of the empire still lacks them. | 16.57% | 16.76% | 15,000/15,000 | -0.47 | +1.50 | +14 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 48 | -9 | `housing-districts` | off (hurts) | Let the baseline governor raise the housing ceiling. | 16.57% | 16.76% | 15,000/15,000 | -0.47 | +0.56 | -108 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 49 | -11 | `endgame-war-runway` | off (unresolved) | Keep a fresh direct declaration out of the final campaign reserve. | 16.56% | 16.77% | 15,000/15,000 | -0.53 | +0.22 | -65 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 50 | -11 | `naval-recon` | off (unresolved) | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | 16.55% | 16.78% | 15,000/15,000 | -0.58 | +0.65 | +41 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 51 | -11 | `siege-role` | off (hurts) | Let the siege train be sized by the wall it has to breach. | 16.55% | 16.78% | 15,000/15,000 | -0.57 | -1.40 | -72 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 52 | -11 | `suzerain-cards` | off (unresolved) | Suzerain policy cards are valued only while a suzerainty actually exists. | 16.55% | 16.78% | 15,000/15,000 | -0.57 | -1.28 | -3 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 53 | -13 | `barbarian-walls-one-tier` | off (unresolved) | Barbarian pressure buys ancient walls and nothing above them. | 16.53% | 16.80% | 15,000/15,000 | -0.67 | +1.46 | +68 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 54 | -13 | `idle-walkers-close-the-pipeline` | off (unresolved) | An idle walker closes the settler pipeline, and a site the walker cannot reach stays retired. | 16.54% | 16.79% | 15,000/15,000 | -0.63 | +0.95 | -39 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 55 | -14 | `housing-buildings` | off (unresolved) | Let a housing-short city prefer a building that raises its ceiling. | 16.53% | 16.81% | 15,000/15,000 | -0.70 | -0.34 | -3 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 56 | -14 | `muster-at-command-radius` | off (unresolved) | Judge force readiness at the radius the group was assembled at. | 16.53% | 16.81% | 15,000/15,000 | -0.71 | -0.23 | -31 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 57 | -16 | `ranged-line-of-sight` | off (unresolved) | Let a ranged unit prefer tiles it can actually shoot from. | 16.51% | 16.83% | 15,000/15,000 | -0.82 | +0.43 | -34 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 58 | -26 | `camp-reach` | off (unresolved) | Count a barbarian camp within nine tiles of a city as home ground the guard clears. | 16.41% | 16.93% | 15,000/15,000 | -1.30 | +0.59 | +19 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 59 | -26 | `recon-flight` | off (unresolved) | Let a recon unit step out of a visible hostile's reach before it explores. | 16.41% | 16.93% | 15,000/15,000 | -1.32 | -0.48 | -27 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 60 | -26 | `wonder-prereq-reach` | off (unresolved) | Credit a wonder's missing prerequisite buildings/districts with a share of the wonder's own production score. | 16.41% | 16.93% | 15,000/15,000 | -1.31 | -0.51 | -58 | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| 61 | -27 | `holy-lane-parity` | off (hurts) | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | 16.40% | 16.93% | 6,000/6,000 | -1.19 | -1.34 | +5 | `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` |

## Awaiting native measurement

These screenable genes have no native on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a native screen is pending.

| Gene | Default | Description |
|---|---|---|
| `barbarian-bargain` | off (unmeasured) | Deliberate camp clearing as a peacetime errand. |
| `barbarian-hunt` | off (unmeasured) | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends it… |
| `builder-worked-tile-priority` | off (unmeasured) | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. |
| `district-lookahead-settle` | off (unmeasured) | A settler scores a site by the districts the plan would build there, each on its own plot. |
| `great-person-housing` | off (unmeasured) | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate… |
| `priced-tile-purchase` | off (unmeasured) | A border plot is bought only when its priced benefit clears its Gold by a margin. |
| `settler-threat-detour` | off (unmeasured) | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±10k (last tracked measurement) | Regime | Win rate (on) | Win rate (off) | Source |
|---|---:|---|---:|---:|---|
| `siege-muster` | +5 | war | 25.05% | 24.95% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `loyalty-policy-defence` | -12 | war | 24.88% | 25.12% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `stacked-escort` | -36 | war | 24.64% | 25.36% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `settler-stack-discipline` | -58 | war | 24.42% | 25.58% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `housing-cards` | -62 | war | 24.38% | 25.62% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `garrison-walls` | -68 | war | 24.32% | 25.68% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `campus-every-city` | -75 | war | 24.25% | 25.75% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `arrival-waves` | -84 | war | 24.16% | 25.84% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |

## Follow-ups

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

_Generated by `tools/heuristic_gene_ranking.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (native, 13,446 pairs), `2026-08-19-p2-war-4p-allseats-3300-pairs.json` (war, 3,300 pairs), `2026-08-20-p3b-war-repaired-4p-allseats-1064-pairs.json` (war, 1,064 pairs), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (native, 1,000 pairs), `2026-08-20-s3-step-and-reassess-war-4p-800-pairs.json` (war, 800 pairs), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (native, 15,000 pairs), `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` (war, 5,844 pairs). The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-ten-thousand view of the same games._
