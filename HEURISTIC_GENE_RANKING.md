# The heuristic gene ranking

**Default on:** both newest columns >0; or avg >+15 with neither <−10; sole reading >+20; pooled *Diff* <0 vetoes.

| Rank | Gene | Description | Default | Scaled ± Wins Last Batch (n seats) | Scaled ± Wins Prior Batch (n seats) | Scaled ± Wins Third Batch (n seats) | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `war-economy` | Send an adaptive Conquest plan through the war production path. | **on** | +118 (n=23,622) | +38 (n=17,574) | +8 (n=15,000) | 16.81% (n=69,642) | 16.53% (n=69,642) | 0.28% | -7 [-130, +117] | 45.8% | +1.43 (z +24.58) helps * | +1.04% ±0.16% | +1.21% ±0.29% |
| 2 | `air-surge` | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. | **on** | +108 (n=23,622) | – | – | 17.74% (n=23,622) | 15.59% (n=23,622) | 2.15% | +108 [+77, +138] | 100.0% | +0.45 (z +7.74) helps * | +0.86% ±0.16% | +0.57% ±0.31% |
| 3 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate works to make room. | **on** | +94 (n=23,622) | +78 (n=17,574) | – | 17.54% (n=41,196) | 15.80% (n=41,196) | 1.74% | +87 [+63, +111] | 100.0% | +0.63 (z +10.83) helps * | +0.10% ±0.16% | +0.16% ±0.29% |
| 4 | `wide-map-capacity` | Price the city ceiling off uncontested land. | **on** | +91 (n=23,622) | +35 (n=17,574) | +29 (n=15,000) | 17.19% (n=69,642) | 16.14% (n=69,642) | 1.05% | +49 [+18, +81] | 99.9% | +0.44 (z +7.61) helps * | +0.49% ±0.16% | +0.24% ±0.29% |
| 5 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | **on** | +61 (n=23,622) | -2 (n=17,574) | +26 (n=15,000) | 16.97% (n=69,642) | 16.36% (n=69,642) | 0.61% | +28 [-0, +57] | 97.4% | +0.28 (z +4.89) helps * | +0.17% ±0.16% | +0.19% ±0.30% |
| 6 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | **on** | +53 (n=23,622) | +30 (n=17,574) | – | 17.10% (n=41,196) | 16.24% (n=41,196) | 0.86% | +43 [+20, +66] | 100.0% | +0.36 (z +6.11) helps * | +0.07% ±0.15% | +0.30% ±0.27% |
| 7 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for peace. | **on** | +49 (n=23,622) | +23 (n=17,574) | – | 17.05% (n=41,196) | 16.29% (n=41,196) | 0.76% | +38 [+12, +63] | 99.8% | +0.39 (z +6.65) helps * | +0.23% ±0.16% | +0.42% ±0.30% |
| 8 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | **on** | +39 (n=23,622) | +36 (n=17,574) | +23 (n=6,000) | 17.02% (n=47,196) | 16.31% (n=47,196) | 0.71% | +29 [+14, +45] | 100.0% | +0.22 (z +3.83) helps * | -0.16% ±0.15% | -0.13% ±0.29% |
| 9 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | **on** | +38 (n=23,622) | +40 (n=17,574) | +73 (n=15,000) | 17.12% (n=69,642) | 16.21% (n=69,642) | 0.90% | +45 [+27, +63] | 100.0% | +0.29 (z +5.03) helps * | +0.08% ±0.16% | +0.21% ±0.30% |
| 10 | `escort-unstick` | Release an escort that is not walking its settler. | **on** | +36 (n=23,622) | +72 (n=17,574) | +7 (n=15,000) | 16.99% (n=69,642) | 16.35% (n=69,642) | 0.64% | +30 [-1, +62] | 97.0% | +0.17 (z +2.85) helps * | +0.40% ±0.16% | +0.45% ±0.29% |
| 11 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | **on** | +34 (n=23,622) | +3 (n=17,574) | -5 (n=15,000) | 16.87% (n=69,642) | 16.46% (n=69,642) | 0.41% | +20 [-4, +44] | 95.0% | +0.15 (z +2.64) helps * | +0.17% ±0.16% | +0.21% ±0.29% |
| 12 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | **on** | +30 (n=23,622) | +48 (n=17,574) | +81 (n=15,000) | 17.12% (n=69,642) | 16.21% (n=69,642) | 0.91% | +46 [+23, +69] | 100.0% | +0.18 (z +3.11) helps * | +0.27% ±0.16% | +0.17% ±0.30% |
| 13 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | **on** | +29 (n=23,622) | +19 (n=17,574) | +39 (n=15,000) | 16.95% (n=69,642) | 16.38% (n=69,642) | 0.57% | +28 [+10, +47] | 99.9% | +0.19 (z +3.16) helps * | +0.12% ±0.16% | +0.13% ±0.29% |
| 14 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good enough to pay for its walk. | **on** | +28 (n=23,622) | +41 (n=17,574) | – | 17.00% (n=41,196) | 16.33% (n=41,196) | 0.67% | +33 [+10, +57] | 99.8% | +0.14 (z +2.35) helps * | -0.01% ±0.15% | +0.02% ±0.28% |
| 15 | `culture-building-debt` | Make the Theater Square owe its buildings. | **on** | +24 (n=23,622) | – | – | 16.91% (n=23,622) | 16.43% (n=23,622) | 0.48% | +24 [-7, +55] | 93.6% | +0.05 (z +0.83) ~ | +0.22% ±0.16% | +0.41% ±0.31% |
| 16 | `theology-for-founders` | A founder researches Theology next. | off | +22 (n=23,622) | -16 (n=17,574) | -5 (n=6,000) | 16.71% (n=47,196) | 16.62% (n=47,196) | 0.09% | +3 [-22, +28] | 58.8% | -0.00 (z -0.03) ~ | -0.05% ±0.16% | -0.12% ±0.30% |
| 17 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | **on** | +22 (n=23,622) | +7 (n=17,574) | -7 (n=15,000) | 16.77% (n=69,642) | 16.56% (n=69,642) | 0.21% | +11 [-8, +29] | 87.3% | -0.03 (z -0.52) ~ | -0.04% ±0.16% | -0.02% ±0.29% |
| 18 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | **on** | +21 (n=23,622) | +1 (n=17,574) | +39 (n=15,000) | 16.82% (n=69,642) | 16.51% (n=69,642) | 0.31% | +16 [-3, +34] | 95.4% | +0.01 (z +0.15) ~ | +0.02% ±0.16% | -0.04% ±0.29% |
| 19 | `holy-lane-parity` | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | **on** | +19 (n=23,622) | +99 (n=7,200) | +63 (n=17,574) | 17.06% (n=54,396) | 16.28% (n=54,396) | 0.78% | +38 [-9, +85] | 94.3% | -0.01 (z -0.21) ~ | -0.14% ±0.16% | -0.13% ±0.30% |
| 20 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | **on** | +18 (n=23,622) | +30 (n=17,574) | -4 (n=15,000) | 16.73% (n=69,642) | 16.60% (n=69,642) | 0.13% | +5 [-20, +31] | 65.6% | +0.01 (z +0.18) ~ | +0.42% ±0.16% | +0.69% ±0.29% |
| 21 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | **on** | +18 (n=23,622) | +30 (n=17,574) | -2 (n=15,000) | 16.82% (n=69,642) | 16.51% (n=69,642) | 0.31% | +16 [-2, +34] | 95.6% | +0.04 (z +0.77) ~ | -0.20% ±0.16% | -0.27% ±0.30% |
| 22 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | **on** | +18 (n=23,622) | +24 (n=17,574) | -3 (n=15,000) | 16.85% (n=69,642) | 16.48% (n=69,642) | 0.36% | +18 [-0, +36] | 97.5% | +0.13 (z +2.28) helps * | +0.19% ±0.15% | +0.36% ±0.29% |
| 23 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | **on** | +18 (n=23,622) | +50 (n=17,574) | – | 16.98% (n=41,196) | 16.35% (n=41,196) | 0.64% | +33 [+2, +64] | 98.0% | +0.10 (z +1.74) ~ | +0.38% ±0.15% | +0.27% ±0.28% |
| 24 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | **on** | +17 (n=23,622) | +12 (n=17,574) | +18 (n=15,000) | 16.77% (n=69,642) | 16.56% (n=69,642) | 0.21% | +11 [-8, +29] | 87.3% | +0.06 (z +1.03) ~ | +0.14% ±0.16% | +0.22% ±0.30% |
| 25 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | **on** | +17 (n=23,622) | +14 (n=17,574) | – | 16.82% (n=41,196) | 16.51% (n=41,196) | 0.31% | +16 [-8, +39] | 90.3% | -0.01 (z -0.20) ~ | +0.05% ±0.16% | +0.16% ±0.30% |
| 26 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | **on** | +16 (n=23,622) | +14 (n=17,574) | +12 (n=15,000) | 16.70% (n=69,642) | 16.63% (n=69,642) | 0.07% | +2 [-23, +27] | 56.6% | -0.05 (z -0.86) ~ | +0.02% ±0.16% | -0.02% ±0.30% |
| 27 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | **on** | +12 (n=23,622) | +23 (n=17,574) | +61 (n=15,000) | 17.00% (n=69,642) | 16.33% (n=69,642) | 0.67% | +35 [+11, +59] | 99.8% | +0.02 (z +0.39) ~ | -0.14% ±0.15% | -0.24% ±0.29% |
| 28 | `religious-units-heal-first` | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. | off | +12 (n=23,622) | – | – | 16.79% (n=23,622) | 16.55% (n=23,622) | 0.24% | +12 [-18, +42] | 78.0% | +0.10 (z +1.73) ~ | +0.27% ±0.16% | +0.53% ±0.29% |
| 29 | `lane-space-race` | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be sited in all read the race rather than an explicitly assigned target, and the pass opens at all. | off | +11 (n=23,622) | – | – | 16.77% (n=23,622) | 16.56% (n=23,622) | 0.21% | +11 [-20, +42] | 74.8% | -0.01 (z -0.23) ~ | +0.15% ±0.16% | +0.16% ±0.29% |
| 30 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | off | +11 (n=23,622) | -5 (n=17,574) | +21 (n=15,000) | 16.78% (n=69,642) | 16.56% (n=69,642) | 0.22% | +11 [-7, +29] | 88.3% | +0.07 (z +1.14) ~ | +0.04% ±0.16% | -0.07% ±0.29% |
| 31 | `barbarian-bargain` | Price a raider's life below a major's. | **on** | +10 (n=23,622) | +5 (n=17,574) | – | 16.74% (n=41,196) | 16.59% (n=41,196) | 0.16% | +8 [-16, +31] | 74.4% | +0.03 (z +0.45) ~ | -0.18% ±0.16% | -0.26% ±0.29% |
| 32 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends its turn beside a free 50-gold clear until the camp spawns the archer that kills it. | off | +10 (n=23,622) | -86 (n=17,574) | – | 16.36% (n=41,196) | 16.98% (n=41,196) | -0.62% | -38 [-132, +57] | 21.8% | -0.13 (z -2.29) hurts * | -0.44% ±0.16% | -0.38% ±0.29% |
| 33 | `enhancer-for-the-corps` | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. | off | +8 (n=23,622) | – | – | 16.74% (n=23,622) | 16.59% (n=23,622) | 0.15% | +8 [-23, +39] | 68.5% | -0.03 (z -0.51) ~ | -0.43% ±0.16% | -0.81% ±0.29% |
| 34 | `lane-congress-ballot` | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion posture that has no lane. | off | +8 (n=23,622) | – | – | 16.75% (n=23,622) | 16.59% (n=23,622) | 0.16% | +8 [-23, +39] | 69.6% | -0.01 (z -0.21) ~ | -0.08% ±0.16% | -0.19% ±0.30% |
| 35 | `lane-culture-spending` | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture whose plan has not named the lane. | off | +8 (n=23,622) | – | – | 16.75% (n=23,622) | 16.59% (n=23,622) | 0.16% | +8 [-23, +39] | 69.6% | -0.07 (z -1.26) ~ | +0.01% ±0.16% | +0.10% ±0.29% |
| 36 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | off | +8 (n=23,622) | -3 (n=17,574) | +21 (n=15,000) | 16.83% (n=69,642) | 16.51% (n=69,642) | 0.32% | +17 [-5, +38] | 93.8% | -0.02 (z -0.35) ~ | +0.13% ±0.16% | +0.13% ±0.29% |
| 37 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | **on** | +7 (n=23,622) | +14 (n=17,574) | +48 (n=6,000) | 16.82% (n=47,196) | 16.52% (n=47,196) | 0.30% | +19 [-3, +42] | 95.2% | -0.09 (z -1.57) ~ | -0.14% ±0.16% | -0.25% ±0.30% |
| 38 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | **on** | +6 (n=23,622) | +14 (n=17,574) | +6 (n=15,000) | 16.71% (n=69,642) | 16.62% (n=69,642) | 0.09% | +5 [-13, +23] | 70.4% | +0.03 (z +0.49) ~ | -0.24% ±0.15% | -0.29% ±0.28% |
| 39 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | off | +6 (n=23,622) | +1 (n=17,574) | +3 (n=15,000) | 16.62% (n=69,642) | 16.72% (n=69,642) | -0.10% | -5 [-24, +14] | 30.7% | -0.00 (z -0.08) ~ | -0.07% ±0.16% | -0.11% ±0.30% |
| 40 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | **on** | +6 (n=23,622) | +18 (n=17,574) | +23 (n=15,000) | 16.86% (n=69,642) | 16.48% (n=69,642) | 0.38% | +19 [+1, +37] | 97.8% | -0.10 (z -1.72) ~ | -0.21% ±0.16% | -0.48% ±0.30% |
| 41 | `come-ashore` | Keep the land army out of the water. | **on** | +5 (n=23,622) | +11 (n=17,574) | +36 (n=15,000) | 16.76% (n=69,642) | 16.58% (n=69,642) | 0.18% | +9 [-10, +29] | 82.3% | +0.01 (z +0.25) ~ | -0.57% ±0.16% | -0.44% ±0.29% |
| 42 | `campus-finishes-first` | The Campus coverage term is scaled by how finished the empire's standing Campuses are. | off | +4 (n=23,622) | – | – | 16.70% (n=23,622) | 16.63% (n=23,622) | 0.08% | +4 [-27, +34] | 59.7% | +0.10 (z +1.64) ~ | -0.08% ±0.15% | -0.19% ±0.28% |
| 43 | `strike-opening` | Let movement credit the attack a tile opens. | **on** | +4 (n=23,622) | +17 (n=17,574) | +21 (n=15,000) | 16.81% (n=69,642) | 16.53% (n=69,642) | 0.28% | +14 [-4, +32] | 93.5% | +0.07 (z +1.13) ~ | +0.08% ±0.16% | +0.15% ±0.29% |
| 44 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | **on** | +3 (n=23,622) | +2 (n=17,574) | +35 (n=6,000) | 16.73% (n=47,196) | 16.60% (n=47,196) | 0.13% | +9 [-11, +30] | 81.2% | -0.04 (z -0.70) ~ | +0.23% ±0.16% | +0.59% ±0.29% |
| 45 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | off | +3 (n=17,574) | -4 (n=15,000) | -18 (n=13,446) | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 46 | `early-contact-window` | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. | off | +2 (n=23,622) | – | – | 16.68% (n=23,622) | 16.65% (n=23,622) | 0.03% | +2 [-29, +32] | 54.3% | +0.02 (z +0.38) ~ | -0.09% ±0.16% | -0.30% ±0.29% |
| 47 | `barbarian-capture-priority` | Take a visible Barbarian Settler or Scout in exact one-turn reach before healing, retreat, or any ordinary tactical choice. | off | +1 (n=23,622) | – | – | 16.68% (n=23,622) | 16.65% (n=23,622) | 0.03% | +1 [-30, +32] | 53.2% | -0.12 (z -2.10) hurts * | -0.04% ±0.16% | +0.03% ±0.29% |
| 48 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | off | +0 (n=23,622) | +30 (n=17,574) | +17 (n=15,000) | 16.85% (n=69,642) | 16.49% (n=69,642) | 0.36% | +18 [-0, +36] | 97.3% | +0.00 (z +0.05) ~ | +0.14% ±0.15% | +0.23% ±0.28% |
| 49 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | off | +0 (n=23,622) | -7 (n=17,574) | +4 (n=15,000) | 16.67% (n=69,642) | 16.67% (n=69,642) | 0.00% | +0 [-18, +18] | 50.0% | -0.06 (z -0.98) ~ | +0.02% ±0.16% | +0.02% ±0.29% |
| 50 | `competition-victory-points` | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. | off | +0 (n=23,622) | – | – | 16.66% (n=23,622) | 16.67% (n=23,622) | -0.01% | -0 [-31, +31] | 48.9% | -0.07 (z -1.17) ~ | +0.13% ±0.16% | +0.25% ±0.30% |
| 51 | `lane-policy-deck` | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. | off | +0 (n=23,622) | – | – | 16.67% (n=23,622) | 16.67% (n=23,622) | 0.00% | +0 [-31, +31] | 50.0% | -0.04 (z -0.74) ~ | -0.13% ±0.15% | -0.15% ±0.29% |
| 52 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | off | -1 (n=23,622) | -6 (n=17,574) | -4 (n=15,000) | 16.63% (n=69,642) | 16.71% (n=69,642) | -0.08% | -4 [-22, +14] | 33.2% | +0.10 (z +1.65) ~ | -0.20% ±0.15% | -0.40% ±0.28% |
| 53 | `district-building-chain` | Make every specialty district owe its own buildings, whatever the lane. | off | -1 (n=23,622) | – | – | 16.66% (n=23,622) | 16.68% (n=23,622) | -0.02% | -1 [-32, +30] | 47.9% | -0.18 (z -3.15) hurts * | -0.24% ±0.15% | -0.30% ±0.28% |
| 54 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | off | -1 (n=23,622) | -12 (n=17,574) | +20 (n=15,000) | 16.73% (n=69,642) | 16.61% (n=69,642) | 0.12% | +6 [-12, +24] | 73.6% | -0.01 (z -0.15) ~ | -0.08% ±0.16% | -0.17% ±0.30% |
| 55 | `lane-great-people` | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. | off | -3 (n=23,622) | – | – | 16.64% (n=23,622) | 16.70% (n=23,622) | -0.06% | -3 [-33, +27] | 42.4% | -0.04 (z -0.66) ~ | +0.03% ±0.16% | -0.03% ±0.29% |
| 56 | `envoy-infrastructure` | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the turn limit, and a first Diplomatic Quarter sees part of the Consulate stream it unlocks. | off | -6 (n=23,622) | – | – | 16.61% (n=23,622) | 16.73% (n=23,622) | -0.12% | -6 [-37, +25] | 35.3% | +0.00 (z +0.05) ~ | -0.18% ±0.16% | -0.48% ±0.29% |
| 57 | `research-tier-premium` | A Campus building's debt is scaled by its own Science against the chain's first rung. | off | -6 (n=23,622) | – | – | 16.61% (n=23,622) | 16.73% (n=23,622) | -0.12% | -6 [-37, +25] | 35.2% | -0.10 (z -1.80) ~ | +0.07% ±0.15% | +0.10% ±0.29% |
| 58 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | off | -6 (n=23,622) | -16 (n=17,574) | +14 (n=15,000) | 16.51% (n=69,642) | 16.82% (n=69,642) | -0.31% | -17 [-46, +12] | 13.0% | +0.19 (z +3.26) helps * | +0.35% ±0.16% | +0.46% ±0.29% |
| 59 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | off | -8 (n=23,622) | +24 (n=17,574) | – | 16.72% (n=41,196) | 16.61% (n=41,196) | 0.11% | +7 [-25, +39] | 66.1% | -0.04 (z -0.75) ~ | +0.24% ±0.16% | +0.26% ±0.29% |
| 60 | `condemn-under-congress` | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. | off | -8 (n=23,622) | – | – | 16.59% (n=23,622) | 16.74% (n=23,622) | -0.15% | -8 [-38, +23] | 31.2% | +0.04 (z +0.64) ~ | +0.11% ±0.16% | +0.22% ±0.29% |
| 61 | `congress-banks-decided` | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact prediction and staking nothing. | off | -8 (n=23,622) | – | – | 16.58% (n=23,622) | 16.75% (n=23,622) | -0.17% | -8 [-39, +22] | 29.2% | -0.04 (z -0.73) ~ | -0.17% ±0.16% | -0.34% ±0.29% |
| 62 | `one-shot-recovery` | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. | off | -8 (n=23,622) | – | – | 16.58% (n=23,622) | 16.75% (n=23,622) | -0.17% | -8 [-39, +22] | 29.3% | +0.02 (z +0.29) ~ | +0.23% ±0.16% | +0.23% ±0.29% |
| 63 | `power-the-laboratory` | A power plant is credited the yields it switches on in its city. | off | -8 (n=23,622) | – | – | 16.58% (n=23,622) | 16.75% (n=23,622) | -0.17% | -8 [-39, +22] | 29.1% | -0.04 (z -0.76) ~ | -0.22% ±0.16% | -0.43% ±0.29% |
| 64 | `religious-defence-scales` | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. | off | -8 (n=23,622) | – | – | 16.58% (n=23,622) | 16.75% (n=23,622) | -0.17% | -8 [-39, +22] | 29.6% | -0.06 (z -1.00) ~ | +0.12% ±0.16% | +0.20% ±0.30% |
| 65 | `science-multiplier-payoff` | Credit a Campus building the beakers its city's multipliers will actually pay it. | off | -8 (n=23,622) | – | – | 16.59% (n=23,622) | 16.74% (n=23,622) | -0.15% | -8 [-38, +23] | 31.1% | -0.01 (z -0.13) ~ | -0.10% ±0.15% | -0.27% ±0.28% |
| 66 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | off | -8 (n=23,622) | -3 (n=17,574) | +13 (n=15,000) | 16.65% (n=69,642) | 16.68% (n=69,642) | -0.03% | -2 [-19, +16] | 43.1% | -0.05 (z -0.78) ~ | -0.06% ±0.15% | -0.08% ±0.28% |
| 67 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | off | -8 (n=23,622) | +13 (n=17,574) | +21 (n=15,000) | 16.73% (n=69,642) | 16.61% (n=69,642) | 0.12% | +6 [-12, +24] | 73.6% | -0.03 (z -0.55) ~ | +0.01% ±0.16% | +0.01% ±0.29% |
| 68 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | off | -10 (n=23,622) | +13 (n=17,574) | – | 16.66% (n=41,196) | 16.67% (n=41,196) | -0.00% | -0 [-24, +23] | 48.8% | +0.00 (z +0.07) ~ | -0.11% ±0.16% | -0.21% ±0.29% |
| 69 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | off | -11 (n=23,622) | +15 (n=17,574) | +24 (n=15,000) | 16.77% (n=69,642) | 16.57% (n=69,642) | 0.20% | +10 [-8, +28] | 85.7% | +0.01 (z +0.22) ~ | +0.04% ±0.15% | +0.08% ±0.28% |
| 70 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | off | -11 (n=23,622) | -31 (n=17,574) | – | 16.47% (n=41,196) | 16.86% (n=41,196) | -0.39% | -20 [-43, +3] | 4.7% | +0.02 (z +0.34) ~ | -0.86% ±0.15% | -0.71% ±0.28% |
| 71 | `science-payback-horizon` | Price the science economy on whether it can still repay rather than on how much of the game is left. | off | -11 (n=23,622) | – | – | 16.56% (n=23,622) | 16.77% (n=23,622) | -0.21% | -11 [-41, +20] | 24.7% | -0.16 (z -2.79) hurts * | -0.33% ±0.16% | -0.38% ±0.31% |
| 72 | `fifteenth-citizen` | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. | off | -12 (n=23,622) | – | – | 16.55% (n=23,622) | 16.79% (n=23,622) | -0.24% | -12 [-42, +19] | 22.4% | -0.07 (z -1.14) ~ | -0.26% ±0.16% | -0.51% ±0.30% |
| 73 | `congress-counter-votes` | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fails costs no Favor. | off | -13 (n=23,622) | – | – | 16.54% (n=23,622) | 16.79% (n=23,622) | -0.25% | -13 [-43, +17] | 20.4% | +0.06 (z +1.11) ~ | -0.15% ±0.16% | -0.15% ±0.29% |
| 74 | `culture-coverage` | Pay for the Theater Square the empire has not got. | off | -14 (n=23,622) | – | – | 16.53% (n=23,622) | 16.81% (n=23,622) | -0.28% | -14 [-44, +16] | 18.2% | +0.08 (z +1.44) ~ | +0.07% ±0.16% | +0.14% ±0.30% |
| 75 | `lane-congress-favor` | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. | off | -14 (n=23,622) | – | – | 16.53% (n=23,622) | 16.81% (n=23,622) | -0.28% | -14 [-44, +17] | 18.4% | -0.09 (z -1.61) ~ | -0.07% ±0.16% | -0.15% ±0.30% |
| 76 | `camp-party` | The peacetime camp party. | off | -15 (n=23,622) | +13 (n=17,574) | +53 (n=15,000) | 16.84% (n=69,642) | 16.49% (n=69,642) | 0.35% | +22 [-10, +53] | 90.8% | -0.01 (z -0.21) ~ | -0.01% ±0.16% | -0.13% ±0.30% |
| 77 | `spread-campaign-persists` | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is spent. | off | -16 (n=23,622) | – | – | 16.51% (n=23,622) | 16.82% (n=23,622) | -0.31% | -16 [-47, +15] | 16.0% | -0.04 (z -0.75) ~ | +0.08% ±0.16% | +0.04% ±0.30% |
| 78 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | off | -17 (n=23,622) | -5 (n=17,574) | -11 (n=15,000) | 16.58% (n=69,642) | 16.75% (n=69,642) | -0.17% | -9 [-27, +9] | 17.4% | -0.06 (z -0.98) ~ | +0.15% ±0.16% | +0.31% ±0.30% |
| 79 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | off | -17 (n=23,622) | +13 (n=17,574) | +39 (n=15,000) | 16.68% (n=69,642) | 16.66% (n=69,642) | 0.02% | +2 [-26, +30] | 55.6% | -0.10 (z -1.63) ~ | -0.13% ±0.16% | -0.08% ±0.29% |
| 80 | `campus-adjacency-threshold` | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. | off | -18 (n=23,622) | – | – | 16.49% (n=23,622) | 16.84% (n=23,622) | -0.36% | -18 [-48, +13] | 12.6% | -0.06 (z -1.02) ~ | +0.01% ±0.16% | -0.09% ±0.29% |
| 81 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | off | -18 (n=23,622) | -15 (n=17,574) | +4 (n=15,000) | 16.52% (n=69,642) | 16.81% (n=69,642) | -0.30% | -15 [-33, +3] | 5.6% | +0.05 (z +0.93) ~ | -0.33% ±0.15% | -0.21% ±0.29% |
| 82 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | off | -18 (n=23,622) | +6 (n=17,574) | +29 (n=15,000) | 16.73% (n=69,642) | 16.60% (n=69,642) | 0.13% | +8 [-14, +30] | 75.9% | +0.03 (z +0.58) ~ | +0.10% ±0.17% | +0.09% ±0.30% |
| 83 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | off | -18 (n=23,622) | +15 (n=17,574) | +1 (n=15,000) | 16.63% (n=69,642) | 16.71% (n=69,642) | -0.08% | -4 [-21, +14] | 34.6% | -0.07 (z -1.21) ~ | +0.01% ±0.15% | +0.13% ±0.29% |
| 84 | `holy-site-where-the-threat-is` | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. | off | -19 (n=23,622) | – | – | 16.48% (n=23,622) | 16.85% (n=23,622) | -0.37% | -19 [-49, +12] | 11.4% | -0.29 (z -4.96) hurts * | +0.11% ±0.15% | +0.44% ±0.29% |
| 85 | `research-floor-holds` | The citizen tilt and the beaker floor hold while the research can still pay. | off | -19 (n=23,622) | – | – | 16.48% (n=23,622) | 16.85% (n=23,622) | -0.37% | -19 [-49, +12] | 11.8% | -0.26 (z -4.42) hurts * | +0.07% ±0.16% | +0.18% ±0.30% |
| 86 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | off | -20 (n=23,622) | +16 (n=17,574) | -11 (n=15,000) | 16.59% (n=69,642) | 16.74% (n=69,642) | -0.16% | -8 [-26, +10] | 19.9% | +0.03 (z +0.51) ~ | +0.72% ±0.16% | +0.70% ±0.30% |
| 87 | `research-grants-first` | A finished research city pays more for its own district's project. | off | -20 (n=23,622) | – | – | 16.46% (n=23,622) | 16.87% (n=23,622) | -0.41% | -20 [-51, +10] | 9.4% | -0.04 (z -0.61) ~ | +0.05% ±0.16% | +0.17% ±0.29% |
| 88 | `district-coverage` | Rank district families by how much of the empire still lacks them. | off | -22 (n=23,622) | +32 (n=17,574) | -9 (n=15,000) | 16.63% (n=69,642) | 16.70% (n=69,642) | -0.07% | -3 [-27, +21] | 40.6% | -0.02 (z -0.36) ~ | +0.28% ±0.16% | +0.38% ±0.29% |
| 89 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | off | -23 (n=23,622) | +24 (n=17,574) | +23 (n=15,000) | 16.66% (n=69,642) | 16.67% (n=69,642) | -0.01% | +0 [-26, +27] | 51.0% | -0.03 (z -0.45) ~ | -0.01% ±0.15% | -0.01% ±0.28% |
| 90 | `chain-tech-lookahead` | The research goal aims at a Campus rung the empire can BUILD, not only one it has already built. | off | -25 (n=23,622) | – | – | 16.42% (n=23,622) | 16.91% (n=23,622) | -0.49% | -25 [-56, +6] | 6.0% | -0.03 (z -0.59) ~ | +0.40% ±0.16% | +0.90% ±0.29% |
| 91 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | off | -27 (n=23,622) | -5 (n=17,574) | +17 (n=15,000) | 16.73% (n=69,642) | 16.60% (n=69,642) | 0.13% | +12 [-28, +51] | 72.3% | -0.05 (z -0.79) ~ | +0.29% ±0.15% | +0.53% ±0.29% |
| 92 | `governor-expansion-lane` | The other half: the governor under Expansion only. | off | -28 (n=23,622) | -30 (n=17,574) | – | 16.38% (n=41,196) | 16.95% (n=41,196) | -0.57% | -29 [-52, -5] | 0.8% | -0.23 (z -4.03) hurts * | -0.01% ±0.16% | -0.16% ±0.29% |
| 93 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | off | -28 (n=23,622) | -19 (n=17,574) | +20 (n=15,000) | 16.57% (n=69,642) | 16.76% (n=69,642) | -0.19% | -9 [-30, +12] | 20.6% | -0.21 (z -3.46) hurts * | -0.17% ±0.16% | -0.28% ±0.30% |
| 94 | `guru-heals-the-corps` | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. | off | -29 (n=23,622) | – | – | 16.37% (n=23,622) | 16.96% (n=23,622) | -0.58% | -29 [-61, +2] | 3.4% | -0.03 (z -0.45) ~ | +0.16% ±0.16% | +0.21% ±0.30% |
| 95 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use its direct empire-wide repair. | off | -34 (n=23,622) | -14 (n=17,574) | -4 (n=15,000) | 16.57% (n=69,642) | 16.76% (n=69,642) | -0.19% | -7 [-34, +20] | 29.8% | -0.11 (z -2.00) hurts * | +0.16% ±0.16% | +0.13% ±0.29% |
| 96 | `housing-districts` | Let the baseline governor raise the housing ceiling. | off | -37 (n=23,622) | +5 (n=17,574) | -9 (n=15,000) | 16.50% (n=69,642) | 16.84% (n=69,642) | -0.34% | -17 [-36, +1] | 3.5% | -0.15 (z -2.47) hurts * | -0.05% ±0.16% | -0.19% ±0.30% |
| 97 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | off | -41 (n=23,622) | -22 (n=17,574) | – | 16.34% (n=41,196) | 16.99% (n=41,196) | -0.66% | -33 [-56, -10] | 0.3% | -0.12 (z -1.99) ~ | +0.42% ±0.16% | +0.49% ±0.29% |
| 98 | `contact-posture` | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answer, or step out of that shooter's envelope. | off | -55 (n=23,622) | – | – | 16.12% (n=23,622) | 17.21% (n=23,622) | -1.09% | -55 [-85, -24] | 0.0% | -0.36 (z -6.15) hurts * | -0.13% ±0.15% | -0.02% ±0.28% |
| 99 | `governor-every-lane` | Run the strategic governor under every lane. | off | -234 (n=23,622) | +13 (n=17,574) | -8 (n=15,000) | 15.78% (n=69,642) | 17.55% (n=69,642) | -1.77% | -72 [-196, +53] | 13.0% | -2.00 (z -35.46) hurts * | -0.19% ±0.16% | +0.20% ±0.29% |
| 100 | `governor-victory-lanes` | Half the composite: the governor under the four victory lanes only. | off | -239 (n=3,600) | -237 (n=23,622) | +46 (n=17,574) | 15.41% (n=44,796) | 17.93% (n=44,796) | -2.52% | -142 [-350, +65] | 9.0% | -2.73 (z -23.76) hurts * | -0.05% ±0.36% | +0.80% ±0.67% |

