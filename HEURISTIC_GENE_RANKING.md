# The heuristic gene ranking

Every screenable heuristic gene on the Advanced controller, ranked most beneficial to least by **± Wins Last 10k** — wins added per 10,000 six-player games at the gene's measured on-rate in its **latest** native screen. *± Wins 10k Prior* is the same figure from the screen before that (– when the gene has only one native reading); movement between the two columns is the gene's trend across cycles. *Default* is the deployment ledger's call (`docs/gene_ledger.json`). The *Total* columns pool every native screen that measured the gene, weighted by games. Every screen is a foldover against the best-genome baseline with shuffled civs and every major seat carrying its own genome (errors clustered by game pair), so a gene's on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the instrument; the paired contrasts, intervals and family-wise verdicts stay in `docs/gene_ledger.json`. Screenable genes awaiting their first native measurement are listed separately below without a rank.

**Reading the table.** A six-player seat wins 1-in-6 by chance (1-in-4 in a four-player screen), so the expected count is 1,667 wins per 10,000 games and the win columns say how far above or below that a seat carrying the gene lands; the whole-genome screen resolves about ±110 wins per 10,000 at 80% power and a single-gene 6,000-seat-pair screen about ±130 — differences inside that band are noise, not nulls. Screens differ in baseline as repairs land, so the *Prior* column reads as history, not a strict A/B against *Last*.

Regenerate with `python3 tools/heuristic_gene_ranking.py --write` after every screen enters the ledger; `tools/test_heuristic_gene_ranking.py` fails when this file is older than the ledger's sources.

| Rank | ± Wins Last 10k | ± Wins 10k Prior | Gene | Description | Default | Total (on) Win rate | Total (off) Win rate | Total Games (on+off) |
|---:|---:|---:|---|---|---|---:|---:|---:|
| 1 | +81 | +29 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | **on** | 17.23% | 16.10% | 56,892 |
| 2 | +73 | +33 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | **on** | 17.21% | 16.13% | 56,892 |
| 3 | +61 | +55 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | **on** | 17.25% | 16.09% | 56,892 |
| 4 | +53 | +43 | `camp-party` | The peacetime camp party. | **on** | 17.15% | 16.19% | 56,892 |
| 5 | +48 | – | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | **on** | 17.15% | 16.18% | 12,000 |
| 6 | +39 | +29 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | **on** | 17.01% | 16.32% | 56,892 |
| 7 | +39 | -26 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | off | 16.75% | 16.58% | 56,892 |
| 8 | +39 | +0 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | **on** | 16.87% | 16.46% | 56,892 |
| 9 | +36 | -17 | `come-ashore` | Keep the land army out of the water. | off | 16.78% | 16.56% | 56,892 |
| 10 | +35 | – | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | off | 17.02% | 16.32% | 12,000 |
| 11 | +29 | +25 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | off | 16.94% | 16.40% | 56,892 |
| 12 | +29 | +35 | `wide-map-capacity` | Price the city ceiling off uncontested land. | **on** | 16.99% | 16.35% | 56,892 |
| 13 | +26 | +23 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | **on** | 16.91% | 16.42% | 56,892 |
| 14 | +24 | +23 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | off | 16.90% | 16.43% | 56,892 |
| 15 | +23 | – | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | **on** | 16.90% | 16.43% | 12,000 |
| 16 | +23 | -21 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | off | 16.69% | 16.65% | 56,892 |
| 17 | +23 | +39 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | off | 16.97% | 16.36% | 56,892 |
| 18 | +21 | +51 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | off | 17.01% | 16.32% | 56,892 |
| 19 | +21 | +5 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | **on** | 16.80% | 16.53% | 56,892 |
| 20 | +21 | +21 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | off | 16.87% | 16.46% | 56,892 |
| 21 | +21 | +20 | `strike-opening` | Let movement credit the attack a tile opens. | off | 16.87% | 16.46% | 56,892 |
| 22 | +20 | +26 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | **on** | 16.90% | 16.44% | 56,892 |
| 23 | +20 | +3 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | off | 16.79% | 16.55% | 56,892 |
| 24 | +18 | -12 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | off | 16.71% | 16.63% | 56,892 |
| 25 | +17 | +36 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | **on** | 16.93% | 16.41% | 56,892 |
| 26 | +17 | +68 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | **on** | 17.08% | 16.25% | 56,892 |
| 27 | +14 | -64 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | **on** | 16.44% | 16.90% | 56,892 |
| 28 | +13 | -4 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | off | 16.71% | 16.62% | 56,892 |
| 29 | +12 | -42 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | off | 16.53% | 16.80% | 56,892 |
| 30 | +8 | -192 | `war-economy` | Send an adaptive Conquest plan through the war production path. | **on** | 15.80% | 17.53% | 56,892 |
| 31 | +7 | +0 | `escort-unstick` | Release an escort that is not walking its settler. | off | 16.70% | 16.63% | 56,892 |
| 32 | +6 | -11 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | off | 16.65% | 16.69% | 56,892 |
| 33 | +4 | +6 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | off | 16.72% | 16.62% | 56,892 |
| 34 | +4 | -30 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | off | 16.55% | 16.79% | 56,892 |
| 35 | +3 | -40 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | off | 16.49% | 16.84% | 56,892 |
| 36 | +1 | -8 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | off | 16.63% | 16.70% | 56,892 |
| 37 | -2 | +13 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | off | 16.72% | 16.61% | 56,892 |
| 38 | -3 | +35 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | **on** | 16.81% | 16.52% | 56,892 |
| 39 | -4 | +33 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the polic… | **on** | 16.80% | 16.53% | 56,892 |
| 40 | -4 | -33 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | **on** | 16.49% | 16.85% | 56,892 |
| 41 | -4 | -6 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | off | 16.62% | 16.72% | 56,892 |
| 42 | -4 | -18 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | **on** | 16.56% | 16.77% | 56,892 |
| 43 | -5 | – | `theology-for-founders` | A founder researches Theology next. | off | 16.62% | 16.72% | 12,000 |
| 44 | -5 | +49 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | off | 16.87% | 16.46% | 56,892 |
| 45 | -7 | +16 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | off | 16.71% | 16.63% | 56,892 |
| 46 | -8 | -56 | `governor-every-lane` | Run the strategic governor under every lane. | off | 16.36% | 16.97% | 56,892 |
| 47 | -9 | -10 | `district-coverage` | Rank district families by how much of the empire still lacks them. | off | 16.57% | 16.77% | 56,892 |
| 48 | -9 | -19 | `housing-districts` | Let the baseline governor raise the housing ceiling. | off | 16.53% | 16.81% | 56,892 |
| 49 | -11 | +4 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | off | 16.63% | 16.71% | 56,892 |
| 50 | -11 | -13 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | off | 16.55% | 16.79% | 56,892 |

