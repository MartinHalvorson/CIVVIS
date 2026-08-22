# The heuristic gene ranking

Every screenable heuristic gene on the Advanced controller, ranked most beneficial to least by **± Wins Last 10k** — wins added per 10,000 six-player games at the gene's measured on-rate in its **latest** native screen. *± Wins 10k Prior* is the same figure from the screen before that (– when the gene has only one native reading); movement between the two columns is the gene's trend across cycles. *Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since 2026-08-22 that call is read off these two win columns: a gene defaults **on** when both are positive, or when their average clears +15 with neither below −10, and **off** otherwise — including every gene with only one native reading, which has no prior column to agree with it. The *Total* columns pool every native screen that measured the gene, weighted by games. Every screen is a foldover against the best-genome baseline with shuffled civs and every major seat carrying its own genome (errors clustered by game pair), so a gene's on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the instrument; the paired contrasts, intervals and family-wise verdicts stay in `docs/gene_ledger.json`. Screenable genes awaiting their first native measurement are listed separately below without a rank.

**Reading the table.** A six-player seat wins 1-in-6 by chance (1-in-4 in a four-player screen), so the expected count is 1,667 wins per 10,000 games and the win columns say how far above or below that a seat carrying the gene lands; the whole-genome screen resolves about ±110 wins per 10,000 at 80% power and a single-gene 6,000-seat-pair screen about ±130 — differences inside that band are noise, not nulls. Screens differ in baseline as repairs land, so the *Prior* column reads as history, not a strict A/B against *Last*.

**Cost.** Positive is slower; negative is faster. *cost (compute)* is the on/off percent change in wall seconds per completed turn, while *cost (time)* is the percent change in whole-game wall seconds and therefore includes games that end earlier or later. Each cell is the newest native estimate ± one standard error. The screen derives both from paired log-ratios on the same maps, fits every randomized gene together with an arm-order intercept, and keeps one timing per game pair; all-seats signs are summed so the answer is the incremental cost of enabling one major's genome. This reuses the screen's existing `secs` and `turn` rows — no hot-path timers and no extra profiling games. A dash means the source analysis predates the estimator and is unknown, never zero.

Regenerate with `python3 tools/heuristic_gene_ranking.py --write` after every screen enters the ledger; `tools/test_heuristic_gene_ranking.py` fails when this file is older than the ledger's sources.