## What the posterior would change

A threshold in column units is not a threshold in evidence. The screens these columns come from resolve between ±29 and ±101 at 80% power — more than three to one — so **+24 decides differently depending only on which screen priced the gene**, and #2294's single-column +20 bar sits below every band the instrument has ever printed. *Posterior (95% CI)* above is the answer to that: a random-effects (DerSimonian–Laird) inverse-variance pool of every screen's on−off difference on this column's own scale, each screen weighted by its own standard error, with the disagreement **between** screens carried in the interval rather than assumed away. `P(>0)` is where the shrinkage shows: two genes can print the same +30 and land at 90% and 99.8%.

**It is published, not in force.** `AUTHORITY` in `tools/genes.py` is the whole switch, it says `columns`, and this table is what the other settings would ship. Two reasons it is not flipped here, neither of them arithmetic: the threshold rule is an explicit operator directive, and **every source in this ledger is the retired `legacy` 60×38 Pangaea shape** — re-deciding the deployment genome now would re-decide it on the wrong instrument.

| Authority | Genes on | Genes moved | What it is |
|---|---:|---:|---|
| `columns` **(in force)** | 33 | 0 | the operator's threshold rule, exactly as it ships: both win columns positive, or their average above +15 with neither below −10, or one column above +20 — and off whatever they say when the pooled *Diff* is negative |
| `posterior-veto` | 34 | 1 | the same columns, with an error bar on the veto: it fires only when the posterior's 95% interval lies **wholly below zero**, instead of on the bare sign of a difference that carries no error at all |
| `posterior` | 34 | 1 | the pooled estimate decides wherever its interval excludes zero, and `posterior-veto` decides where it straddles |

