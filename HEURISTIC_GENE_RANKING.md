# The heuristic gene ranking

| Rank | Gene | Description | Default | ± Wins / 10k seats | ± Wins / 10k seats prior | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `holy-lane-parity` | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | **on** | +99 | +63 | 17.21% (n=30,774) | 16.13% (n=30,774) | 1.08% | +45 [-24, +114] | 89.9% | +0.08 (z +1.23) ~ | +0.49% ±0.31% | +1.07% ±0.70% |
| 2 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate works to make room. | **on** | +78 | – | 17.45% (n=17,574) | 15.89% (n=17,574) | 1.56% | +78 [+42, +114] | 100.0% | +0.60 (z +9.27) helps * | -0.01% ±0.34% | -0.29% ±0.60% |
| 3 | `escort-unstick` | Release an escort that is not walking its settler. | **on** | +72 | +7 | 16.96% (n=46,020) | 16.37% (n=46,020) | 0.60% | +27 [-20, +74] | 87.3% | +0.15 (z +2.22) helps * | -0.26% ±0.33% | -0.08% ±0.59% |
| 4 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | **on** | +50 | – | 17.17% (n=17,574) | 16.17% (n=17,574) | 1.00% | +50 [+14, +86] | 99.7% | +0.04 (z +0.56) ~ | +0.47% ±0.39% | +0.51% ±0.64% |
| 5 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | **on** | +48 | +81 | 17.20% (n=46,020) | 16.13% (n=46,020) | 1.07% | +53 [+25, +82] | 100.0% | +0.05 (z +0.72) ~ | +0.78% ±0.37% | +0.49% ±0.62% |
| 6 | `governor-victory-lanes` | Half the composite: the governor under the four victory lanes only. | **on** | +46 | – | 17.12% (n=17,574) | 16.21% (n=17,574) | 0.91% | +46 [+9, +82] | 99.3% | -1.02 (z -15.92) hurts * | +1.16% ±0.34% | +2.17% ±0.60% |
| 7 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good enough to pay for its walk. | **on** | +41 | – | 17.08% (n=17,574) | 16.26% (n=17,574) | 0.82% | +41 [+5, +76] | 98.8% | +0.17 (z +2.68) helps * | -0.04% ±0.36% | -0.23% ±0.61% |
| 8 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | **on** | +40 | +73 | 17.16% (n=46,020) | 16.18% (n=46,020) | 0.98% | +49 [+25, +73] | 100.0% | +0.62 (z +9.72) helps * | +0.21% ±0.34% | +0.83% ±0.60% |
| 9 | `war-economy` | Send an adaptive Conquest plan through the war production path. | off | +38 | +8 | 16.28% (n=46,020) | 17.06% (n=46,020) | -0.78% | -48 [-185, +88] | 24.4% | +0.70 (z +10.64) helps * | -0.31% ±0.40% | -0.82% ±0.62% |
| 10 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | **on** | +36 | +23 | 16.99% (n=23,574) | 16.34% (n=23,574) | 0.65% | +26 [+9, +44] | 99.8% | -0.03 (z -0.47) ~ | +0.31% ±0.34% | +0.53% ±0.60% |
| 11 | `wide-map-capacity` | Price the city ceiling off uncontested land. | **on** | +35 | +29 | 17.00% (n=46,020) | 16.33% (n=46,020) | 0.66% | +33 [+11, +55] | 99.8% | +0.40 (z +6.05) helps * | +0.23% ±0.34% | -0.15% ±0.58% |
| 12 | `district-coverage` | Rank district families by how much of the empire still lacks them. | off | +32 | -9 | 16.73% (n=46,020) | 16.60% (n=46,020) | 0.13% | +5 [-23, +34] | 64.6% | +0.10 (z +1.49) ~ | -0.72% ±0.33% | -1.28% ±0.60% |
| 13 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | off | +30 | -4 | 16.67% (n=46,020) | 16.66% (n=46,020) | 0.01% | -1 [-37, +35] | 48.2% | +0.15 (z +2.35) helps * | -0.18% ±0.34% | -0.27% ±0.59% |
| 14 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | **on** | +30 | +17 | 16.94% (n=46,020) | 16.39% (n=46,020) | 0.55% | +27 [+5, +50] | 99.2% | +0.07 (z +1.03) ~ | +0.52% ±0.36% | +0.37% ±0.62% |
| 15 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | **on** | +30 | – | 16.96% (n=17,574) | 16.37% (n=17,574) | 0.59% | +30 [-6, +65] | 95.0% | +0.14 (z +2.12) helps * | +0.74% ±0.33% | +0.57% ±0.59% |
| 16 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | off | +30 | -2 | 16.81% (n=46,020) | 16.52% (n=46,020) | 0.29% | +15 [-8, +37] | 89.8% | -0.04 (z -0.60) ~ | -0.09% ±0.38% | -0.08% ±0.62% |
| 17 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | **on** | +24 | – | 16.91% (n=17,574) | 16.42% (n=17,574) | 0.49% | +24 [-11, +60] | 91.3% | +0.01 (z +0.14) ~ | -0.23% ±0.34% | -0.58% ±0.60% |
| 18 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | off | +24 | -3 | 16.85% (n=46,020) | 16.48% (n=46,020) | 0.37% | +18 [-4, +41] | 94.4% | +0.21 (z +3.14) helps * | -0.27% ±0.32% | -0.23% ±0.59% |
| 19 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | **on** | +24 | +23 | 16.77% (n=46,020) | 16.56% (n=46,020) | 0.21% | +10 [-18, +38] | 76.4% | +0.06 (z +0.89) ~ | -0.12% ±0.37% | -0.09% ±0.62% |
| 20 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | **on** | +23 | +61 | 17.11% (n=46,020) | 16.22% (n=46,020) | 0.90% | +45 [+21, +68] | 100.0% | +0.06 (z +0.88) ~ | +0.11% ±0.35% | +0.65% ±0.61% |
| 21 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for peace. | **on** | +23 | – | 16.89% (n=17,574) | 16.44% (n=17,574) | 0.46% | +23 [-14, +59] | 88.9% | +0.26 (z +4.03) helps * | +1.53% ±0.39% | +2.57% ±0.63% |
| 22 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | **on** | +19 | +39 | 16.95% (n=46,020) | 16.38% (n=46,020) | 0.57% | +28 [+6, +51] | 99.3% | +0.20 (z +3.10) helps * | -0.02% ±0.32% | -0.17% ±0.58% |
| 23 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | **on** | +18 | +23 | 16.92% (n=46,020) | 16.41% (n=46,020) | 0.51% | +25 [+3, +47] | 98.6% | +0.02 (z +0.30) ~ | +0.28% ±0.38% | +0.47% ±0.64% |
| 24 | `strike-opening` | Let movement credit the attack a tile opens. | **on** | +17 | +21 | 16.86% (n=46,020) | 16.48% (n=46,020) | 0.38% | +19 [-3, +41] | 95.3% | +0.11 (z +1.65) ~ | -0.27% ±0.35% | -0.17% ±0.60% |
| 25 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | off | +16 | -11 | 16.65% (n=46,020) | 16.68% (n=46,020) | -0.03% | -1 [-23, +21] | 46.3% | +0.01 (z +0.10) ~ | +0.23% ±0.38% | +0.25% ±0.61% |
| 26 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | **on** | +15 | +24 | 16.87% (n=46,020) | 16.46% (n=46,020) | 0.41% | +20 [-2, +42] | 96.5% | +0.06 (z +0.94) ~ | +0.00% ±0.34% | -0.27% ±0.59% |
| 27 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | **on** | +15 | +1 | 16.70% (n=46,020) | 16.63% (n=46,020) | 0.07% | +4 [-18, +26] | 63.2% | +0.16 (z +2.48) helps * | +0.45% ±0.34% | +0.69% ±0.59% |
| 28 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | off | +14 | +12 | 16.64% (n=46,020) | 16.70% (n=46,020) | -0.06% | -4 [-38, +30] | 40.7% | +0.05 (z +0.75) ~ | +0.13% ±0.33% | +0.19% ±0.58% |
| 29 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | off | +14 | – | 16.81% (n=17,574) | 16.52% (n=17,574) | 0.28% | +14 [-22, +50] | 77.9% | +0.09 (z +1.42) ~ | +0.03% ±0.32% | +0.32% ±0.58% |
| 30 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | **on** | +14 | +48 | 16.90% (n=23,574) | 16.44% (n=23,574) | 0.46% | +29 [-4, +62] | 95.6% | -0.04 (z -0.59) ~ | +0.03% ±0.34% | +0.53% ±0.59% |
| 31 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | **on** | +14 | +6 | 16.71% (n=46,020) | 16.63% (n=46,020) | 0.08% | +4 [-18, +27] | 65.0% | -0.06 (z -0.93) ~ | +0.12% ±0.34% | +0.25% ±0.59% |
| 32 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | off | +13 | – | 16.79% (n=17,574) | 16.54% (n=17,574) | 0.25% | +13 [-23, +48] | 75.3% | -0.01 (z -0.08) ~ | -0.04% ±0.32% | -0.04% ±0.57% |
| 33 | `camp-party` | The peacetime camp party. | **on** | +13 | +53 | 17.01% (n=46,020) | 16.32% (n=46,020) | 0.69% | +35 [+10, +60] | 99.7% | -0.04 (z -0.59) ~ | -0.27% ±0.35% | -0.55% ±0.59% |
| 34 | `governor-every-lane` | Run the strategic governor under every lane. | off | +13 | -8 | 16.53% (n=46,020) | 16.81% (n=46,020) | -0.28% | -15 [-54, +23] | 21.8% | -1.10 (z -16.93) hurts * | +1.11% ±0.33% | +1.18% ±0.59% |
| 35 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | **on** | +13 | +39 | 16.77% (n=46,020) | 16.56% (n=46,020) | 0.20% | +10 [-26, +45] | 70.3% | +0.00 (z +0.02) ~ | -0.78% ±0.31% | -0.90% ±0.57% |
| 36 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | **on** | +13 | +21 | 16.80% (n=46,020) | 16.53% (n=46,020) | 0.27% | +13 [-9, +36] | 87.9% | +0.02 (z +0.28) ~ | +0.35% ±0.34% | +1.28% ±0.60% |
| 37 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | **on** | +12 | +18 | 16.74% (n=46,020) | 16.60% (n=46,020) | 0.14% | +7 [-15, +29] | 73.6% | -0.08 (z -1.18) ~ | -0.07% ±0.33% | +0.57% ±0.60% |
| 38 | `come-ashore` | Keep the land army out of the water. | **on** | +11 | +36 | 16.78% (n=46,020) | 16.56% (n=46,020) | 0.22% | +11 [-18, +40] | 77.4% | +0.11 (z +1.67) ~ | -0.13% ±0.38% | -0.58% ±0.62% |
| 39 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | off | +7 | -7 | 16.72% (n=46,020) | 16.62% (n=46,020) | 0.10% | +5 [-18, +27] | 66.3% | -0.08 (z -1.18) ~ | -0.03% ±0.34% | -0.10% ±0.59% |
| 40 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | **on** | +6 | +29 | 16.86% (n=46,020) | 16.48% (n=46,020) | 0.38% | +19 [-3, +41] | 95.5% | +0.04 (z +0.65) ~ | +0.11% ±0.35% | +0.47% ±0.62% |
| 41 | `barbarian-bargain` | Price a raider's life below a major's. | off | +5 | – | 16.71% (n=17,574) | 16.62% (n=17,574) | 0.09% | +5 [-32, +41] | 59.6% | +0.06 (z +0.93) ~ | -0.31% ±0.34% | -0.23% ±0.59% |
| 42 | `housing-districts` | Let the baseline governor raise the housing ceiling. | off | +5 | -9 | 16.60% (n=46,020) | 16.74% (n=46,020) | -0.14% | -7 [-29, +16] | 27.7% | +0.01 (z +0.13) ~ | +0.19% ±0.35% | +0.42% ±0.58% |
| 43 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | off | +3 | -4 | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 44 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | off | +3 | -5 | 16.80% (n=46,020) | 16.53% (n=46,020) | 0.27% | +14 [-17, +46] | 81.1% | +0.01 (z +0.18) ~ | +0.09% ±0.34% | +0.26% ±0.60% |
| 45 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | **on** | +2 | +35 | 16.77% (n=23,574) | 16.56% (n=23,574) | 0.21% | +16 [-16, +47] | 83.5% | +0.05 (z +0.81) ~ | +0.05% ±0.32% | -0.30% ±0.57% |
| 46 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | **on** | +1 | +39 | 16.80% (n=46,020) | 16.54% (n=46,020) | 0.26% | +13 [-12, +38] | 84.9% | +0.10 (z +1.53) ~ | +0.18% ±0.32% | +0.14% ±0.58% |
| 47 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | off | +1 | +3 | 16.56% (n=46,020) | 16.77% (n=46,020) | -0.21% | -11 [-37, +15] | 20.9% | +0.05 (z +0.73) ~ | -0.25% ±0.33% | -0.20% ±0.57% |
| 48 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | off | -2 | +26 | 16.81% (n=46,020) | 16.52% (n=46,020) | 0.29% | +14 [-8, +36] | 89.5% | +0.16 (z +2.52) helps * | +0.01% ±0.36% | -0.18% ±0.60% |
| 49 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | off | -3 | +13 | 16.68% (n=46,020) | 16.65% (n=46,020) | 0.03% | +2 [-20, +24] | 56.2% | -0.06 (z -0.96) ~ | -0.41% ±0.36% | -0.85% ±0.60% |
| 50 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | off | -3 | +21 | 16.87% (n=46,020) | 16.46% (n=46,020) | 0.40% | +21 [-9, +52] | 91.6% | +0.02 (z +0.34) ~ | -0.03% ±0.35% | -0.02% ±0.60% |
| 51 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | off | -5 | -11 | 16.62% (n=46,020) | 16.71% (n=46,020) | -0.09% | -4 [-27, +18] | 34.9% | +0.05 (z +0.83) ~ | +0.17% ±0.33% | +0.48% ±0.60% |
| 52 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | off | -5 | +17 | 16.90% (n=46,020) | 16.43% (n=46,020) | 0.47% | +26 [-16, +68] | 88.7% | +0.11 (z +1.69) ~ | -0.15% ±0.39% | -0.03% ±0.63% |
| 53 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | off | -5 | +21 | 16.78% (n=46,020) | 16.56% (n=46,020) | 0.22% | +11 [-12, +33] | 82.6% | -0.01 (z -0.09) ~ | +0.38% ±0.34% | -0.02% ±0.59% |
| 54 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | off | -6 | -4 | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.11% | -5 [-28, +17] | 31.6% | +0.17 (z +2.57) helps * | +0.10% ±0.37% | +0.50% ±0.62% |
| 55 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | off | -7 | +4 | 16.67% (n=46,020) | 16.66% (n=46,020) | 0.01% | +0 [-22, +22] | 50.8% | +0.05 (z +0.84) ~ | +0.02% ±0.35% | +0.30% ±0.60% |
| 56 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | off | -12 | +20 | 16.76% (n=46,020) | 16.57% (n=46,020) | 0.19% | +10 [-14, +33] | 78.4% | -0.01 (z -0.12) ~ | +0.14% ±0.35% | +0.39% ±0.60% |
| 57 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use its direct empire-wide repair. | off | -14 | -4 | 16.70% (n=46,020) | 16.64% (n=46,020) | 0.06% | +3 [-24, +31] | 59.0% | -0.03 (z -0.53) ~ | +0.34% ±0.33% | +0.29% ±0.57% |
| 58 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | off | -15 | +4 | 16.54% (n=46,020) | 16.80% (n=46,020) | -0.26% | -13 [-35, +10] | 13.0% | -0.06 (z -0.97) ~ | -0.62% ±0.32% | -0.57% ±0.57% |
| 59 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | off | -16 | +14 | 16.46% (n=46,020) | 16.87% (n=46,020) | -0.40% | -21 [-64, +21] | 16.1% | +0.02 (z +0.34) ~ | +0.27% ±0.34% | -0.45% ±0.56% |
| 60 | `theology-for-founders` | A founder researches Theology next. | off | -16 | -5 | 16.54% (n=23,574) | 16.80% (n=23,574) | -0.26% | -12 [-40, +16] | 20.4% | -0.07 (z -1.08) ~ | +0.07% ±0.34% | -0.20% ±0.59% |
| 61 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | off | -19 | +20 | 16.67% (n=46,020) | 16.67% (n=46,020) | 0.00% | -0 [-23, +23] | 49.1% | -0.16 (z -2.50) hurts * | -0.48% ±0.34% | -0.96% ±0.59% |
| 62 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | off | -22 | – | 16.45% (n=17,574) | 16.88% (n=17,574) | -0.43% | -22 [-57, +14] | 11.5% | -0.02 (z -0.25) ~ | +0.73% ±0.38% | +0.91% ±0.62% |
| 63 | `governor-expansion-lane` | The other half: the governor under Expansion only. | off | -30 | – | 16.37% (n=17,574) | 16.97% (n=17,574) | -0.60% | -30 [-66, +6] | 5.2% | -0.02 (z -0.36) ~ | +0.48% ±0.35% | +0.51% ±0.60% |
| 64 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | off | -31 | – | 16.36% (n=17,574) | 16.97% (n=17,574) | -0.61% | -31 [-66, +5] | 4.4% | -0.08 (z -1.25) ~ | -0.37% ±0.37% | -0.55% ±0.64% |
| 65 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends its turn beside a free 50-gold clear until the camp spawns the archer that kills it. | off | -86 | – | 15.80% (n=17,574) | 17.53% (n=17,574) | -1.73% | -86 [-123, -50] | 0.0% | -0.50 (z -7.54) hurts * | -0.94% ±0.32% | -0.78% ±0.58% |

