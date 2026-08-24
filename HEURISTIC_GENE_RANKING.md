# The heuristic gene ranking

**Deployment default:** operator-pinned (52 genes): retains the prior 36 selections and explicitly promotes `unit-cost-efficiency`, `unit-objective-memory`, `camp-party`, `slot-kind-tiebreak`, `promote-when-wounded`, `religion-sues-peace`, `lane-great-people`, `one-launch-pad`, `civilian-rescue`, `missionary-evades-raiders`, `district-planning`, `missionary-last-charge-explores`, `settlement-gap-target`, `religious-defence-scales`, `lane-policy-deck`, `science-multiplier-payoff`. Screen columns, *Diff*, and posterior values are evidence only; new batches do not automatically change defaults.

| Rank | Gene | Description | Best version | Default | Wins ± /10k total seats — Last Batch (n=5,988 total seats) | Wins ± /10k total seats — Prior Batch (n=4,266 total seats) | Wins ± /10k total seats — Third Batch (n=21,030 total seats) | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `air-surge` | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. | — | **on** | +30 | +25 | +34 | 17.30% (n=75,719) | 15.50% (n=40,969) | 1.80% | +97 [+76, +119] | 100.0% | +0.38 (z +3.26) helps * | +0.52% ±0.31% | +0.47% ±0.47% |
| 2 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate works to make room. | — | **on** | +29 | +52 | +40 | 17.30% (n=93,222) | 15.66% (n=58,614) | 1.64% | +88 [+69, +106] | 100.0% | +0.47 (z +3.98) helps * | +0.41% ±0.29% | +0.57% ±0.45% |
| 3 | `wide-map-capacity` | Price the city ceiling off uncontested land. | — | **on** | +41 | +23 | +26 | 17.17% (n=121,713) | 15.96% (n=87,015) | 1.21% | +64 [+38, +90] | 100.0% | +0.29 (z +2.44) helps * | +0.28% ±0.29% | +0.21% ±0.44% |
| 4 | `one-war-at-a-time` | Fight one war at a time: keep one campaign front and sue every other major for peace, hold a fresh declaration while a war is on, press the front while a city is breaking or tiles are in reach to pillage, and offer peace once the exchange has run against us for long enough with nothing left to take. | — | off | +21 | – | – | 17.49% (n=1,498) | 16.39% (n=4,490) | 1.10% | +55 [-54, +163] | 83.9% | -0.13 (z -0.59) ~ | +0.14% ±0.87% | +0.79% ±1.14% |
| 5 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | — | **on** | +11 | +35 | +35 | 17.09% (n=93,221) | 15.99% (n=58,615) | 1.10% | +59 [+37, +81] | 100.0% | +0.42 (z +3.59) helps * | -0.31% ±0.31% | -0.28% ±0.46% |
| 6 | `price-the-suzerainty` | Let the envoy scorer see the suzerainty it is walking toward. | — | **on** | -1 | +8 | +30 | 17.13% (n=37,448) | 16.13% (n=31,996) | 0.99% | +51 [+23, +80] | 100.0% | +0.38 (z +3.68) helps * | +0.06% ±0.26% | +0.28% ±0.41% |
| 7 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | — | **on** | +35 | +27 | +24 | 17.08% (n=121,681) | 16.09% (n=87,047) | 0.99% | +52 [+36, +68] | 100.0% | +0.17 (z +1.41) ~ | +0.23% ±0.30% | +0.08% ±0.46% |
| 8 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for peace. | — | **on** | +35 | +23 | +27 | 17.05% (n=93,215) | 16.06% (n=58,621) | 0.98% | +52 [+33, +71] | 100.0% | +0.49 (z +4.24) helps * | -0.15% ±0.28% | -0.15% ±0.44% |
| 9 | `maintenance-aware-deck` | Let the deck counterfactual see the unit-maintenance bill. | — | **on** | -22 | -2 | +22 | 17.07% (n=37,270) | 16.20% (n=32,174) | 0.87% | +34 [-8, +77] | 94.4% | +0.33 (z +3.11) helps * | +0.58% ±0.26% | +0.84% ±0.41% |
| 10 | `engine-faith-price` | THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE. | — | **on** | +28 | -20 | +5 | 17.04% (n=37,410) | 16.23% (n=32,034) | 0.81% | +36 [-7, +79] | 95.0% | -0.06 (z -0.59) ~ | -0.28% ±0.25% | -0.42% ±0.38% |
| 11 | `missionary-last-charge-explores` | A Missionary on its last charge explores the fog within ten tiles for up to twelve turns before spending it, unless a city of ours is slipping or an untouched city stands beside it. | — | **on** | +20 | +12 | – | 17.13% (n=3,696) | 16.41% (n=6,558) | 0.72% | +39 [-40, +117] | 83.1% | -0.22 (z -1.01) ~ | -0.37% ±0.60% | +0.23% ±0.85% |
| 12 | `escort-unstick` | Release an escort that is not walking its settler. | 1 | **on** | +46 | -25 | +12 | v1 16.99% (n=103,891) · v2 16.67% (n=17,695) | v1 16.35% (n=104,837) · v2 16.67% (n=51,749) | 0.65% | +31 [+9, +53] | 99.7% | +0.10 (z +0.99) ~ | +0.00% ±0.30% | +0.12% ±0.46% |
| 13 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | — | **on** | +18 | +11 | -11 | 16.93% (n=121,487) | 16.30% (n=87,241) | 0.64% | +34 [+14, +53] | 100.0% | +0.14 (z +1.14) ~ | +0.03% ±0.30% | -0.23% ±0.46% |
| 14 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good enough to pay for its walk. | — | **on** | -3 | +56 | +6 | 16.91% (n=93,409) | 16.28% (n=58,427) | 0.63% | +34 [+15, +53] | 100.0% | +0.01 (z +0.11) ~ | +0.47% ±0.30% | +0.74% ±0.46% |
| 15 | `war-economy` | Send an adaptive Conquest plan through the war production path. | — | **on** | -12 | +41 | +32 | 16.92% (n=121,873) | 16.31% (n=86,855) | 0.61% | +27 [-50, +104] | 75.4% | +0.82 (z +6.74) helps * | +0.93% ±0.30% | +1.07% ±0.46% |
| 16 | `shoot-and-scoot` | A ranged unit inside a hostile melee body's reach steps to a firing tile inside strictly fewer hostile envelopes and fires at that body, in war and against barbarians. | — | off | +11 | – | – | 17.10% (n=1,532) | 16.52% (n=4,456) | 0.58% | +29 [-79, +137] | 70.2% | -0.18 (z -0.83) ~ | -0.08% ±0.88% | -0.20% ±1.19% |
| 17 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | — | **on** | +4 | +27 | +10 | 16.90% (n=121,900) | 16.34% (n=86,828) | 0.56% | +29 [+12, +47] | 100.0% | +0.09 (z +0.75) ~ | +0.14% ±0.30% | +0.37% ±0.47% |
| 18 | `district-planning` | The city plans its districts, sites and tile buys together: wished districts get jointly assigned, reserved plots over rings 1-3, and the tile a very valuable site needs is bought. | — | **on** | +8 | +33 | +26 | 16.96% (n=33,313) | 16.40% (n=36,131) | 0.56% | +28 [+0, +56] | 97.6% | +0.17 (z +1.62) ~ | -0.11% ±0.25% | -0.06% ±0.38% |
| 19 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | — | **on** | +25 | -15 | +14 | 16.89% (n=121,784) | 16.35% (n=86,944) | 0.54% | +30 [+12, +47] | 99.9% | +0.30 (z +2.46) helps * | +0.29% ±0.30% | +0.59% ±0.46% |
| 20 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | — | **on** | -14 | +8 | +8 | 16.89% (n=121,637) | 16.35% (n=87,091) | 0.54% | +29 [+13, +44] | 100.0% | +0.11 (z +0.92) ~ | -0.10% ±0.30% | +0.05% ±0.46% |
| 21 | `holy-lane-parity` | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | — | **on** | +18 | -47 | -5 | 16.86% (n=106,445) | 16.38% (n=71,791) | 0.48% | +20 [-15, +54] | 86.6% | -0.05 (z -0.42) ~ | -0.21% ±0.28% | -0.29% ±0.42% |
| 22 | `pantheon-board` | Choose the pantheon from the land the empire holds rather than from a fixed order. | — | off | +83 | -19 | +14 | 16.91% (n=33,102) | 16.44% (n=36,342) | 0.47% | +43 [-28, +114] | 88.4% | +0.06 (z +0.60) ~ | +0.02% ±0.26% | -0.02% ±0.40% |
| 23 | `religious-veto-defence` | The religious defence grows with how much of a rival's religious victory is already done — every civilization is a veto on it — naming and targeting the stakes faith from half a victory and spending on it from match point; the Inquisitor walks to the heresy instead of spending its charges where it was bought. | — | off | +8 | – | – | 16.97% (n=1,538) | 16.56% (n=4,450) | 0.41% | +20 [-89, +130] | 64.3% | +0.56 (z +2.46) helps * | +1.25% ±0.87% | +2.13% ±1.12% |
| 24 | `camp-party` | The peacetime camp party. | — | **on** | +20 | -8 | +13 | 16.87% (n=106,872) | 16.46% (n=101,856) | 0.41% | +22 [+3, +41] | 98.8% | +0.06 (z +0.56) ~ | +0.22% ±0.25% | +0.68% ±0.40% |
| 25 | `settlement-gap-target` | Make the settlement-gap redirect and the Settler ranking honour the same city target the cascade settles toward. | — | **on** | +11 | +29 | +11 | 16.88% (n=33,286) | 16.47% (n=36,158) | 0.41% | +21 [-7, +49] | 92.8% | +0.24 (z +2.35) helps * | +0.24% ±0.26% | +0.29% ±0.40% |
| 26 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | — | **on** | +30 | +28 | +15 | 16.83% (n=121,717) | 16.43% (n=87,011) | 0.40% | +21 [+5, +37] | 99.5% | +0.06 (z +0.46) ~ | -0.13% ±0.29% | -0.17% ±0.45% |
| 27 | `missionary-evades-raiders` | A religious unit steps out of the tiles a visible barbarian raider can reach next turn, and never steps into them on the way to anything, holding when no safe step makes progress. | — | **on** | -6 | +30 | – | 16.91% (n=3,636) | 16.53% (n=6,618) | 0.38% | +20 [-59, +98] | 69.1% | +0.38 (z +1.65) ~ | +1.01% ±0.63% | +0.63% ±0.84% |
| 28 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | — | **on** | +36 | -51 | +19 | 16.81% (n=99,460) | 16.44% (n=64,376) | 0.37% | +24 [-2, +49] | 96.6% | +0.24 (z +1.98) ~ | +0.21% ±0.30% | +0.47% ±0.48% |
| 29 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | — | **on** | +38 | +22 | +4 | 16.82% (n=121,588) | 16.46% (n=87,140) | 0.36% | +19 [+3, +35] | 99.0% | +0.02 (z +0.18) ~ | +0.06% ±0.29% | +0.22% ±0.46% |
| 30 | `barbarian-bargain` | Price a raider's life below a major's. | — | **on** | +24 | +17 | +5 | 16.80% (n=93,378) | 16.45% (n=58,458) | 0.35% | +18 [-1, +37] | 96.9% | -0.18 (z -1.48) ~ | -0.31% ±0.29% | -0.50% ±0.45% |
| 31 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | — | **on** | -7 | -4 | -13 | 16.80% (n=99,169) | 16.46% (n=64,667) | 0.34% | +20 [+3, +36] | 99.1% | -0.09 (z -0.76) ~ | -0.19% ±0.28% | -0.52% ±0.43% |
| 32 | `deals-for-our-gain` | A quote is chosen by our own net value instead of the most balanced exchange on the board (`min(our gain, their gain)`), which threw away the ordering `Game::quick_deals` already produced by our gain. | — | off | +6 | – | – | 16.91% (n=1,526) | 16.58% (n=4,462) | 0.32% | +16 [-92, +124] | 61.5% | -0.26 (z -1.17) ~ | -1.15% ±0.87% | -1.94% ±1.14% |
| 33 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | — | **on** | +26 | -5 | -3 | 16.80% (n=121,644) | 16.48% (n=87,084) | 0.32% | +17 [+1, +33] | 98.1% | +0.02 (z +0.19) ~ | -0.35% ±0.30% | -0.25% ±0.46% |
| 34 | `promote-when-wounded` |  | — | **on** | +38 | -10 | +11 | 16.81% (n=37,294) | 16.50% (n=32,150) | 0.31% | +17 [-12, +45] | 87.4% | +0.15 (z +1.39) ~ | -0.26% ±0.26% | -0.60% ±0.40% |
| 35 | `religious-defence-scales` | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. | — | **on** | -12 | +20 | +41 | 16.82% (n=57,012) | 16.52% (n=59,676) | 0.31% | +20 [-18, +58] | 85.0% | +0.25 (z +2.37) helps * | +0.25% ±0.26% | +0.50% ±0.40% |
| 36 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | — | **on** | +17 | +59 | +13 | 16.81% (n=106,805) | 16.52% (n=101,923) | 0.29% | +16 [-2, +34] | 95.5% | -0.00 (z -0.01) ~ | +0.06% ±0.26% | -0.17% ±0.39% |
| 37 | `lane-space-race` | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be sited in all read the race rather than an explicitly assigned target, and the pass opens at all. | — | off | +8 | -15 | +40 | 16.81% (n=56,890) | 16.53% (n=59,798) | 0.29% | +17 [-19, +54] | 82.5% | +0.19 (z +1.83) ~ | +0.06% ±0.25% | +0.12% ±0.40% |
| 38 | `unit-cost-efficiency` | Credit strength-per-production and the civ's own unique unit in the military production arm. | — | **on** | +1 | -4 | +8 | 16.80% (n=37,153) | 16.52% (n=32,291) | 0.28% | +14 [-14, +43] | 83.8% | +0.21 (z +2.02) helps * | +0.10% ±0.25% | +0.15% ±0.38% |
| 39 | `unit-objective-memory` | Let a unit retain its campaign objective and a short, threat-driven retreat across turns. | — | **on** | -18 | +1 | +18 | 16.79% (n=37,172) | 16.52% (n=32,272) | 0.27% | +14 [-14, +42] | 83.5% | +0.07 (z +0.64) ~ | +0.11% ±0.26% | +0.34% ±0.42% |
| 40 | `competition-victory-points` | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. | — | **on** | +26 | -7 | +0 | 16.80% (n=60,933) | 16.53% (n=55,755) | 0.27% | +13 [-8, +34] | 88.7% | +0.05 (z +0.44) ~ | -0.19% ±0.25% | -0.33% ±0.39% |
| 41 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | — | off | +19 | -3 | +12 | 16.80% (n=102,891) | 16.53% (n=105,837) | 0.27% | +13 [-3, +28] | 94.9% | +0.07 (z +0.68) ~ | +0.29% ±0.26% | +0.24% ±0.40% |
| 42 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | — | **on** | +21 | +28 | +3 | 16.79% (n=106,824) | 16.53% (n=101,904) | 0.26% | +13 [-2, +28] | 94.9% | +0.11 (z +1.04) ~ | -0.16% ±0.25% | -0.20% ±0.38% |
| 43 | `lane-policy-deck` | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. | — | **on** | -11 | +5 | +11 | 16.80% (n=57,044) | 16.54% (n=59,644) | 0.26% | +12 [-9, +33] | 87.1% | +0.00 (z +0.00) ~ | +0.53% ±0.26% | +0.62% ±0.40% |
| 44 | `culture-building-debt` | Make the Theater Square owe its buildings. | — | **on** | -1 | +12 | -19 | 16.75% (n=75,762) | 16.51% (n=40,926) | 0.24% | +10 [-20, +40] | 73.7% | -0.07 (z -0.60) ~ | +0.26% ±0.31% | +0.60% ±0.50% |
| 45 | `come-ashore` | Keep the land army out of the water. | — | off | +38 | -28 | +24 | 16.77% (n=117,716) | 16.54% (n=91,012) | 0.23% | +14 [-8, +35] | 89.2% | +0.19 (z +1.61) ~ | -0.07% ±0.29% | +0.05% ±0.43% |
| 46 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | — | **on** | -37 | -10 | -2 | 16.76% (n=121,717) | 16.54% (n=87,011) | 0.22% | +9 [-12, +30] | 81.2% | +0.05 (z +0.42) ~ | +0.89% ±0.30% | +1.68% ±0.48% |
| 47 | `science-multiplier-payoff` | Credit a Campus building the beakers its city's multipliers will actually pay it. | — | **on** | +1 | +20 | +12 | 16.77% (n=56,970) | 16.57% (n=59,718) | 0.21% | +9 [-12, +29] | 80.0% | -0.20 (z -1.90) ~ | +0.23% ±0.25% | +0.13% ±0.39% |
| 48 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | — | **on** | +5 | +6 | -31 | 16.74% (n=93,431) | 16.54% (n=58,405) | 0.20% | +4 [-33, +42] | 59.0% | -0.19 (z -1.58) ~ | +0.38% ±0.30% | +0.27% ±0.46% |
| 49 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | — | **on** | +11 | +22 | +0 | 16.75% (n=121,697) | 16.55% (n=87,031) | 0.20% | +10 [-6, +26] | 89.9% | +0.04 (z +0.32) ~ | +0.11% ±0.30% | +0.43% ±0.48% |
| 50 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | — | off | +36 | +26 | +14 | 16.76% (n=102,729) | 16.57% (n=105,999) | 0.19% | +8 [-7, +23] | 85.4% | +0.05 (z +0.47) ~ | +0.05% ±0.26% | +0.06% ±0.38% |
| 51 | `strike-opening` | Let movement credit the attack a tile opens. | — | **on** | +22 | -10 | -8 | 16.74% (n=121,660) | 16.56% (n=87,068) | 0.18% | +10 [-5, +26] | 90.1% | -0.07 (z -0.60) ~ | +0.46% ±0.29% | +0.44% ±0.45% |
| 52 | `early-contact-window` | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. | — | **on** | +14 | -4 | +5 | 16.75% (n=60,832) | 16.57% (n=55,856) | 0.18% | +9 [-12, +29] | 79.5% | -0.11 (z -1.07) ~ | -0.13% ±0.26% | -0.42% ±0.41% |
| 53 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | — | **on** | -3 | -7 | +0 | 16.74% (n=121,504) | 16.56% (n=87,224) | 0.18% | +10 [-6, +26] | 88.9% | +0.14 (z +1.13) ~ | +0.32% ±0.30% | +0.30% ±0.49% |
| 54 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | — | off | +15 | -9 | +2 | 16.74% (n=89,233) | 16.56% (n=62,603) | 0.18% | +10 [-8, +29] | 86.1% | +0.00 (z +0.01) ~ | +0.00% ±0.30% | -0.24% ±0.45% |
| 55 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | — | **on** | +67 | -29 | +1 | 16.74% (n=121,867) | 16.56% (n=86,861) | 0.18% | +11 [-16, +39] | 79.1% | +0.02 (z +0.20) ~ | -1.00% ±0.29% | -1.21% ±0.44% |
| 56 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | — | off | +11 | +6 | -6 | 16.74% (n=117,837) | 16.57% (n=90,891) | 0.17% | +10 [-6, +26] | 89.6% | +0.00 (z +0.01) ~ | +0.08% ±0.32% | +0.18% ±0.50% |
| 57 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | — | **on** | +30 | +54 | -3 | 16.75% (n=106,705) | 16.58% (n=102,023) | 0.17% | +12 [-8, +33] | 88.1% | -0.01 (z -0.12) ~ | -0.15% ±0.25% | -0.45% ±0.37% |
| 58 | `coordinated-finish` | Admit the friendly-volley extension without the rest of the closed war-half bundle. | — | off | +41 | -22 | +6 | 16.75% (n=33,354) | 16.59% (n=36,090) | 0.16% | +11 [-28, +50] | 71.1% | -0.04 (z -0.34) ~ | -0.15% ±0.26% | +0.07% ±0.39% |
| 59 | `lane-culture-spending` | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture whose plan has not named the lane. | — | **on** | +25 | +2 | -3 | 16.74% (n=60,748) | 16.59% (n=55,940) | 0.16% | +8 [-13, +29] | 77.4% | -0.15 (z -1.44) ~ | -0.43% ±0.27% | -0.58% ±0.41% |
| 60 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | — | off | +3 | -29 | -6 | 16.74% (n=102,837) | 16.59% (n=105,891) | 0.15% | +8 [-18, +34] | 72.9% | +0.02 (z +0.18) ~ | -0.16% ±0.25% | -0.17% ±0.38% |
| 61 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | — | off | +32 | +9 | -9 | 16.74% (n=102,850) | 16.59% (n=105,878) | 0.15% | +8 [-7, +23] | 84.6% | +0.04 (z +0.43) ~ | +0.19% ±0.25% | +0.50% ±0.39% |
| 62 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | — | **on** | +19 | -14 | +3 | 16.72% (n=99,301) | 16.58% (n=64,535) | 0.14% | +9 [-8, +27] | 85.5% | -0.02 (z -0.18) ~ | +0.44% ±0.30% | +0.36% ±0.45% |
| 63 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | — | off | -7 | +61 | +2 | 16.72% (n=117,825) | 16.60% (n=90,903) | 0.12% | +7 [-11, +24] | 76.9% | +0.04 (z +0.29) ~ | -0.52% ±0.30% | -0.78% ±0.46% |
| 64 | `research-tier-premium` | A Campus building's debt is scaled by its own Science against the chain's first rung. | — | off | +10 | -28 | -2 | 16.72% (n=56,964) | 16.62% (n=59,724) | 0.10% | +4 [-16, +25] | 65.9% | -0.07 (z -0.72) ~ | +0.02% ±0.26% | -0.11% ±0.40% |
| 65 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | — | off | -1 | +33 | -10 | 16.72% (n=102,911) | 16.62% (n=105,817) | 0.10% | +7 [-13, +26] | 74.6% | -0.24 (z -2.28) hurts * | -0.26% ±0.25% | -0.62% ±0.39% |
| 66 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | — | off | -8 | +22 | +27 | 16.71% (n=102,990) | 16.63% (n=105,738) | 0.08% | +4 [-14, +21] | 66.2% | +0.09 (z +0.86) ~ | +0.23% ±0.25% | +0.48% ±0.39% |
| 67 | `enhancer-for-the-corps` | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. | — | off | +35 | +6 | -4 | 16.71% (n=56,814) | 16.63% (n=59,874) | 0.08% | +4 [-16, +25] | 65.6% | -0.03 (z -0.24) ~ | -0.21% ±0.24% | -0.15% ±0.38% |
| 68 | `campus-adjacency-threshold` | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. | — | off | +33 | -14 | +3 | 16.70% (n=56,751) | 16.63% (n=59,937) | 0.07% | +4 [-22, +30] | 62.4% | +0.09 (z +0.88) ~ | +0.45% ±0.26% | +0.85% ±0.40% |
| 69 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | — | off | -41 | +9 | -14 | 16.70% (n=102,586) | 16.64% (n=106,142) | 0.06% | +2 [-20, +24] | 57.6% | -0.08 (z -0.73) ~ | -0.16% ±0.26% | -0.51% ±0.40% |
| 70 | `theology-for-founders` | A founder researches Theology next. | — | **on** | -3 | -10 | +0 | 16.69% (n=84,426) | 16.64% (n=79,410) | 0.05% | +2 [-14, +19] | 61.0% | +0.00 (z +0.04) ~ | +0.17% ±0.24% | +0.25% ±0.36% |
| 71 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | — | off | +39 | +39 | -9 | 16.69% (n=102,874) | 16.64% (n=105,854) | 0.05% | +3 [-14, +20] | 63.2% | +0.01 (z +0.05) ~ | +0.61% ±0.26% | +1.27% ±0.41% |
| 72 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | — | off | +30 | +2 | -18 | 16.69% (n=117,630) | 16.64% (n=91,098) | 0.04% | +1 [-19, +22] | 55.5% | -0.07 (z -0.61) ~ | +0.07% ±0.29% | +0.10% ±0.44% |
| 73 | `fifteenth-citizen` | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. | — | off | +16 | +26 | +15 | 16.68% (n=56,983) | 16.65% (n=59,705) | 0.03% | +1 [-20, +21] | 52.6% | +0.02 (z +0.15) ~ | +0.07% ±0.25% | +0.31% ±0.39% |
| 74 | `lane-great-people` | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. | — | **on** | +1 | -28 | -19 | 16.68% (n=60,823) | 16.66% (n=55,865) | 0.02% | -3 [-33, +27] | 42.8% | -0.18 (z -1.79) ~ | -0.13% ±0.25% | -0.19% ±0.39% |
| 75 | `wonder-score-tally` | A wonder lane any civilization can reach on merit: the `Item::Wonder` arm learns the fifteen points `Game::score_parts` pays for a finished wonder, under a density bar and the live race's own development guards. | — | off | +3 | +13 | -3 | 16.68% (n=14,014) | 16.66% (n=17,270) | 0.02% | +1 [-41, +43] | 51.7% | -0.04 (z -0.43) ~ | +0.53% ±0.26% | +0.72% ±0.42% |
| 76 | `district-coverage` | Rank district families by how much of the empire still lacks them. | — | off | -26 | +34 | +2 | 16.67% (n=102,818) | 16.66% (n=105,910) | 0.01% | +1 [-17, +19] | 52.2% | +0.11 (z +1.01) ~ | +0.24% ±0.26% | +0.47% ±0.40% |
| 77 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | — | off | +11 | -23 | -7 | 16.67% (n=74,769) | 16.66% (n=77,067) | 0.01% | +0 [-17, +18] | 52.1% | -0.05 (z -0.48) ~ | -0.14% ±0.27% | -0.34% ±0.40% |
| 78 | `escort-unstick-2` | Version 2 of `escort_unstick`: the same two-turn release, refused while a visible barbarian raider can reach the settler's tile. | 1 | off | -41 | +16 | +23 | v1 16.99% (n=103,891) · v2 16.67% (n=17,695) | v1 16.35% (n=104,837) · v2 16.67% (n=51,749) | -0.00% | -2 [-66, +63] | 47.8% | +0.18 (z +1.48) ~ | +0.14% ±0.36% | +0.35% ±0.56% |
| 79 | `condemn-under-congress` | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. | — | off | -10 | +30 | +15 | 16.67% (n=57,022) | 16.67% (n=59,666) | -0.00% | -1 [-21, +20] | 47.4% | +0.01 (z +0.07) ~ | +0.33% ±0.25% | +0.43% ±0.40% |
| 80 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | — | off | +21 | -62 | +3 | 16.66% (n=102,672) | 16.67% (n=106,056) | -0.01% | -0 [-15, +15] | 47.7% | +0.00 (z +0.04) ~ | -0.04% ±0.25% | -0.21% ±0.38% |
| 81 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use its direct empire-wide repair. | — | off | +24 | +3 | -2 | 16.66% (n=102,864) | 16.67% (n=105,864) | -0.01% | +1 [-19, +21] | 52.2% | -0.08 (z -0.79) ~ | +0.16% ±0.26% | +0.54% ±0.41% |
| 82 | `campus-finishes-first` | The Campus coverage term is scaled by how finished the empire's standing Campuses are. | — | off | -17 | -1 | +10 | 16.66% (n=56,814) | 16.68% (n=59,874) | -0.02% | -1 [-21, +20] | 47.9% | +0.19 (z +1.82) ~ | +0.07% ±0.26% | +0.19% ±0.41% |
| 83 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | — | off | -3 | +14 | +3 | 16.64% (n=103,025) | 16.69% (n=105,703) | -0.05% | -3 [-19, +12] | 33.1% | -0.10 (z -0.94) ~ | +0.27% ±0.26% | +0.78% ±0.41% |
| 84 | `religious-units-heal-first` | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. | — | **on** | +24 | -33 | -27 | 16.64% (n=60,755) | 16.70% (n=55,933) | -0.06% | -7 [-41, +28] | 35.5% | -0.18 (z -1.79) ~ | +0.16% ±0.25% | +0.49% ±0.39% |
| 85 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | — | **on** | +49 | -5 | -17 | 16.63% (n=106,768) | 16.70% (n=101,960) | -0.07% | -3 [-19, +12] | 33.3% | -0.15 (z -1.37) ~ | -0.26% ±0.27% | -0.49% ±0.41% |
| 86 | `guru-heals-the-corps` | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. | — | off | +36 | -68 | +5 | 16.63% (n=56,891) | 16.70% (n=59,797) | -0.07% | -4 [-49, +41] | 43.4% | -0.00 (z -0.03) ~ | +0.34% ±0.26% | +0.59% ±0.39% |
| 87 | `deals-at-the-ceiling` | The chosen quote's Gold is moved to the counterparty's walk-away less two Gold — a sale asks for more, a purchase pays less — where the shipped quote split the surplus down the middle; the midpoint quote stays the fallback. | — | off | -1 | – | – | 16.61% (n=1,517) | 16.69% (n=4,471) | -0.07% | -4 [-114, +106] | 47.4% | +0.09 (z +0.42) ~ | -1.03% ±0.87% | -1.88% ±1.25% |
| 88 | `fortify-idle-units` | Fortify units the planner gave nothing to do. | — | off | +17 | +29 | -1 | 16.63% (n=33,260) | 16.70% (n=36,184) | -0.08% | -4 [-32, +24] | 39.3% | -0.08 (z -0.80) ~ | +0.08% ±0.26% | -0.08% ±0.39% |
| 89 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | — | off | +43 | -24 | -13 | 16.63% (n=102,857) | 16.71% (n=105,871) | -0.08% | -3 [-24, +19] | 40.2% | -0.06 (z -0.53) ~ | -0.06% ±0.25% | +0.03% ±0.40% |
| 90 | `power-the-laboratory` | A power plant is credited the yields it switches on in its city. | — | off | +12 | +15 | -24 | 16.61% (n=57,016) | 16.72% (n=59,672) | -0.10% | -5 [-29, +18] | 32.7% | -0.11 (z -1.06) ~ | +0.13% ±0.26% | +0.44% ±0.41% |
| 91 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | — | off | – | – | – | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 92 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | — | off | -8 | -34 | +7 | 16.61% (n=74,594) | 16.72% (n=77,242) | -0.11% | -5 [-27, +17] | 32.5% | +0.08 (z +0.74) ~ | +0.33% ±0.24% | +0.54% ±0.37% |
| 93 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | — | off | -33 | +11 | -19 | 16.61% (n=102,757) | 16.72% (n=105,971) | -0.12% | -6 [-27, +15] | 27.5% | -0.19 (z -1.84) ~ | -0.02% ±0.27% | +0.10% ±0.42% |
| 94 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | — | off | -10 | -16 | -13 | 16.61% (n=103,045) | 16.72% (n=105,683) | -0.12% | -6 [-21, +9] | 21.4% | -0.14 (z -1.29) ~ | +0.22% ±0.26% | -0.02% ±0.41% |
| 95 | `housing-districts` | Let the baseline governor raise the housing ceiling. | — | off | -11 | +26 | +11 | 16.59% (n=102,986) | 16.74% (n=105,742) | -0.15% | -8 [-24, +8] | 15.5% | +0.20 (z +1.88) ~ | -0.12% ±0.26% | -0.26% ±0.40% |
| 96 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | — | off | +8 | -5 | -8 | 16.59% (n=102,825) | 16.74% (n=105,903) | -0.15% | -9 [-29, +12] | 20.2% | +0.08 (z +0.71) ~ | +0.35% ±0.27% | +0.69% ±0.40% |
| 97 | `flip-nearby-city-states` | A city-state's place enters the envoy score: up to ninety for one on our border, two hundred more when its sitting suzerain is at war with us, amortised over the envoys the flip still needs. | — | off | -3 | – | – | 16.55% (n=1,462) | 16.70% (n=4,526) | -0.15% | -8 [-119, +104] | 44.7% | +0.01 (z +0.04) ~ | +0.96% ±0.93% | +1.35% ±1.22% |
| 98 | `lane-congress-ballot` | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion posture that has no lane. | — | off | +11 | -23 | -2 | 16.59% (n=57,027) | 16.74% (n=59,661) | -0.15% | -7 [-27, +14] | 26.9% | +0.08 (z +0.75) ~ | -0.52% ±0.27% | -0.77% ±0.42% |
| 99 | `congress-banks-decided` | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact prediction and staking nothing. | — | off | +27 | -55 | -5 | 16.58% (n=56,859) | 16.75% (n=59,829) | -0.17% | -9 [-35, +17] | 24.2% | +0.07 (z +0.68) ~ | -0.08% ±0.26% | +0.05% ±0.40% |
| 100 | `barbarian-capture-priority` | Take a visible Barbarian Settler or Scout in exact one-turn reach before healing, retreat, or any ordinary tactical choice. | — | off | +13 | -11 | -13 | 16.58% (n=56,668) | 16.75% (n=60,020) | -0.17% | -8 [-29, +13] | 22.4% | -0.04 (z -0.37) ~ | +0.05% ±0.26% | +0.01% ±0.41% |
| 101 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | — | off | +11 | -22 | -4 | 16.58% (n=103,297) | 16.76% (n=105,431) | -0.18% | -10 [-25, +6] | 10.9% | +0.09 (z +0.91) ~ | -0.13% ±0.26% | -0.02% ±0.40% |
| 102 | `lane-congress-favor` | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. | — | off | +22 | -25 | -3 | 16.57% (n=56,800) | 16.76% (n=59,888) | -0.19% | -10 [-30, +11] | 17.4% | -0.09 (z -0.83) ~ | +0.37% ±0.25% | +0.53% ±0.38% |
| 103 | `coupled-expansion` | Enable the evaluator-only paid expansion treatment. | — | off | +40 | -1 | -18 | 16.56% (n=33,371) | 16.77% (n=36,073) | -0.21% | -4 [-47, +38] | 41.8% | -0.15 (z -1.44) ~ | -0.20% ±0.26% | -0.14% ±0.40% |
| 104 | `envoy-infrastructure` | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the turn limit, and a first Diplomatic Quarter sees part of the Consulate stream it unlocks. | — | off | -43 | -6 | -11 | 16.55% (n=56,861) | 16.78% (n=59,827) | -0.23% | -12 [-33, +10] | 14.2% | -0.03 (z -0.25) ~ | +0.16% ±0.26% | +0.34% ±0.38% |
| 105 | `one-shot-recovery` | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. | — | off | -13 | -48 | -16 | 16.55% (n=56,850) | 16.78% (n=59,838) | -0.23% | -12 [-33, +9] | 14.0% | -0.19 (z -1.75) ~ | -0.16% ±0.26% | -0.38% ±0.41% |
| 106 | `congress-counter-votes` | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fails costs no Favor. | — | off | -34 | -5 | +12 | 16.55% (n=56,783) | 16.78% (n=59,905) | -0.24% | -12 [-34, +10] | 14.6% | -0.10 (z -0.95) ~ | -0.24% ±0.27% | -0.55% ±0.41% |
| 107 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | — | off | -9 | -25 | -4 | 16.54% (n=102,840) | 16.79% (n=105,888) | -0.24% | -12 [-28, +3] | 5.6% | -0.07 (z -0.66) ~ | -0.07% ±0.26% | -0.14% ±0.40% |
| 108 | `holy-site-where-the-threat-is` | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. | — | off | -15 | +4 | -27 | 16.54% (n=56,896) | 16.78% (n=59,792) | -0.24% | -14 [-43, +14] | 16.0% | -0.23 (z -2.20) hurts * | -0.15% ±0.26% | -0.17% ±0.40% |
| 109 | `tactical-strategy` | Enable explicit battlefield roles: the land-unit counter cycle, safe ranged standoff, wall-focused siege/support, and cavalry job priority. | — | off | -10 | -27 | +7 | 16.54% (n=33,397) | 16.78% (n=36,047) | -0.24% | -13 [-41, +15] | 19.0% | +0.02 (z +0.17) ~ | +0.03% ±0.25% | +0.19% ±0.39% |
| 110 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends its turn beside a free 50-gold clear until the camp spawns the archer that kills it. | — | off | +21 | +0 | +17 | 16.54% (n=74,469) | 16.79% (n=77,367) | -0.26% | -6 [-49, +37] | 39.3% | +0.10 (z +0.96) ~ | -1.11% ±0.27% | -1.52% ±0.41% |
| 111 | `culture-coverage` | Pay for the Theater Square the empire has not got. | — | off | -2 | +3 | -23 | 16.52% (n=56,764) | 16.80% (n=59,924) | -0.28% | -14 [-35, +7] | 9.0% | -0.05 (z -0.46) ~ | +0.01% ±0.26% | +0.14% ±0.40% |
| 112 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | — | off | +6 | +21 | -3 | 16.52% (n=74,414) | 16.81% (n=77,422) | -0.29% | -15 [-36, +6] | 8.5% | +0.04 (z +0.37) ~ | -0.03% ±0.27% | -0.15% ±0.42% |
| 113 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | — | off | -43 | +57 | -13 | 16.52% (n=74,583) | 16.81% (n=77,253) | -0.29% | -15 [-44, +13] | 14.3% | -0.21 (z -1.94) ~ | -0.34% ±0.25% | +0.08% ±0.39% |
| 114 | `spread-campaign-persists` | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is spent. | — | off | +16 | +23 | -15 | 16.52% (n=56,901) | 16.81% (n=59,787) | -0.29% | -15 [-36, +6] | 8.3% | -0.11 (z -1.02) ~ | -0.37% ±0.26% | -0.49% ±0.39% |
| 115 | `district-building-chain` | Make every specialty district owe its own buildings, whatever the lane. | — | off | -20 | -6 | -6 | 16.52% (n=56,725) | 16.81% (n=59,963) | -0.29% | -14 [-35, +7] | 9.6% | -0.04 (z -0.37) ~ | -0.34% ±0.25% | -0.61% ±0.39% |
| 116 | `lane-commit` | From the midpoint of the game an adaptive seat commits to the victory lane it leads the field in and holds that plan, in place of the per-turn best-progress pick. | — | off | -6 | – | – | 16.42% (n=1,510) | 16.75% (n=4,478) | -0.32% | -16 [-126, +94] | 38.6% | +0.12 (z +0.56) ~ | -0.70% ±0.88% | -0.65% ±1.24% |
| 117 | `settle-plan-ahead` | Rank a settle site by the cities it leaves room for as well as its own ground, so a Settler stops taking the one plot in a pocket that would have held two. | — | off | -17 | -1 | +1 | 16.45% (n=33,099) | 16.87% (n=36,345) | -0.42% | -21 [-49, +7] | 6.7% | +0.03 (z +0.25) ~ | +0.46% ±0.26% | +0.73% ±0.39% |
| 118 | `builder-reward-survey` | Price Builder production by a survey of the work it would do. | — | off | -21 | -94 | -23 | 16.10% (n=33,309) | 17.19% (n=36,135) | -1.09% | -66 [-112, -20] | 0.2% | -0.28 (z -2.69) hurts * | +1.59% ±0.26% | +1.76% ±0.40% |
| 119 | `contact-posture` | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answer, or step out of that shooter's envelope. | — | off | -17 | -34 | -31 | 16.08% (n=56,753) | 17.22% (n=59,935) | -1.15% | -58 [-78, -37] | 0.0% | -0.24 (z -2.36) hurts * | -0.20% ±0.26% | -0.21% ±0.40% |
| 120 | `pillage-to-heal` | A unit at or below 65 health pillages a heal-type improvement it stands on, or steps one tile onto one and pillages it, before the recovery path walks it home. | — | off | -23 | – | – | 15.79% (n=1,545) | 16.97% (n=4,443) | -1.18% | -59 [-165, +47] | 13.8% | +0.02 (z +0.11) ~ | +0.75% ±0.90% | +1.68% ±1.19% |
| 121 | `zoc-screen` | A melee unit the attack scan found nothing for stands where its zone of control takes the most enemy reaches off our shooters and wounded, read exactly off `attack_reach`, and holds only while the stand is load-bearing. | — | off | -26 | – | – | 15.61% (n=1,505) | 17.02% (n=4,483) | -1.41% | -70 [-176, +35] | 9.6% | -0.36 (z -1.66) ~ | +1.52% ±0.87% | +2.33% ±1.21% |
| 122 | `naval-production-policy` | Reach for the naval-production discount while hulls are wanted. | — | off | -80 | -65 | -66 | 15.73% (n=33,235) | 17.52% (n=36,209) | -1.79% | -121 [-189, -53] | 0.0% | -0.60 (z -5.71) hurts * | -0.14% ±0.25% | -0.41% ±0.37% |
| 123 | `no-free-passage` | Friendship and alliance proposals no longer bundle one-way Open Borders, which every ask handed out for nothing once Early Empire was in; passage is sold through the quote lane. | — | off | -36 | – | – | 15.25% (n=1,534) | 17.15% (n=4,454) | -1.90% | -95 [-199, +9] | 3.7% | -0.38 (z -1.78) ~ | -0.09% ±0.87% | -0.21% ±1.18% |
| 124 | `fog-honest` | Put this controller behind the turn-level fog boundary. | 1 | off | -95 | -145 | -112 | v1 11.70% (n=7,126) · v2 4.22% (n=7,054) | v1 18.13% (n=24,158) · v2 20.29% (n=24,230) | -6.43% | -366 [-464, -269] | 0.0% | -1.11 (z -9.73) hurts * | +0.65% ±0.30% | +1.60% ±0.45% |
| 125 | `fog-honest-2` | Version 2 of `fog_honest`: the same fair-play boundary and the same information contract, plus one re-plan when the authoritative board refuses a planned order. | 1 | off | -187 | -275 | -308 | v1 11.70% (n=7,126) · v2 4.22% (n=7,054) | v1 18.13% (n=24,158) · v2 20.29% (n=24,230) | -16.06% | -820 [-885, -755] | 0.0% | -4.58 (z -48.06) hurts * | +5.28% ±0.34% | +5.99% ±0.51% |