### The genes that move

Each row is a gene whose shipped default one of the settings above would change. `on`/`off` in bold is a move.

⚠ Read what these rows do and do not say. Every one is a **re-admission**, and not one of them has a positive point estimate: the posterior is not saying these genes help, it is saying the veto that removed them **could not tell**. The shipped rule fires on the sign of a pooled difference that carries no error at all — −0.78, −0.21 and −0.06 pp — and every one of those three intervals straddles zero. Where the interval straddles, the `posterior` setting inherits the columns' answer, because `default_on` has to be a pure function of the sources and the only other candidate is whatever shipped yesterday. That deferral is the reason *Where a direct arm pays* exists.

| Gene | Shipped | `posterior-veto` | `posterior` | Posterior (95% CI) | P(>0) | Pooled *Diff* |
|---|---|---|---|---:|---:|---:|
| `siege-commitment` | off | **on** | **on** | -5 [-24, +14] | 30.7% | -0.10% |

### What the posterior can decide at all

Of 100 priced genes the interval clears zero for **13 upward** and **3 downward**; **84 sit inside the interval either way** and are the boundary set below. A straddling interval is not a null — it is the instrument saying it cannot tell, which is exactly what a fixed ±15 bar cannot say.

| Gene | Posterior (95% CI) | P(>0) | Screens | Shipped | Posterior call |
|---|---:|---:|---:|---|---|
| `air-surge` | +108 [+77, +138] | 100.0% | 1 | on | **on** |
| `barbarian-scouts-are-scouts` | +35 [+11, +59] | 99.8% | 4 | on | **on** |
| `bounded-recovery` | +28 [+10, +47] | 99.9% | 4 | on | **on** |
| `great-person-housing` | +87 [+63, +111] | 100.0% | 2 | on | **on** |
| `idle-faith-patronage` | +29 [+14, +45] | 100.0% | 3 | on | **on** |
| `loyalty-rate-alarm` | +45 [+27, +63] | 100.0% | 4 | on | **on** |
| `opportunistic-war` | +38 [+12, +63] | 99.8% | 2 | on | **on** |
| `raid-pillage-prizes` | +43 [+20, +66] | 100.0% | 2 | on | **on** |
| `recon-replacement` | +46 [+23, +69] | 100.0% | 4 | on | **on** |
| `settle-sooner` | +33 [+10, +57] | 99.8% | 2 | on | **on** |
| `settler-threat-detour` | +33 [+2, +64] | 98.0% | 2 | on | **on** |
| `whole-turn-backtrack-guard` | +19 [+1, +37] | 97.8% | 4 | on | **on** |
| `wide-map-capacity` | +49 [+18, +81] | 99.9% | 4 | on | **on** |
| `contact-posture` | -55 [-85, -24] | 0.0% | 1 | off | **off** |
| `district-lookahead-settle` | -33 [-56, -10] | 0.3% | 2 | off | **off** |
| `governor-expansion-lane` | -29 [-52, -5] | 0.8% | 2 | off | **off** |