| Rank | Gene | Description | Default | ± Wins Last 10k | ± Wins 10k Prior | Total (on) Win rate | Total (off) Win rate | Total Games (on+off) | cost (compute) | cost (time) |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate… | off | +78 | – | 17.45% | 15.89% | 35,148 | -0.01% ±0.34% | -0.29% ±0.60% |
| 2 | `escort-unstick` | Release an escort that is not walking its settler. | **on** | +72 | +7 | 16.96% | 16.37% | 92,040 | -0.26% ±0.33% | -0.08% ±0.59% |
| 3 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | off | +50 | – | 17.17% | 16.17% | 35,148 | +0.47% ±0.39% | +0.51% ±0.64% |
| 4 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | **on** | +48 | +81 | 17.20% | 16.13% | 92,040 | +0.78% ±0.37% | +0.49% ±0.62% |
| 5 | `governor-victory-lanes` | Half the composite: the governor under the four victory lanes only. | off | +46 | – | 17.12% | 16.21% | 35,148 | +1.16% ±0.34% | +2.17% ±0.60% |
| 6 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good e… | off | +41 | – | 17.08% | 16.26% | 35,148 | -0.04% ±0.36% | -0.23% ±0.61% |
| 7 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | **on** | +40 | +73 | 17.16% | 16.18% | 92,040 | +0.21% ±0.34% | +0.83% ±0.60% |
| 8 | `war-economy` | Send an adaptive Conquest plan through the war production path. | **on** | +38 | +8 | 16.28% | 17.06% | 92,040 | -0.31% ±0.40% | -0.82% ±0.62% |
| 9 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | **on** | +36 | +23 | 16.99% | 16.34% | 47,148 | +0.31% ±0.34% | +0.53% ±0.60% |
| 10 | `wide-map-capacity` | Price the city ceiling off uncontested land. | **on** | +35 | +29 | 17.00% | 16.33% | 92,040 | +0.23% ±0.34% | -0.15% ±0.58% |
| 11 | `district-coverage` | Rank district families by how much of the empire still lacks them. | off | +32 | -9 | 16.73% | 16.60% | 92,040 | -0.72% ±0.33% | -1.28% ±0.60% |
| 12 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | off | +30 | -4 | 16.67% | 16.66% | 92,040 | -0.18% ±0.34% | -0.27% ±0.59% |
| 13 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | **on** | +30 | +17 | 16.94% | 16.39% | 92,040 | +0.52% ±0.36% | +0.37% ±0.62% |
| 14 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | off | +30 | – | 16.96% | 16.37% | 35,148 | +0.74% ±0.33% | +0.57% ±0.59% |
| 15 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | off | +30 | -2 | 16.81% | 16.52% | 92,040 | -0.09% ±0.38% | -0.08% ±0.62% |
| 16 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | off | +24 | – | 16.91% | 16.42% | 35,148 | -0.23% ±0.34% | -0.58% ±0.60% |
| 17 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | off | +24 | -3 | 16.85% | 16.48% | 92,040 | -0.27% ±0.32% | -0.23% ±0.59% |
| 18 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | **on** | +24 | +23 | 16.77% | 16.56% | 92,040 | -0.12% ±0.37% | -0.09% ±0.62% |
| 19 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | **on** | +23 | +61 | 17.11% | 16.22% | 92,040 | +0.11% ±0.35% | +0.65% ±0.61% |
| 20 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for p… | off | +23 | – | 16.89% | 16.44% | 35,148 | +1.53% ±0.39% | +2.57% ±0.63% |
| 21 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | **on** | +19 | +39 | 16.95% | 16.38% | 92,040 | -0.02% ±0.32% | -0.17% ±0.58% |
| 22 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | **on** | +18 | +23 | 16.92% | 16.41% | 92,040 | +0.28% ±0.38% | +0.47% ±0.64% |
| 23 | `strike-opening` | Let movement credit the attack a tile opens. | **on** | +17 | +21 | 16.86% | 16.48% | 92,040 | -0.27% ±0.35% | -0.17% ±0.60% |
| 24 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | off | +16 | -11 | 16.65% | 16.68% | 92,040 | +0.23% ±0.38% | +0.25% ±0.61% |
| 25 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | **on** | +15 | +24 | 16.87% | 16.46% | 92,040 | +0.00% ±0.34% | -0.27% ±0.59% |
| 26 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | **on** | +15 | +1 | 16.70% | 16.63% | 92,040 | +0.45% ±0.34% | +0.69% ±0.59% |
| 27 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | **on** | +14 | +12 | 16.64% | 16.70% | 92,040 | +0.13% ±0.33% | +0.19% ±0.58% |
| 28 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | off | +14 | – | 16.81% | 16.52% | 35,148 | +0.03% ±0.32% | +0.32% ±0.58% |
| 29 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | **on** | +14 | +48 | 16.90% | 16.44% | 47,148 | +0.03% ±0.34% | +0.53% ±0.59% |
| 30 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | **on** | +14 | +6 | 16.71% | 16.63% | 92,040 | +0.12% ±0.34% | +0.25% ±0.59% |
| 31 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | off | +13 | – | 16.79% | 16.54% | 35,148 | -0.04% ±0.32% | -0.04% ±0.57% |
| 32 | `camp-party` | The peacetime camp party. | **on** | +13 | +53 | 17.01% | 16.32% | 92,040 | -0.27% ±0.35% | -0.55% ±0.59% |
| 33 | `governor-every-lane` | Run the strategic governor under every lane. | off | +13 | -8 | 16.53% | 16.81% | 92,040 | +1.11% ±0.33% | +1.18% ±0.59% |
| 34 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | **on** | +13 | +39 | 16.77% | 16.56% | 92,040 | -0.78% ±0.31% | -0.90% ±0.57% |
| 35 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | **on** | +13 | +21 | 16.80% | 16.53% | 92,040 | +0.35% ±0.34% | +1.28% ±0.60% |
| 36 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | **on** | +12 | +18 | 16.74% | 16.60% | 92,040 | -0.07% ±0.33% | +0.57% ±0.60% |
| 37 | `come-ashore` | Keep the land army out of the water. | **on** | +11 | +36 | 16.78% | 16.56% | 92,040 | -0.13% ±0.38% | -0.58% ±0.62% |
| 38 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | off | +7 | -7 | 16.72% | 16.62% | 92,040 | -0.03% ±0.34% | -0.10% ±0.59% |
| 39 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | **on** | +6 | +29 | 16.86% | 16.48% | 92,040 | +0.11% ±0.35% | +0.47% ±0.62% |
| 40 | `barbarian-bargain` | Price a raider's life below a major's. | off | +5 | – | 16.71% | 16.62% | 35,148 | -0.31% ±0.34% | -0.23% ±0.59% |
| 41 | `housing-districts` | Let the baseline governor raise the housing ceiling. | off | +5 | -9 | 16.60% | 16.74% | 92,040 | +0.19% ±0.35% | +0.42% ±0.58% |
| 42 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | off | +3 | -4 | 16.61% | 16.72% | 92,040 | +27.29% ±0.47% | +27.69% ±0.79% |
| 43 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | off | +3 | -5 | 16.80% | 16.53% | 92,040 | +0.09% ±0.34% | +0.26% ±0.60% |
| 44 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | **on** | +2 | +35 | 16.77% | 16.56% | 47,148 | +0.05% ±0.32% | -0.30% ±0.57% |
| 45 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | **on** | +1 | +39 | 16.80% | 16.54% | 92,040 | +0.18% ±0.32% | +0.14% ±0.58% |
| 46 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | **on** | +1 | +3 | 16.56% | 16.77% | 92,040 | -0.25% ±0.33% | -0.20% ±0.57% |
| 47 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | off | -2 | +26 | 16.81% | 16.52% | 92,040 | +0.01% ±0.36% | -0.18% ±0.60% |
| 48 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | off | -3 | +13 | 16.68% | 16.65% | 92,040 | -0.41% ±0.36% | -0.85% ±0.60% |
| 49 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | off | -3 | +21 | 16.87% | 16.46% | 92,040 | -0.03% ±0.35% | -0.02% ±0.60% |
| 50 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | off | -5 | -11 | 16.62% | 16.71% | 92,040 | +0.17% ±0.33% | +0.48% ±0.60% |
| 51 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | off | -5 | +17 | 16.90% | 16.43% | 92,040 | -0.15% ±0.39% | -0.03% ±0.63% |
| 52 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | off | -5 | +21 | 16.78% | 16.56% | 92,040 | +0.38% ±0.34% | -0.02% ±0.59% |
| 53 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | off | -6 | -4 | 16.61% | 16.72% | 92,040 | +0.10% ±0.37% | +0.50% ±0.62% |
| 54 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | off | -7 | +4 | 16.67% | 16.66% | 92,040 | +0.02% ±0.35% | +0.30% ±0.60% |
| 55 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | off | -12 | +20 | 16.76% | 16.57% | 92,040 | +0.14% ±0.35% | +0.39% ±0.60% |
| 56 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the polic… | off | -14 | -4 | 16.70% | 16.64% | 92,040 | +0.34% ±0.33% | +0.29% ±0.57% |
| 57 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | off | -15 | +4 | 16.54% | 16.80% | 92,040 | -0.62% ±0.32% | -0.57% ±0.57% |
| 58 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | off | -16 | +14 | 16.46% | 16.87% | 92,040 | +0.27% ±0.34% | -0.45% ±0.56% |
| 59 | `theology-for-founders` | A founder researches Theology next. | off | -16 | -5 | 16.54% | 16.80% | 47,148 | +0.07% ±0.34% | -0.20% ±0.59% |
| 60 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | off | -19 | +20 | 16.67% | 16.67% | 92,040 | -0.48% ±0.34% | -0.96% ±0.59% |
| 61 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | off | -22 | – | 16.45% | 16.88% | 35,148 | +0.73% ±0.38% | +0.91% ±0.62% |
| 62 | `governor-expansion-lane` | The other half: the governor under Expansion only. | off | -30 | – | 16.37% | 16.97% | 35,148 | +0.48% ±0.35% | +0.51% ±0.60% |
| 63 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | off | -31 | – | 16.36% | 16.97% | 35,148 | -0.37% ±0.37% | -0.55% ±0.64% |
| 64 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends it… | off | -86 | – | 15.80% | 17.53% | 35,148 | -0.94% ±0.32% | -0.78% ±0.58% |