## What the posterior would change

A threshold in column units is not a threshold in evidence. The screens these columns come from resolve between ±29 and ±101 at 80% power — more than three to one — so **+24 decides differently depending only on which screen priced the gene**, and #2294's single-column +20 bar sits below every band the instrument has ever printed. *Posterior (95% CI)* above is the answer to that: a random-effects (DerSimonian–Laird) inverse-variance pool of every screen's on−off difference on this column's own scale, each screen weighted by its own standard error, with the disagreement **between** screens carried in the interval rather than assumed away. `P(>0)` is where the shrinkage shows: two genes can print the same +30 and land at 90% and 99.8%.

**It is published, not in force.** `AUTHORITY` in `tools/gene_ledger.py` is the whole switch, it says `columns`, and this table is what the other settings would ship. Two reasons it is not flipped here, neither of them arithmetic: the threshold rule is an explicit operator directive, and **every source in this ledger is the retired `legacy` 60×38 Pangaea shape** — re-deciding the deployment genome now would re-decide it on the wrong instrument.

| Authority | Genes on | Genes moved | What it is |
|---|---:|---:|---|
| `columns` **(in force)** | 31 | 0 | the operator's threshold rule, exactly as it ships: both win columns positive, or their average above +15 with neither below −10, or one column above +20 — and off whatever they say when the pooled *Diff* is negative |
| `posterior-veto` | 34 | 3 | the same columns, with an error bar on the veto: it fires only when the posterior's 95% interval lies **wholly below zero**, instead of on the bare sign of a difference that carries no error at all |
| `posterior` | 34 | 3 | the pooled estimate decides wherever its interval excludes zero, and `posterior-veto` decides where it straddles |

