# The heuristic gene ranking

**Deployment default:** operator-pinned (43 genes): retains the prior 36 selections and explicitly adds `unit-cost-efficiency`, `unit-objective-memory`, `camp-party`, `slot-kind-tiebreak`, `promote-when-wounded`, `religion-sues-peace`, `lane-great-people`. Screen columns, *Diff*, and posterior values are evidence only; new batches do not automatically change defaults.

| Rank | Gene | Description | Best version | Default | Wins ± /10k total seats — Last Batch (n=38,160 total seats) | Wins ± /10k total seats — Prior Batch (n=41,628 total seats) | Wins ± /10k total seats — Third Batch (n=10,002 total seats) | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `air-surge` | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. | — | **on** | +33 | +25 | +43 | 17.24% (n=91,046) | 15.53% (n=45,988) | 1.72% | +93 [+73, +113] | 100.0% | +0.70 (z +3.86) helps * | +0.24% ±0.40% | -0.76% ±0.69% |
| 2 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate works to make room. | — | **on** | +28 | +29 | +41 | 17.25% (n=108,390) | 15.68% (n=63,792) | 1.56% | +84 [+66, +102] | 100.0% | +0.64 (z +3.66) helps * | +0.50% ±0.42% | +0.59% ±0.75% |
| 3 | `maintenance-aware-deck` | Let the deck counterfactual see the unit-maintenance bill. | — | **on** | +31 | +49 | +9 | 17.40% (n=44,859) | 15.93% (n=44,931) | 1.47% | +68 [+29, +107] | 100.0% | +0.36 (z +2.45) helps * | -0.26% ±0.37% | -0.67% ±0.66% |
| 4 | `price-the-suzerainty` | Let the envoy scorer see the suzerainty it is walking toward. | — | **on** | +28 | +49 | +2 | 17.36% (n=44,957) | 15.97% (n=44,833) | 1.40% | +61 [+15, +107] | 99.5% | +0.17 (z +1.09) ~ | -0.30% ±0.38% | -0.43% ±0.70% |
| 5 | `engine-faith-price` | THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE. | — | **on** | +31 | +23 | +58 | 17.28% (n=44,925) | 16.05% (n=44,865) | 1.22% | +64 [+33, +95] | 100.0% | +0.38 (z +2.44) helps * | -0.71% ±0.35% | -1.39% ±0.62% |
| 6 | `wide-map-capacity` | Price the city ceiling off uncontested land. | — | **on** | +41 | +27 | +32 | 17.16% (n=136,857) | 15.94% (n=92,217) | 1.22% | +64 [+39, +89] | 100.0% | +0.67 (z +3.98) helps * | +0.45% ±0.45% | -0.07% ±0.81% |
| 7 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for peace. | — | **on** | +29 | +32 | +27 | 17.06% (n=108,469) | 16.00% (n=63,713) | 1.07% | +58 [+34, +82] | 100.0% | +0.58 (z +3.41) helps * | +0.14% ±0.43% | +0.52% ±0.79% |
| 8 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | — | **on** | +27 | +24 | +39 | 17.07% (n=137,269) | 16.06% (n=91,805) | 1.01% | +55 [+37, +73] | 100.0% | +0.18 (z +1.01) ~ | +0.79% ±0.42% | +0.90% ±0.77% |
| 9 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | — | **on** | +32 | +11 | +20 | 17.00% (n=108,635) | 16.09% (n=63,547) | 0.91% | +49 [+29, +69] | 100.0% | +0.46 (z +2.56) helps * | -0.17% ±0.43% | +0.14% ±0.75% |
| 10 | `war-economy` | Send an adaptive Conquest plan through the war production path. | — | **on** | +35 | +29 | +37 | 16.95% (n=137,143) | 16.24% (n=91,931) | 0.71% | +34 [-44, +112] | 80.2% | +1.06 (z +5.92) helps * | +0.49% ±0.43% | +0.36% ±0.74% |
| 11 | `escort-unstick` | Release an escort that is not walking its settler. | 1 | **on** | +19 | +14 | +0 | v1 16.95% (n=127,412) · v2 16.34% (n=9,529) | v1 16.32% (n=101,662) · v2 16.78% (n=28,631) | 0.63% | +32 [+12, +51] | 99.9% | +0.02 (z +0.09) ~ | -0.39% ±0.45% | -0.63% ±0.79% |
| 12 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good enough to pay for its walk. | — | **on** | +14 | +10 | +15 | 16.90% (n=108,670) | 16.27% (n=63,512) | 0.62% | +33 [+16, +51] | 100.0% | +0.10 (z +0.61) ~ | -0.23% ±0.42% | -0.53% ±0.73% |
| 13 | `unit-cost-efficiency` | Credit strength-per-production and the civ's own unique unit in the military production arm. | — | **on** | +9 | +24 | +6 | 16.98% (n=44,706) | 16.36% (n=45,084) | 0.62% | +31 [+7, +56] | 99.4% | +0.06 (z +0.40) ~ | +0.05% ±0.38% | -0.22% ±0.65% |
| 14 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | — | **on** | +17 | +5 | +37 | 16.90% (n=136,898) | 16.32% (n=92,176) | 0.59% | +31 [+16, +46] | 100.0% | +0.57 (z +3.26) helps * | -0.22% ±0.42% | -0.61% ±0.75% |
| 15 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | — | **on** | +4 | -2 | +2 | 16.90% (n=136,840) | 16.32% (n=92,234) | 0.59% | +32 [+12, +51] | 99.9% | +0.18 (z +1.02) ~ | +0.33% ±0.45% | +0.49% ±0.81% |
| 16 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | — | **on** | +10 | +7 | +18 | 16.88% (n=137,043) | 16.34% (n=92,031) | 0.54% | +28 [+11, +46] | 99.9% | +0.16 (z +0.93) ~ | +0.68% ±0.43% | +1.09% ±0.77% |
| 17 | `holy-lane-parity` | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | — | **on** | +3 | +7 | -6 | 16.87% (n=121,779) | 16.34% (n=76,803) | 0.53% | +26 [-4, +56] | 95.6% | -0.21 (z -1.23) ~ | -0.18% ±0.45% | -0.27% ±0.80% |
| 18 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | — | **on** | -1 | +4 | +30 | 16.85% (n=114,413) | 16.36% (n=69,769) | 0.49% | +26 [+12, +39] | 100.0% | +0.37 (z +2.06) helps * | -0.35% ±0.42% | -0.82% ±0.73% |
| 19 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | — | **on** | +1 | +1 | +26 | 16.86% (n=137,099) | 16.38% (n=91,975) | 0.49% | +27 [+8, +47] | 99.7% | +0.12 (z +0.67) ~ | +0.41% ±0.43% | +0.56% ±0.76% |
| 20 | `unit-objective-memory` | Let a unit retain its campaign objective and a short, threat-driven retreat across turns. | — | **on** | +5 | +20 | -5 | 16.88% (n=44,781) | 16.45% (n=45,009) | 0.43% | +22 [-4, +47] | 95.2% | -0.06 (z -0.42) ~ | -0.08% ±0.38% | -0.58% ±0.67% |
| 21 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | — | **on** | +8 | +14 | +3 | 16.81% (n=136,663) | 16.45% (n=92,411) | 0.36% | +19 [+4, +34] | 99.3% | +0.13 (z +0.72) ~ | -0.48% ±0.45% | -0.72% ±0.81% |
| 22 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | — | **on** | +0 | +2 | +2 | 16.80% (n=108,615) | 16.44% (n=63,567) | 0.36% | +20 [+2, +38] | 98.6% | -0.20 (z -1.12) ~ | +0.61% ±0.43% | +0.79% ±0.77% |
| 23 | `culture-building-debt` | Make the Theater Square owe its buildings. | — | **on** | +11 | +3 | -1 | 16.79% (n=91,009) | 16.43% (n=46,025) | 0.36% | +20 [-1, +40] | 96.7% | +0.03 (z +0.15) ~ | +0.63% ±0.43% | +1.10% ±0.77% |
| 24 | `camp-party` | The peacetime camp party. | — | **on** | +14 | +4 | +2 | 16.84% (n=114,458) | 16.50% (n=114,616) | 0.34% | +19 [-0, +37] | 97.3% | -0.07 (z -0.47) ~ | +0.39% ±0.39% | +0.81% ±0.70% |
| 25 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | — | **on** | +0 | +1 | +16 | 16.79% (n=136,892) | 16.49% (n=92,182) | 0.30% | +16 [-0, +32] | 97.4% | +0.35 (z +1.95) ~ | +0.83% ±0.44% | +1.23% ±0.75% |
| 26 | `religious-units-heal-first` | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. | — | **on** | +3 | +10 | +16 | 16.81% (n=68,109) | 16.53% (n=68,925) | 0.28% | +14 [-5, +33] | 92.6% | -0.00 (z -0.02) ~ | +0.23% ±0.38% | +0.72% ±0.70% |
| 27 | `competition-victory-points` | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. | — | **on** | +18 | +4 | +9 | 16.80% (n=68,438) | 16.53% (n=68,596) | 0.27% | +13 [-6, +32] | 90.4% | +0.01 (z +0.06) ~ | -0.25% ±0.37% | -0.21% ±0.66% |
| 28 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | — | off | -11 | +12 | +4 | 16.77% (n=137,111) | 16.51% (n=91,963) | 0.26% | +14 [-1, +30] | 96.8% | -0.23 (z -1.31) ~ | -0.21% ±0.43% | -0.67% ±0.75% |
| 29 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | — | **on** | +9 | +14 | +24 | 16.77% (n=137,140) | 16.51% (n=91,934) | 0.26% | +13 [-6, +33] | 90.7% | +0.14 (z +0.79) ~ | -0.19% ±0.45% | -0.57% ±0.79% |
| 30 | `barbarian-bargain` | Price a raider's life below a major's. | — | **on** | +16 | +2 | -5 | 16.76% (n=108,458) | 16.51% (n=63,724) | 0.25% | +13 [-5, +31] | 92.1% | +0.02 (z +0.10) ~ | -0.56% ±0.41% | -1.03% ±0.72% |
| 31 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | — | off | +9 | +7 | +8 | 16.79% (n=114,713) | 16.54% (n=114,361) | 0.25% | +12 [-3, +26] | 94.7% | -0.04 (z -0.25) ~ | +0.36% ±0.38% | +0.63% ±0.67% |
| 32 | `district-planning` | The city plans its districts, sites and tile buys together: wished districts get jointly assigned, reserved plots over rings 1-3, and the tile a very valuable site needs is bought. | — | off | +6 | – | – | 16.79% (n=19,090) | 16.54% (n=19,070) | 0.24% | +12 [-25, +49] | 74.1% | +0.14 (z +1.84) ~ | -0.09% ±0.20% | -0.04% ±0.35% |
| 33 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | — | off | +15 | -1 | +33 | 16.78% (n=114,417) | 16.55% (n=114,657) | 0.23% | +17 [-9, +43] | 89.6% | +0.41 (z +2.64) helps * | -0.90% ±0.36% | -1.71% ±0.62% |
| 34 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | — | **on** | +13 | +10 | -1 | 16.78% (n=114,161) | 16.55% (n=114,913) | 0.23% | +11 [-4, +25] | 92.5% | -0.21 (z -1.37) ~ | -0.45% ±0.39% | -0.60% ±0.70% |
| 35 | `promote-when-wounded` |  | — | **on** | +4 | +10 | -4 | 16.78% (n=45,059) | 16.55% (n=44,731) | 0.22% | +11 [-13, +36] | 81.6% | +0.05 (z +0.30) ~ | +0.00% ±0.37% | +0.06% ±0.67% |
| 36 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | — | **on** | +4 | +3 | -31 | 16.76% (n=136,923) | 16.53% (n=92,151) | 0.22% | +12 [-5, +29] | 92.2% | -0.28 (z -1.54) ~ | -0.06% ±0.43% | -0.05% ±0.76% |
| 37 | `strike-opening` | Let movement credit the attack a tile opens. | — | **on** | +1 | +2 | -3 | 16.74% (n=136,923) | 16.55% (n=92,151) | 0.19% | +11 [-5, +26] | 91.5% | -0.02 (z -0.13) ~ | -0.76% ±0.43% | -0.87% ±0.76% |
| 38 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | — | **on** | +12 | -12 | -2 | 16.74% (n=136,904) | 16.56% (n=92,170) | 0.18% | +10 [-8, +28] | 85.9% | +0.01 (z +0.07) ~ | +0.08% ±0.44% | +0.18% ±0.80% |
| 39 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | — | off | -2 | +0 | +18 | 16.73% (n=108,623) | 16.55% (n=63,559) | 0.18% | +10 [-8, +28] | 87.2% | +0.14 (z +0.78) ~ | +0.04% ±0.48% | +0.13% ±0.87% |
| 40 | `settlement-gap-target` | Make the settlement-gap redirect and the Settler ranking honour the same city target the cascade settles toward. | — | off | +7 | +2 | +1 | 16.75% (n=44,920) | 16.58% (n=44,870) | 0.17% | +8 [-16, +33] | 74.9% | +0.15 (z +0.97) ~ | -0.12% ±0.38% | -0.61% ±0.68% |
| 41 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | — | **on** | +2 | +6 | +12 | 16.75% (n=114,307) | 16.59% (n=114,767) | 0.16% | +8 [-7, +22] | 85.4% | -0.00 (z -0.02) ~ | +0.21% ±0.36% | +0.47% ±0.64% |
| 42 | `lane-great-people` | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. | — | **on** | +17 | +1 | -9 | 16.74% (n=68,375) | 16.59% (n=68,659) | 0.15% | +7 [-12, +26] | 75.3% | -0.18 (z -1.18) ~ | +0.45% ±0.37% | +0.57% ±0.67% |
| 43 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | — | **on** | +13 | -3 | +9 | 16.72% (n=136,931) | 16.58% (n=92,143) | 0.14% | +8 [-8, +23] | 83.5% | +0.15 (z +0.83) ~ | -0.25% ±0.45% | -0.51% ±0.78% |
| 44 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | — | **on** | +8 | -6 | -15 | 16.72% (n=114,762) | 16.58% (n=69,420) | 0.14% | +11 [-8, +30] | 87.1% | +0.05 (z +0.26) ~ | +0.62% ±0.42% | +0.77% ±0.76% |
| 45 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | — | off | -11 | -1 | +3 | 16.74% (n=114,398) | 16.60% (n=114,676) | 0.14% | +8 [-7, +23] | 84.9% | -0.05 (z -0.36) ~ | +0.50% ±0.39% | +1.02% ±0.70% |
| 46 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | — | **on** | +3 | +5 | -5 | 16.72% (n=114,438) | 16.58% (n=69,744) | 0.14% | +9 [-8, +26] | 85.5% | -0.04 (z -0.21) ~ | +0.67% ±0.45% | +1.04% ±0.79% |
| 47 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | — | off | +0 | +11 | -13 | 16.72% (n=136,711) | 16.59% (n=92,363) | 0.14% | +7 [-11, +25] | 77.0% | -0.06 (z -0.35) ~ | +0.32% ±0.44% | +0.35% ±0.76% |
| 48 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | — | off | +0 | +24 | +4 | 16.73% (n=114,473) | 16.60% (n=114,601) | 0.13% | +6 [-10, +21] | 76.0% | -0.03 (z -0.23) ~ | +0.20% ±0.37% | +0.69% ±0.67% |
| 49 | `come-ashore` | Keep the land army out of the water. | — | off | -2 | +0 | +15 | 16.72% (n=137,047) | 16.59% (n=92,027) | 0.12% | +7 [-8, +22] | 81.6% | +0.03 (z +0.16) ~ | -0.70% ±0.42% | -0.97% ±0.73% |
| 50 | `religious-defence-scales` | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. | — | off | +6 | +2 | +30 | 16.73% (n=68,544) | 16.61% (n=68,490) | 0.12% | +5 [-14, +24] | 69.2% | +0.23 (z +1.53) ~ | -0.21% ±0.39% | -0.22% ±0.73% |
| 51 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | — | off | +16 | -10 | +1 | 16.72% (n=114,210) | 16.61% (n=114,864) | 0.11% | +5 [-9, +20] | 77.2% | +0.11 (z +0.68) ~ | -0.34% ±0.36% | -0.59% ±0.65% |
| 52 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | — | **on** | +7 | -8 | -4 | 16.71% (n=136,815) | 16.60% (n=92,259) | 0.11% | +6 [-9, +22] | 79.6% | -0.19 (z -1.11) ~ | -0.49% ±0.41% | -0.95% ±0.73% |
| 53 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | — | off | +0 | -4 | +1 | 16.72% (n=114,533) | 16.62% (n=114,541) | 0.10% | +6 [-9, +20] | 77.8% | -0.24 (z -1.54) ~ | -0.11% ±0.36% | -0.34% ±0.63% |
| 54 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | — | off | -16 | +9 | +41 | 16.71% (n=86,115) | 16.62% (n=86,067) | 0.09% | +9 [-19, +37] | 73.3% | +0.25 (z +1.63) ~ | +0.28% ±0.38% | +0.40% ±0.69% |
| 55 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | — | off | -10 | -1 | +12 | 16.70% (n=136,905) | 16.62% (n=92,169) | 0.07% | +5 [-11, +20] | 72.4% | +0.11 (z +0.68) ~ | -0.10% ±0.43% | -0.11% ±0.77% |
| 56 | `science-multiplier-payoff` | Credit a Campus building the beakers its city's multipliers will actually pay it. | — | off | +11 | +1 | -5 | 16.70% (n=68,561) | 16.63% (n=68,473) | 0.07% | +3 [-16, +22] | 61.0% | -0.04 (z -0.27) ~ | +0.38% ±0.36% | +0.17% ±0.62% |
| 57 | `holy-site-where-the-threat-is` | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. | — | off | +10 | +6 | +3 | 16.70% (n=68,350) | 16.64% (n=68,684) | 0.06% | +2 [-17, +20] | 56.3% | -0.22 (z -1.44) ~ | -0.03% ±0.37% | -0.02% ±0.66% |
| 58 | `lane-policy-deck` | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. | — | off | +15 | -14 | +22 | 16.69% (n=68,688) | 16.64% (n=68,346) | 0.06% | +5 [-24, +34] | 64.0% | +0.20 (z +1.29) ~ | -0.42% ±0.37% | -0.41% ±0.66% |
| 59 | `spread-campaign-persists` | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is spent. | — | off | -10 | +17 | +19 | 16.69% (n=68,538) | 16.64% (n=68,496) | 0.05% | +4 [-25, +34] | 61.4% | -0.23 (z -1.47) ~ | +0.44% ±0.37% | +0.86% ±0.67% |
| 60 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | — | off | +8 | +2 | +15 | 16.69% (n=114,493) | 16.64% (n=114,581) | 0.05% | +2 [-13, +16] | 59.5% | -0.11 (z -0.72) ~ | -0.73% ±0.39% | -0.82% ±0.71% |
| 61 | `campus-adjacency-threshold` | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. | — | off | +11 | +2 | +10 | 16.69% (n=68,216) | 16.64% (n=68,818) | 0.05% | +1 [-18, +20] | 53.6% | +0.24 (z +1.58) ~ | +0.02% ±0.36% | -0.36% ±0.64% |
| 62 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | — | off | +12 | -7 | +12 | 16.68% (n=114,550) | 16.65% (n=114,524) | 0.04% | +1 [-13, +16] | 57.6% | -0.02 (z -0.12) ~ | +0.07% ±0.38% | -0.55% ±0.68% |
| 63 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | — | off | +9 | +12 | -14 | 16.68% (n=114,596) | 16.65% (n=114,478) | 0.02% | +0 [-15, +15] | 51.2% | -0.20 (z -1.31) ~ | +0.08% ±0.37% | -0.10% ±0.64% |
| 64 | `power-the-laboratory` | A power plant is credited the yields it switches on in its city. | — | off | +7 | -2 | +7 | 16.68% (n=68,683) | 16.66% (n=68,351) | 0.02% | +0 [-19, +19] | 51.2% | +0.22 (z +1.41) ~ | -0.25% ±0.37% | -0.34% ±0.67% |
| 65 | `theology-for-founders` | A founder researches Theology next. | — | **on** | +1 | -5 | +4 | 16.68% (n=91,950) | 16.66% (n=92,232) | 0.02% | +1 [-15, +17] | 54.8% | -0.05 (z -0.33) ~ | +0.80% ±0.38% | +1.19% ±0.67% |
| 66 | `guru-heals-the-corps` | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. | — | off | +12 | +3 | +16 | 16.67% (n=68,424) | 16.66% (n=68,610) | 0.01% | +3 [-26, +31] | 57.1% | +0.11 (z +0.69) ~ | +0.15% ±0.37% | +0.30% ±0.66% |
| 67 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | — | off | +6 | -6 | +7 | 16.67% (n=86,251) | 16.66% (n=85,931) | 0.01% | +1 [-16, +17] | 52.5% | -0.19 (z -1.26) ~ | -0.02% ±0.40% | +0.12% ±0.69% |
| 68 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | — | off | -14 | -4 | -33 | 16.67% (n=114,737) | 16.66% (n=114,337) | 0.01% | +1 [-21, +22] | 53.1% | -0.14 (z -0.94) ~ | +0.64% ±0.37% | +0.77% ±0.66% |
| 69 | `lane-space-race` | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be sited in all read the race rather than an explicitly assigned target, and the pass opens at all. | — | off | -6 | +1 | -9 | 16.66% (n=68,403) | 16.67% (n=68,631) | -0.01% | +0 [-19, +19] | 50.7% | -0.11 (z -0.72) ~ | -0.47% ±0.39% | -0.57% ±0.67% |
| 70 | `lane-culture-spending` | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture whose plan has not named the lane. | — | **on** | +5 | -14 | +18 | 16.66% (n=68,513) | 16.67% (n=68,521) | -0.01% | +0 [-22, +22] | 51.0% | +0.13 (z +0.89) ~ | -0.36% ±0.38% | -1.27% ±0.66% |
| 71 | `research-grants-first` | A finished research city pays more for its own district's project. | — | off | +2 | +3 | +20 | 16.66% (n=68,446) | 16.67% (n=68,588) | -0.02% | -2 [-21, +17] | 40.8% | +0.05 (z +0.34) ~ | -0.56% ±0.39% | -0.67% ±0.70% |
| 72 | `lane-congress-ballot` | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion posture that has no lane. | — | off | -15 | +6 | +4 | 16.66% (n=68,602) | 16.68% (n=68,432) | -0.02% | -1 [-20, +19] | 47.8% | +0.14 (z +0.93) ~ | -0.60% ±0.36% | -1.31% ±0.64% |
| 73 | `district-coverage` | Rank district families by how much of the empire still lacks them. | — | off | +7 | -7 | +13 | 16.65% (n=114,531) | 16.68% (n=114,543) | -0.02% | -1 [-17, +15] | 43.8% | +0.19 (z +1.25) ~ | +0.38% ±0.36% | +0.50% ±0.64% |
| 74 | `coupled-expansion` | Enable the evaluator-only paid expansion treatment. | — | off | -6 | +2 | +6 | 16.65% (n=45,027) | 16.68% (n=44,763) | -0.03% | -1 [-26, +23] | 45.5% | +0.12 (z +0.75) ~ | -0.44% ±0.37% | -0.41% ±0.66% |
| 75 | `enhancer-for-the-corps` | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. | — | off | -2 | -10 | +16 | 16.64% (n=68,467) | 16.69% (n=68,567) | -0.05% | -2 [-21, +17] | 42.6% | +0.09 (z +0.57) ~ | -0.04% ±0.41% | -0.13% ±0.73% |
| 76 | `campus-finishes-first` | The Campus coverage term is scaled by how finished the empire's standing Campuses are. | — | off | -6 | +2 | -9 | 16.64% (n=68,410) | 16.69% (n=68,624) | -0.05% | -2 [-21, +17] | 41.7% | +0.13 (z +0.83) ~ | +0.10% ±0.36% | +0.06% ±0.64% |
| 77 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | — | off | +11 | +13 | -26 | 16.64% (n=114,486) | 16.70% (n=114,588) | -0.06% | -7 [-31, +17] | 29.0% | -0.05 (z -0.33) ~ | +0.10% ±0.38% | +0.12% ±0.68% |
| 78 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | — | off | -7 | -4 | +13 | 16.64% (n=114,516) | 16.70% (n=114,558) | -0.06% | -2 [-18, +14] | 38.9% | +0.18 (z +1.20) ~ | -0.05% ±0.39% | -0.26% ±0.71% |
| 79 | `fortify-idle-units` | Fortify units the planner gave nothing to do. | — | off | -9 | +2 | +12 | 16.64% (n=45,018) | 16.70% (n=44,772) | -0.06% | -3 [-27, +21] | 40.4% | -0.11 (z -0.67) ~ | -0.01% ±0.40% | +0.21% ±0.74% |
| 80 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use its direct empire-wide repair. | — | off | +13 | -6 | -1 | 16.63% (n=114,742) | 16.70% (n=114,332) | -0.07% | -3 [-22, +16] | 37.3% | -0.02 (z -0.12) ~ | +0.54% ±0.37% | +0.78% ±0.66% |
| 81 | `condemn-under-congress` | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. | — | off | -6 | +5 | -7 | 16.63% (n=68,782) | 16.70% (n=68,252) | -0.07% | -4 [-23, +15] | 34.0% | +0.01 (z +0.08) ~ | +0.39% ±0.37% | +0.60% ±0.66% |
| 82 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | — | off | +5 | -5 | -7 | 16.63% (n=114,742) | 16.71% (n=114,332) | -0.08% | -4 [-18, +11] | 30.6% | -0.01 (z -0.07) ~ | +0.11% ±0.38% | +0.03% ±0.71% |
| 83 | `research-tier-premium` | A Campus building's debt is scaled by its own Science against the chain's first rung. | — | off | +14 | -18 | +1 | 16.62% (n=68,550) | 16.71% (n=68,484) | -0.09% | -4 [-32, +24] | 39.6% | -0.02 (z -0.10) ~ | +0.42% ±0.39% | +0.69% ±0.67% |
| 84 | `envoy-infrastructure` | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the turn limit, and a first Diplomatic Quarter sees part of the Consulate stream it unlocks. | — | off | +0 | +3 | -29 | 16.62% (n=68,624) | 16.71% (n=68,410) | -0.09% | -5 [-24, +14] | 30.6% | -0.09 (z -0.61) ~ | -0.08% ±0.38% | -0.45% ±0.67% |
| 85 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | — | off | – | – | – | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 86 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | — | off | +1 | -8 | -36 | 16.61% (n=114,489) | 16.72% (n=114,585) | -0.11% | -5 [-19, +9] | 24.1% | -0.32 (z -2.07) hurts * | +0.29% ±0.38% | +0.39% ±0.70% |
| 87 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | — | off | +5 | -4 | -13 | 16.61% (n=114,661) | 16.73% (n=114,413) | -0.12% | -6 [-21, +8] | 20.1% | -0.05 (z -0.34) ~ | +0.66% ±0.36% | +0.93% ±0.67% |
| 88 | `early-contact-window` | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. | — | **on** | +8 | -11 | -34 | 16.61% (n=68,422) | 16.73% (n=68,612) | -0.12% | -8 [-34, +18] | 26.9% | -0.31 (z -2.11) hurts * | +0.10% ±0.37% | -0.04% ±0.67% |
| 89 | `lane-congress-favor` | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. | — | off | -5 | +0 | +7 | 16.60% (n=67,888) | 16.73% (n=69,146) | -0.13% | -7 [-26, +12] | 23.8% | +0.09 (z +0.57) ~ | -0.23% ±0.37% | -0.53% ±0.65% |
| 90 | `culture-coverage` | Pay for the Theater Square the empire has not got. | — | off | +0 | -3 | +0 | 16.60% (n=68,287) | 16.73% (n=68,747) | -0.13% | -7 [-26, +12] | 23.4% | +0.00 (z +0.01) ~ | +0.43% ±0.38% | +0.85% ±0.67% |
| 91 | `tactical-strategy` | Enable explicit battlefield roles: the land-unit counter cycle, safe ranged standoff, wall-focused siege/support, and cavalry job priority. | — | off | -10 | -4 | +23 | 16.59% (n=44,895) | 16.74% (n=44,895) | -0.14% | -5 [-35, +24] | 36.0% | +0.11 (z +0.71) ~ | +0.10% ±0.37% | +0.24% ±0.66% |
| 92 | `barbarian-capture-priority` | Take a visible Barbarian Settler or Scout in exact one-turn reach before healing, retreat, or any ordinary tactical choice. | — | off | -8 | -3 | -10 | 16.59% (n=68,025) | 16.74% (n=69,009) | -0.15% | -7 [-26, +12] | 23.8% | -0.08 (z -0.53) ~ | -0.20% ±0.37% | -0.45% ±0.66% |
| 93 | `congress-banks-decided` | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact prediction and staking nothing. | — | off | -3 | -3 | -6 | 16.59% (n=68,671) | 16.74% (n=68,363) | -0.15% | -8 [-27, +11] | 21.5% | +0.08 (z +0.53) ~ | +0.40% ±0.40% | +0.54% ±0.69% |
| 94 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | — | off | -3 | -20 | -2 | 16.59% (n=114,524) | 16.75% (n=114,550) | -0.16% | -6 [-27, +14] | 27.2% | -0.13 (z -0.82) ~ | +0.62% ±0.38% | +1.25% ±0.69% |
| 95 | `fifteenth-citizen` | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. | — | off | -4 | -6 | +8 | 16.58% (n=68,708) | 16.75% (n=68,326) | -0.17% | -9 [-28, +10] | 18.7% | -0.07 (z -0.46) ~ | +0.48% ±0.38% | +1.26% ±0.68% |
| 96 | `congress-counter-votes` | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fails costs no Favor. | — | off | -11 | -7 | +14 | 16.54% (n=68,516) | 16.79% (n=68,518) | -0.25% | -13 [-32, +6] | 9.5% | +0.12 (z +0.78) ~ | +0.41% ±0.38% | +0.60% ±0.70% |
| 97 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | — | off | +5 | +2 | -6 | 16.54% (n=85,820) | 16.80% (n=86,362) | -0.26% | -14 [-35, +7] | 9.7% | -0.30 (z -1.93) ~ | +0.29% ±0.40% | -0.05% ±0.73% |
| 98 | `science-payback-horizon` | Price the science economy on whether it can still repay rather than on how much of the game is left. | — | off | -6 | -7 | -17 | 16.53% (n=68,638) | 16.81% (n=68,396) | -0.28% | -14 [-33, +5] | 7.6% | -0.05 (z -0.33) ~ | -0.58% ±0.36% | -0.52% ±0.64% |
| 99 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | — | off | +0 | -14 | +15 | 16.52% (n=85,987) | 16.81% (n=86,195) | -0.28% | -15 [-32, +2] | 4.2% | -0.04 (z -0.27) ~ | -0.59% ±0.38% | -0.89% ±0.67% |
| 100 | `governor-expansion-lane` | The other half: the governor under Expansion only. | — | off | -11 | +6 | +11 | 16.52% (n=86,209) | 16.81% (n=85,973) | -0.29% | -15 [-34, +3] | 5.1% | -0.00 (z -0.01) ~ | +0.19% ±0.37% | +0.46% ±0.64% |
| 101 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | — | off | +5 | -17 | -17 | 16.52% (n=114,668) | 16.81% (n=114,406) | -0.30% | -15 [-29, -0] | 2.3% | +0.02 (z +0.12) ~ | -0.57% ±0.37% | -0.78% ±0.65% |
| 102 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | — | off | -10 | -1 | -64 | 16.52% (n=114,657) | 16.82% (n=114,417) | -0.30% | -17 [-40, +6] | 7.1% | -0.50 (z -3.17) hurts * | +0.19% ±0.38% | +0.42% ±0.65% |
| 103 | `district-building-chain` | Make every specialty district owe its own buildings, whatever the lane. | — | off | -15 | +0 | -44 | 16.52% (n=68,247) | 16.82% (n=68,787) | -0.30% | -18 [-46, +9] | 9.7% | -0.31 (z -2.07) hurts * | -0.20% ±0.35% | +0.33% ±0.63% |
| 104 | `one-shot-recovery` | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. | — | off | +4 | -24 | -8 | 16.50% (n=68,624) | 16.83% (n=68,410) | -0.33% | -16 [-42, +9] | 10.9% | -0.18 (z -1.13) ~ | +0.48% ±0.36% | +0.94% ±0.63% |
| 105 | `housing-districts` | Let the baseline governor raise the housing ceiling. | — | off | +3 | -22 | +5 | 16.50% (n=114,317) | 16.83% (n=114,757) | -0.33% | -16 [-32, -0] | 2.5% | +0.09 (z +0.60) ~ | -0.33% ±0.38% | -0.60% ±0.67% |
| 106 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends its turn beside a free 50-gold clear until the camp spawns the archer that kills it. | — | off | -5 | +0 | -20 | 16.47% (n=86,139) | 16.86% (n=86,043) | -0.39% | -24 [-62, +14] | 10.8% | -0.30 (z -1.94) ~ | -0.97% ±0.37% | -1.25% ±0.65% |
| 107 | `escort-unstick-2` | Version 2 of `escort_unstick`: the same two-turn release, refused while a visible barbarian raider can reach the settler's tile. | 1 | off | -8 | – | – | v1 16.95% (n=127,412) · v2 16.34% (n=9,529) | v1 16.32% (n=101,662) · v2 16.78% (n=28,631) | -0.44% | -22 [-64, +21] | 15.8% | -0.06 (z -0.70) ~ | +0.47% ±0.28% | +0.57% ±0.50% |
| 108 | `pantheon-board` | Choose the pantheon from the land the empire holds rather than from a fixed order. | — | off | +3 | -20 | -31 | 16.43% (n=44,774) | 16.90% (n=45,016) | -0.47% | -26 [-65, +12] | 8.7% | -0.30 (z -1.94) ~ | -0.24% ±0.38% | -0.35% ±0.67% |
| 109 | `coordinated-finish` | Admit the friendly-volley extension without the rest of the closed war-half bundle. | — | off | +0 | -20 | -26 | 16.42% (n=44,813) | 16.91% (n=44,977) | -0.49% | -25 [-56, +5] | 5.2% | -0.09 (z -0.60) ~ | -0.19% ±0.37% | -0.06% ±0.67% |
| 110 | `settle-plan-ahead` | Rank a settle site by the cities it leaves room for as well as its own ground, so a Settler stops taking the one plot in a pocket that would have held two. | — | off | -16 | – | – | 16.34% (n=18,965) | 16.99% (n=19,195) | -0.66% | -33 [-70, +4] | 4.2% | -0.27 (z -3.45) hurts * | -0.16% ±0.20% | -0.36% ±0.36% |
| 111 | `research-floor-holds` | The citizen tilt and the beaker floor hold while the research can still pay. | — | off | -20 | -19 | -26 | 16.34% (n=68,547) | 17.00% (n=68,487) | -0.66% | -32 [-51, -13] | 0.0% | -0.31 (z -2.07) hurts * | -0.39% ±0.38% | -0.86% ±0.67% |
| 112 | `chain-tech-lookahead` | The research goal aims at a Campus rung the empire can BUILD, not only one it has already built. | — | off | -15 | -27 | -25 | 16.30% (n=68,305) | 17.03% (n=68,729) | -0.74% | -36 [-55, -17] | 0.0% | -0.19 (z -1.24) ~ | -0.33% ±0.38% | -0.39% ±0.69% |
| 113 | `builder-reward-survey` | Price Builder production by a survey of the work it would do. | — | off | -23 | -24 | -17 | 16.21% (n=45,034) | 17.12% (n=44,756) | -0.91% | -45 [-70, -21] | 0.0% | +0.10 (z +0.66) ~ | +1.47% ±0.37% | +1.88% ±0.63% |
| 114 | `contact-posture` | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answer, or step out of that shooter's envelope. | — | off | -30 | -18 | -2 | 16.20% (n=68,404) | 17.13% (n=68,630) | -0.93% | -47 [-66, -28] | 0.0% | -0.05 (z -0.36) ~ | +0.15% ±0.39% | +0.73% ±0.69% |
| 115 | `naval-production-policy` | Reach for the naval-production discount while hulls are wanted. | — | off | -25 | -43 | -42 | 15.95% (n=44,588) | 17.37% (n=45,202) | -1.42% | -71 [-96, -47] | 0.0% | -0.67 (z -4.42) hurts * | -0.26% ±0.38% | +0.09% ±0.67% |
| 116 | `governor-every-lane` | Run the strategic governor under every lane. | — | off | -86 | -74 | -108 | 15.48% (n=114,363) | 17.85% (n=114,711) | -2.38% | -117 [-194, -40] | 0.1% | -1.42 (z -9.25) hurts * | +0.06% ±0.36% | +0.56% ±0.64% |
| 117 | `governor-victory-lanes` | Half the composite: the governor under the four victory lanes only. | — | off | -55 | -82 | -82 | 15.33% (n=89,855) | 18.01% (n=89,527) | -2.68% | -143 [-237, -50] | 0.1% | -1.41 (z -9.27) hurts * | +0.08% ±0.39% | +0.35% ±0.67% |