## Evidence for future operator selections

The deployment genome is explicitly operator-pinned. The win columns, pooled *Diff*, posterior, and score-share readings below remain useful evidence, but a new source does not promote or demote a gene automatically. To change a default, update the pinned list with an explicit operator decision and regenerate this ledger.

*Posterior (95% CI)* is a random-effects (DerSimonian–Laird) inverse-variance pool of every screen's on−off difference on the win column's scale. It weights each screen by its standard error and carries between-screen disagreement in the interval; `P(>0)` makes the resulting precision visible.

### What the posterior resolves

Of 110 priced genes the interval clears zero for **21 upward** and **3 downward**; **86 straddle zero**. Those are evidence states, not automatic deployment calls.

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
| `contact-posture` | -57 [-80, -33] | 0.0% | 2 | off | **off** |
| `naval-production-policy` | -51 [-88, -13] | 0.4% | 1 | off | **off** |

## The two shapes, apart

`τ` (tau) is the between-screen standard deviation the random-effects pool estimates. It is the statistic that answers *“is 'both columns positive' two confirmations?”*: when screens agree to within their errors it is zero and the pool is the ordinary inverse-variance one; when they do not, it widens the interval instead of averaging two worlds into a confident wrong answer. `POSTERIOR_SHAPES` in `tools/genes.py` says which shapes the published pool admits and is currently `standard, legacy`.