## The two shapes, apart

`τ` (tau) is the between-screen standard deviation the random-effects pool estimates. It is the statistic that answers *“is 'both columns positive' two confirmations?”*: when screens agree to within their errors it is zero and the pool is the ordinary inverse-variance one; when they do not, it widens the interval instead of averaging two worlds into a confident wrong answer. `POSTERIOR_SHAPES` in `tools/genes.py` says which shapes the published pool admits and is currently `standard, legacy`.

| Shape | Sources | Seat pairs | Genes priced |
|---|---:|---:|---:|
| standard | 2 | 27,222 | 99 |
| legacy | 7 | 66,220 | 65 |

Genes priced at both shapes. **A row whose two intervals do not overlap is not a gene with one number; it is two instruments disagreeing**, and the pooled column beside it should be read as a warning rather than an answer.

| Gene | legacy | standard | pooled | τ | overlap |
|---|---:|---:|---:|---:|---|
| `amenity-district-path` | +7 [-15, +29] | +17 [-14, +48] | +11 [-8, +29] | 0 | yes |
| `amenity-project-preemption` | +3 [-24, +31] | -34 [-65, -3] | -7 [-34, +20] | 20 | yes |
| `apostle-promotion-by-role` | -4 [-38, +30] | +16 [-15, +47] | +2 [-23, +27] | 17 | yes |
| `army-target-weighs-enemy` | -1 [-37, +35] | +18 [-13, +49] | +5 [-20, +31] | 18 | yes |
| `barbarian-bargain` | +5 [-32, +41] | +10 [-20, +41] | +8 [-16, +31] | 0 | yes |
| `barbarian-hunt` | -86 [-123, -50] | +10 [-21, +41] | -38 [-132, +57] | 66 | **no** |
| `barbarian-ranged-answer` | +14 [-22, +50] | +17 [-14, +47] | +16 [-8, +39] | 0 | yes |
| `barbarian-scouts-are-scouts` | +45 [+21, +68] | +12 [-18, +43] | +35 [+11, +59] | 15 | yes |
| `blind-objective-strength` | +27 [+5, +50] | -0 [-31, +30] | +18 [-0, +36] | 0 | yes |
| `blind-objective-units` | +0 [-22, +22] | -0 [-30, +29] | +0 [-18, +18] | 0 | yes |
| `bounded-recovery` | +28 [+6, +51] | +29 [-2, +59] | +28 [+10, +47] | 0 | yes |
| `builder-barbarian-safety` | +13 [-23, +48] | -10 [-40, +21] | -0 [-24, +23] | 0 | yes |
| `builder-worked-tile-priority` | +24 [-11, +60] | -8 [-39, +22] | +7 [-25, +39] | 16 | yes |
| `buildings-before-projects` | +14 [-8, +36] | +61 [+31, +92] | +28 [-0, +57] | 22 | yes |
| `camp-party` | +35 [+10, +60] | -15 [-46, +16] | +22 [-10, +53] | 26 | yes |
| `civilian-rescue` | -5 [-28, +17] | -1 [-32, +30] | -4 [-22, +14] | 0 | yes |
| `come-ashore` | +11 [-18, +40] | +5 [-26, +36] | +9 [-10, +29] | 6 | yes |
| `district-coverage` | +5 [-23, +34] | -22 [-52, +8] | -3 [-27, +21] | 17 | yes |
| `district-lookahead-settle` | -22 [-57, +14] | -41 [-71, -11] | -33 [-56, -10] | 0 | yes |
| `endgame-war-runway` | -4 [-27, +18] | -17 [-47, +14] | -9 [-27, +9] | 0 | yes |
| `escort-unstick` | +27 [-20, +74] | +36 [+5, +67] | +30 [-1, +62] | 26 | yes |
| `founder-temple` | +29 [-4, +62] | +7 [-24, +38] | +19 [-3, +42] | 7 | yes |
| `garrison-under-fire` | +26 [-16, +68] | -27 [-57, +3] | +12 [-28, +51] | 36 | yes |
| `governor-every-lane` | -15 [-54, +23] | -234 [-264, -204] | -72 [-196, +53] | 126 | **no** |
| `governor-expansion-lane` | -30 [-66, +6] | -28 [-58, +3] | -29 [-52, -5] | 0 | yes |
| `governor-victory-lanes` | +46 [+9, +82] | -237 [-265, -209] | -142 [-350, +65] | 182 | **no** |
| `great-person-housing` | +78 [+42, +114] | +94 [+63, +124] | +87 [+63, +111] | 0 | yes |
| `holy-lane-parity` | +45 [-24, +114] | +19 [-12, +51] | +38 [-9, +85] | 44 | yes |
| `home-defense` | -13 [-35, +10] | -18 [-49, +13] | -15 [-33, +3] | 0 | yes |
| `housing-districts` | -7 [-29, +16] | -37 [-68, -6] | -17 [-36, +1] | 4 | yes |
| `housing-research` | +10 [-26, +45] | -17 [-48, +13] | +2 [-26, +30] | 22 | yes |
| `idle-faith-patronage` | +26 [+9, +44] | +39 [+8, +69] | +29 [+14, +45] | 0 | yes |
| `inquisition-on-threat` | +16 [-16, +47] | +3 [-28, +34] | +9 [-11, +30] | 0 | yes |
| `loyalty-rate-alarm` | +49 [+25, +73] | +38 [+7, +69] | +45 [+27, +63] | 0 | yes |
| `naval-recon` | -1 [-23, +21] | -20 [-51, +10] | -8 [-26, +10] | 0 | yes |
| `one-launch-pad` | +20 [-2, +42] | -11 [-41, +20] | +10 [-8, +28] | 0 | yes |
| `opportunistic-war` | +23 [-14, +59] | +49 [+18, +80] | +38 [+12, +63] | 7 | yes |
| `peacetime-deterrence` | +13 [-12, +38] | +21 [-10, +51] | +16 [-3, +34] | 0 | yes |
| `priced-tile-purchase` | -31 [-66, +5] | -11 [-42, +19] | -20 [-43, +3] | 0 | yes |
| `raid-pillage-prizes` | +30 [-6, +65] | +53 [+23, +83] | +43 [+20, +66] | 0 | yes |
| `recon-replacement` | +53 [+25, +82] | +30 [-1, +60] | +46 [+23, +69] | 15 | yes |
| `recorded-tactical-step` | +15 [-8, +37] | +18 [-13, +48] | +16 [-2, +34] | 0 | yes |
| `relief-targets-the-siege` | +4 [-18, +27] | +6 [-25, +36] | +5 [-13, +23] | 0 | yes |
| `religion-sues-peace` | +19 [-3, +41] | -18 [-48, +13] | +8 [-14, +30] | 13 | yes |
| `score-horizon` | +18 [-4, +41] | +18 [-13, +48] | +18 [-0, +36] | 0 | yes |
| `settle-sooner` | +41 [+5, +76] | +28 [-3, +59] | +33 [+10, +57] | 0 | yes |
| `settler-guard-holds` | +2 [-20, +24] | -8 [-39, +23] | -2 [-19, +16] | 0 | yes |
| `settler-site-agreement` | +10 [-18, +38] | -23 [-53, +8] | +0 [-26, +27] | 19 | yes |
| `settler-target-hysteresis` | +4 [-18, +26] | -18 [-49, +13] | -4 [-21, +14] | 0 | yes |
| `settler-threat-detour` | +50 [+14, +86] | +18 [-13, +49] | +33 [+2, +64] | 15 | yes |
| `siege-commitment` | -11 [-37, +15] | +6 [-25, +37] | -5 [-24, +14] | 7 | yes |
| `siege-is-progress` | -21 [-64, +21] | -6 [-37, +25] | -17 [-46, +12] | 23 | yes |
| `siege-tracks-wall` | +21 [-9, +52] | +8 [-23, +38] | +17 [-5, +38] | 11 | yes |
| `slot-kind-tiebreak` | +10 [-14, +33] | -1 [-31, +30] | +6 [-12, +24] | 0 | yes |
| `stranded-settler-discount` | +13 [-9, +36] | -8 [-39, +22] | +6 [-12, +24] | 0 | yes |
| `strategic-wonders` | +11 [-12, +33] | +11 [-19, +42] | +11 [-7, +29] | 0 | yes |
| `strike-opening` | +19 [-3, +41] | +4 [-26, +35] | +14 [-4, +32] | 0 | yes |
| `theology-for-founders` | -12 [-40, +16] | +22 [-8, +53] | +3 [-22, +28] | 11 | yes |
| `war-economy` | -48 [-185, +88] | +118 [+87, +148] | -7 [-130, +117] | 124 | yes |
| `war-patience` | -0 [-23, +23] | -28 [-58, +3] | -9 [-30, +12] | 11 | yes |
| `war-reinforcement` | +14 [-17, +46] | +34 [+3, +64] | +20 [-4, +44] | 16 | yes |
| `whole-turn-backtrack-guard` | +25 [+3, +47] | +6 [-25, +37] | +19 [+1, +37] | 0 | yes |
| `wide-map-capacity` | +33 [+11, +55] | +91 [+60, +121] | +49 [+18, +81] | 26 | **no** |
| `wonder-ring-settle-value` | +5 [-18, +27] | +22 [-9, +53] | +11 [-8, +29] | 0 | yes |