## Evidence for future operator selections

The deployment genome is explicitly operator-pinned. The win columns, pooled *Diff*, posterior, and score-share readings below remain useful evidence, but a new source does not promote or demote a gene automatically. To change a default, update the pinned list with an explicit operator decision and regenerate this ledger.

*Posterior (95% CI)* is a random-effects (DerSimonian–Laird) inverse-variance pool of every screen's on−off difference on the win column's scale. It weights each screen by its standard error and carries between-screen disagreement in the interval; `P(>0)` makes the resulting precision visible.

### What the posterior resolves

Of 117 priced genes the interval clears zero for **21 upward** and **6 downward**; **90 straddle zero**. Those are evidence states, not automatic deployment calls.

| Gene | Posterior (95% CI) | P(>0) | Screens | Pinned | Evidence call |
|---|---:|---:|---:|---|---|
| `air-surge` | +101 [+76, +125] | 100.0% | 2 | on | **on** |
| `barbarian-scouts-are-scouts` | +30 [+8, +51] | 99.6% | 5 | on | **on** |
| `bounded-recovery` | +31 [+14, +48] | 100.0% | 5 | on | **on** |
| `buildings-before-projects` | +28 [+5, +51] | 99.2% | 5 | on | **on** |
| `culture-building-debt` | +26 [+1, +51] | 97.8% | 2 | on | **on** |
| `engine-faith-price` | +63 [+25, +101] | 99.9% | 1 | on | **on** |
| `escort-unstick` | +32 [+8, +57] | 99.5% | 5 | on | **on** |
| `founder-temple` | +19 [+1, +38] | 97.8% | 4 | on | **on** |
| `great-person-housing` | +84 [+64, +105] | 100.0% | 3 | on | **on** |
| `idle-faith-patronage` | +26 [+11, +40] | 100.0% | 4 | on | **on** |
| `loyalty-rate-alarm` | +40 [+22, +58] | 100.0% | 5 | on | **on** |
| `maintenance-aware-deck` | +61 [+24, +99] | 99.9% | 1 | on | **on** |
| `opportunistic-war` | +48 [+20, +76] | 100.0% | 3 | on | **on** |
| `peacetime-deterrence` | +18 [+1, +35] | 98.2% | 5 | on | **on** |
| `price-the-suzerainty` | +56 [+18, +93] | 99.8% | 1 | on | **on** |
| `raid-pillage-prizes` | +54 [+25, +83] | 100.0% | 3 | on | **on** |
| `recon-replacement` | +51 [+30, +72] | 100.0% | 5 | on | **on** |
| `recorded-tactical-step` | +17 [+0, +33] | 97.5% | 5 | on | **on** |
| `score-horizon` | +17 [+0, +34] | 97.6% | 5 | on | **on** |
| `settle-sooner` | +35 [+14, +55] | 100.0% | 3 | on | **on** |
| `wide-map-capacity` | +61 [+28, +93] | 100.0% | 5 | on | **on** |
| `builder-reward-survey` | -46 [-83, -9] | 0.8% | 1 | off | **off** |
| `chain-tech-lookahead` | -26 [-50, -3] | 1.5% | 2 | off | **off** |
| `contact-posture` | -57 [-80, -33] | 0.0% | 2 | off | **off** |
| `governor-expansion-lane` | -27 [-47, -7] | 0.4% | 3 | off | **off** |
| `naval-production-policy` | -51 [-88, -13] | 0.4% | 1 | off | **off** |
| `research-floor-holds` | -27 [-51, -4] | 1.2% | 2 | off | **off** |