| Shape | Sources | Player seats | Genes priced |
|---|---:|---:|---:|
| standard | 3 | 92,604 | 109 |
| legacy | 7 | 132,440 | 62 |

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
| `war-economy` | +13 [-89, +115] | 60.1% | on | +12.4 | 116,162 |
| `whole-turn-backtrack-guard` | +12 [-8, +31] | 88.2% | off | +11.8 | 96,625 |
| `stranded-settler-discount` | +11 [-5, +27] | 90.8% | off | +11.0 | 93,635 |
| `barbarian-ranged-answer` | +11 [-10, +32] | 85.1% | off | +10.9 | 126,996 |
| `blind-objective-strength` | +11 [-10, +31] | 84.9% | off | +10.8 | 131,056 |
| `research-tier-premium` | +10 [-24, +43] | 71.2% | off | +10.3 | 208,314 |
| `strategic-wonders` | +9 [-7, +25] | 85.6% | off | +8.8 | 192,886 |
| `siege-tracks-wall` | +9 [-15, +32] | 76.1% | off | +8.6 | 251,903 |
| `pantheon-board` | +5 [-32, +43] | 61.0% | off | +7.5 | 728,917 |
| `come-ashore` | +7 [-10, +24] | 79.3% | off | +7.0 | 353,593 |
| `guru-heals-the-corps` | -4 [-56, +49] | 44.7% | off | +5.7 | 1,648,811 |
| `wonder-ring-settle-value` | +5 [-12, +22] | 71.7% | off | +4.9 | 798,478 |
| `army-target-weighs-enemy` | +5 [-16, +25] | 67.3% | off | +4.9 | 903,207 |
| `campus-adjacency-threshold` | +0 [-38, +38] | 50.3% | off | +4.5 | 829,756,465 |
| `coordinated-finish` | -0 [-38, +37] | 49.1% | off | +4.1 | 110,743,705 |
| `holy-site-where-the-threat-is` | -1 [-38, +36] | 47.7% | off | +3.8 | 17,424,370 |
| `builder-barbarian-safety` | +3 [-16, +23] | 62.9% | off | +3.6 | 1,859,339 |
| `enhancer-for-the-corps` | +3 [-21, +26] | 58.6% | off | +3.6 | 2,978,891 |
| `civilian-rescue` | -3 [-20, +13] | 34.5% | on | +3.4 | 1,836,091 |
| `settler-guard-holds` | +3 [-13, +19] | 65.5% | off | +3.4 | 1,871,682 |
| `lane-space-race` | +1 [-22, +25] | 54.3% | off | +2.7 | 12,292,990 |
| `power-the-laboratory` | +1 [-23, +24] | 52.0% | off | +2.2 | 57,902,647 |
| `religious-defence-scales` | -0 [-24, +24] | 49.1% | on | +2.1 | 290,353,107 |
| `housing-research` | +0 [-22, +23] | 51.6% | off | +1.9 | 97,478,820 |
| `amenity-project-preemption` | -1 [-26, +25] | 48.2% | off | +1.9 | 62,802,848 |
| `promote-when-wounded` | +7 [-31, +45] | 64.5% | on | +1.7 | 396,852 |
| `district-coverage` | +0 [-20, +20] | 50.8% | off | +1.5 | 558,338,216 |
| `builder-worked-tile-priority` | -5 [-36, +26] | 37.3% | off | +1.2 | 796,622 |
| `one-shot-recovery` | -2 [-25, +22] | 43.7% | off | +1.1 | 5,846,162 |
| `lane-congress-ballot` | -9 [-45, +27] | 31.5% | off | +1.1 | 249,847 |
| `blind-objective-units` | +0 [-16, +16] | 51.4% | off | +1.1 | 245,127,417 |
| `unit-objective-memory` | +10 [-28, +47] | 69.7% | on | +1.1 | 201,764 |
| `settler-target-hysteresis` | +0 [-16, +16] | 50.7% | off | +1.0 | 1,036,197,883 |
| `barbarian-hunt` | -28 [-86, +30] | 16.9% | off | +0.9 | 20,099 |
| `science-multiplier-payoff` | +6 [-24, +35] | 64.5% | on | +0.9 | 639,181 |
| `campus-finishes-first` | -3 [-26, +21] | 41.0% | off | +0.9 | 2,800,136 |
| `coupled-expansion` | -12 [-49, +26] | 26.9% | off | +0.8 | 136,894 |
| `district-planning` | +12 [-25, +49] | 74.1% | on | +0.7 | 125,973 |
| `settler-site-agreement` | -3 [-24, +18] | 40.5% | off | +0.6 | 3,159,094 |
| `envoy-infrastructure` | -4 [-28, +20] | 37.3% | off | +0.6 | 1,334,442 |
| `settlement-gap-target` | +14 [-23, +52] | 77.5% | on | +0.5 | 87,669 |
| `lane-great-people` | +13 [-22, +48] | 77.0% | on | +0.4 | 103,314 |
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
| `lane-policy-deck` | +13 [-16, +42] | 80.9% | on | +0.1 | 103,406 |
| `culture-coverage` | -8 [-32, +15] | 24.5% | off | +0.1 | 268,265 |
| `lane-culture-spending` | +9 [-15, +32] | 75.9% | on | +0.1 | 250,826 |
| `condemn-under-congress` | -9 [-33, +14] | 22.4% | off | +0.1 | 216,182 |
| `religious-units-heal-first` | +9 [-14, +33] | 78.0% | on | +0.0 | 208,321 |
| `district-building-chain` | -13 [-40, +14] | 17.7% | off | +0.0 | 98,805 |
| `fifteenth-citizen` | -10 [-34, +14] | 20.6% | off | +0.0 | 176,078 |
| `endgame-war-runway` | -5 [-21, +11] | 27.5% | off | +0.0 | 778,289 |
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
| `one-launch-pad` | +11 [-5, +28] | 91.6% | on | +0.0 | 81,761 |
| `strike-opening` | +12 [-4, +29] | 92.5% | on | +0.0 | 65,525 |