### The genes that move

Each row is a gene whose shipped default one of the settings above would change. `on`/`off` in bold is a move.

⚠ Read what these rows do and do not say. Every one is a **re-admission**, and not one of them has a positive point estimate: the posterior is not saying these genes help, it is saying the veto that removed them **could not tell**. The shipped rule fires on the sign of a pooled difference that carries no error at all — −0.78, −0.21 and −0.06 pp — and every one of those three intervals straddles zero. Where the interval straddles, the `posterior` setting inherits the columns' answer, because `default_on` has to be a pure function of the sources and the only other candidate is whatever shipped yesterday. That deferral is the reason *Where a direct arm pays* exists.

| Gene | Shipped | `posterior-veto` | `posterior` | Posterior (95% CI) | P(>0) | Pooled *Diff* |
|---|---|---|---|---:|---:|---:|
| `apostle-promotion-by-role` | off | **on** | **on** | -4 [-38, +30] | 40.7% | -0.06% |
| `siege-commitment` | off | **on** | **on** | -11 [-37, +15] | 20.9% | -0.21% |
| `war-economy` | off | **on** | **on** | -48 [-185, +88] | 24.4% | -0.78% |

### What the posterior can decide at all

Of 65 priced genes the interval clears zero for **13 upward** and **1 downward**; **51 sit inside the interval either way** and are the boundary set below. A straddling interval is not a null — it is the instrument saying it cannot tell, which is exactly what a fixed ±15 bar cannot say.

