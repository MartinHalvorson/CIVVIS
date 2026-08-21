# The heuristic gene ranking

Every boolean heuristic gene on the Advanced controller, ranked most beneficial to least, from the cycle-4 whole-genome screen: **15,000 foldover pairs = 30,000 six-player seat-games** (seeds 52000000.., 6p 60×38 Online-250, all victory lanes, shuffled civs, every major seat carrying its own random genome, errors clustered by game pair; baseline = the deployment gene ledger). Each gene was ON in exactly one arm of every pair, so its on/off columns cover the same 15,000 maps. `docs/GENE_SCREEN.md` documents the instrument; `docs/gene_ledger.json` holds the machine-readable verdicts.

**Reading the table.** A six-player seat wins 1-in-6 by chance, so out of 10,000 games the expected count is **1,667**; column 2 is how many wins above or below that a seat carrying the gene would collect at its measured on-rate. The screen resolves a win Δ of ±1.1 pp (≈ ±110 wins per 10,000) at 80% power; differences inside that band are noise, not nulls. Regenerate after each whole-genome screen: the numbers move as repairs land.

| Rank | Wins ±10k (vs 1,667) | Gene | Description | Wins (on) | Games (on) | Win rate (on) | Wins (off) | Games (off) | Win rate (off) |
|---:|---:|---|---|---:|---:|---:|---:|---:|---:|
| 1 | +81 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | 2,622 | 15,000 | 17.48% | 2,378 | 15,000 | 15.85% |
| 2 | +73 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | 2,610 | 15,000 | 17.40% | 2,390 | 15,000 | 15.93% |
| 3 | +61 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | 2,591 | 15,000 | 17.27% | 2,409 | 15,000 | 16.06% |
| 4 | +53 | `camp-party` | The peacetime camp party. | 2,579 | 15,000 | 17.19% | 2,421 | 15,000 | 16.14% |
| 5 | +39 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | 2,559 | 15,000 | 17.06% | 2,441 | 15,000 | 16.27% |
| 6 | +39 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | 2,559 | 15,000 | 17.06% | 2,441 | 15,000 | 16.27% |
| 7 | +39 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | 2,558 | 15,000 | 17.05% | 2,442 | 15,000 | 16.28% |
| 8 | +36 | `come-ashore` | Keep the land army out of the water. | 2,554 | 15,000 | 17.03% | 2,446 | 15,000 | 16.31% |
| 9 | +29 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | 2,544 | 15,000 | 16.96% | 2,456 | 15,000 | 16.37% |
| 10 | +29 | `wide-map-capacity` | Price the city ceiling off uncontested land. | 2,544 | 15,000 | 16.96% | 2,456 | 15,000 | 16.37% |
| 11 | +26 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | 2,539 | 15,000 | 16.93% | 2,461 | 15,000 | 16.41% |
| 12 | +24 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | 2,536 | 15,000 | 16.91% | 2,464 | 15,000 | 16.43% |
| 13 | +23 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | 2,534 | 15,000 | 16.89% | 2,466 | 15,000 | 16.44% |
| 14 | +23 | `settler-site-agreement` | See Self::settler_site_agreement. | 2,534 | 15,000 | 16.89% | 2,466 | 15,000 | 16.44% |
| 15 | +21 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | 2,531 | 15,000 | 16.87% | 2,469 | 15,000 | 16.46% |
| 16 | +21 | `strike-opening` | Let movement credit the attack a tile opens. | 2,531 | 15,000 | 16.87% | 2,469 | 15,000 | 16.46% |
| 17 | +21 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | 2,531 | 15,000 | 16.87% | 2,469 | 15,000 | 16.46% |
| 18 | +21 | `strategic-wonders` | Production may start a wonder the strategic scorer values for the active lane. | 2,531 | 15,000 | 16.87% | 2,469 | 15,000 | 16.46% |
| 19 | +20 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | 2,530 | 15,000 | 16.87% | 2,470 | 15,000 | 16.47% |
| 20 | +20 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | 2,530 | 15,000 | 16.87% | 2,470 | 15,000 | 16.47% |
| 21 | +18 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | 2,527 | 15,000 | 16.85% | 2,473 | 15,000 | 16.49% |
| 22 | +17 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | 2,526 | 15,000 | 16.84% | 2,474 | 15,000 | 16.49% |
| 23 | +17 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | 2,525 | 15,000 | 16.83% | 2,475 | 15,000 | 16.50% |
| 24 | +14 | `siege-is-progress` | See Self::siege_is_progress. | 2,521 | 15,000 | 16.81% | 2,479 | 15,000 | 16.53% |
| 25 | +13 | `settler-guard-holds` | See Self::settler_guard_holds. | 2,519 | 15,000 | 16.79% | 2,481 | 15,000 | 16.54% |
| 26 | +12 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | 2,518 | 15,000 | 16.79% | 2,482 | 15,000 | 16.55% |
| 27 | +8 | `war-economy` | Residual protective halves only: bankruptcy recovery and emergency conscription (the Conquest production routing was removed 2026-08-20 by measurement). | 2,512 | 15,000 | 16.75% | 2,488 | 15,000 | 16.59% |
| 28 | +7 | `escort-unstick` | Release an escort that is not walking its settler. | 2,510 | 15,000 | 16.73% | 2,490 | 15,000 | 16.60% |
| 29 | +6 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | 2,509 | 15,000 | 16.73% | 2,491 | 15,000 | 16.61% |
| 30 | +4 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | 2,506 | 15,000 | 16.71% | 2,494 | 15,000 | 16.63% |
| 31 | +4 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | 2,506 | 15,000 | 16.71% | 2,494 | 15,000 | 16.63% |
| 32 | +3 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | 2,504 | 15,000 | 16.69% | 2,496 | 15,000 | 16.64% |
| 33 | +1 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | 2,501 | 15,000 | 16.67% | 2,499 | 15,000 | 16.66% |
| 34 | -2 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | 2,497 | 15,000 | 16.65% | 2,503 | 15,000 | 16.69% |
| 35 | -3 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | 2,495 | 15,000 | 16.63% | 2,505 | 15,000 | 16.70% |
| 36 | -4 | `army-target-weighs-enemy` | Let the army-size target account for the enemy it has to beat. | 2,494 | 15,000 | 16.63% | 2,506 | 15,000 | 16.71% |
| 37 | -4 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | 2,494 | 15,000 | 16.63% | 2,506 | 15,000 | 16.71% |
| 38 | -4 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use… | 2,494 | 15,000 | 16.63% | 2,506 | 15,000 | 16.71% |
| 39 | -4 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | 2,494 | 15,000 | 16.63% | 2,506 | 15,000 | 16.71% |
| 40 | -5 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | 2,492 | 15,000 | 16.61% | 2,508 | 15,000 | 16.72% |
| 41 | -7 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | 2,490 | 15,000 | 16.60% | 2,510 | 15,000 | 16.73% |
| 42 | -8 | `governor-every-lane` | Run the strategic governor under every lane. | 2,488 | 15,000 | 16.59% | 2,512 | 15,000 | 16.75% |
| 43 | -9 | `housing-districts` | Let the baseline governor raise the housing ceiling. | 2,486 | 15,000 | 16.57% | 2,514 | 15,000 | 16.76% |
| 44 | -9 | `district-coverage` | Rank district families by how much of the empire still lacks them. | 2,486 | 15,000 | 16.57% | 2,514 | 15,000 | 16.76% |
| 45 | -11 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | 2,484 | 15,000 | 16.56% | 2,516 | 15,000 | 16.77% |
| 46 | -11 | `siege-role` | Let the siege train be sized by the wall it has to breach. | 2,483 | 15,000 | 16.55% | 2,517 | 15,000 | 16.78% |
| 47 | -11 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | 2,483 | 15,000 | 16.55% | 2,517 | 15,000 | 16.78% |
| 48 | -11 | `suzerain-cards` | Suzerain policy cards are valued only while a suzerainty actually exists. | 2,483 | 15,000 | 16.55% | 2,517 | 15,000 | 16.78% |
| 49 | -13 | `idle-walkers-close-the-pipeline` | See Self::idle_walkers_close_the_pipeline. | 2,481 | 15,000 | 16.54% | 2,519 | 15,000 | 16.79% |
| 50 | -13 | `barbarian-walls-one-tier` | See BasicAi::barbarian_walls_one_tier. | 2,480 | 15,000 | 16.53% | 2,520 | 15,000 | 16.80% |
| 51 | -14 | `muster-at-command-radius` | Judge force readiness at the radius the group was assembled at. | 2,479 | 15,000 | 16.53% | 2,521 | 15,000 | 16.81% |
| 52 | -14 | `housing-buildings` | Let a housing-short city prefer a building that raises its ceiling. | 2,479 | 15,000 | 16.53% | 2,521 | 15,000 | 16.81% |
| 53 | -16 | `ranged-line-of-sight` | Let a ranged unit prefer tiles it can actually shoot from. | 2,476 | 15,000 | 16.51% | 2,524 | 15,000 | 16.83% |
| 54 | -26 | `recon-flight` | Let a recon unit step out of a visible hostile's reach before it explores. | 2,461 | 15,000 | 16.41% | 2,539 | 15,000 | 16.93% |
| 55 | -26 | `camp-reach` | Count a barbarian camp within nine tiles of a city as home ground the guard clears. | 2,461 | 15,000 | 16.41% | 2,539 | 15,000 | 16.93% |
| 56 | -26 | `wonder-prereq-reach` | Credit a wonder's missing prerequisite buildings/districts with a share of the wonder's own production score. | 2,461 | 15,000 | 16.41% | 2,539 | 15,000 | 16.93% |
| 57 | -27 | `step-and-reassess` | A blind-planned unit stops at the first step that revealed new ground and finishes its movement sighted; on the bridge its walk is cut at the first unrevealed hex so t… | 2,460 | 15,000 | 16.40% | 2,540 | 15,000 | 16.93% |
| 58 | -33 | `housing-cards` | Put medina_quarter and insulae in the deck when a city is short of housing and already carries the districts they key off. | 2,450 | 15,000 | 16.33% | 2,550 | 15,000 | 17.00% |
| 59 | -34 | `arrival-waves` | Rear reinforcements arrive at an engaged front as a wave, not one at a time. | 2,449 | 15,000 | 16.33% | 2,551 | 15,000 | 17.01% |
| 60 | -35 | `loyalty-policy-defence` | Hold a promotion until its healing would land. | 2,448 | 15,000 | 16.32% | 2,552 | 15,000 | 17.01% |
| 61 | -36 | `siege-muster` | Let a besieged city raise its standing-army floor against hostiles it has no diplomatic state with. | 2,446 | 15,000 | 16.31% | 2,554 | 15,000 | 17.03% |
| 62 | -81 | `settler-stack-discipline` | Settlers decide before the engagement, price capture as capture and trust only a guard on their tile. | 2,379 | 15,000 | 15.86% | 2,621 | 15,000 | 17.47% |
| 63 | -83 | `stacked-escort` | Escort settlers by stacked co-movement instead of formations. | 2,375 | 15,000 | 15.83% | 2,625 | 15,000 | 17.50% |
| 64 | -84 | `garrison-walls` | Order our own ancient walls in the capital and small frontier cities once Masonry is in. | 2,374 | 15,000 | 15.83% | 2,626 | 15,000 | 17.51% |
| 65 | -90 | `campus-every-city` | Keep asking for a Campus in every city that can still repay one. | 2,365 | 15,000 | 15.77% | 2,635 | 15,000 | 17.57% |

_Generated from `gene_screen` run p7 (2026-08-21). The paired on−off contrast, intervals, and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-million view of the same games._