The top 8 that one batch could actually resolve (≤ 60,000 seat pairs each), as an argument list:

```sh
gene_screen --genes barbarian-hunt,escort-unstick-2,unit-cost-efficiency,fortify-idle-units,tactical-strategy,district-lookahead-settle,holy-lane-parity,settle-plan-ahead
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
| `lane-policy-deck` | **on** | +29 | +0.08 (z +1.07) ~ | +13 [-16, +42] | unresolved |
| `lane-culture-spending` | **on** | +9 | -0.01 (z -0.13) ~ | +9 [-15, +32] | unresolved |
| `lane-space-race` | off | -12 | -0.10 (z -1.32) ~ | +1 [-22, +25] | unresolved |
| `competition-victory-points` | **on** | +35 | +0.04 (z +0.46) ~ | +16 [-19, +50] | unresolved |

## Awaiting measurement

These screenable genes have no on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a screen is pending.

| Gene | Default | Description | Best version |
|---|---|---|---:|
| `campaign-pillage` | off (unmeasured) | A soldier at war standing on a tile it may pillage spends the movement its march does not use on the pillage — waiting with its force, unable to move on, or in the siege ring with its blow declined — and never a tile of advance. | — |
| `city-campaign` | off (unmeasured) | Appraise the neighbours on public military power and tech count, plan the holdable city — or two, or three — of a weaker one that the field army can take with units to spare, and launch when the staging ring carries that bill. | — |
| `naval-threat-triage` | off (unmeasured) | Ignore nearby naval raiders that cannot project a meaningful blow, while retaining legal ranged shots into them for combat experience. | — |
| `pass-picket` | off (unmeasured) | A recon unit with nothing left to explore holds the pass toward a neighbour — the first tile outside their borders whose removal cuts the land walk between the two capitals — or, when no single tile cuts it, watches the border tile that walk leaves their ground by. | — |
| `settler-factory-coordination` | off (unmeasured) | Keep every early Settler pipeline slot, but allocate new slots to competitive factories with distinct reachable claims. | — |
| `settler-screen` | off (unmeasured) | A seen rival Settler near our cities is screened: up to four of our nearby land units, recon first, take the stands that add the most expected steps to its likeliest walks — a tile a foreign unit holds cannot be entered at peace — and hold them while the plan names them. | — |
| `solvency-first-trade-slot` | off (unmeasured) | Reserve the first usable empty trade slot before ordinary production in a city that can start a locally safe route. | — |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±/10k seats (last tracked measurement) | Win rate (on) | Win rate (off) | Source |
|---|---:|---:|---:|---|
| `suzerain-cards` | +42 | 17.09% | 16.25% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `wonder-prereq-reach` | +29 | 16.96% | 16.38% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `governor-expansion-lane` | +12 | 16.79% | 16.55% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `camp-reach` | +10 | 16.77% | 16.56% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `research-grants-first` | +9 | 16.76% | 16.57% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `housing-buildings` | +8 | 16.75% | 16.59% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `ranged-line-of-sight` | +4 | 16.71% | 16.63% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `recon-flight` | -1 | 16.66% | 16.67% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-cards` | -4 | 16.62% | 16.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `arrival-waves` | -7 | 16.59% | 16.74% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `idle-walkers-close-the-pipeline` | -10 | 16.56% | 16.77% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `muster-at-command-radius` | -12 | 16.55% | 16.79% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `barbarian-walls-one-tier` | -13 | 16.54% | 16.80% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `research-floor-holds` | -14 | 16.52% | 16.81% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `science-payback-horizon` | -25 | 16.41% | 16.92% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `siege-muster` | -26 | 16.41% | 16.93% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `siege-role` | -39 | 16.27% | 17.06% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `chain-tech-lookahead` | -47 | 16.19% | 17.13% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `garrison-walls` | -54 | 16.12% | 17.21% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `loyalty-policy-defence` | -54 | 16.13% | 17.20% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `campus-every-city` | -94 | 15.73% | 17.60% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `stacked-escort` | -104 | 15.63% | 17.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `settler-stack-discipline` | -116 | 15.51% | 17.83% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `governor-every-lane` | -140 | 15.27% | 18.06% | `2026-08-24-standard-continuous-21030-total-seats.json` |
| `governor-victory-lanes` | -171 | 14.96% | 18.36% | `2026-08-24-standard-continuous-21030-total-seats.json` |

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