| Gene | Posterior (95% CI) | P(>0) | Screens | Shipped | Posterior call |
|---|---:|---:|---:|---|---|
| `barbarian-scouts-are-scouts` | +45 [+21, +68] | 100.0% | 3 | on | **on** |
| `blind-objective-strength` | +27 [+5, +50] | 99.2% | 3 | on | **on** |
| `bounded-recovery` | +28 [+6, +51] | 99.3% | 3 | on | **on** |
| `camp-party` | +35 [+10, +60] | 99.7% | 3 | on | **on** |
| `governor-victory-lanes` | +46 [+9, +82] | 99.3% | 1 | on | **on** |
| `great-person-housing` | +78 [+42, +114] | 100.0% | 1 | on | **on** |
| `idle-faith-patronage` | +26 [+9, +44] | 99.8% | 2 | on | **on** |
| `loyalty-rate-alarm` | +49 [+25, +73] | 100.0% | 3 | on | **on** |
| `recon-replacement` | +53 [+25, +82] | 100.0% | 3 | on | **on** |
| `settle-sooner` | +41 [+5, +76] | 98.8% | 1 | on | **on** |
| `settler-threat-detour` | +50 [+14, +86] | 99.7% | 1 | on | **on** |
| `whole-turn-backtrack-guard` | +25 [+3, +47] | 98.6% | 3 | on | **on** |
| `wide-map-capacity` | +33 [+11, +55] | 99.8% | 3 | on | **on** |
| `barbarian-hunt` | -86 [-123, -50] | 0.0% | 1 | off | **off** |

## The two shapes, apart

`τ` (tau) is the between-screen standard deviation the random-effects pool estimates. It is the statistic that answers *“is 'both columns positive' two confirmations?”*: when screens agree to within their errors it is zero and the pool is the ordinary inverse-variance one; when they do not, it widens the interval instead of averaging two worlds into a confident wrong answer. `POSTERIOR_SHAPES` in `tools/gene_ledger.py` says which shapes the published pool admits and is currently `standard, legacy`.

| Shape | Sources | Seat pairs | Genes priced |
|---|---:|---:|---:|
| standard | 0 | 0 | 0 |
| legacy | 7 | 66,220 | 65 |

⚠ **No `standard` source is in the ledger yet**, so every figure in this file is the retired Pangaea instrument and the per-gene split below is empty. It fills the moment a screen at the deployment shape enters `docs/gene_ledger.json`, and `docs/gene_ranking_notes.md` carries what the first one already says about the genes it disagrees with.

## Where a direct arm pays: the boundary genes