## Awaiting native measurement

These screenable genes have no native on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a native screen is pending.

| Gene | Default | Description |
|---|---|---|
| `barbarian-bargain` | off (unmeasured) | Price a raider's life below a major's. |
| `barbarian-hunt` | off (unmeasured) | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends it… |
| `barbarian-ranged-answer` | off (unmeasured) | Answer a ring of shooters with a shooter. |
| `builder-barbarian-safety` | off (unmeasured) | Keep Builders from entering a visible Barbarian-capture envelope. |
| `builder-worked-tile-priority` | off (unmeasured) | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. |
| `condemn-under-congress` | off (unmeasured) | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. |
| `contact-posture` | off (unmeasured) | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answ… |
| `culture-building-debt` | off (unmeasured) | Make the Theater Square owe its buildings. |
| `culture-coverage` | off (unmeasured) | Pay for the Theater Square the empire has not got. |
| `district-building-chain` | off (unmeasured) | Make every specialty district owe its own buildings, whatever the lane. |
| `district-lookahead-settle` | off (unmeasured) | A settler scores a site by the districts the plan would build there, each on its own plot. |
| `early-contact-window` | off (unmeasured) | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. |
| `enhancer-for-the-corps` | off (unmeasured) | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. |
| `governor-expansion-lane` | off (unmeasured) | The other half: the governor under Expansion only. |
| `governor-victory-lanes` | off (unmeasured) | Half the composite: the governor under the four victory lanes only. |
| `great-person-housing` | off (unmeasured) | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate… |
| `guru-heals-the-corps` | off (unmeasured) | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. |
| `holy-site-where-the-threat-is` | off (unmeasured) | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. |
| `one-shot-recovery` | off (unmeasured) | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. |
| `opportunistic-war` | off (unmeasured) | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for p… |
| `priced-tile-purchase` | off (unmeasured) | A border plot is bought only when its priced benefit clears its Gold by a margin. |
| `raid-pillage-prizes` | off (unmeasured) | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. |
| `religious-defence-scales` | off (unmeasured) | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. |
| `religious-units-heal-first` | off (unmeasured) | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. |
| `settle-sooner` | off (unmeasured) | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good e… |
| `settler-threat-detour` | off (unmeasured) | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. |
| `spread-campaign-persists` | off (unmeasured) | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is… |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±10k (last tracked measurement) | Regime | Win rate (on) | Win rate (off) | Source |
|---|---:|---|---:|---:|---|
| `siege-muster` | +5 | war | 25.05% | 24.95% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `siege-role` | -11 | native | 16.55% | 16.78% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `suzerain-cards` | -11 | native | 16.55% | 16.78% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `loyalty-policy-defence` | -12 | war | 24.88% | 25.12% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `barbarian-walls-one-tier` | -13 | native | 16.53% | 16.80% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `idle-walkers-close-the-pipeline` | -13 | native | 16.54% | 16.79% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `housing-buildings` | -14 | native | 16.53% | 16.81% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `muster-at-command-radius` | -14 | native | 16.53% | 16.81% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `ranged-line-of-sight` | -16 | native | 16.51% | 16.83% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `camp-reach` | -26 | native | 16.41% | 16.93% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `recon-flight` | -26 | native | 16.41% | 16.93% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `wonder-prereq-reach` | -26 | native | 16.41% | 16.93% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `holy-lane-parity` | -27 | native | 16.40% | 16.93% | `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` |
| `stacked-escort` | -36 | war | 24.64% | 25.36% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `settler-stack-discipline` | -58 | war | 24.42% | 25.58% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `housing-cards` | -62 | war | 24.38% | 25.62% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `garrison-walls` | -68 | war | 24.32% | 25.68% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `campus-every-city` | -75 | war | 24.25% | 25.75% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `arrival-waves` | -84 | war | 24.16% | 25.84% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |

## Follow-ups

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

_Generated by `tools/heuristic_gene_ranking.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (native, 13,446 pairs), `2026-08-19-p2-war-4p-allseats-3300-pairs.json` (war, 3,300 pairs), `2026-08-20-p3b-war-repaired-4p-allseats-1064-pairs.json` (war, 1,064 pairs), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (native, 1,000 pairs), `2026-08-20-s3-step-and-reassess-war-4p-800-pairs.json` (war, 800 pairs), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (native, 15,000 pairs), `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` (war, 5,844 pairs). The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-ten-thousand view of the same games._