**`governor-victory-lanes` is the clearest historical correction.** It had previously been enabled by the retired single-column policy. At the deployment shape the whole-genome screen read **-4.73 pp, z -15.37** -- **-237 wins per 10,000 on-arm seats, 95% CI [-267, -206]** -- and a pre-registered direct arm confirmed **-4.78 pp at win z -6.11**. The legacy and standard readings do not agree, so their random-effects pool widens rather than manufacturing certainty. On 2026-08-24, this row and its two governor-lane siblings left the code under the explicit `Diff < -0.05 pp` / at-least-30,000-seat cull criterion; their measurements remain in the ranking's **Removed from the code** table.

**Four research-planning genes joined that historical record under the same explicit criterion.** At 38,160 seats, `chain-tech-lookahead` read **-0.531591 pp**, `research-floor-holds` **-0.564379 pp**, `research-grants-first` **-0.181882 pp**, and `science-payback-horizon` **-0.232622 pp**. All were off-default, and their fields, flags, registry rows, probes, and focused tests left the code on 2026-08-24. Their screen rows remain in the ranking's **Removed from the code** table so the cull is auditable without keeping their runtime branches alive.

*The legacy share axis already showed the disagreement.* P10 priced this gene at win z **+2.46** and score-share z **-15.92** -- a recorded `conflict`. The later standard win reading was z -15.37, within half a sigma of that legacy share reading. This is evidence for publishing both axes, not a rule that either axis may silently rewrite a default.

*The composite, decomposed.* `governor-every-lane` (the composite) read **-4.68**, `governor-victory-lanes` (the victory-lane half) **-4.73**, and `governor-expansion-lane` **-0.55**. The victory half carried essentially the whole of the composite's harm. All three were pinned off before the 2026-08-24 cull removed their implementation.

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

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 26,892 seats), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 2,000 seats), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 30,000 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 35,148 seats), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 14,400 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 47,244 seats), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 7,200 seats), `2026-08-24-standard-continuous-38160-total-seats.json` (standard, 38,160 seats). The fixed display batches are: `2026-08-24-standard-continuous-5988-total-seats.json` (5,988 seats), `2026-08-24-standard-continuous-4266-total-seats.json` (4,266 seats), `2026-08-24-standard-continuous-21030-total-seats.json` (21,030 seats). The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells are the operator's wins-per-ten-thousand-total-seat reporting view._