## The two shapes, apart

`τ` (tau) is the between-screen standard deviation the random-effects pool estimates. It is the statistic that answers *“is 'both columns positive' two confirmations?”*: when screens agree to within their errors it is zero and the pool is the ordinary inverse-variance one; when they do not, it widens the interval instead of averaging two worlds into a confident wrong answer. `POSTERIOR_SHAPES` in `tools/genes.py` says which shapes the published pool admits and is currently `standard, legacy`.

| Shape | Sources | Player seats | Genes priced |
|---|---:|---:|---:|
| standard | 3 | 92,604 | 116 |
| legacy | 7 | 132,440 | 65 |

Genes priced at both shapes. **A row whose two intervals do not overlap is not a gene with one number; it is two instruments disagreeing**, and the pooled column beside it should be read as a warning rather than an answer.

| Gene | legacy | standard | pooled | τ | overlap |
|---|---:|---:|---:|---:|---|
| `amenity-district-path` | +7 [-15, +29] | +17 [-8, +42] | +12 [-5, +28] | 0 | yes |
| `amenity-project-preemption` | +3 [-24, +31] | -5 [-63, +53] | -1 [-26, +25] | 22 | yes |
| `apostle-promotion-by-role` | -4 [-38, +30] | +19 [-6, +44] | +6 [-15, +27] | 15 | yes |
| `army-target-weighs-enemy` | -1 [-37, +35] | +12 [-14, +37] | +5 [-16, +25] | 13 | yes |
| `barbarian-bargain` | +5 [-32, +41] | +24 [-9, +56] | +16 [-5, +38] | 5 | yes |
| `barbarian-hunt` | -86 [-123, -50] | +2 [-22, +26] | -28 [-86, +30] | 48 | **no** |
| `barbarian-ranged-answer` | +14 [-22, +50] | +9 [-16, +34] | +11 [-10, +32] | 0 | yes |
| `barbarian-scouts-are-scouts` | +45 [+21, +68] | +9 [-16, +34] | +30 [+8, +51] | 16 | yes |
| `blind-objective-strength` | +27 [+5, +50] | -9 [-33, +14] | +11 [-10, +31] | 14 | yes |
| `blind-objective-units` | +0 [-22, +22] | +0 [-23, +24] | +0 [-16, +16] | 0 | yes |
| `bounded-recovery` | +28 [+6, +51] | +34 [+10, +59] | +31 [+14, +48] | 0 | yes |
| `builder-barbarian-safety` | +13 [-23, +48] | -1 [-24, +23] | +3 [-16, +23] | 0 | yes |
| `builder-worked-tile-priority` | +24 [-11, +60] | -18 [-42, +6] | -5 [-36, +26] | 21 | yes |
| `buildings-before-projects` | +14 [-8, +36] | +48 [+14, +81] | +28 [+5, +51] | 18 | yes |
| `camp-party` | +35 [+10, +60] | +5 [-37, +48] | +22 [-3, +47] | 21 | yes |
| `civilian-rescue` | -5 [-28, +17] | -1 [-25, +23] | -3 [-20, +13] | 0 | yes |
| `come-ashore` | +11 [-18, +40] | +1 [-24, +27] | +7 [-10, +24] | 0 | yes |
| `district-coverage` | +5 [-23, +34] | -6 [-41, +29] | +0 [-20, +20] | 14 | yes |
| `district-lookahead-settle` | -22 [-57, +14] | -16 [-67, +34] | -19 [-48, +11] | 19 | yes |
| `endgame-war-runway` | -4 [-27, +18] | -5 [-31, +21] | -5 [-21, +11] | 0 | yes |
| `escort-unstick` | +27 [-20, +74] | +37 [+13, +61] | +32 [+8, +57] | 21 | yes |
| `founder-temple` | +29 [-4, +62] | +12 [-13, +38] | +19 [+1, +38] | 0 | yes |
| `garrison-under-fire` | +26 [-16, +68] | +1 [-56, +58] | +15 [-17, +48] | 32 | yes |
| `governor-every-lane` | -15 [-54, +23] | -204 [-265, -144] | -92 [-193, +9] | 114 | **no** |
| `governor-expansion-lane` | -30 [-66, +6] | -25 [-49, -2] | -27 [-47, -7] | 0 | yes |
| `governor-victory-lanes` | +46 [+9, +82] | -193 [-287, -100] | -134 [-273, +6] | 140 | **no** |
| `great-person-housing` | +78 [+42, +114] | +87 [+62, +112] | +84 [+64, +105] | 0 | yes |
| `holy-lane-parity` | +45 [-24, +114] | +16 [-9, +41] | +32 [-6, +71] | 39 | yes |
| `home-defense` | -13 [-35, +10] | -6 [-34, +22] | -10 [-26, +6] | 0 | yes |
| `housing-districts` | -7 [-29, +16] | -17 [-59, +26] | -13 [-30, +5] | 6 | yes |
| `housing-research` | +10 [-26, +45] | -12 [-36, +11] | +0 [-22, +23] | 17 | yes |
| `idle-faith-patronage` | +26 [+9, +44] | +21 [-19, +61] | +26 [+11, +40] | 0 | yes |
| `inquisition-on-threat` | +16 [-16, +47] | +5 [-21, +30] | +9 [-9, +28] | 0 | yes |
| `loyalty-rate-alarm` | +49 [+25, +73] | +28 [+2, +54] | +40 [+22, +58] | 9 | yes |
| `naval-recon` | -1 [-23, +21] | -3 [-40, +34] | -3 [-19, +13] | 0 | yes |
| `one-launch-pad` | +20 [-2, +42] | +2 [-26, +30] | +11 [-5, +28] | 0 | yes |
| `opportunistic-war` | +23 [-14, +59] | +59 [+33, +85] | +48 [+20, +76] | 17 | yes |
| `peacetime-deterrence` | +13 [-12, +38] | +24 [-1, +49] | +18 [+1, +35] | 0 | yes |
| `priced-tile-purchase` | -31 [-66, +5] | -7 [-30, +17] | -14 [-34, +5] | 0 | yes |
| `raid-pillage-prizes` | +30 [-6, +65] | +65 [+35, +96] | +54 [+25, +83] | 18 | yes |
| `recon-replacement` | +53 [+25, +82] | +49 [+7, +90] | +51 [+30, +72] | 14 | yes |
| `recorded-tactical-step` | +15 [-8, +37] | +19 [-6, +44] | +17 [+0, +33] | 0 | yes |
| `relief-targets-the-siege` | +4 [-18, +27] | +17 [-11, +45] | +10 [-7, +26] | 0 | yes |
| `religion-sues-peace` | +19 [-3, +41] | -9 [-33, +15] | +7 [-11, +24] | 7 | yes |
| `score-horizon` | +18 [-4, +41] | +15 [-10, +40] | +17 [+0, +34] | 0 | yes |
| `settle-sooner` | +41 [+5, +76] | +31 [+7, +56] | +35 [+14, +55] | 0 | yes |
| `settler-guard-holds` | +2 [-20, +24] | +6 [-25, +38] | +3 [-13, +19] | 0 | yes |
| `settler-site-agreement` | +10 [-18, +38] | -19 [-43, +4] | -3 [-24, +18] | 15 | yes |
| `settler-target-hysteresis` | +4 [-18, +26] | -2 [-37, +32] | +0 [-16, +16] | 0 | yes |
| `settler-threat-detour` | +50 [+14, +86] | +12 [-13, +38] | +24 [-2, +51] | 14 | yes |
| `siege-commitment` | -11 [-37, +15] | +7 [-16, +31] | -2 [-18, +14] | 0 | yes |
| `siege-is-progress` | -21 [-64, +21] | +6 [-21, +34] | -9 [-36, +18] | 25 | yes |
| `siege-tracks-wall` | +21 [-9, +52] | -8 [-43, +26] | +9 [-15, +32] | 19 | yes |
| `slot-kind-tiebreak` | +10 [-14, +33] | +10 [-15, +35] | +9 [-7, +26] | 0 | yes |
| `stranded-settler-discount` | +13 [-9, +36] | +11 [-30, +51] | +11 [-5, +27] | 0 | yes |
| `strategic-wonders` | +11 [-12, +33] | +7 [-17, +30] | +9 [-7, +25] | 0 | yes |
| `strike-opening` | +19 [-3, +41] | +4 [-21, +29] | +12 [-4, +29] | 0 | yes |
| `theology-for-founders` | -12 [-40, +16] | +14 [-9, +38] | +3 [-15, +22] | 0 | yes |
| `war-economy` | -48 [-185, +88] | +109 [+84, +134] | +13 [-89, +115] | 115 | yes |
| `war-patience` | -0 [-23, +23] | -24 [-48, -1] | -12 [-29, +5] | 4 | yes |
| `war-reinforcement` | +14 [-17, +46] | +21 [-12, +53] | +17 [-4, +37] | 13 | yes |
| `whole-turn-backtrack-guard` | +25 [+3, +47] | -7 [-41, +26] | +12 [-8, +31] | 11 | yes |
| `wide-map-capacity` | +33 [+11, +55] | +98 [+73, +122] | +61 [+28, +93] | 32 | **no** |
| `wonder-ring-settle-value` | +5 [-18, +27] | -1 [-49, +47] | +5 [-12, +22] | 0 | yes |