## Awaiting native measurement

These screenable genes have no native on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a native screen is pending.

| Gene | Default | Description |
|---|---|---|
| `air-surge` | off (unmeasured) | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. |
| `campus-adjacency-threshold` | off (unmeasured) | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. |
| `campus-finishes-first` | off (unmeasured) | The Campus coverage term is scaled by how finished the empire's standing Campuses are. |
| `chain-tech-lookahead` | off (unmeasured) | The research goal aims at a Campus rung the empire can BUILD, not only one it has already built. |
| `competition-victory-points` | off (unmeasured) | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. |
| `condemn-under-congress` | off (unmeasured) | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. |
| `congress-banks-decided` | off (unmeasured) | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact pre… |
| `congress-counter-votes` | off (unmeasured) | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fail… |
| `contact-posture` | off (unmeasured) | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answ… |
| `culture-building-debt` | off (unmeasured) | Make the Theater Square owe its buildings. |
| `culture-coverage` | off (unmeasured) | Pay for the Theater Square the empire has not got. |
| `district-building-chain` | off (unmeasured) | Make every specialty district owe its own buildings, whatever the lane. |
| `early-contact-window` | off (unmeasured) | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. |
| `enhancer-for-the-corps` | off (unmeasured) | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. |
| `envoy-infrastructure` | off (unmeasured) | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the tur… |
| `fifteenth-citizen` | off (unmeasured) | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. |
| `guru-heals-the-corps` | off (unmeasured) | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. |
| `holy-site-where-the-threat-is` | off (unmeasured) | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. |
| `lane-congress-ballot` | off (unmeasured) | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion postu… |
| `lane-congress-favor` | off (unmeasured) | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. |
| `lane-culture-spending` | off (unmeasured) | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture w… |
| `lane-great-people` | off (unmeasured) | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. |
| `lane-policy-deck` | off (unmeasured) | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. |
| `lane-space-race` | off (unmeasured) | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be site… |
| `one-shot-recovery` | off (unmeasured) | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. |
| `power-the-laboratory` | off (unmeasured) | A power plant is credited the yields it switches on in its city. |
| `religious-defence-scales` | off (unmeasured) | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. |
| `religious-units-heal-first` | off (unmeasured) | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. |
| `research-floor-holds` | off (unmeasured) | The citizen tilt and the beaker floor hold while the research can still pay. |
| `research-grants-first` | off (unmeasured) | A finished research city pays more for its own district's project. |
| `research-tier-premium` | off (unmeasured) | A Campus building's debt is scaled by its own Science against the chain's first rung. |
| `science-multiplier-payoff` | off (unmeasured) | Credit a Campus building the beakers its city's multipliers will actually pay it. |
| `science-payback-horizon` | off (unmeasured) | Price the science economy on whether it can still repay rather than on how much of the game is left. |
| `spread-campaign-persists` | off (unmeasured) | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is… |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±10k (last tracked measurement) | Regime | Win rate (on) | Win rate (off) | Source |
|---|---:|---|---:|---:|---|
| `holy-lane-parity` | +63 | native | 17.30% | 16.04% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `suzerain-cards` | +42 | native | 17.09% | 16.25% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `wonder-prereq-reach` | +29 | native | 16.96% | 16.38% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `camp-reach` | +10 | native | 16.77% | 16.56% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-buildings` | +8 | native | 16.75% | 16.59% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `siege-muster` | +5 | war | 25.05% | 24.95% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `ranged-line-of-sight` | +4 | native | 16.71% | 16.63% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `recon-flight` | -1 | native | 16.66% | 16.67% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `idle-walkers-close-the-pipeline` | -10 | native | 16.56% | 16.77% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `loyalty-policy-defence` | -12 | war | 24.88% | 25.12% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `muster-at-command-radius` | -12 | native | 16.55% | 16.79% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `barbarian-walls-one-tier` | -13 | native | 16.54% | 16.80% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `stacked-escort` | -36 | war | 24.64% | 25.36% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `siege-role` | -39 | native | 16.27% | 17.06% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `settler-stack-discipline` | -58 | war | 24.42% | 25.58% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `housing-cards` | -62 | war | 24.38% | 25.62% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `garrison-walls` | -68 | war | 24.32% | 25.68% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `campus-every-city` | -75 | war | 24.25% | 25.75% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |
| `arrival-waves` | -84 | war | 24.16% | 25.84% | `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` |

## Follow-ups

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

**P10 ended early at the operator's request.** Its 2,929 complete map seeds provide 5,858 controlled games and 17,574 treated-seat pairs; the analysis excludes 11 interrupted one-arm seeds (66 raw seat rows), with zero duplicate or invalid tuples. The new *Wins Last 10k* value extrapolates each measured on-arm rate to 10,000 games as `round((win_on − 1/6) × 10,000)`; it does not invent synthetic games. The former *Wins Last 10k* reading shifts intact to *Wins 10k Prior*. P10 used the 6p all-six native regime on seeds 100000000–100002962, 60×38 pangaea/online/250 turns, shuffled civilizations, every major seat treated, and foldover against the best-genome baseline. Its fixed binary came from `d23f92d944cd889aa4c9dfe58c37aceb8e55eabd` (SHA-256 `79385db96e89e91cc0b6fd8389e837cb66dc05ccaa4eee493576f152daf627ed`), before later gene removals and additions; ledger generation drops obsolete tags and retains newer genes from their existing sources.

_Generated by `tools/heuristic_gene_ranking.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (native, 13,446 pairs), `2026-08-19-p2-war-4p-allseats-3300-pairs.json` (war, 3,300 pairs), `2026-08-20-p3b-war-repaired-4p-allseats-1064-pairs.json` (war, 1,064 pairs), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (native, 1,000 pairs), `2026-08-20-s3-step-and-reassess-war-4p-800-pairs.json` (war, 800 pairs), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (native, 6,000 pairs), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (native, 15,000 pairs), `2026-08-21-s8-war-rerank-vs-best-4p-allseats.json` (war, 5,844 pairs), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (native, 17,574 pairs). The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-ten-thousand view of the same games._