**The efficient plan is two stage, and it is not a partial foldover.** The whole-genome screen is the efficient way to RANK: `p10` priced 75 genes at ±51 each on 17,574 seat pairs, and the same budget split into 75 single-gene screens of 234 pairs would give ±145 each even at the best pairing gain this repository has measured — 2.84× wider, which is **8× the games** for the same band. A single-gene arm resolves far tighter per pair once it is aimed (`s7`: ±29 on 6,000 pairs at a 3.32× pairing gain, against `p10`'s 1.09×). So the screen ranks and direct arms resolve the boundary. `docs/GENE_SCREEN.md` carries the arithmetic; do not re-derive it into a blocked or partial foldover, which is neither stage — four-gene `s6` resolves ±64 over 6,000 pairs where one-gene `s7` resolves ±29 over the same 6,000.

*Buys* is the expected value of one direct arm of **7,200 seat pairs**, in wins per 10,000 on-arm seats, read against the gene's **shipped** state — so a gene the evidence likes and the genome already plays has little to buy, and a gene the evidence likes that the rule holds off has the whole effect to buy. *Pairs to resolve* is how many matched seat pairs that arm needs before the combined interval clears zero, if it reads the gene's current pooled effect. Both are sized from `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json`, the widest single-gene arm this repository has actually run (24.3 per-column SE at 7,200 pairs) — the conservative end, since a gene that rarely fires cancels far more and resolves tighter.

| Gene | Posterior (95% CI) | P(>0) | Shipped | Buys | Pairs to resolve |
|---|---:|---:|---|---:|---:|
| `garrison-under-fire` | +26 [-16, +68] | 88.7% | off | +26.2 | 14,974 |
| `siege-tracks-wall` | +21 [-9, +52] | 91.6% | off | +21.3 | 18,204 |
| `score-horizon` | +18 [-4, +41] | 94.4% | off | +18.3 | 16,727 |
| `barbarian-ranged-answer` | +14 [-22, +50] | 77.9% | off | +14.8 | 68,449 |
| `recorded-tactical-step` | +15 [-8, +37] | 89.8% | off | +14.6 | 44,430 |
| `war-reinforcement` | +14 [-17, +46] | 81.1% | off | +14.4 | 64,503 |
| `buildings-before-projects` | +14 [-8, +36] | 89.5% | off | +14.2 | 48,325 |
| `builder-barbarian-safety` | +13 [-23, +48] | 75.3% | off | +13.2 | 91,754 |
| `strategic-wonders` | +11 [-12, +33] | 82.6% | off | +10.7 | 110,639 |
| `slot-kind-tiebreak` | +10 [-14, +33] | 78.4% | off | +9.6 | 151,700 |
| `war-economy` | -48 [-185, +88] | 24.4% | off | +8.8 | 6,108 |
| `barbarian-bargain` | +5 [-32, +41] | 59.6% | off | +7.2 | 778,312 |
| `district-coverage` | +5 [-23, +34] | 64.6% | off | +6.5 | 531,138 |
| `wonder-ring-settle-value` | +5 [-18, +27] | 66.3% | off | +5.2 | 669,597 |
| `amenity-project-preemption` | +3 [-24, +31] | 59.0% | off | +4.6 | 1,601,818 |
| `army-target-weighs-enemy` | -1 [-37, +35] | 48.2% | off | +4.0 | 24,340,573 |
| `settler-guard-holds` | +2 [-20, +24] | 56.2% | off | +2.9 | 5,235,433 |
| `apostle-promotion-by-role` | -4 [-38, +30] | 40.7% | off | +2.3 | 956,215 |
| `blind-objective-units` | +0 [-22, +22] | 50.8% | off | +2.0 | 288,120,952 |
| `war-patience` | -0 [-23, +23] | 49.1% | off | +1.9 | 224,277,755 |
| `naval-recon` | -1 [-23, +21] | 46.3% | off | +1.4 | 14,697,345 |
| `housing-research` | +10 [-26, +45] | 70.3% | on | +1.1 | 161,581 |
| `holy-lane-parity` | +45 [-24, +114] | 89.9% | on | +0.8 | 4,698 |
| `governor-every-lane` | -15 [-54, +23] | 21.8% | off | +0.7 | 57,876 |
| `settler-target-hysteresis` | +4 [-18, +26] | 63.2% | on | +0.6 | 1,125,166 |
| `relief-targets-the-siege` | +4 [-18, +27] | 65.0% | on | +0.5 | 808,996 |
| `endgame-war-runway` | -4 [-27, +18] | 34.9% | off | +0.5 | 803,132 |
| `siege-is-progress` | -21 [-64, +21] | 16.1% | off | +0.4 | 26,569 |
| `escort-unstick` | +27 [-20, +74] | 87.3% | on | +0.4 | 14,574 |
| `joint-tactics` | -5 [-28, +17] | 32.6% | off | +0.4 | 586,049 |
| `civilian-rescue` | -5 [-28, +17] | 31.6% | off | +0.3 | 521,764 |
| `come-ashore` | +11 [-18, +40] | 77.4% | on | +0.2 | 113,742 |
| `settler-site-agreement` | +10 [-18, +38] | 76.4% | on | +0.2 | 134,764 |
| `housing-districts` | -7 [-29, +16] | 27.7% | off | +0.2 | 324,421 |
| `theology-for-founders` | -12 [-40, +16] | 20.4% | off | +0.2 | 95,752 |
| `amenity-district-path` | +7 [-15, +29] | 73.6% | on | +0.1 | 285,817 |
| `inquisition-on-threat` | +16 [-16, +47] | 83.5% | on | +0.1 | 50,286 |
| `siege-commitment` | -11 [-37, +15] | 20.9% | off | +0.1 | 115,850 |
| `opportunistic-war` | +23 [-14, +59] | 88.9% | on | +0.1 | 19,312 |
| `district-lookahead-settle` | -22 [-57, +14] | 11.5% | off | +0.1 | 21,945 |
| `builder-worked-tile-priority` | +24 [-11, +60] | 91.3% | on | +0.0 | 14,150 |
| `peacetime-deterrence` | +13 [-12, +38] | 84.9% | on | +0.0 | 70,225 |
| `governor-expansion-lane` | -30 [-66, +6] | 5.2% | off | +0.0 | 5,592 |
| `raid-pillage-prizes` | +30 [-6, +65] | 95.0% | on | +0.0 | 5,586 |
| `priced-tile-purchase` | -31 [-66, +5] | 4.4% | off | +0.0 | 4,172 |
| `home-defense` | -13 [-35, +10] | 13.0% | off | +0.0 | 66,475 |
| `stranded-settler-discount` | +13 [-9, +36] | 87.9% | on | +0.0 | 58,899 |
| `founder-temple` | +29 [-4, +62] | 95.6% | on | +0.0 | 4,719 |
| `strike-opening` | +19 [-3, +41] | 95.3% | on | +0.0 | 12,540 |
| `religion-sues-peace` | +19 [-3, +41] | 95.5% | on | +0.0 | 11,805 |
| `one-launch-pad` | +20 [-2, +42] | 96.5% | on | +0.0 | 5,991 |

The top 8 that one batch could actually resolve (≤ 60,000 seat pairs each), as an argument list:

```sh
gene_screen --genes garrison-under-fire,siege-tracks-wall,score-horizon,recorded-tactical-step,buildings-before-projects,war-economy,holy-lane-parity,governor-every-lane
```

`python3 tools/heuristic_gene_ranking.py --boundary` prints this list on its own, with `--arm-pairs` and `--max-arm-pairs` to size it.

## Lane genes and the share axis

At the standing 250-turn Online clock a **science or congress gene cannot pay through the win axis at all**: science and diplomatic victories land at median t283 and t285, past the clock, so they are 1–2% of endings and `docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for exactly that reason. The seat a lane gene would have carried to a science victory shows up as a score win or a score loss instead. The decision axis stays WINS — `docs/GENOME.md` records what happened the one time selection ran on a correlate — so the share reading is a **pre-registered secondary**, fixed in `docs/GENE_SCREEN.md` before the next screen rather than chosen after it.

The set is discovered from the code: every gene whose flag field `src/ai/advanced/victory_lane.rs` reads. A gene joins it by being a lane gene, not by being listed here.

| Lane gene | Default | ± Wins / 10k seats | Share Δpp (z) | Posterior (95% CI) | Status |
|---|---|---:|---|---:|---|
| `lane-congress-ballot` | off | – | – | – | awaiting its first screen |
| `lane-congress-favor` | off | – | – | – | awaiting its first screen |
| `lane-great-people` | off | – | – | – | awaiting its first screen |
| `lane-policy-deck` | off | – | – | – | awaiting its first screen |
| `lane-culture-spending` | off | – | – | – | awaiting its first screen |
| `lane-space-race` | off | – | – | – | awaiting its first screen |
| `competition-victory-points` | off | – | – | – | awaiting its first screen |

## Awaiting measurement

These screenable genes have no on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a screen is pending.

| Gene | Default | Description |
|---|---|---|
| `air-surge` | off (unmeasured) | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. |
| `barbarian-capture-priority` | off (unmeasured) | Take a visible Barbarian Settler or Scout in exact one-turn reach before healing, retreat, or any ordinary tactical choice. |
| `builder-reward-survey` | off (unmeasured) | Price Builder production by a survey of the work it would do. |
| `campus-adjacency-threshold` | off (unmeasured) | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. |
| `campus-finishes-first` | off (unmeasured) | The Campus coverage term is scaled by how finished the empire's standing Campuses are. |
| `chain-tech-lookahead` | off (unmeasured) | The research goal aims at a Campus rung the empire can BUILD, not only one it has already built. |
| `competition-victory-points` | off (unmeasured) | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. |
| `condemn-under-congress` | off (unmeasured) | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. |
| `congress-banks-decided` | off (unmeasured) | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact prediction and staking nothing. |
| `congress-counter-votes` | off (unmeasured) | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fails costs no Favor. |
| `contact-posture` | off (unmeasured) | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answer, or step out of that shooter's envelope. |
| `coordinated-finish` | off (unmeasured) | Admit the friendly-volley extension without the rest of the closed war-half bundle. |
| `coupled-expansion` | off (unmeasured) | Enable the evaluator-only paid expansion treatment. |
| `culture-building-debt` | off (unmeasured) | Make the Theater Square owe its buildings. |
| `culture-coverage` | off (unmeasured) | Pay for the Theater Square the empire has not got. |
| `district-building-chain` | off (unmeasured) | Make every specialty district owe its own buildings, whatever the lane. |
| `early-contact-window` | off (unmeasured) | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. |
| `engine-faith-price` | off (unmeasured) | THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE. |
| `enhancer-for-the-corps` | off (unmeasured) | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. |
| `envoy-infrastructure` | off (unmeasured) | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the turn limit, and a first Diplomatic Quarter sees part of the Consulate stream it unlocks. |
| `fifteenth-citizen` | off (unmeasured) | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. |
| `fortify-idle-units` | off (unmeasured) | Fortify units the planner gave nothing to do. |
| `guru-heals-the-corps` | off (unmeasured) | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. |
| `holy-site-where-the-threat-is` | off (unmeasured) | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. |
| `lane-congress-ballot` | off (unmeasured) | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion posture that has no lane. |
| `lane-congress-favor` | off (unmeasured) | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. |
| `lane-culture-spending` | off (unmeasured) | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture whose plan has not named the lane. |
| `lane-great-people` | off (unmeasured) | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. |
| `lane-policy-deck` | off (unmeasured) | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. |
| `lane-space-race` | off (unmeasured) | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be sited in all read the race rather than an explicitly assigned target, and the pass opens at all. |
| `maintenance-aware-deck` | off (unmeasured) | Let the deck counterfactual see the unit-maintenance bill. |
| `naval-production-policy` | off (unmeasured) | Reach for the naval-production discount while hulls are wanted. |
| `one-shot-recovery` | off (unmeasured) | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. |
| `pantheon-board` | off (unmeasured) | Choose the pantheon from the land the empire holds rather than from a fixed order. |
| `power-the-laboratory` | off (unmeasured) | A power plant is credited the yields it switches on in its city. |
| `price-the-suzerainty` | off (unmeasured) | Let the envoy scorer see the suzerainty it is walking toward. |
| `promote-when-wounded` | off (unmeasured) |  |
| `religious-defence-scales` | off (unmeasured) | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. |
| `religious-units-heal-first` | off (unmeasured) | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. |
| `research-floor-holds` | off (unmeasured) | The citizen tilt and the beaker floor hold while the research can still pay. |
| `research-grants-first` | off (unmeasured) | A finished research city pays more for its own district's project. |
| `research-tier-premium` | off (unmeasured) | A Campus building's debt is scaled by its own Science against the chain's first rung. |
| `science-multiplier-payoff` | off (unmeasured) | Credit a Campus building the beakers its city's multipliers will actually pay it. |
| `science-payback-horizon` | off (unmeasured) | Price the science economy on whether it can still repay rather than on how much of the game is left. |
| `settlement-gap-target` | off (unmeasured) | Make the settlement-gap redirect and the Settler ranking honour the same city target the cascade settles toward. |
| `spread-campaign-persists` | off (unmeasured) | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is spent. |
| `tactical-strategy` | off (unmeasured) | Enable explicit battlefield roles: the land-unit counter cycle, safe ranged standoff, wall-focused siege/support, and cavalry job priority. |
| `unit-cost-efficiency` | off (unmeasured) | Credit strength-per-production and the civ's own unique unit in the military production arm. |
| `unit-objective-memory` | off (unmeasured) | Let a unit retain its campaign objective and a short, threat-driven retreat across turns. |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±/10k seats (last tracked measurement) | Win rate (on) | Win rate (off) | Source |
|---|---:|---:|---:|---|
| `suzerain-cards` | +42 | 17.09% | 16.25% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `wonder-prereq-reach` | +29 | 16.96% | 16.38% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `camp-reach` | +10 | 16.77% | 16.56% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-buildings` | +8 | 16.75% | 16.59% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `ranged-line-of-sight` | +4 | 16.71% | 16.63% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `recon-flight` | -1 | 16.66% | 16.67% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-cards` | -4 | 16.62% | 16.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `arrival-waves` | -7 | 16.59% | 16.74% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `idle-walkers-close-the-pipeline` | -10 | 16.56% | 16.77% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `muster-at-command-radius` | -12 | 16.55% | 16.79% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `barbarian-walls-one-tier` | -13 | 16.54% | 16.80% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `siege-muster` | -26 | 16.41% | 16.93% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `siege-role` | -39 | 16.27% | 17.06% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `garrison-walls` | -54 | 16.12% | 17.21% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `loyalty-policy-defence` | -54 | 16.13% | 17.20% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `campus-every-city` | -94 | 15.73% | 17.60% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `stacked-escort` | -104 | 15.63% | 17.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `settler-stack-discipline` | -116 | 15.51% | 17.83% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |

## How to read this

Every screenable heuristic gene on the Advanced controller, ranked most beneficial to least by **± Wins / 10k seats** — wins added per 10,000 six-player on-arm seats at the gene's measured on-rate in its **latest** screen. *± Wins / 10k seats prior* is the same figure from the screen before that (– when the gene has only one reading); movement between the two columns is the gene's trend across cycles. *Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since 2026-08-22 that call is read off these two win columns: a gene defaults **on** when both are positive, or when their average clears +15 with neither below −10; with exactly one populated column it defaults **on** when that reading is above +20. It defaults **off** otherwise. The *Total* win-rate columns pool every screen that measured the gene, weighted by on-arm seats, and each carries its own on-arm seat count `n` — the two arms are only equal when every screen that measured the gene split them evenly. *Diff* is the on rate minus the off rate, rendered as a percentage: the **whole** on−off difference, so it stands at roughly twice the scale of the win columns beside it and must be read against a screen’s difference band rather than the halved column band below. **A negative *Diff* vetoes the default** (operator, 2026-08-22): a gene that has not won more than it lost across its whole record ships off however its two win columns read. That is the one clause that lets a screen older than the last two speak, and it is one-way — a positive *Diff* promotes nothing on its own, the columns still have to clear their bars. Three genes ship off on it alone: `war-economy`, `apostle-promotion-by-role` and `siege-commitment`, each carrying positive recent columns over a 2026-08-20 screen they have not made back. **There is one screen** (operator, 2026-08-22): six majors on 74x46 continents with nine city-states, Online speed to its own 250-turn clock, all six victory lanes, a foldover against the best-genome baseline with shuffled civs and every major seat carrying its own genome (errors clustered by game pair), so a gene's on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the instrument; the paired contrasts, intervals and family-wise verdicts stay in `docs/gene_ledger.json`. Screenable genes awaiting their first measurement are listed separately below without a rank.

**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected count is 1,667 wins per 10,000 on-arm seats and the win columns say how far above or below that a seat carrying the gene lands. **A column is half its screen’s on−off difference** — a foldover puts the two arms either side of chance — so the band that says whether a column is real is half the band on that difference. The two are not interchangeable: the ±110/10k figure this paragraph used to quote, and #2266 used to call eight removals noise, is the *difference*’s band and is twice too wide for the column beside it. Each screen’s own band is below, derived from its errors rather than quoted. Screens differ in baseline as repairs land, so the *Prior* column reads as history, not a strict A/B against *Last*.

**⚠ Every column below is `legacy`.** The shape marked `legacy` in the screen table is the pre-2026-08-22 instrument: 60x38 Pangaea, six city-states, where 48% of games ended in a religious conversion against 28% on continents. Those readings are what the deployment genome stands on and they are kept for that reason, but a gene is only priced at the screen once a `standard` row appears beside it. The four-player `domination,score` war columns are gone: a 1-in-4 chance base made them incomparable with the six-player columns printed next to them.

**What each screen resolves.** The median gene’s column standard error times 2.8 — a two-sided 5% test at 80% power. Judge a column against the band of the screen named beside it, never against a single number for the instrument: these differ by more than three to one.

*Pairing gain* is how far a screen’s error per pair sits below the unpaired baseline, and it is what separates them. A foldover cancels only to the extent its two arms play a similar game, so the gain reads on the **genes**, not the design — a gene that rarely fires leaves most pairs identical and cancels almost everything, while a whole-genome screen flips every gene between arms and cancels almost nothing. ⚠ Gene count is not the driver, though the rows below invite that reading — the falsifier is in them. `h1` carries **one** gene over **7,200** pairs and resolves ±68 at a 1.28× gain, *wider* than four-gene `s6` over 6,000. Its gene changes nearly every game; `s7`'s rarely fires. That, not the count, is the difference.

| Screen | Shape | Genes | Seat pairs | 1 SE | ±80% power | Pairing gain |
|---|---|---:|---:|---:|---:|---:|
| `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` | legacy | 1 | 7,200 | 24.3 | ±68 | 1.28× |
| `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` | legacy | 75 | 17,574 | 18.3 | ±51 | 1.09× |
| `2026-08-21-p7-native-6p-allseats-15000-pairs.json` | legacy | 57 | 15,000 | 19.9 | ±56 | 1.08× |
| `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` | legacy | 1 | 6,000 | 10.3 | ±29 | 3.32× |
| `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` | legacy | 4 | 6,000 | 22.9 | ±64 | 1.49× |
| `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` | legacy | 1 | 1,000 | 36.1 | ±101 | 2.68× |
| `2026-08-20-p4-native-6p-allseats-13446-pairs.json` | legacy | 64 | 13,446 | 21.5 | ±60 | 1.06× |

**Posterior (95% CI), P(>0), Share Δpp (z).** *Posterior* is a random-effects (DerSimonian–Laird) inverse-variance pool of **every** screen that priced the gene, on this column's own scale: each screen's on−off difference weighted by its own standard error, with the between-screen disagreement (τ) carried in the interval instead of assumed away. It is the answer to two things the columns cannot express — that the same +24 means different things from a ±29 screen and a ±64 one, and that two positive columns from screens differing in baseline, build and shape are not two confirmations (#2283/#2284 measured that: five of seven lane genes changed sign on disjoint seeds). *P(>0)* is where the shrinkage lands. *Share Δpp (z)* is the newest screen's score-share contrast and its verdict, published beside the win columns because the deployment rule reads the win axis only and a lane gene cannot pay on that axis at 250 turns. **None of these three decides anything today**; `AUTHORITY` in `tools/gene_ledger.py` says `columns` and *What the posterior would change* above is the delta.

**Cost.** Positive is slower; negative is faster. *cost (compute)* is the on/off percent change in wall seconds per completed turn, while *cost (time)* is the percent change in whole-game wall seconds and therefore includes games that end earlier or later. Each cell is the newest estimate ± one standard error. The screen derives both from paired log-ratios on the same maps, fits every randomized gene together with an arm-order intercept, and keeps one timing per game pair; all-seats signs are summed so the answer is the incremental cost of enabling one major's genome. This reuses the screen's existing `secs` and `turn` rows — no hot-path timers and no extra profiling games. A dash means the source analysis predates the estimator and is unknown, never zero.

Regenerate with `python3 tools/heuristic_gene_ranking.py --write` after every screen enters the ledger; `tools/test_heuristic_gene_ranking.py` fails when this file is older than the ledger's sources.

## Follow-ups

**The first standard-shape screen disagrees with this whole file, and the posterior says which parts of the disagreement are real.** A 74x46 Continents / 9 CS / Online-250 all-seats foldover against the best-genome baseline -- 3,937 complete map pairs, **23,622 matched seat comparisons per gene**, seeds 141000000-141003936, source `b3ad9f00` -- is published in `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` (PR #2323). ⚠ It is **not a ledger source**: no `gene_screen --analyze --json` file for it exists yet, so no column, no default and no posterior in `HEURISTIC_GENE_RANKING.md` reads it. The figures below are computed from its published table and are recorded here so the next decision is taken with them in view; `tools/test_heuristic_gene_ranking.py::TheStandardScreenPreview` recomputes every one of them, so this paragraph cannot go stale silently.

*`governor-victory-lanes` is the single largest correctable defect in the shipped genome.* It **defaults on**, promoted by #2294's one-column clause on P10's **+46**. At the deployment shape it reads **-4.73 pp, z -15.37** -- **-237 wins per 10,000 on-arm seats, 95% CI [-267, -206]**. The two readings are not close: legacy resolves it at [+9, +82] and standard at [-267, -206], and nothing lies between them. Pooling the two shapes gives **-96 [-372, +181] with tau = 199**, an interval 550 wide -- which is the random-effects estimator doing its job, saying *these are two instruments, not two draws from one number*. **The correct reading is standard-only, and standard-only resolves it OFF.**

*The legacy share axis already knew.* P10 priced this gene at win z **+2.46** and score-share z **-15.92** -- a recorded `conflict` -- and the deployment rule reads the win axis only, so the +46 promoted it. The deployment shape's **win** axis now reads z -15.37, within half a sigma of what the legacy **share** axis said a day earlier. That is the case for the pre-registered secondary axis in `docs/GENE_SCREEN.md`, made by the genome's own worst row.

*The composite, decomposed.* `governor-every-lane` (the composite) reads **-4.68**, `governor-victory-lanes` (the victory-lane half) **-4.73**, `governor-expansion-lane` (the other half) **-0.55**. The victory half carries essentially the whole of the composite's harm and the halves are close to additive, so this is not "the composite is bad": it is *one named half* is bad, and the repository already has that half as its own gene. The composite and the expansion half both already default off; the harmful half is the one that ships. A confirmation arm on `governor-victory-lanes` at the standard shape (600 map pairs, seeds 150000000-150000599) was running when this was written.

*Six of the eight defaults the standard screen would move are decided by noise.* Entered as a ledger source, the pooled-`Diff` veto alone flips eight genes: `governor-victory-lanes` (z -15.37) and `war-economy` (z +7.50) on real signals, and `settler-site-agreement` (-1.47), `settler-target-hysteresis` (-1.16), `housing-research` (-1.10), `religion-sues-peace` (-1.14), `apostle-promotion-by-role` (+1.02) and `theology-for-founders` (+1.43) on readings of |z| about 1. **The posterior read standard-only resolves exactly two of the eight -- the two with signal** (`governor-victory-lanes` [-267, -206], `war-economy` [+87, +148]) -- and declines all six of the others, whose standard-only intervals all straddle zero. Read *pooled* it resolves none of the eight, because tau is 199 and 124 on the two real ones. Both halves of that are the recommendation: adopt the posterior, and when a `standard` source lands set `POSTERIOR_SHAPES = ("standard",)` in `tools/gene_ledger.py` so the deployment shape decides and the Pangaea record stays history.

*What survives the change of instrument.* `great-person-housing` +78 -> +94, `raid-pillage-prizes` +30 -> +53 and `opportunistic-war` +23 -> +49 read positive at both shapes and pool tighter, not wider; `wide-map-capacity` goes +33 -> +90. Rank 1 does not: `holy-lane-parity`, +99 on legacy and confirmed by its own direct arm, reads **+20 [-12, +51]** at the deployment shape. Two genes the legacy ledger has never priced arrive resolved -- `air-surge` **+108 [+77, +138]** and `contact-posture` **-54 [-85, -24]**.


**`holy-lane-parity` came back and was confirmed directly.** It left the code with #2266's bottom ten on a **-27** reading from the four-gene `s6` screen, whose column band is +/-64 -- a null, not a measurement against the gene. P10 was already running when that cull merged (its binary predates it by 1h43m), so it priced the gene after the code was gone: **+63** at z +3.48, past P10's own family-wise bar and the only such reading among the nineteen genes in *Removed from the code*. #2299 restored the code and ran the direct arm the cull never got to: 1,200 map pairs on seeds 110M, every other treatment held at the deployment genome, all 2,400 games complete. It reads **+99 wins/10k, z +4.05, 95% CI [+51, +147]** (`HELPS **`), against a run that resolves +/-68 -- an independent seed window agreeing with P10's independent instrument. Score share is null (+0.08 pp, z +1.23): the gene converts games, it does not accumulate score. The 850 it pays is `(Culture, theater_square)`'s own figure, deliberately an upper bound rather than a tuned value, so a positive result opens the tuning question rather than answering it. The operator recorded this 60x38 Pangaea confirmation as a **legacy** ledger source, so the row below reads **+99/+63** -- both columns positive -- and the gene **defaults on**, taking the genome to 34. That moves the incumbent every recorded Elo result is filed against; it is the first gene to be removed by a cull, restored, and promoted. See [the confirmation](docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md).

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

**P10 ended early at the operator's request.** Its 2,929 complete map seeds provide 5,858 controlled games and 17,574 treated-seat pairs; the analysis excludes 11 interrupted one-arm seeds (66 raw seat rows), with zero duplicate or invalid tuples. The new *Wins / 10k seats* value extrapolates each measured on-arm rate to 10,000 on-arm seats as `round((win_on − 1/6) × 10,000)`; it does not invent synthetic games. The former *Wins / 10k seats* reading shifts intact to *Wins / 10k seats prior*. P10 used the 6p all-six native regime on seeds 100000000–100002962, 60×38 pangaea/online/250 turns, shuffled civilizations, every major seat treated, and foldover against the best-genome baseline. Its fixed binary came from `d23f92d944cd889aa4c9dfe58c37aceb8e55eabd` (SHA-256 `79385db96e89e91cc0b6fd8389e837cb66dc05ccaa4eee493576f152daf627ed`), before later gene removals and additions; ledger generation drops obsolete tags and retains newer genes from their existing sources.

**The bottom of the table was not culled, because the standard screen does not agree with it.** #2330 was launched to remove `barbarian-hunt` under the standing directive that the bottom of the ranking leaves the code. Its row here is the worst in the table -- **-86 wins/10k seats, -1.73 pp, win z -4.65, share z -7.54**, family-wise on P10's own bar, and P10 replicates it internally (its two tranches read -2.08 and -1.27). Nothing about that reading is marginal, and on the instrument that produced it the cull rule fires cleanly. It is still the wrong action, for one reason: **that instrument is `legacy`, and the standard one measured the same gene and disagreed.** [`docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`](docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md) -- source `b3ad9f00`, 3,937 complete map pairs, **23,622 matched seat comparisons per gene** against P10's 17,574, on 74x46 Continents with nine city-states -- reads `barbarian-hunt` at **+0.20 pp, win z +0.65, +10 wins/10k seats**. The sign is opposite and the magnitude is a tenth. This is not the two screens failing to resolve the same small effect: `governor-victory-lanes` at -4.73 pp / z -15.37 in that file puts its standard error near **0.31 pp** on the difference, so P10's -1.73 pp sits about **six standard errors** from what the standard screen measured. One of the two instruments is wrong about this gene, and the paragraph above this one says which of them prices a gene at all: *a gene is only priced at the screen once a `standard` row appears beside it.* Culling on a legacy column contradicts this document's own reading rule.

The legacy board has a second witness and it agrees with the standard screen, not with P10. [The direct arm](docs/eval/2026-08-21-barbarian-hunt-direct.md) held every other treatment at the deployment genome and varied only this gene across 300 map pairs / 1,800 clustered seat-pairs: **+0.56 pp, z +0.51**. That run resolves only +/-3.1 pp at 80% power, so on its own it cannot refute -1.73 -- but it is the third reading in a row whose sign is positive, and it is the arm #2299 used to settle exactly this question for `holy-lane-parity`.

So the gene stays in the code, `off` and unresolved, and **the cull rule does not fire on a `legacy` column**. What would decide it: a `barbarian-hunt` row from a **`standard`-shape screen entering `docs/gene_ledger.json`**. If that row is below the screen's own column band with the sign P10 gave it, the gene leaves the code with the standard number recorded beside it; if it lands where the 23,622-seat screen already put it, P10's -86 was an artefact of the 60x38 Pangaea room -- where 48% of games ended in a religious conversion against 8% at the standard shape -- and the row is a null that never licensed anything.

`lane-congress-ballot` was the same question and gets the same answer for a different reason. [`docs/VICTORY_GENES.md`](docs/VICTORY_GENES.md) §8 records it negative in every window of both regimes and reaching `share hurts *` at z -2.31, and says in terms that if a dedicated arm confirms it *"the gene should leave the code by the same rule that culled the bottom of the ranking"*. That rule is about the bottom of the **ranking**, and an unmeasured gene has no rank -- it is in *Awaiting measurement* above, not in the table -- so nothing was licensed even before the new evidence. The new evidence closes it anyway: the same standard screen reads `lane-congress-ballot` at **+0.16 pp, win z +0.51**, a null. The confirming arm §8 asked for would now have to overturn a 23,622-seat reading, not merely add to five small ones.

⚠ **This is the third and fourth time, and the pattern is now the finding.** #2266 removed ten genes at once against a band that was twice too wide -- the +/-110 it quoted is the *difference*'s band, and the column beside it resolves half that (#2300). `holy-lane-parity` was one of the ten, left on a -27 null, was priced at **+63** by a screen already running when the cull merged, was restored in #2299 and confirmed at **+99, z +4.05** in #2307. Now `barbarian-hunt` and `lane-congress-ballot`: proposed for removal on a legacy column and a five-window trend, and re-priced as nulls by the standard screen before either could be cut. Four genes, three episodes, one mechanism -- **a reading from the wrong instrument, acted on irreversibly.**

So the rule, for whoever culls next. A cull is not the symmetric opposite of a default. A gene left `off` costs one row in a foldover screen and **no games**, and it can be re-priced by every screen that runs afterwards; a gene removed can never be re-priced by anything, and restoring it costs a dedicated confirmation run (1,200 map pairs for `holy-lane-parity`). So the bar for deleting code is not "the worst reading available" -- `barbarian-hunt`'s -86 was the worst reading in the table and it was still the wrong number. It is **a reading on the instrument the agent is actually being screened on**, and the three questions that establish it: is this column `standard` or `legacy`; is there a screen in flight or unmerged that has already priced this gene (check `batch.source_commit` against the cull date, and check the open pull requests); and does a direct arm against the deployment genome agree. `barbarian-hunt` failed all three.

_Generated by `tools/heuristic_gene_ranking.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 13,446 pairs), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 1,000 pairs), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 6,000 pairs), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 6,000 pairs), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 15,000 pairs), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 17,574 pairs), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 7,200 pairs). The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-ten-thousand-seat view of the same observations._