## Where a direct arm pays: the boundary genes

**The efficient plan is two stage, and it is not a partial foldover.** The whole-genome screen is the efficient way to RANK: `p10` priced 75 genes at ±51 each on 17,574 seat pairs, and the same budget split into 75 single-gene screens of 234 pairs would give ±145 each even at the best pairing gain this repository has measured — 2.84× wider, which is **8× the games** for the same band. A single-gene arm resolves far tighter per pair once it is aimed (`s7`: ±29 on 6,000 pairs at a 3.32× pairing gain, against `p10`'s 1.09×). So the screen ranks and direct arms resolve the boundary. `docs/GENE_SCREEN.md` carries the arithmetic; do not re-derive it into a blocked or partial foldover, which is neither stage — four-gene `s6` resolves ±64 over 6,000 pairs where one-gene `s7` resolves ±29 over the same 6,000.

*Buys* is the expected value of one direct arm of **7,200 seat pairs**, in wins per 10,000 on-arm seats, read against the gene's **pinned** state — so a gene the evidence likes and the genome already plays has little to buy, and a gene the evidence likes that the pinned genome holds off has the whole effect to buy. *Pairs to resolve* is how many matched seat pairs that arm needs before the combined interval clears zero, if it reads the gene's current pooled effect. Both are sized from `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json`, the widest single-gene arm this repository has actually run (27.6 per-column SE at 7,200 pairs) — the conservative end, since a gene that rarely fires cancels far more and resolves tighter.

| Gene | Posterior (95% CI) | P(>0) | Pinned | Buys | Pairs to resolve |
|---|---:|---:|---|---:|---:|
| `garrison-under-fire` | +15 [-17, +48] | 82.6% | off | +15.5 | 68,721 |
| `settlement-gap-target` | +14 [-23, +52] | 77.5% | off | +14.8 | 87,669 |
| `district-planning` | +12 [-25, +49] | 74.1% | off | +12.9 | 125,973 |
| `lane-policy-deck` | +13 [-16, +42] | 80.9% | off | +12.9 | 103,406 |
| `war-economy` | +13 [-89, +115] | 60.1% | on | +12.4 | 116,162 |
| `whole-turn-backtrack-guard` | +12 [-8, +31] | 88.2% | off | +11.8 | 96,625 |
| `one-launch-pad` | +11 [-5, +28] | 91.6% | off | +11.4 | 81,761 |
| `stranded-settler-discount` | +11 [-5, +27] | 90.8% | off | +11.0 | 93,635 |
| `barbarian-ranged-answer` | +11 [-10, +32] | 85.1% | off | +10.9 | 126,996 |
| `blind-objective-strength` | +11 [-10, +31] | 84.9% | off | +10.8 | 131,056 |
| `research-tier-premium` | +10 [-24, +43] | 71.2% | off | +10.3 | 208,314 |
| `strategic-wonders` | +9 [-7, +25] | 85.6% | off | +8.8 | 192,886 |
| `siege-tracks-wall` | +9 [-15, +32] | 76.1% | off | +8.6 | 251,903 |
| `pantheon-board` | +5 [-32, +43] | 61.0% | off | +7.5 | 728,917 |
| `come-ashore` | +7 [-10, +24] | 79.3% | off | +7.0 | 353,593 |
| `science-multiplier-payoff` | +6 [-24, +35] | 64.5% | off | +6.6 | 639,181 |
| `guru-heals-the-corps` | -4 [-56, +49] | 44.7% | off | +5.7 | 1,648,811 |
| `wonder-ring-settle-value` | +5 [-12, +22] | 71.7% | off | +4.9 | 798,478 |
| `army-target-weighs-enemy` | +5 [-16, +25] | 67.3% | off | +4.9 | 903,207 |
| `campus-adjacency-threshold` | +0 [-38, +38] | 50.3% | off | +4.5 | 829,756,465 |
| `coordinated-finish` | -0 [-38, +37] | 49.1% | off | +4.1 | 110,743,705 |
| `holy-site-where-the-threat-is` | -1 [-38, +36] | 47.7% | off | +3.8 | 17,424,370 |
| `builder-barbarian-safety` | +3 [-16, +23] | 62.9% | off | +3.6 | 1,859,339 |
| `enhancer-for-the-corps` | +3 [-21, +26] | 58.6% | off | +3.6 | 2,978,891 |
| `settler-guard-holds` | +3 [-13, +19] | 65.5% | off | +3.4 | 1,871,682 |
| `lane-space-race` | +1 [-22, +25] | 54.3% | off | +2.7 | 12,292,990 |
| `power-the-laboratory` | +1 [-23, +24] | 52.0% | off | +2.2 | 57,902,647 |
| `housing-research` | +0 [-22, +23] | 51.6% | off | +1.9 | 97,478,820 |
| `amenity-project-preemption` | -1 [-26, +25] | 48.2% | off | +1.9 | 62,802,848 |
| `religious-defence-scales` | -0 [-24, +24] | 49.1% | off | +1.8 | 290,353,107 |
| `promote-when-wounded` | +7 [-31, +45] | 64.5% | on | +1.7 | 396,852 |
| `district-coverage` | +0 [-20, +20] | 50.8% | off | +1.5 | 558,338,216 |
| `builder-worked-tile-priority` | -5 [-36, +26] | 37.3% | off | +1.2 | 796,622 |
| `one-shot-recovery` | -2 [-25, +22] | 43.7% | off | +1.1 | 5,846,162 |
| `lane-congress-ballot` | -9 [-45, +27] | 31.5% | off | +1.1 | 249,847 |
| `blind-objective-units` | +0 [-16, +16] | 51.4% | off | +1.1 | 245,127,417 |
| `unit-objective-memory` | +10 [-28, +47] | 69.7% | on | +1.1 | 201,764 |
| `settler-target-hysteresis` | +0 [-16, +16] | 50.7% | off | +1.0 | 1,036,197,883 |
| `barbarian-hunt` | -28 [-86, +30] | 16.9% | off | +0.9 | 20,099 |
| `campus-finishes-first` | -3 [-26, +21] | 41.0% | off | +0.9 | 2,800,136 |
| `coupled-expansion` | -12 [-49, +26] | 26.9% | off | +0.8 | 136,894 |
| `settler-site-agreement` | -3 [-24, +18] | 40.5% | off | +0.6 | 3,159,094 |
| `envoy-infrastructure` | -4 [-28, +20] | 37.3% | off | +0.6 | 1,334,442 |
| `governor-victory-lanes` | -134 [-273, +6] | 3.0% | off | +0.5 | 96 |
| `lane-great-people` | +13 [-22, +48] | 77.0% | on | +0.4 | 103,314 |
| `governor-every-lane` | -92 [-193, +9] | 3.8% | off | +0.4 | 446 |
| `barbarian-capture-priority` | -6 [-30, +18] | 32.0% | off | +0.3 | 615,022 |
| `escort-unstick-2` | -22 [-64, +21] | 15.8% | off | +0.3 | 32,838 |
| `siege-commitment` | -2 [-18, +14] | 40.5% | off | +0.3 | 5,199,392 |
| `unit-cost-efficiency` | +17 [-20, +55] | 81.7% | on | +0.3 | 55,043 |
| `joint-tactics` | -5 [-28, +17] | 32.6% | off | +0.3 | 755,823 |
| `fortify-idle-units` | -18 [-55, +20] | 17.7% | off | +0.2 | 52,153 |
| `competition-victory-points` | +16 [-19, +50] | 81.3% | on | +0.2 | 67,501 |
| `siege-is-progress` | -9 [-36, +18] | 25.3% | off | +0.2 | 222,389 |
| `theology-for-founders` | +3 [-15, +22] | 64.7% | on | +0.2 | 1,662,178 |
| `tactical-strategy` | -21 [-58, +17] | 14.0% | off | +0.1 | 34,566 |
| `early-contact-window` | +8 [-16, +31] | 73.8% | on | +0.1 | 318,670 |
| `congress-banks-decided` | -8 [-31, +16] | 26.2% | off | +0.1 | 321,129 |
| `naval-recon` | -3 [-19, +13] | 35.6% | off | +0.1 | 2,174,265 |
| `apostle-promotion-by-role` | +6 [-15, +27] | 71.4% | on | +0.1 | 520,461 |
| `civilian-rescue` | -3 [-20, +13] | 34.5% | off | +0.1 | 1,836,091 |
| `culture-coverage` | -8 [-32, +15] | 24.5% | off | +0.1 | 268,265 |
| `lane-culture-spending` | +9 [-15, +32] | 75.9% | on | +0.1 | 250,826 |
| `condemn-under-congress` | -9 [-33, +14] | 22.4% | off | +0.1 | 216,182 |
| `religious-units-heal-first` | +9 [-14, +33] | 78.0% | on | +0.0 | 208,321 |
| `district-building-chain` | -13 [-40, +14] | 17.7% | off | +0.0 | 98,805 |
| `fifteenth-citizen` | -10 [-34, +14] | 20.6% | off | +0.0 | 176,078 |
| `research-grants-first` | -10 [-34, +14] | 20.0% | off | +0.0 | 160,824 |
| `endgame-war-runway` | -5 [-21, +11] | 27.5% | off | +0.0 | 778,289 |
| `science-payback-horizon` | -12 [-35, +12] | 16.9% | off | +0.0 | 121,408 |
| `district-lookahead-settle` | -19 [-48, +11] | 10.4% | off | +0.0 | 34,720 |
| `lane-congress-favor` | -12 [-36, +11] | 15.3% | off | +0.0 | 101,961 |
| `holy-lane-parity` | +32 [-6, +71] | 95.0% | on | +0.0 | 5,971 |
| `religion-sues-peace` | +7 [-11, +24] | 77.2% | on | +0.0 | 421,343 |
| `settle-plan-ahead` | -33 [-70, +4] | 4.2% | off | +0.0 | 4,280 |
| `inquisition-on-threat` | +9 [-9, +28] | 83.2% | on | +0.0 | 189,468 |
| `congress-counter-votes` | -16 [-40, +7] | 8.5% | off | +0.0 | 39,981 |
| `spread-campaign-persists` | -17 [-41, +7] | 8.0% | off | +0.0 | 35,582 |
| `relief-targets-the-siege` | +10 [-7, +26] | 87.0% | on | +0.0 | 153,392 |
| `settler-threat-detour` | +24 [-2, +51] | 96.4% | on | +0.0 | 5,630 |
| `barbarian-bargain` | +16 [-5, +38] | 93.3% | on | +0.0 | 32,522 |
| `slot-kind-tiebreak` | +9 [-7, +26] | 87.4% | on | +0.0 | 156,019 |
| `camp-party` | +22 [-3, +47] | 96.1% | on | +0.0 | 8,154 |
| `priced-tile-purchase` | -14 [-34, +5] | 7.8% | off | +0.0 | 50,051 |
| `home-defense` | -10 [-26, +6] | 11.7% | off | +0.0 | 137,225 |
| `war-reinforcement` | +17 [-4, +37] | 94.7% | on | +0.0 | 24,014 |
| `war-patience` | -12 [-29, +5] | 8.8% | off | +0.0 | 81,488 |
| `amenity-district-path` | +12 [-5, +28] | 91.2% | on | +0.0 | 83,886 |
| `housing-districts` | -13 [-30, +5] | 7.6% | off | +0.0 | 62,666 |
| `strike-opening` | +12 [-4, +29] | 92.5% | on | +0.0 | 65,525 |

The top 8 that one batch could actually resolve (≤ 60,000 seat pairs each), as an argument list:

```sh
gene_screen --genes barbarian-hunt,governor-victory-lanes,governor-every-lane,escort-unstick-2,unit-cost-efficiency,fortify-idle-units,tactical-strategy,district-lookahead-settle
```

`python3 tools/genes.py boundary` prints this list on its own, with `--arm-pairs` and `--max-arm-pairs` to size it.

## Lane genes and the share axis

At the standing 250-turn Online clock a **science or congress gene cannot pay through the win axis at all**: science and diplomatic victories land at median t283 and t285, past the clock, so they are 1–2% of endings and `docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for exactly that reason. The seat a lane gene would have carried to a science victory shows up as a score win or a score loss instead. The decision axis stays WINS — `docs/GENOME.md` records what happened the one time selection ran on a correlate — so the share reading is a **pre-registered secondary**, fixed in `docs/GENE_SCREEN.md` before the next screen rather than chosen after it.

The set is discovered from the code: every gene whose flag field `src/ai/advanced/victory_lane.rs` reads. A gene joins it by being a lane gene, not by being listed here.

| Lane gene | Default | ± Wins / 10k seats | Share Δpp (z) | Posterior (95% CI) | Status |
|---|---|---:|---|---:|---|
| `lane-congress-ballot` | off | -29 | -0.09 (z -1.17) ~ | -9 [-45, +27] | unresolved |
| `lane-congress-favor` | off | -10 | +0.08 (z +1.02) ~ | -12 [-36, +11] | unresolved |
| `lane-great-people` | **on** | +33 | +0.10 (z +1.26) ~ | +13 [-22, +48] | unresolved |
| `lane-policy-deck` | off | +29 | +0.08 (z +1.07) ~ | +13 [-16, +42] | unresolved |
| `lane-culture-spending` | **on** | +9 | -0.01 (z -0.13) ~ | +9 [-15, +32] | unresolved |
| `lane-space-race` | off | -12 | -0.10 (z -1.32) ~ | +1 [-22, +25] | unresolved |
| `competition-victory-points` | **on** | +35 | +0.04 (z +0.46) ~ | +16 [-19, +50] | unresolved |

## Awaiting measurement

These screenable genes have no on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a screen is pending.

| Gene | Default | Description | Best version |
|---|---|---|---:|
| `fog-honest` | off (unmeasured) | Put this controller behind the turn-level fog boundary. | — |
| `fog-honest-2` | off (unmeasured) | Version 2 of `fog_honest`: the same fair-play boundary and the same information contract, plus one re-plan when the authoritative board refuses a planned order. | — |
| `missionary-evades-raiders` | off (unmeasured) | A religious unit steps out of the tiles a visible barbarian raider can reach next turn, and never steps into them on the way to anything, holding when no safe step makes progress. | — |
| `missionary-last-charge-explores` | off (unmeasured) | A Missionary on its last charge explores the fog within ten tiles for up to twelve turns before spending it, unless a city of ours is slipping or an untouched city stands beside it. | — |
| `wonder-score-tally` | off (unmeasured) | A wonder lane any civilization can reach on merit: the `Item::Wonder` arm learns the fifteen points `Game::score_parts` pays for a finished wonder, under a density bar and the live race's own development guards. | — |

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

Every screenable heuristic gene on the Advanced controller, ranked by the displayed pooled *Diff* from highest to lowest (alphabetically by tag on a tie). Each batch header carries its actual player-seat count once; cells show the enabled arm's excess projected to 10,000 **total** player seats, where a six-player chance expectation is 1,667 wins. A dash means that batch did not screen the gene. The *Total* win-rate columns pool the displayed observations and retain their real per-gene on/off seat counts in every row. *Diff* is that display total's on rate minus off rate, in percentage points. *Default* is the explicit operator-pinned deployment selection: sources and display batches update evidence, not defaults. Screenable genes awaiting every displayed measurement are listed separately below without a rank.

**Versioned genes.** An improvement to a gene is a new gene `<base>-<n>` (`docs/GENE_SCREEN.md`, *Versioning a gene*), priced on its own row: a version's *on* is the seats that played that version, and every other seat — off, or a sibling version on — is its *off*. *Best version* names the family's best version (`1` is the original) on every row of the family: the pinned version, else the priced version with the highest tracked wins. A versioned row's *Total (on)* and *Total (off)* cells show the best two versions' rates side by side, best first, each with its own `n`; `—` marks a gene with no versions.

**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected count is 1,667 wins per 10,000 total seats. The batch cells are the enabled arm's excess over that chance rate, scaled from actual completed seats; they do not invent games or seats. The independent latest batch can have unequal on/off arms, which is why the pooled *Total (on)* and *Total (off)* cells retain their own `n` on every row.

**Batch provenance.** The newest displayed batch is the completed current-standard 6-major Continents screen (74×46, nine city-states, Online speed through turn 250, all six victory lanes, shuffled civilizations and best-genome baseline). Its completed seats update the published evidence only; the operator-pinned default does not move until explicitly edited. Older displayed batches remain visible for trend context.

**What each screen resolves.** The median gene’s column standard error times 2.8 — a two-sided 5% test at 80% power. Judge a column against the band of the screen named beside it, never against a single number for the instrument: these differ by more than three to one.

*Pairing gain* is how far a screen’s error per pair sits below the unpaired baseline, and it is what separates them. A foldover cancels only to the extent its two arms play a similar game, so the gain reads on the **genes**, not the design — a gene that rarely fires leaves most pairs identical and cancels almost everything, while a whole-genome screen flips every gene between arms and cancels almost nothing. ⚠ Gene count is not the driver, though the rows below invite that reading — the falsifier is in them. `h1` carries **one** gene over **14,400 player seats** and resolves ±68 at a 1.28× gain, *wider* than four-gene `s6` over 12,000 seats. Its gene changes nearly every game; `s7`'s rarely fires. That, not the count, is the difference.

| Screen | Shape | Genes | Player seats | 1 SE | ±80% power | Pairing gain |
|---|---|---:|---:|---:|---:|---:|
| `2026-08-24-standard-continuous-38160-total-seats.json` | standard | 116 | 38,160 | 19.1 | ±54 | 0.00× |
| `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` | standard | 1 | 7,200 | 39.1 | ±109 | 1.12× |
| `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` | standard | 99 | 47,244 | 15.6 | ±44 | 1.10× |
| `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` | legacy | 1 | 14,400 | 24.3 | ±68 | 1.28× |
| `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` | legacy | 75 | 35,148 | 18.3 | ±51 | 1.09× |
| `2026-08-21-p7-native-6p-allseats-15000-pairs.json` | legacy | 57 | 30,000 | 19.9 | ±56 | 1.08× |
| `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` | legacy | 1 | 12,000 | 10.3 | ±29 | 3.32× |
| `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` | legacy | 4 | 12,000 | 22.9 | ±64 | 1.49× |
| `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` | legacy | 1 | 2,000 | 36.1 | ±101 | 2.68× |
| `2026-08-20-p4-native-6p-allseats-13446-pairs.json` | legacy | 64 | 26,892 | 21.5 | ±60 | 1.06× |

**Posterior (95% CI), P(>0), Share Δpp (z).** *Posterior* is a random-effects (DerSimonian–Laird) inverse-variance pool of **every** screen that priced the gene, on this column's own scale: each screen's on−off difference weighted by its own standard error, with the between-screen disagreement (τ) carried in the interval instead of assumed away. It is the answer to two things the columns cannot express — that the same +24 means different things from a ±29 screen and a ±64 one, and that two positive columns from screens differing in baseline, build and shape are not two confirmations (#2283/#2284 measured that: five of seven lane genes changed sign on disjoint seeds). *P(>0)* is where the shrinkage lands. *Share Δpp (z)* is the newest screen's score-share contrast and its verdict, published beside the win columns because a lane gene can fail to pay on the win axis at 250 turns. **None of these three automatically decides a default**; they are evidence for a later explicit operator selection.

**Cost.** Positive is slower; negative is faster. *cost (compute)* is the on/off percent change in wall seconds per completed turn, while *cost (time)* is the percent change in whole-game wall seconds and therefore includes games that end earlier or later. Each cell is the newest estimate ± one standard error. The screen derives both from paired log-ratios on the same maps, fits every randomized gene together with an arm-order intercept, and keeps one timing per game pair; all-seats signs are summed so the answer is the incremental cost of enabling one major's genome. This reuses the screen's existing `secs` and `turn` rows — no hot-path timers and no extra profiling games. A dash means the source analysis predates the estimator and is unknown, never zero.

Regenerate with `python3 tools/genes.py write` after every screen enters the ledger; `tools/test_genes.py` fails when this file is older than the ledger's sources.

## Follow-ups

## Historical screen evidence and current selection

The observations below remain useful, but they no longer implement a deployment rule. As of 2026-08-24, `OPERATOR_DEFAULT_ON` is the complete operator-pinned selection: a source refresh updates evidence only, while a default changes only through an explicit edit to that list. References to a promotion, veto, or deciding column describe the former process, not a current automatic action.

**The first standard-shape screen supplied the clearest evidence that legacy and standard screens are different instruments.** A 74x46 Continents / 9 CS / Online-250 all-seats foldover against the best-genome baseline -- 3,937 complete map pairs, **23,622 matched seat comparisons per gene**, seeds 141000000-141003936, source `b3ad9f00` -- is published in `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` (PR #2323). It is recorded in `docs/gene_screens/2026-08-22-standard-10k-6p-allseats-23622-pairs.json`. `tools/test_genes.py::TheStandardScreen` checks the historical hand-read figures against that source, so the note and the evidence cannot drift apart. The screen predates the build stamp of #2331, so the ledger records it as `pre-fingerprint`; its gene set was verified by hand against the then-current registry.

**`governor-victory-lanes` is the clearest historical correction.** It had previously been enabled by the retired single-column policy. At the deployment shape the whole-genome screen read **-4.73 pp, z -15.37** -- **-237 wins per 10,000 on-arm seats, 95% CI [-267, -206]** -- and a pre-registered direct arm confirmed **-4.78 pp at win z -6.11**. The legacy and standard readings do not agree, so their random-effects pool widens rather than manufacturing certainty. The current pinned selection keeps this gene off; the evidence explains that operator choice but does not mechanically make it.

*The legacy share axis already showed the disagreement.* P10 priced this gene at win z **+2.46** and score-share z **-15.92** -- a recorded `conflict`. The later standard win reading was z -15.37, within half a sigma of that legacy share reading. This is evidence for publishing both axes, not a rule that either axis may silently rewrite a default.

*The composite, decomposed.* `governor-every-lane` (the composite) read **-4.68**, `governor-victory-lanes` (the victory-lane half) **-4.73**, and `governor-expansion-lane` **-0.55**. The victory half carried essentially the whole of the composite's harm. All three are currently pinned off.

*Why automatic promotion and demotion were retired.* The eight former pooled-*Diff* candidates included two strong standard-shape signals (`governor-victory-lanes` and `war-economy`) and six readings with |z| about one. Standard-only posterior intervals excluded zero for the two signals and straddled it for the other six; the cross-shape pool resolved none because of between-shape disagreement. Those intervals remain decision evidence for a future explicit operator review. They are not a fallback policy.

*What survives the change of instrument.* `great-person-housing` +78 -> +94, `raid-pillage-prizes` +30 -> +53 and `opportunistic-war` +23 -> +49 read positive at both shapes and pool tighter, not wider; `wide-map-capacity` goes +33 -> +90. Rank 1 does not: `holy-lane-parity`, +99 on legacy and confirmed by its own direct arm, reads **+20 [-12, +51]** at the deployment shape. Two genes the legacy ledger has never priced arrive resolved -- `air-surge` **+108 [+77, +138]** and `contact-posture` **-54 [-85, -24]**.


⚠ **One debt the fires gate just wrote off, recorded here so it does not vanish.** Entering this screen proved all seventeen remaining entries in `tools/gene_fire_waivers.json` and emptied the file: every gene in the tables has now been shown to fire by a committed screen row, which is the ratchet's own definition of proof. Sixteen of those are real -- genes that predated the gate and had simply never been screened. The seventeenth is not. `competition-victory-points` prices a scored competition's first place by the Diplomatic Victory Points it pays, and **it cannot fire in a screen at all**: `GameOptions::native_competitions` is `false` (`src/game.rs`), `src/bin/gene_screen.rs` never sets it, and `Game::open_competition` returns early without it -- so no competition is ever scored. Its own single-gene probe (`docs/gene_screens/fires/competition-victory-points.json`) is duly zero-width. What cleared its waiver is a **-0.01 pp / z -0.03** row in this 23,622-seat multi-gene screen: noise from the other ninety-eight genes' arms, which `tools/gene_fires.py`'s own header already warns is "proven for a weaker reason" because a non-zero contrast on a multi-gene screen is not attributable to the gene. The gate is right that the list can only shrink and wrong about this one row; a zero-width **single-gene probe** ought to outweigh a non-zero multi-gene one, and until it does the debt lives here. The gene has held a genome bit since #2274, consumes a column in every screen, and returns nothing. The resolution is unchanged: enable competitions in the screen, or take the gene out of the tables.

**`holy-lane-parity` came back and was confirmed directly.** It left the code with #2266's bottom ten on a **-27** reading from the four-gene `s6` screen, whose column band is +/-64 -- a null, not a measurement against the gene. P10 then read **+63** at z +3.48, and #2299 restored the code and ran a direct arm: **+99 wins/10k, z +4.05, 95% CI [+51, +147]** (`HELPS **`). The standard screen later read **+19 [-12, +51]**, unresolved at the deployment shape. This history explains why `holy-lane-parity` remains in the pinned selection; its current on state no longer follows a pair of columns. See [the confirmation](docs/eval/2026-08-22-holy-lane-parity-direct-confirmation.md).

**Direct follow-up.** This is a ranking screen, not a promotion queue. The subsequent [P9 direct confirmation](docs/eval/2026-08-21-current-genome-settler-guard-direct-confirmation.md) held every other deployment gene fixed and flipped only `settler-guard-holds` across 300 maps / 1,800 treated-seat pairs. It measured exactly **+0.0 pp** on wins and score share; the flag remains unresolved and off. Its +13 row below is retained as historical p7 screen output, not a current recommendation.

**P10 ended early at the operator's request.** Its 2,929 complete map seeds provide 5,858 controlled games and 17,574 treated-seat pairs; the analysis excludes 11 interrupted one-arm seeds (66 raw seat rows), with zero duplicate or invalid tuples. The table keeps up to three chronological windows so a reader can judge trend and disagreement. All of them are evidence only under the pinned policy. P10 used the 6p all-six native regime on seeds 100000000–100002962, 60×38 pangaea/online/250 turns, shuffled civilizations, every major seat treated, and foldover against the best-genome baseline.

**The bottom of the table was not culled, because the standard screen does not agree with it.** #2330 was launched to remove `barbarian-hunt` under the standing directive that the bottom of the ranking leaves the code. Its row here is the worst in the table -- **-86 wins/10k seats, -1.73 pp, win z -4.65, share z -7.54**, family-wise on P10's own bar, and P10 replicates it internally (its two tranches read -2.08 and -1.27). Nothing about that reading is marginal, and on the instrument that produced it the cull rule fires cleanly. It is still the wrong action, for one reason: **that instrument is `legacy`, and the standard one measured the same gene and disagreed.** [`docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`](docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md) -- source `b3ad9f00`, 3,937 complete map pairs, **23,622 matched seat comparisons per gene** against P10's 17,574, on 74x46 Continents with nine city-states -- reads `barbarian-hunt` at **+0.20 pp, win z +0.65, +10 wins/10k seats**. The sign is opposite and the magnitude is a tenth. This is not the two screens failing to resolve the same small effect: `governor-victory-lanes` at -4.73 pp / z -15.37 in that file puts its standard error near **0.31 pp** on the difference, so P10's -1.73 pp sits about **six standard errors** from what the standard screen measured. One of the two instruments is wrong about this gene, and the paragraph above this one says which of them prices a gene at all: *a gene is only priced at the screen once a `standard` row appears beside it.* Culling on a legacy column contradicts this document's own reading rule.

The legacy board has a second witness and it agrees with the standard screen, not with P10. [The direct arm](docs/eval/2026-08-21-barbarian-hunt-direct.md) held every other treatment at the deployment genome and varied only this gene across 300 map pairs / 1,800 clustered seat-pairs: **+0.56 pp, z +0.51**. That run resolves only +/-3.1 pp at 80% power, so on its own it cannot refute -1.73 -- but it is the third reading in a row whose sign is positive, and it is the arm #2299 used to settle exactly this question for `holy-lane-parity`.

So the gene stays in the code, `off` and unresolved, and **the cull rule does not fire on a `legacy` column**. What would decide it: a `barbarian-hunt` row from a **`standard`-shape screen entering `docs/gene_ledger.json`**. If that row is below the screen's own column band with the sign P10 gave it, the gene leaves the code with the standard number recorded beside it; if it lands where the 23,622-seat screen already put it, P10's -86 was an artefact of the 60x38 Pangaea room -- where 48% of games ended in a religious conversion against 8% at the standard shape -- and the row is a null that never licensed anything.

⭐ **That row landed on 2026-08-23, and it was the second answer.** The screen is now a ledger source and `barbarian-hunt` reads **+10 [-21, +41]** in the newest window against **-86** in the one behind it -- a null at the deployment shape, well inside the screen's own band, with the opposite sign to the reading the cull was launched on. The gene stays. The cull rule is now satisfied on its own terms rather than held off on a technicality, and the third window is what keeps the -86 visible beside the +10 instead of dropping it out of the table the moment a fourth screen prices the gene.

`lane-congress-ballot` was the same question and gets the same answer for a different reason. [`docs/VICTORY_GENES.md`](docs/VICTORY_GENES.md) §8 records it negative in every window of both regimes and reaching `share hurts *` at z -2.31, and says in terms that if a dedicated arm confirms it *"the gene should leave the code by the same rule that culled the bottom of the ranking"*. That rule is about the bottom of the **ranking**, and an unmeasured gene has no rank -- it is in *Awaiting measurement* above, not in the table -- so nothing was licensed even before the new evidence. The new evidence closes it anyway: the same standard screen reads `lane-congress-ballot` at **+0.16 pp, win z +0.51**, a null. The confirming arm §8 asked for would now have to overturn a 23,622-seat reading, not merely add to five small ones.

⚠ **This is the third and fourth time, and the pattern is now the finding.** #2266 removed ten genes at once against a band that was twice too wide -- the +/-110 it quoted is the *difference*'s band, and the column beside it resolves half that (#2300). `holy-lane-parity` was one of the ten, left on a -27 null, was priced at **+63** by a screen already running when the cull merged, was restored in #2299 and confirmed at **+99, z +4.05** in #2307. Now `barbarian-hunt` and `lane-congress-ballot`: proposed for removal on a legacy column and a five-window trend, and re-priced as nulls by the standard screen before either could be cut. Four genes, three episodes, one mechanism -- **a reading from the wrong instrument, acted on irreversibly.**

So the rule, for whoever culls next. A cull is not the symmetric opposite of a default. A gene left `off` costs one row in a foldover screen and **no games**, and it can be re-priced by every screen that runs afterwards; a gene removed can never be re-priced by anything, and restoring it costs a dedicated confirmation run (1,200 map pairs for `holy-lane-parity`). So the bar for deleting code is not "the worst reading available" -- `barbarian-hunt`'s -86 was the worst reading in the table and it was still the wrong number. It is **a reading on the instrument the agent is actually being screened on**, and the three questions that establish it: is this column `standard` or `legacy`; is there a screen in flight or unmerged that has already priced this gene (check `batch.source_commit` against the cull date, and check the open pull requests); and does a direct arm against the deployment genome agree. `barbarian-hunt` failed all three.

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 26,892 seats), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 2,000 seats), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 30,000 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 35,148 seats), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 14,400 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 47,244 seats), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 7,200 seats), `2026-08-24-standard-continuous-38160-total-seats.json` (standard, 38,160 seats). The fixed display batches are: `2026-08-24-standard-continuous-38160-total-seats.json` (38,160 seats), `2026-08-23-standard-gene-screen-41628-total-seats.json` (41,628 seats), `2026-08-23-standard-gene-screen-10000-total-seats.json` (10,002 seats). The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells are the operator's wins-per-ten-thousand-total-seat reporting view._