## Where a direct arm pays: the boundary genes

**The efficient plan is two stage, and it is not a partial foldover.** The whole-genome screen is the efficient way to RANK: `p10` priced 75 genes at ±51 each on 17,574 seat pairs, and the same budget split into 75 single-gene screens of 234 pairs would give ±145 each even at the best pairing gain this repository has measured — 2.84× wider, which is **8× the games** for the same band. A single-gene arm resolves far tighter per pair once it is aimed (`s7`: ±29 on 6,000 pairs at a 3.32× pairing gain, against `p10`'s 1.09×). So the screen ranks and direct arms resolve the boundary. `docs/GENE_SCREEN.md` carries the arithmetic; do not re-derive it into a blocked or partial foldover, which is neither stage — four-gene `s6` resolves ±64 over 6,000 pairs where one-gene `s7` resolves ±29 over the same 6,000.

*Buys* is the expected value of one direct arm of **7,200 seat pairs**, in wins per 10,000 on-arm seats, read against the gene's **shipped** state — so a gene the evidence likes and the genome already plays has little to buy, and a gene the evidence likes that the rule holds off has the whole effect to buy. *Pairs to resolve* is how many matched seat pairs that arm needs before the combined interval clears zero, if it reads the gene's current pooled effect. Both are sized from `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json`, the widest single-gene arm this repository has actually run (27.6 per-column SE at 7,200 pairs) — the conservative end, since a gene that rarely fires cancels far more and resolves tighter.

| Gene | Posterior (95% CI) | P(>0) | Shipped | Buys | Pairs to resolve |
|---|---:|---:|---|---:|---:|
| `war-economy` | -7 [-130, +117] | 45.8% | on | +26.5 | 480,419 |
| `camp-party` | +22 [-10, +53] | 90.8% | off | +21.5 | 24,670 |
| `blind-objective-strength` | +18 [-0, +36] | 97.3% | off | +17.7 | 2,262 |
| `siege-tracks-wall` | +17 [-5, +38] | 93.8% | off | +16.6 | 29,423 |
| `garrison-under-fire` | +12 [-28, +51] | 72.3% | off | +12.9 | 135,744 |
| `religious-units-heal-first` | +12 [-18, +42] | 78.0% | off | +12.0 | 127,130 |
| `strategic-wonders` | +11 [-7, +29] | 88.3% | off | +10.9 | 111,562 |
| `lane-space-race` | +11 [-20, +42] | 74.8% | off | +10.9 | 166,669 |
| `one-launch-pad` | +10 [-8, +28] | 85.7% | off | +9.8 | 155,622 |
| `lane-culture-spending` | +8 [-23, +39] | 69.6% | off | +8.6 | 304,266 |
| `lane-congress-ballot` | +8 [-23, +39] | 69.6% | off | +8.6 | 304,169 |
| `enhancer-for-the-corps` | +8 [-23, +39] | 68.5% | off | +8.3 | 341,897 |
| `religion-sues-peace` | +8 [-14, +30] | 75.9% | off | +8.0 | 294,570 |
| `builder-worked-tile-priority` | +7 [-25, +39] | 66.1% | off | +7.8 | 431,195 |
| `stranded-settler-discount` | +6 [-12, +24] | 73.6% | off | +5.8 | 561,390 |
| `slot-kind-tiebreak` | +6 [-12, +24] | 73.6% | off | +5.8 | 567,347 |
| `campus-finishes-first` | +4 [-27, +34] | 59.7% | off | +5.3 | 1,432,763 |
| `barbarian-hunt` | -38 [-132, +57] | 21.8% | off | +4.2 | 12,546 |
| `early-contact-window` | +2 [-29, +32] | 54.3% | off | +4.0 | 7,345,811 |
| `governor-victory-lanes` | -142 [-350, +65] | 9.0% | off | +3.9 | 553 |
| `housing-research` | +2 [-26, +30] | 55.6% | off | +3.8 | 5,060,335 |
| `barbarian-capture-priority` | +1 [-30, +32] | 53.2% | off | +3.8 | 13,077,439 |
| `theology-for-founders` | +3 [-22, +28] | 58.8% | off | +3.8 | 2,633,862 |
| `governor-every-lane` | -72 [-196, +53] | 13.0% | off | +3.1 | 2,763 |
| `lane-policy-deck` | +0 [-31, +31] | 50.0% | off | +3.1 | – |
| `competition-victory-points` | -0 [-31, +31] | 48.9% | off | +2.9 | 117,874,324 |
| `district-building-chain` | -1 [-32, +30] | 47.9% | off | +2.7 | 29,451,758 |
| `settler-site-agreement` | +0 [-26, +27] | 51.0% | off | +2.5 | 185,035,961 |
| `lane-great-people` | -3 [-33, +27] | 42.4% | off | +1.8 | 2,383,266 |
| `builder-barbarian-safety` | -0 [-24, +23] | 48.8% | off | +1.7 | 178,691,514 |
| `apostle-promotion-by-role` | +2 [-23, +27] | 56.6% | on | +1.2 | 4,679,671 |
| `blind-objective-units` | +0 [-18, +18] | 50.0% | off | +1.1 | 1,851,598,792,308 |
| `envoy-infrastructure` | -6 [-37, +25] | 35.3% | off | +1.0 | 579,212 |
| `research-tier-premium` | -6 [-37, +25] | 35.2% | off | +1.0 | 578,983 |
| `district-coverage` | -3 [-27, +21] | 40.6% | off | +0.9 | 2,356,319 |
| `condemn-under-congress` | -8 [-38, +23] | 31.2% | off | +0.6 | 341,081 |
| `science-multiplier-payoff` | -8 [-38, +23] | 31.1% | off | +0.6 | 340,841 |
| `religious-defence-scales` | -8 [-39, +22] | 29.6% | off | +0.6 | 272,602 |
| `settler-guard-holds` | -2 [-19, +16] | 43.1% | off | +0.5 | 8,359,065 |
| `army-target-weighs-enemy` | +5 [-20, +31] | 65.6% | on | +0.5 | 734,908 |
| `one-shot-recovery` | -8 [-39, +22] | 29.3% | off | +0.5 | 271,910 |
| `congress-banks-decided` | -8 [-39, +22] | 29.2% | off | +0.5 | 271,842 |
| `power-the-laboratory` | -8 [-39, +22] | 29.1% | off | +0.5 | 271,495 |
| `amenity-project-preemption` | -7 [-34, +20] | 29.8% | off | +0.3 | 371,605 |
| `science-payback-horizon` | -11 [-41, +20] | 24.7% | off | +0.3 | 165,751 |
| `joint-tactics` | -5 [-28, +17] | 32.6% | off | +0.3 | 755,823 |
| `fifteenth-citizen` | -12 [-42, +19] | 22.4% | off | +0.2 | 127,814 |
| `settler-target-hysteresis` | -4 [-21, +14] | 34.6% | off | +0.1 | 1,559,209 |
| `congress-counter-votes` | -13 [-43, +17] | 20.4% | off | +0.1 | 107,748 |
| `civilian-rescue` | -4 [-22, +14] | 33.2% | off | +0.1 | 1,247,507 |
| `lane-congress-favor` | -14 [-44, +17] | 18.4% | off | +0.1 | 85,512 |
| `barbarian-bargain` | +8 [-16, +31] | 74.4% | on | +0.1 | 304,958 |
| `siege-commitment` | -5 [-24, +14] | 30.7% | off | +0.1 | 788,779 |
| `culture-coverage` | -14 [-44, +16] | 18.2% | off | +0.1 | 85,100 |
| `spread-campaign-persists` | -16 [-47, +15] | 16.0% | off | +0.1 | 63,928 |
| `relief-targets-the-siege` | +5 [-13, +23] | 70.4% | on | +0.1 | 799,683 |
| `holy-lane-parity` | +38 [-9, +85] | 94.3% | on | +0.0 | 5,154 |
| `campus-adjacency-threshold` | -18 [-48, +13] | 12.6% | off | +0.0 | 43,930 |
| `siege-is-progress` | -17 [-46, +12] | 13.0% | off | +0.0 | 49,723 |
| `research-floor-holds` | -19 [-49, +12] | 11.8% | off | +0.0 | 38,558 |
| `war-patience` | -9 [-30, +12] | 20.6% | off | +0.0 | 219,110 |
| `holy-site-where-the-threat-is` | -19 [-49, +12] | 11.4% | off | +0.0 | 37,784 |
| `inquisition-on-threat` | +9 [-11, +30] | 81.2% | on | +0.0 | 191,257 |
| `research-grants-first` | -20 [-51, +10] | 9.4% | off | +0.0 | 28,021 |
| `naval-recon` | -8 [-26, +10] | 19.9% | off | +0.0 | 284,865 |
| `come-ashore` | +9 [-10, +29] | 82.3% | on | +0.0 | 193,576 |
| `culture-building-debt` | +24 [-7, +55] | 93.6% | on | +0.0 | 14,374 |
| `chain-tech-lookahead` | -25 [-56, +6] | 6.0% | off | +0.0 | 12,991 |
| `endgame-war-runway` | -9 [-27, +9] | 17.4% | off | +0.0 | 217,831 |
| `barbarian-ranged-answer` | +16 [-8, +39] | 90.3% | on | +0.0 | 48,868 |
| `guru-heals-the-corps` | -29 [-61, +2] | 3.4% | off | +0.0 | 3,213 |
| `escort-unstick` | +30 [-1, +62] | 97.0% | on | +0.0 | 1,912 |
| `wonder-ring-settle-value` | +11 [-8, +29] | 87.3% | on | +0.0 | 124,143 |
| `amenity-district-path` | +11 [-8, +29] | 87.3% | on | +0.0 | 126,711 |
| `war-reinforcement` | +20 [-4, +44] | 95.0% | on | +0.0 | 15,775 |
| `buildings-before-projects` | +28 [-0, +57] | 97.4% | on | +0.0 | 516 |
| `priced-tile-purchase` | -20 [-43, +3] | 4.7% | off | +0.0 | 14,832 |
| `founder-temple` | +19 [-3, +42] | 95.2% | on | +0.0 | 16,080 |
| `strike-opening` | +14 [-4, +32] | 93.5% | on | +0.0 | 44,900 |
| `home-defense` | -15 [-33, +3] | 5.6% | off | +0.0 | 33,434 |
| `peacetime-deterrence` | +16 [-3, +34] | 95.4% | on | +0.0 | 22,549 |
| `recorded-tactical-step` | +16 [-2, +34] | 95.6% | on | +0.0 | 20,937 |
| `housing-districts` | -17 [-36, +1] | 3.5% | off | +0.0 | 10,738 |
| `score-horizon` | +18 [-0, +36] | 97.5% | on | +0.0 | 183 |

The top 8 that one batch could actually resolve (≤ 60,000 seat pairs each), as an argument list:

```sh
gene_screen --genes camp-party,blind-objective-strength,siege-tracks-wall,barbarian-hunt,governor-victory-lanes,governor-every-lane,holy-lane-parity,campus-adjacency-threshold
```

`python3 tools/genes.py boundary` prints this list on its own, with `--arm-pairs` and `--max-arm-pairs` to size it.

## Lane genes and the share axis

At the standing 250-turn Online clock a **science or congress gene cannot pay through the win axis at all**: science and diplomatic victories land at median t283 and t285, past the clock, so they are 1–2% of endings and `docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for exactly that reason. The seat a lane gene would have carried to a science victory shows up as a score win or a score loss instead. The decision axis stays WINS — `docs/GENOME.md` records what happened the one time selection ran on a correlate — so the share reading is a **pre-registered secondary**, fixed in `docs/GENE_SCREEN.md` before the next screen rather than chosen after it.

The set is discovered from the code: every gene whose flag field `src/ai/advanced/victory_lane.rs` reads. A gene joins it by being a lane gene, not by being listed here.

| Lane gene | Default | ± Wins / 10k seats | Share Δpp (z) | Posterior (95% CI) | Status |
|---|---|---:|---|---:|---|
| `lane-congress-ballot` | off | +8 | -0.01 (z -0.21) ~ | +8 [-23, +39] | unresolved |
| `lane-congress-favor` | off | -14 | -0.09 (z -1.61) ~ | -14 [-44, +17] | unresolved |
| `lane-great-people` | off | -3 | -0.04 (z -0.66) ~ | -3 [-33, +27] | unresolved |
| `lane-policy-deck` | off | +0 | -0.04 (z -0.74) ~ | +0 [-31, +31] | unresolved |
| `lane-culture-spending` | off | +8 | -0.07 (z -1.26) ~ | +8 [-23, +39] | unresolved |
| `lane-space-race` | off | +11 | -0.01 (z -0.23) ~ | +11 [-20, +42] | unresolved |
| `competition-victory-points` | off | +0 | -0.07 (z -1.17) ~ | -0 [-31, +31] | unresolved |

## Awaiting measurement

These screenable genes have no on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a screen is pending.

| Gene | Default | Description |
|---|---|---|
| `builder-reward-survey` | off (unmeasured) | Price Builder production by a survey of the work it would do. |
| `coordinated-finish` | off (unmeasured) | Admit the friendly-volley extension without the rest of the closed war-half bundle. |
| `coupled-expansion` | off (unmeasured) | Enable the evaluator-only paid expansion treatment. |
| `district-planning` | off (unmeasured) | The city plans its districts, sites and tile buys together: wished districts get jointly assigned, reserved plots over rings 1-3, and the tile a very valuable site needs is bought. |
| `engine-faith-price` | off (unmeasured) | THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE. |
| `fortify-idle-units` | off (unmeasured) | Fortify units the planner gave nothing to do. |
| `maintenance-aware-deck` | off (unmeasured) | Let the deck counterfactual see the unit-maintenance bill. |
| `naval-production-policy` | off (unmeasured) | Reach for the naval-production discount while hulls are wanted. |
| `pantheon-board` | off (unmeasured) | Choose the pantheon from the land the empire holds rather than from a fixed order. |
| `price-the-suzerainty` | off (unmeasured) | Let the envoy scorer see the suzerainty it is walking toward. |
| `promote-when-wounded` | off (unmeasured) |  |
| `settlement-gap-target` | off (unmeasured) | Make the settlement-gap redirect and the Settler ranking honour the same city target the cascade settles toward. |
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

Every screenable heuristic gene on the Advanced controller, ranked most beneficial to least by **Scaled ± Wins Last Batch (n seats)**. Each batch column scales that batch's on-arm win rate to 10,000 seats; its `(n=...)` is the same batch's on-arm seat count. *Scaled ± Wins Prior Batch (n seats)* is the screen before that, and *Scaled ± Wins Third Batch (n seats)* the one before that again (– where the gene has no reading that far back): three chronological windows, newest first, so every new screen shifts a gene's readings one column right and drops the fourth-oldest off the table. Movement across the three is the gene's trend, and it is the column the two-column rule cannot see — a pair of positives that is the tail of a decline reads the same as one that is a rise until the third window is printed beside it. **The third column is published, not in force**: the rule below reads the first two and nothing else. *Default* is the deployment ledger's call (`docs/gene_ledger.json`), and since 2026-08-22 that call is read off the first two win columns: a gene defaults **on** when both are positive, or when their average clears +15 with neither below −10; with exactly one populated column it defaults **on** when that reading is above +20. It defaults **off** otherwise. The *Total* win-rate columns pool every screen that measured the gene, weighted by on-arm seats, and each carries its own on-arm seat count `n` — the two arms are only equal when every screen that measured the gene split them evenly. *Diff* is the on rate minus the off rate, rendered as a percentage: the **whole** on−off difference, so it stands at roughly twice the scale of the win columns beside it and must be read against a screen’s difference band rather than the halved column band below. **A negative *Diff* vetoes the default** (operator, 2026-08-22): a gene that has not won more than it lost across its whole record ships off however its two win columns read. That is the one clause that lets a screen older than the last two speak, and it is one-way — a positive *Diff* promotes nothing on its own, the columns still have to clear their bars. Three genes ship off on it alone: `war-economy`, `apostle-promotion-by-role` and `siege-commitment`, each carrying positive recent columns over a 2026-08-20 screen they have not made back. **There is one screen** (operator, 2026-08-22): six majors on 74x46 continents with nine city-states, Online speed to its own 250-turn clock, all six victory lanes, a foldover against the best-genome baseline with shuffled civs and every major seat carrying its own genome (errors clustered by game pair), so a gene's on/off readings cover the same maps. `docs/GENE_SCREEN.md` documents the instrument; the paired contrasts, intervals and family-wise verdicts stay in `docs/gene_ledger.json`. Screenable genes awaiting their first measurement are listed separately below without a rank.

**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected count is 1,667 wins per 10,000 on-arm seats and the win columns say how far above or below that a seat carrying the gene lands. **A column is half its screen’s on−off difference** — a foldover puts the two arms either side of chance — so the band that says whether a column is real is half the band on that difference. The two are not interchangeable: the ±110/10k figure this paragraph used to quote, and #2266 used to call eight removals noise, is the *difference*’s band and is twice too wide for the column beside it. Each screen’s own band is below, derived from its errors rather than quoted. Screens differ in baseline as repairs land, so the *Prior* column reads as history, not a strict A/B against *Last*.

**⚠ Every column below is `legacy`.** The shape marked `legacy` in the screen table is the pre-2026-08-22 instrument: 60x38 Pangaea, six city-states, where 48% of games ended in a religious conversion against 28% on continents. Those readings are what the deployment genome stands on and they are kept for that reason, but a gene is only priced at the screen once a `standard` row appears beside it. The four-player `domination,score` war columns are gone: a 1-in-4 chance base made them incomparable with the six-player columns printed next to them.

**What each screen resolves.** The median gene’s column standard error times 2.8 — a two-sided 5% test at 80% power. Judge a column against the band of the screen named beside it, never against a single number for the instrument: these differ by more than three to one.

*Pairing gain* is how far a screen’s error per pair sits below the unpaired baseline, and it is what separates them. A foldover cancels only to the extent its two arms play a similar game, so the gain reads on the **genes**, not the design — a gene that rarely fires leaves most pairs identical and cancels almost everything, while a whole-genome screen flips every gene between arms and cancels almost nothing. ⚠ Gene count is not the driver, though the rows below invite that reading — the falsifier is in them. `h1` carries **one** gene over **7,200** pairs and resolves ±68 at a 1.28× gain, *wider* than four-gene `s6` over 6,000. Its gene changes nearly every game; `s7`'s rarely fires. That, not the count, is the difference.

| Screen | Shape | Genes | Seat pairs | 1 SE | ±80% power | Pairing gain |
|---|---|---:|---:|---:|---:|---:|
| `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` | standard | 1 | 3,600 | 39.1 | ±109 | 1.12× |
| `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` | standard | 99 | 23,622 | 15.6 | ±44 | 1.10× |
| `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` | legacy | 1 | 7,200 | 24.3 | ±68 | 1.28× |
| `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` | legacy | 75 | 17,574 | 18.3 | ±51 | 1.09× |
| `2026-08-21-p7-native-6p-allseats-15000-pairs.json` | legacy | 57 | 15,000 | 19.9 | ±56 | 1.08× |
| `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` | legacy | 1 | 6,000 | 10.3 | ±29 | 3.32× |
| `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` | legacy | 4 | 6,000 | 22.9 | ±64 | 1.49× |
| `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` | legacy | 1 | 1,000 | 36.1 | ±101 | 2.68× |
| `2026-08-20-p4-native-6p-allseats-13446-pairs.json` | legacy | 64 | 13,446 | 21.5 | ±60 | 1.06× |

**Posterior (95% CI), P(>0), Share Δpp (z).** *Posterior* is a random-effects (DerSimonian–Laird) inverse-variance pool of **every** screen that priced the gene, on this column's own scale: each screen's on−off difference weighted by its own standard error, with the between-screen disagreement (τ) carried in the interval instead of assumed away. It is the answer to two things the columns cannot express — that the same +24 means different things from a ±29 screen and a ±64 one, and that two positive columns from screens differing in baseline, build and shape are not two confirmations (#2283/#2284 measured that: five of seven lane genes changed sign on disjoint seeds). *P(>0)* is where the shrinkage lands. *Share Δpp (z)* is the newest screen's score-share contrast and its verdict, published beside the win columns because the deployment rule reads the win axis only and a lane gene cannot pay on that axis at 250 turns. **None of these three decides anything today**; `AUTHORITY` in `tools/genes.py` says `columns` and *What the posterior would change* above is the delta.

**Cost.** Positive is slower; negative is faster. *cost (compute)* is the on/off percent change in wall seconds per completed turn, while *cost (time)* is the percent change in whole-game wall seconds and therefore includes games that end earlier or later. Each cell is the newest estimate ± one standard error. The screen derives both from paired log-ratios on the same maps, fits every randomized gene together with an arm-order intercept, and keeps one timing per game pair; all-seats signs are summed so the answer is the incremental cost of enabling one major's genome. This reuses the screen's existing `secs` and `turn` rows — no hot-path timers and no extra profiling games. A dash means the source analysis predates the estimator and is unknown, never zero.

Regenerate with `python3 tools/genes.py write` after every screen enters the ledger; `tools/test_genes.py` fails when this file is older than the ledger's sources.

## Follow-ups

**The first standard-shape screen disagrees with this whole file, and the posterior says which parts of the disagreement are real.** A 74x46 Continents / 9 CS / Online-250 all-seats foldover against the best-genome baseline -- 3,937 complete map pairs, **23,622 matched seat comparisons per gene**, seeds 141000000-141003936, source `b3ad9f00` -- is published in `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` (PR #2323). ⭐ **It is now a ledger source** (2026-08-23): `docs/gene_screens/2026-08-22-standard-10k-6p-allseats-23622-pairs.json`, recorded oldest-first ahead of `g1`, so it is the deciding **± Wins / 10k seats** column for 99 of the ledger's 101 genes. Everything below was written while it was still only a published table, with every figure read out of that document by hand; `tools/test_genes.py::TheStandardScreen` now checks those hand figures against the recorded JSON gene by gene, so the note and the source cannot drift apart. ⚠ The screen was played by the `b3ad9f00` binary, which predates the build stamp of #2331, so the ledger records it as `pre-fingerprint`; the check that stamp would have run was done by hand instead and passed exactly — the gene set in its header hashes to `e6015634ff769890`, which is the fingerprint of `ENGINE_REPAIR_TREATMENTS` + `PRODUCTION_TREATMENTS` + `PRODUCTION_OPT_INS` at `b3ad9f00`, same 100 tags in the same order, and every gene it prices is still registered.

**`governor-victory-lanes` was the single largest correctable defect in the shipped genome, and on 2026-08-23 it was resolved OFF.** It **defaulted on**, promoted by #2294's one-column clause on P10's **+46**. At the deployment shape the whole-genome screen read it **-4.73 pp, z -15.37** -- **-237 wins per 10,000 on-arm seats, 95% CI [-267, -206]**. The two readings were not close: legacy resolves it at [+9, +82] and standard at [-267, -206], and nothing lies between them. Pooling the two shapes gave **-96 [-372, +181] with tau = 199** at the time of writing (over all three screens the ledger now records it at **-142 [-350, +65], tau = 182**), an interval more than 400 wide -- the random-effects estimator doing its job, saying *these are two instruments, not two draws from one number*. **The correct reading was standard-only, and standard-only resolved it OFF.** A pre-registered single-gene direct arm then confirmed it independently: **600 of 600 map pairs on seeds 150000000-150000599**, disjoint from the whole-genome screen's maps, every other deployment gene held fixed -- **-4.78 pp at win z -6.11, 95% CI [-6.3, -3.2]**, score share **-2.73 pp at z -23.76** (`docs/eval/2026-08-23-governor-victory-lanes-direct-confirmation.md`). The point estimate matches the whole-genome screen to within 0.05 pp. Entering it moved exactly one default, 31 -> 30, with both clauses of the rule agreeing rather than the marginal Diff veto alone. ⭐ The whole-genome screen entered the ledger behind it on 2026-08-23, so the row now reads **-239 / -237 / +46** across the three windows: the direct arm, the whole-genome screen, and the legacy column that promoted the gene. The two standard readings are 0.05 pp apart and pool with **tau = 0**. ⚠ It is a native-screen effect: on the live bridge the gene is inert (#2335 replayed 768 recorded turns byte-identically), so the flip costs the live seat nothing.

*The legacy share axis already knew.* P10 priced this gene at win z **+2.46** and score-share z **-15.92** -- a recorded `conflict` -- and the deployment rule reads the win axis only, so the +46 promoted it. The deployment shape's **win** axis now reads z -15.37, within half a sigma of what the legacy **share** axis said a day earlier. That is the case for the pre-registered secondary axis in `docs/GENE_SCREEN.md`, made by the genome's own worst row.

*The composite, decomposed.* `governor-every-lane` (the composite) reads **-4.68**, `governor-victory-lanes` (the victory-lane half) **-4.73**, `governor-expansion-lane` (the other half) **-0.55**. The victory half carries essentially the whole of the composite's harm and the halves are close to additive, so this is not "the composite is bad": it is *one named half* is bad, and the repository already has that half as its own gene. The composite and the expansion half both already default off; the harmful half is the one that ships. A confirmation arm on `governor-victory-lanes` at the standard shape (600 map pairs, seeds 150000000-150000599) was running when this was written.

*Six of the eight defaults the standard screen would move are decided by noise.* (It was entered as a ledger source on 2026-08-23; this paragraph was written just before that, and every figure in it held.) The pooled-`Diff` veto alone flips eight genes: `governor-victory-lanes` (z -15.37) and `war-economy` (z +7.50) on real signals, and `settler-site-agreement` (-1.47), `settler-target-hysteresis` (-1.16), `housing-research` (-1.10), `religion-sues-peace` (-1.14), `apostle-promotion-by-role` (+1.02) and `theology-for-founders` (+1.43) on readings of |z| about 1. **The posterior read standard-only resolves exactly two of the eight -- the two with signal** (`governor-victory-lanes` [-267, -206], `war-economy` [+87, +148]) -- and declines all six of the others, whose standard-only intervals all straddle zero. Read *pooled* it resolves none of the eight, because tau is 199 and 124 on the two real ones. Both halves still hold now that the screen is recorded: standard-only resolves exactly `governor-victory-lanes` **off** and `war-economy` **on**, and the cross-shape pool resolves none of the eight. Both halves of that are the recommendation: adopt the posterior, and when a `standard` source lands set `POSTERIOR_SHAPES = ("standard",)` in `tools/genes.py` so the deployment shape decides and the Pangaea record stays history.

*What survives the change of instrument.* `great-person-housing` +78 -> +94, `raid-pillage-prizes` +30 -> +53 and `opportunistic-war` +23 -> +49 read positive at both shapes and pool tighter, not wider; `wide-map-capacity` goes +33 -> +90. Rank 1 does not: `holy-lane-parity`, +99 on legacy and confirmed by its own direct arm, reads **+20 [-12, +51]** at the deployment shape. Two genes the legacy ledger has never priced arrive resolved -- `air-surge` **+108 [+77, +138]** and `contact-posture` **-54 [-85, -24]**.


⚠ **One debt the fires gate just wrote off, recorded here so it does not vanish.** Entering this screen proved all seventeen remaining entries in `tools/gene_fire_waivers.json` and emptied the file: every gene in the tables has now been shown to fire by a committed screen row, which is the ratchet's own definition of proof. Sixteen of those are real -- genes that predated the gate and had simply never been screened. The seventeenth is not. `competition-victory-points` prices a scored competition's first place by the Diplomatic Victory Points it pays, and **it cannot fire in a screen at all**: `GameOptions::native_competitions` is `false` (`src/game.rs`), `src/bin/gene_screen.rs` never sets it, and `Game::open_competition` returns early without it -- so no competition is ever scored. Its own single-gene probe (`docs/gene_screens/fires/competition-victory-points.json`) is duly zero-width. What cleared its waiver is a **-0.01 pp / z -0.03** row in this 23,622-seat multi-gene screen: noise from the other ninety-eight genes' arms, which `tools/gene_fires.py`'s own header already warns is "proven for a weaker reason" because a non-zero contrast on a multi-gene screen is not attributable to the gene. The gate is right that the list can only shrink and wrong about this one row; a zero-width **single-gene probe** ought to outweigh a non-zero multi-gene one, and until it does the debt lives here. The gene has held a genome bit since #2274, consumes a column in every screen, and returns nothing. The resolution is unchanged: enable competitions in the screen, or take the gene out of the tables.

**`holy-lane-parity` came back and was confirmed directly.** It left the code with #2266's bottom ten on a **-27** reading from the four-gene `s6` screen, whose column band is +/-64 -- a null, not a measurement against the gene. P10 was already running when that cull merged (its binary predates it by 1h43m), so it priced the gene after the code was gone: **+63** at z +3.48, past P10's own family-wise bar and the only such reading among the nineteen genes in *Removed from the code*. #2299 restored the code and ran the direct arm the cull never got to: 1,200 map pairs on seeds 110M, every other treatment held at the deployment genome, all 2,400 games complete. It reads **+99 wins/10k, z +4.05, 95% CI [+51, +147]** (`HELPS **`), against a run that resolves +/-68 -- an independent seed window agreeing with P10's independent instrument. Score share is null (+0.08 pp, z +1.23): the gene converts games, it does not accumulate score. The 850 it pays is `(Culture, theater_square)`'s own figure, deliberately an upper bound rather than a tuned value, so a positive result opens the tuning question rather than answering it. The operator recorded this 60x38 Pangaea confirmation as a **legacy** ledger source, so the row below read **+99/+63** -- both columns positive -- and the gene **defaults on**. ⭐ The standard screen has since become the newest column and reads it **+19 [-12, +51]**: a null at the deployment shape, and the row is now **+19 / +99 / +63**. Both of the two columns the rule reads are still positive, so it still ships, but it is no longer rank 1 and the promotion now rests on a legacy pair with a standard null in front of it. That moves the incumbent every recorded Elo result is filed against; it is the first gene to be removed by a cull, restored, and promoted. See [the confirmation](docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md).

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

**P10 ended early at the operator's request.** Its 2,929 complete map seeds provide 5,858 controlled games and 17,574 treated-seat pairs; the analysis excludes 11 interrupted one-arm seeds (66 raw seat rows), with zero duplicate or invalid tuples. The new *Wins / 10k seats* value extrapolates each measured on-arm rate to 10,000 on-arm seats as `round((win_on − 1/6) × 10,000)`; it does not invent synthetic games. The former *Wins / 10k seats* reading shifts intact to *Wins / 10k seats prior*, and since 2026-08-23 the one behind that shifts to *Wins / 10k seats third* (operator request): three chronological windows, newest first, each new screen pushing a gene's readings one column right and the fourth-oldest off the table. **Only the first two decide anything** -- the rule is unchanged -- and the third is there because a pair of readings cannot show whether it is the tail of a decline or the start of a rise. P10 used the 6p all-six native regime on seeds 100000000–100002962, 60×38 pangaea/online/250 turns, shuffled civilizations, every major seat treated, and foldover against the best-genome baseline. Its fixed binary came from `d23f92d944cd889aa4c9dfe58c37aceb8e55eabd` (SHA-256 `79385db96e89e91cc0b6fd8389e837cb66dc05ccaa4eee493576f152daf627ed`), before later gene removals and additions; ledger generation drops obsolete tags and retains newer genes from their existing sources.

**The bottom of the table was not culled, because the standard screen does not agree with it.** #2330 was launched to remove `barbarian-hunt` under the standing directive that the bottom of the ranking leaves the code. Its row here is the worst in the table -- **-86 wins/10k seats, -1.73 pp, win z -4.65, share z -7.54**, family-wise on P10's own bar, and P10 replicates it internally (its two tranches read -2.08 and -1.27). Nothing about that reading is marginal, and on the instrument that produced it the cull rule fires cleanly. It is still the wrong action, for one reason: **that instrument is `legacy`, and the standard one measured the same gene and disagreed.** [`docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`](docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md) -- source `b3ad9f00`, 3,937 complete map pairs, **23,622 matched seat comparisons per gene** against P10's 17,574, on 74x46 Continents with nine city-states -- reads `barbarian-hunt` at **+0.20 pp, win z +0.65, +10 wins/10k seats**. The sign is opposite and the magnitude is a tenth. This is not the two screens failing to resolve the same small effect: `governor-victory-lanes` at -4.73 pp / z -15.37 in that file puts its standard error near **0.31 pp** on the difference, so P10's -1.73 pp sits about **six standard errors** from what the standard screen measured. One of the two instruments is wrong about this gene, and the paragraph above this one says which of them prices a gene at all: *a gene is only priced at the screen once a `standard` row appears beside it.* Culling on a legacy column contradicts this document's own reading rule.

The legacy board has a second witness and it agrees with the standard screen, not with P10. [The direct arm](docs/eval/2026-08-21-barbarian-hunt-direct.md) held every other treatment at the deployment genome and varied only this gene across 300 map pairs / 1,800 clustered seat-pairs: **+0.56 pp, z +0.51**. That run resolves only +/-3.1 pp at 80% power, so on its own it cannot refute -1.73 -- but it is the third reading in a row whose sign is positive, and it is the arm #2299 used to settle exactly this question for `holy-lane-parity`.

So the gene stays in the code, `off` and unresolved, and **the cull rule does not fire on a `legacy` column**. What would decide it: a `barbarian-hunt` row from a **`standard`-shape screen entering `docs/gene_ledger.json`**. If that row is below the screen's own column band with the sign P10 gave it, the gene leaves the code with the standard number recorded beside it; if it lands where the 23,622-seat screen already put it, P10's -86 was an artefact of the 60x38 Pangaea room -- where 48% of games ended in a religious conversion against 8% at the standard shape -- and the row is a null that never licensed anything.

⭐ **That row landed on 2026-08-23, and it was the second answer.** The screen is now a ledger source and `barbarian-hunt` reads **+10 [-21, +41]** in the newest window against **-86** in the one behind it -- a null at the deployment shape, well inside the screen's own band, with the opposite sign to the reading the cull was launched on. The gene stays. The cull rule is now satisfied on its own terms rather than held off on a technicality, and the third window is what keeps the -86 visible beside the +10 instead of dropping it out of the table the moment a fourth screen prices the gene.

`lane-congress-ballot` was the same question and gets the same answer for a different reason. [`docs/VICTORY_GENES.md`](docs/VICTORY_GENES.md) §8 records it negative in every window of both regimes and reaching `share hurts *` at z -2.31, and says in terms that if a dedicated arm confirms it *"the gene should leave the code by the same rule that culled the bottom of the ranking"*. That rule is about the bottom of the **ranking**, and an unmeasured gene has no rank -- it is in *Awaiting measurement* above, not in the table -- so nothing was licensed even before the new evidence. The new evidence closes it anyway: the same standard screen reads `lane-congress-ballot` at **+0.16 pp, win z +0.51**, a null. The confirming arm §8 asked for would now have to overturn a 23,622-seat reading, not merely add to five small ones.

⚠ **This is the third and fourth time, and the pattern is now the finding.** #2266 removed ten genes at once against a band that was twice too wide -- the +/-110 it quoted is the *difference*'s band, and the column beside it resolves half that (#2300). `holy-lane-parity` was one of the ten, left on a -27 null, was priced at **+63** by a screen already running when the cull merged, was restored in #2299 and confirmed at **+99, z +4.05** in #2307. Now `barbarian-hunt` and `lane-congress-ballot`: proposed for removal on a legacy column and a five-window trend, and re-priced as nulls by the standard screen before either could be cut. Four genes, three episodes, one mechanism -- **a reading from the wrong instrument, acted on irreversibly.**

So the rule, for whoever culls next. A cull is not the symmetric opposite of a default. A gene left `off` costs one row in a foldover screen and **no games**, and it can be re-priced by every screen that runs afterwards; a gene removed can never be re-priced by anything, and restoring it costs a dedicated confirmation run (1,200 map pairs for `holy-lane-parity`). So the bar for deleting code is not "the worst reading available" -- `barbarian-hunt`'s -86 was the worst reading in the table and it was still the wrong number. It is **a reading on the instrument the agent is actually being screened on**, and the three questions that establish it: is this column `standard` or `legacy`; is there a screen in flight or unmerged that has already priced this gene (check `batch.source_commit` against the cull date, and check the open pull requests); and does a direct arm against the deployment genome agree. `barbarian-hunt` failed all three.

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 13,446 pairs), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 1,000 pairs), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 6,000 pairs), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 6,000 pairs), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 15,000 pairs), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 17,574 pairs), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 7,200 pairs), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 23,622 pairs), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 3,600 pairs). The paired contrasts, intervals and family-wise verdicts live in `docs/gene_ledger.json`; this table is the operator's wins-per-ten-thousand-seat view of the same observations._
