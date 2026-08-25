# The heuristic gene ranking

**Deployment default:** operator-pinned (57 genes): retains the prior 36 selections and explicitly promotes `unit-cost-efficiency`, `unit-objective-memory`, `camp-party`, `slot-kind-tiebreak`, `promote-when-wounded`, `religion-sues-peace`, `lane-great-people`, `one-launch-pad`, `civilian-rescue`, `missionary-evades-raiders`, `district-planning`, `missionary-last-charge-explores`, `settlement-gap-target`, `religious-defence-scales`, `lane-policy-deck`, `science-multiplier-payoff`, `science-victory-drive`, `solvency-first-trade-slot`, `settler-factory-coordination`, `one-war-at-a-time`, `religious-veto-defence`. Screen columns, *Diff*, and posterior values are evidence only; new batches do not automatically change defaults.

| Rank | Gene | Description | Best version | Default | Wins ± /10k total seats — Last Batch (n=4,476 total seats) | Wins ± /10k total seats — Prior Batch (n=5,988 total seats) | Wins ± /10k total seats — Third Batch (n=4,266 total seats) | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `solvency-first-trade-slot` | Reserve the first empty trade route slot ahead of ordinary production in any city that can start a safe route. | — | **on** | +144 | – | – | 22.85% (n=1,046) | 14.78% (n=3,430) | 8.07% | +403 [+265, +541] | 100.0% | +2.08 (z +7.93) helps * | +2.60% ±1.00% | +2.66% ±1.40% |
| 2 | `settler-factory-coordination` | Give early Settler pipeline slots to cities that finish fastest and hold distinct reachable claim sites. | — | **on** | +35 | – | – | 18.03% (n=1,148) | 16.20% (n=3,328) | 1.84% | +92 [-37, +220] | 91.9% | +0.32 (z +1.28) ~ | +1.56% ±0.91% | +2.58% ±1.30% |
| 3 | `air-surge` | Beeline Advanced Flight, build an Aerodrome and bombers, and take the appointed city with cavalry behind them. | — | **on** | +3 | +30 | +25 | 17.31% (n=63,265) | 15.56% (n=36,869) | 1.76% | +95 [+72, +118] | 100.0% | +0.09 (z +0.33) ~ | +0.63% ±0.71% | +0.57% ±0.97% |
| 4 | `great-person-housing` | Reserve a city to build whatever unblocks an earned Great Person, selling duplicate works to make room. | — | **on** | +29 | +29 | +52 | 17.31% (n=80,785) | 15.72% (n=54,497) | 1.59% | +85 [+65, +105] | 100.0% | +0.38 (z +1.52) ~ | -0.11% ±0.75% | +0.02% ±1.05% |
| 5 | `wide-map-capacity` | Price the city ceiling off the passable land actually visible, at one city per 45 tiles, capped at twelve. | — | **on** | +16 | +41 | +23 | 17.19% (n=109,368) | 15.98% (n=82,806) | 1.20% | +62 [+35, +90] | 100.0% | +0.51 (z +1.95) ~ | +0.21% ±0.73% | -0.07% ±1.06% |
| 6 | `engine-faith-price` | Read Faith purchase prices from the engine at the game's speed and discounts, instead of a Standard-speed literal. | — | **on** | +51 | +28 | -20 | 17.17% (n=30,196) | 16.00% (n=22,694) | 1.17% | +62 [+10, +114] | 99.0% | -0.43 (z -1.54) ~ | -0.63% ±0.75% | -1.21% ±1.09% |
| 7 | `raid-pillage-prizes` | Count a neighbour's unpillaged improvements within reach as raid prizes and send raiders to pillage them. | — | **on** | +47 | +11 | +35 | 17.09% (n=80,814) | 16.04% (n=54,468) | 1.05% | +56 [+33, +78] | 100.0% | +0.40 (z +1.56) ~ | -1.33% ±0.70% | -1.54% ±1.07% |
| 8 | `one-war-at-a-time` | Fight one campaign front at a time, seeking peace with every other major and once the front turns against us. | — | **on** | +16 | +21 | – | 17.41% (n=2,630) | 16.42% (n=7,834) | 1.00% | +50 [-32, +132] | 88.4% | -0.13 (z -0.59) ~ | +0.14% ±0.87% | +0.79% ±1.14% |
| 9 | `recon-replacement` | Rebuild the recon arm when every scout is gone and unexplored ground remains to chart. | — | **on** | +3 | +35 | +27 | 17.08% (n=109,387) | 16.12% (n=82,787) | 0.96% | +51 [+34, +67] | 100.0% | +0.14 (z +0.55) ~ | +1.74% ±0.78% | +1.17% ±1.05% |
| 10 | `opportunistic-war` | Open a surprise war on a neighbour whose Settlers, Builders or tiles lie exposed nearby, then sue for peace. | — | **on** | +16 | +35 | +23 | 17.04% (n=80,822) | 16.11% (n=54,460) | 0.93% | +49 [+29, +69] | 100.0% | +0.50 (z +1.87) ~ | +0.21% ±0.78% | -0.05% ±1.14% |
| 11 | `religious-veto-defence` | Scale religious defence with a rival's progress toward religious victory, and walk the Inquisitor to the heresy. | — | **on** | +31 | +8 | – | 17.36% (n=2,685) | 16.43% (n=7,779) | 0.93% | +46 [-36, +129] | 86.5% | +0.56 (z +2.46) helps * | +1.25% ±0.87% | +2.13% ±1.12% |
| 12 | `price-the-suzerainty` | Credit the envoy scorer with the resources, bonuses and points a suzerainty pays, amortised over envoys still needed. | — | **on** | +7 | -1 | +8 | 17.04% (n=30,329) | 16.17% (n=22,561) | 0.87% | +46 [+13, +79] | 99.7% | +0.03 (z +0.09) ~ | -0.12% ±0.79% | -0.36% ±1.14% |
| 13 | `maintenance-aware-deck` | Subtract the unit-maintenance bill inside the policy counterfactual so maintenance-discount cards score above zero. | — | **on** | +15 | -22 | -2 | 17.03% (n=30,097) | 16.18% (n=22,793) | 0.85% | +24 [-34, +82] | 79.1% | +0.29 (z +1.07) ~ | +0.25% ±0.73% | -0.74% ±1.06% |
| 14 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns until the city flips rather than by its current loyalty level. | — | **on** | -3 | +18 | +11 | 16.98% (n=109,141) | 16.25% (n=83,033) | 0.73% | +39 [+23, +55] | 100.0% | +0.37 (z +1.38) ~ | -0.85% ±0.86% | -1.60% ±1.17% |
| 15 | `settle-sooner` | Price a Settler's walk per turn, rising the longer it has walked, so it settles sooner. | — | **on** | +2 | -3 | +56 | 16.93% (n=81,054) | 16.27% (n=54,228) | 0.66% | +35 [+16, +55] | 100.0% | +0.69 (z +2.68) helps * | -0.44% ±0.72% | -0.52% ±1.03% |
| 16 | `escort-unstick` | Release a settler's linked escort after two turns without progress so the settler marches on by itself. | 1 | **on** | +8 | +46 | -25 | v1 17.00% (n=95,565) · v2 16.33% (n=13,748) | v1 16.34% (n=96,609) · v2 16.79% (n=39,142) | 0.66% | +31 [+8, +54] | 99.6% | +0.11 (z +0.46) ~ | -0.01% ±0.81% | -0.17% ±1.19% |
| 17 | `bounded-recovery` | Let the defensive Recovery posture expire after a turn limit instead of trapping the empire in it permanently. | — | **on** | +13 | -14 | +8 | 16.91% (n=109,248) | 16.35% (n=82,926) | 0.56% | +29 [+13, +46] | 100.0% | -0.38 (z -1.42) ~ | +0.18% ±0.77% | +0.80% ±1.14% |
| 18 | `barbarian-scouts-are-scouts` | Stop pricing a barbarian Scout as a threat, since it never attacks or captures; settlers and scouts ignore it. | — | **on** | +40 | +25 | -15 | 16.91% (n=109,366) | 16.35% (n=82,808) | 0.56% | +31 [+11, +52] | 99.8% | -0.06 (z -0.22) ~ | -0.55% ±0.69% | -0.79% ±1.02% |
| 19 | `buildings-before-projects` | Make a repeatable district project wait behind the science and production buildings the city can already build. | — | **on** | -12 | +4 | +27 | 16.90% (n=109,505) | 16.36% (n=82,669) | 0.55% | +28 [+8, +47] | 99.8% | +0.40 (z +1.47) ~ | -0.11% ±0.76% | -0.47% ±1.09% |
| 20 | `holy-lane-parity` | Price the Religion lane's Holy Site at what the Culture lane pays for its Theater Square. | — | **on** | -15 | +18 | -47 | 16.89% (n=94,083) | 16.36% (n=67,599) | 0.53% | +20 [-16, +57] | 86.3% | -0.19 (z -0.70) ~ | -0.12% ±0.75% | -0.19% ±1.14% |
| 21 | `war-economy` | Route an adaptive plan that switched to Conquest through the war production path instead of the basic governor. | — | **on** | -4 | -12 | +41 | 16.89% (n=109,448) | 16.38% (n=82,726) | 0.51% | +16 [-65, +96] | 64.9% | +1.00 (z +3.93) helps * | +1.98% ±0.76% | +2.21% ±1.13% |
| 22 | `missionary-last-charge-explores` | Let a Missionary on its last charge explore nearby fog for a few turns before spending it. | — | **on** | +3 | +20 | +12 | 16.93% (n=7,031) | 16.43% (n=7,699) | 0.49% | +30 [-37, +96] | 80.7% | -0.22 (z -1.01) ~ | -0.37% ±0.60% | +0.23% ±0.85% |
| 23 | `culture-building-debt` | Make a Theater Square owe its Amphitheater, Museum and Broadcast Center the way a Campus owes its buildings. | — | **on** | +25 | -1 | +12 | 16.85% (n=63,275) | 16.36% (n=36,859) | 0.49% | +26 [+3, +50] | 98.5% | +0.26 (z +1.01) ~ | +0.43% ±0.77% | -0.23% ±1.16% |
| 24 | `settler-threat-detour` | Send a Settler to the best safe alternative site when a visible threat blocks its route. | — | **on** | +19 | +5 | +6 | 16.85% (n=80,994) | 16.40% (n=54,288) | 0.45% | +25 [+5, +45] | 99.3% | +0.20 (z +0.70) ~ | +2.59% ±0.78% | +3.28% ±1.21% |
| 25 | `idle-faith-patronage` | Let a seat with no religion and 600+ banked Faith patronize Great People whatever the points shortfall. | — | **on** | -18 | -7 | -4 | 16.84% (n=86,878) | 16.42% (n=60,404) | 0.42% | +24 [+10, +38] | 100.0% | -0.08 (z -0.29) ~ | +0.68% ±0.79% | +0.93% ±1.15% |
| 26 | `promote-when-wounded` | Defer a unit's promotion until it is wounded enough to use the promotion's heal instead of wasting it. | — | **on** | +49 | +38 | -10 | 16.85% (n=30,161) | 16.43% (n=22,729) | 0.42% | +43 [-21, +108] | 90.5% | -0.04 (z -0.17) ~ | +0.92% ±0.77% | +1.34% ±1.12% |
| 27 | `settlement-gap-target` | Make the settlement-gap redirect and the Settler ranking read the same city target as the baseline cascade. | — | **on** | +17 | +11 | +29 | 16.87% (n=26,048) | 16.47% (n=26,842) | 0.41% | +21 [-11, +54] | 90.3% | +0.35 (z +1.60) ~ | +1.10% ±0.65% | +2.13% ±0.93% |
| 28 | `recorded-tactical-step` | Record each tactical step so a unit moved twice in one turn cannot return to the tile it just left. | — | **on** | +36 | +38 | +22 | 16.84% (n=109,258) | 16.44% (n=82,916) | 0.40% | +21 [+5, +37] | 99.4% | -0.15 (z -0.54) ~ | +0.56% ±0.81% | +1.31% ±1.17% |
| 29 | `score-horizon` | Skip a space race or Manhattan Project that cannot finish before the turn limit ends the game. | — | **on** | +8 | +30 | +28 | 16.83% (n=109,284) | 16.46% (n=82,890) | 0.37% | +19 [+3, +36] | 99.0% | +0.63 (z +2.27) helps * | +0.70% ±0.80% | +1.17% ±1.11% |
| 30 | `barbarian-bargain` | Price a fight against a barbarian below a fight against a major, since barbarians carry no war costs. | — | **on** | +3 | +24 | +17 | 16.81% (n=80,860) | 16.45% (n=54,422) | 0.36% | +18 [-1, +38] | 96.5% | +0.36 (z +1.35) ~ | -0.06% ±0.75% | +0.04% ±1.06% |
| 31 | `camp-party` | In peacetime let the whole army answer home threats, ranking a nearby barbarian camp above countryside raiders. | — | **on** | -30 | +20 | -8 | 16.84% (n=99,742) | 16.48% (n=92,432) | 0.36% | +19 [-4, +42] | 94.6% | -0.40 (z -1.51) ~ | -0.58% ±0.70% | -0.30% ±1.01% |
| 32 | `competition-victory-points` | Price first place in a scored competition by the Diplomatic Victory Points it pays. | — | **on** | +17 | +26 | -7 | 16.83% (n=53,713) | 16.48% (n=46,421) | 0.34% | +17 [-6, +39] | 92.4% | -0.08 (z -0.30) ~ | +0.24% ±0.78% | +0.97% ±1.13% |
| 33 | `peacetime-deterrence` | Let the strongest met major raise the army target in peacetime, so deterrence exists before any declaration. | — | **on** | -47 | +26 | -5 | 16.80% (n=109,183) | 16.49% (n=82,991) | 0.31% | +16 [-3, +35] | 95.3% | -0.00 (z -0.01) ~ | -0.61% ±0.82% | -0.66% ±1.21% |
| 34 | `unit-cost-efficiency` | Credit strength per production and the civilization's own unique unit when pricing military production. | — | **on** | +9 | +1 | -4 | 16.79% (n=30,018) | 16.51% (n=22,872) | 0.28% | +15 [-18, +48] | 80.6% | +0.23 (z +0.90) ~ | -0.43% ±0.66% | -0.55% ±0.93% |
| 35 | `slot-kind-tiebreak` | Break a production cost tie between museums by which great-work slots the empire can actually fill. | — | **on** | +10 | +17 | +59 | 16.80% (n=99,723) | 16.53% (n=92,451) | 0.27% | +15 [-4, +34] | 93.7% | +0.22 (z +0.81) ~ | -0.76% ±0.67% | -0.64% ±0.95% |
| 36 | `pantheon-board` | Choose the pantheon by what it would pay on the tiles the empire owns, not from a fixed order. | — | off | -37 | +83 | -19 | 16.82% (n=23,715) | 16.54% (n=29,175) | 0.27% | +21 [-86, +128] | 65.1% | +0.02 (z +0.07) ~ | -0.34% ±0.65% | -0.87% ±0.93% |
| 37 | `founder-temple` | Have a founder outside the Religion lane still build the Shrine and Temple an Apostle needs. | — | **on** | -15 | +36 | -51 | 16.78% (n=87,117) | 16.51% (n=60,165) | 0.27% | +17 [-10, +45] | 89.6% | -0.39 (z -1.44) ~ | -0.07% ±0.78% | -0.14% ±1.09% |
| 38 | `flip-nearby-city-states` | Add a city-state's proximity and hostile suzerain to the envoy score, amortised over envoys the flip needs. | — | off | +15 | -3 | – | 16.87% (n=2,531) | 16.60% (n=7,933) | 0.27% | +14 [-71, +98] | 62.5% | +0.01 (z +0.04) ~ | +0.96% ±0.93% | +1.35% ±1.22% |
| 39 | `war-reinforcement` | Keep marching newly built rear units to the campaign objective after war is declared, not only before. | — | **on** | +15 | -37 | -10 | 16.78% (n=109,330) | 16.52% (n=82,844) | 0.26% | +12 [-10, +34] | 85.9% | -0.64 (z -2.33) hurts * | +0.26% ±0.85% | -0.03% ±1.23% |
| 40 | `one-launch-pad` | Let only one city at a time claim the 3,000-point first Spaceport bonus, instead of every city at once. | — | **on** | -12 | +21 | +28 | 16.79% (n=99,835) | 16.53% (n=92,339) | 0.26% | +13 [-3, +28] | 94.2% | +0.00 (z +0.01) ~ | +0.27% ±0.79% | +0.59% ±1.18% |
| 41 | `strike-opening` | Credit a movement tile for the attack it opens next, not only charge it for the threat it accepts. | — | **on** | +16 | +22 | -10 | 16.77% (n=109,320) | 16.53% (n=82,854) | 0.25% | +13 [-3, +29] | 94.6% | +0.28 (z +1.03) ~ | +0.18% ±0.69% | -0.23% ±0.95% |
| 42 | `wonder-score-tally` | Let any civilization build wonders on merit by pricing the fifteen score points a finished wonder pays. | — | off | +1 | +3 | +13 | 16.83% (n=4,754) | 16.59% (n=9,976) | 0.24% | +13 [-53, +79] | 64.7% | +0.30 (z +1.27) ~ | +0.13% ±0.65% | +0.20% ±0.97% |
| 43 | `stranded-settler-discount` | Discount a Settler that has stopped walking from the expansion gate, and found where it stands when stalled. | — | off | -3 | +19 | -3 | 16.79% (n=93,484) | 16.55% (n=98,690) | 0.23% | +11 [-5, +27] | 91.6% | +0.18 (z +0.80) ~ | +0.54% ±0.64% | +0.58% ±0.91% |
| 44 | `science-multiplier-payoff` | Credit a Campus building the science its city's multipliers will actually pay, not its raw spec yield. | — | **on** | +53 | +1 | +20 | 16.78% (n=49,768) | 16.55% (n=50,366) | 0.23% | +18 [-16, +52] | 84.8% | -0.15 (z -0.69) ~ | +0.18% ±0.72% | +0.72% ±1.03% |
| 45 | `barbarian-ranged-answer` | Build a ranged defender, not a melee one, when the barbarian ring around a city is mostly shooters. | — | off | +34 | +15 | -9 | 16.77% (n=74,439) | 16.54% (n=60,843) | 0.23% | +13 [-7, +33] | 89.9% | -0.16 (z -0.68) ~ | -0.81% ±0.70% | -1.09% ±0.93% |
| 46 | `relief-targets-the-siege` | Send a relief force at the besiegers actually damaging the city, not the enemy nearest the force. | — | **on** | +17 | +11 | +22 | 16.77% (n=109,351) | 16.54% (n=82,823) | 0.23% | +12 [-5, +28] | 91.9% | -0.06 (z -0.24) ~ | -1.14% ±0.78% | -1.59% ±1.05% |
| 47 | `whole-turn-backtrack-guard` | Refuse any step onto a tile the unit already stood on this turn, closing three-hop loops too. | — | off | +5 | +11 | +6 | 16.77% (n=103,069) | 16.55% (n=89,105) | 0.22% | +12 [-4, +28] | 92.9% | -0.10 (z -0.48) ~ | +0.18% ±0.61% | +0.20% ±0.94% |
| 48 | `district-planning` | Plan a city's districts, their plots and tile purchases jointly, reserving plots in rings one to three. | — | **on** | -30 | +8 | +33 | 16.78% (n=26,056) | 16.56% (n=26,834) | 0.21% | +11 [-21, +44] | 75.0% | +0.24 (z +1.06) ~ | -0.21% ±0.69% | -0.60% ±0.99% |
| 49 | `amenity-district-path` | Price an amenity district by the building it will hold, and a regional amenity building by every city it reaches. | — | **on** | +17 | -3 | -7 | 16.76% (n=109,149) | 16.55% (n=83,025) | 0.21% | +11 [-5, +27] | 91.1% | -0.00 (z -0.00) ~ | +0.14% ±0.74% | +0.16% ±1.05% |
| 50 | `religion-sues-peace` | Have a Religion strategy offer peace to every at-war major so its missionaries can reach their cities. | — | **on** | +8 | +30 | +54 | 16.77% (n=99,651) | 16.56% (n=92,523) | 0.21% | +15 [-7, +37] | 91.2% | +0.27 (z +1.07) ~ | -0.45% ±0.77% | -0.30% ±1.06% |
| 51 | `lane-culture-spending` | Run the Culture lane's Faith purchases, Naturalists and Rock Bands, for an empire racing Culture under an Expansion plan. | — | **on** | -7 | +25 | +2 | 16.76% (n=53,674) | 16.56% (n=46,460) | 0.20% | +10 [-12, +33] | 81.1% | +0.04 (z +0.15) ~ | +0.69% ±0.77% | +0.67% ±1.16% |
| 52 | `strategic-wonders` | Price a wonder's effects in the victory lane's currency and build the ones that lane needs. | — | off | -13 | +32 | +9 | 16.77% (n=93,545) | 16.57% (n=98,629) | 0.19% | +10 [-6, +26] | 88.9% | -0.25 (z -1.10) ~ | -0.27% ±0.65% | -0.55% ±0.94% |
| 53 | `blind-objective-strength` | Price a fogged objective city from its last sighting instead of treating unseen ground as empty. | — | off | +68 | -41 | +9 | 16.77% (n=93,198) | 16.57% (n=98,976) | 0.19% | +12 [-16, +40] | 79.6% | +0.31 (z +1.37) ~ | -0.38% ±0.60% | -0.97% ±0.83% |
| 54 | `lane-policy-deck` | Choose policy cards for the victory the empire is racing while its plan is still Expansion. | — | **on** | -16 | -11 | +5 | 16.76% (n=49,871) | 16.58% (n=50,263) | 0.18% | +8 [-14, +31] | 77.1% | -0.04 (z -0.19) ~ | -0.99% ±0.75% | -1.09% ±1.03% |
| 55 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire needs rather than the largest number on the card. | — | **on** | -4 | +67 | -29 | 16.74% (n=109,425) | 16.56% (n=82,749) | 0.18% | +12 [-18, +41] | 78.0% | -0.14 (z -0.51) ~ | +0.41% ±0.73% | +0.75% ±1.05% |
| 56 | `religious-units-heal-first` | Let a wounded spreader in its own Holy Site's heal ring hold and heal instead of spending a weak charge. | — | **on** | +14 | +24 | -33 | 16.75% (n=53,567) | 16.57% (n=46,567) | 0.18% | +10 [-12, +32] | 80.5% | -0.74 (z -2.68) hurts * | -0.65% ±0.78% | -0.37% ±1.13% |
| 57 | `siege-tracks-wall` | Size the siege train by the wall at the target city instead of always asking for one siege unit. | — | off | +28 | -1 | +33 | 16.76% (n=93,580) | 16.58% (n=98,594) | 0.18% | +11 [-9, +32] | 86.0% | +0.23 (z +1.02) ~ | -0.04% ±0.65% | -0.33% ±1.00% |
| 58 | `garrison-under-fire` | Treat a city that is losing hitpoints as besieged even when fog hides every attacker. | — | off | -29 | +3 | -29 | 16.75% (n=93,426) | 16.59% (n=98,748) | 0.17% | +7 [-22, +36] | 67.6% | -0.23 (z -1.03) ~ | -0.16% ±0.65% | -0.40% ±0.95% |
| 59 | `settler-guard-holds` | Count a stacked guard as protection only when it can hold, and make it stay with its settler. | — | off | +22 | +36 | +26 | 16.75% (n=93,330) | 16.59% (n=98,844) | 0.16% | +7 [-9, +23] | 80.8% | +0.48 (z +2.08) helps * | -0.66% ±0.65% | -0.94% ±0.99% |
| 60 | `come-ashore` | Keep land units out of the water: no water exploration goals, and disembark units already at sea. | — | off | +26 | +38 | -28 | 16.74% (n=102,963) | 16.58% (n=89,211) | 0.16% | +9 [-10, +28] | 82.6% | +0.07 (z +0.29) ~ | +0.05% ±0.64% | -0.06% ±0.93% |
| 61 | `lane-great-people` | Rank Great Person classes and project points by the victory the empire is racing, even during a war. | — | **on** | -11 | +1 | -28 | 16.74% (n=53,652) | 16.58% (n=46,482) | 0.16% | +7 [-16, +30] | 72.7% | -0.03 (z -0.12) ~ | +0.47% ±0.74% | +0.44% ±1.13% |
| 62 | `early-contact-window` | Buy the second and third Scout early, before Early Empire closes borders and city-states become unreachable. | — | **on** | -11 | +14 | -4 | 16.74% (n=53,593) | 16.59% (n=46,541) | 0.15% | +7 [-15, +30] | 73.9% | -0.14 (z -0.54) ~ | +1.40% ±0.72% | +1.31% ±1.06% |
| 63 | `pillage-to-heal` | Let a unit at or below 65 health pillage a healing improvement on or beside its tile before retreating. | — | off | +36 | -23 | – | 16.77% (n=2,677) | 16.63% (n=7,787) | 0.14% | +15 [-137, +167] | 57.5% | +0.02 (z +0.11) ~ | +0.75% ±0.90% | +1.68% ±1.19% |
| 64 | `research-tier-premium` | Scale a missing Campus building's debt by its own science yield, so Universities and Labs outrank Libraries. | — | off | -4 | +10 | -28 | 16.74% (n=47,572) | 16.60% (n=52,562) | 0.13% | +6 [-17, +28] | 69.1% | +0.09 (z +0.36) ~ | -0.87% ±0.60% | -1.25% ±0.85% |
| 65 | `inquisition-on-threat` | Let a founder under conversion pressure buy one Apostle to launch the Inquisition after its Missionaries. | — | **on** | -6 | +19 | -14 | 16.72% (n=86,856) | 16.59% (n=60,426) | 0.13% | +9 [-9, +27] | 83.7% | +0.39 (z +1.46) ~ | -0.98% ±0.73% | -1.16% ±1.02% |
| 66 | `wonder-ring-settle-value` | Credit a settle site for the modeled appeal and yields of a revealed natural wonder's neighbouring tiles. | — | off | +8 | -7 | +61 | 16.73% (n=103,103) | 16.60% (n=89,071) | 0.13% | +7 [-12, +26] | 77.4% | +0.31 (z +1.33) ~ | -1.08% ±0.59% | -1.10% ±0.84% |
| 67 | `unit-objective-memory` | Let a unit remember its campaign objective, dangerous approaches and a short retreat commitment across turns. | — | **on** | +16 | -18 | +1 | 16.72% (n=30,014) | 16.60% (n=22,876) | 0.12% | +6 [-27, +39] | 64.9% | -0.19 (z -0.71) ~ | +0.85% ±0.75% | +1.14% ±1.10% |
| 68 | `enhancer-for-the-corps` | Choose the enhancer beliefs that multiply religious spread while the corps has a job to do. | — | off | -10 | +35 | +6 | 16.73% (n=47,375) | 16.61% (n=52,759) | 0.11% | +6 [-17, +28] | 69.4% | -0.06 (z -0.25) ~ | -0.02% ±0.64% | +0.04% ±0.93% |
| 69 | `campus-adjacency-threshold` | Credit a Campus plot that reaches the Rationalism adjacency threshold with the science bonus crossing it unlocks. | — | off | +34 | +33 | -14 | 16.72% (n=47,363) | 16.62% (n=52,771) | 0.11% | +12 [-24, +49] | 74.5% | +0.13 (z +0.56) ~ | -0.74% ±0.62% | -1.30% ±0.90% |
| 70 | `missionary-evades-raiders` | Keep religious units out of every tile a visible barbarian raider can reach next turn. | — | **on** | -12 | -6 | +30 | 16.72% (n=6,961) | 16.62% (n=7,769) | 0.10% | +6 [-61, +72] | 56.6% | +0.38 (z +1.65) ~ | +1.01% ±0.63% | +0.63% ±0.84% |
| 71 | `power-the-laboratory` | Credit a power plant the powered yields it switches on, above all the Research Lab's extra science. | — | off | +10 | +12 | +15 | 16.72% (n=47,534) | 16.62% (n=52,600) | 0.10% | +4 [-18, +26] | 63.2% | +0.16 (z +0.69) ~ | +1.12% ±0.72% | +1.09% ±1.05% |
| 72 | `army-target-weighs-enemy` | Raise the wartime army target when the enemy outweighs us, instead of counting only our own cities. | — | off | -35 | +30 | +2 | 16.71% (n=103,009) | 16.62% (n=89,165) | 0.10% | +4 [-17, +25] | 64.7% | -0.00 (z -0.01) ~ | +0.74% ±0.75% | +1.06% ±0.96% |
| 73 | `lane-space-race` | Open the Spaceport and launch pass for an empire racing Science even while its plan is still Expansion. | — | off | +53 | +8 | -15 | 16.72% (n=47,588) | 16.62% (n=52,546) | 0.09% | +7 [-24, +39] | 67.6% | -0.12 (z -0.51) ~ | +0.46% ±0.70% | +0.47% ±1.01% |
| 74 | `fortify-idle-units` | Fortify any unit the planner gave nothing to do, not only one in a stand-down window. | — | off | +51 | +17 | +29 | 16.72% (n=23,844) | 16.63% (n=29,046) | 0.09% | +37 [-30, +105] | 86.1% | -0.00 (z -0.00) ~ | +0.08% ±0.60% | +0.14% ±0.86% |
| 75 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the ranking for several turns instead of re-picking it immediately. | — | off | -10 | +39 | +39 | 16.71% (n=93,500) | 16.63% (n=98,674) | 0.08% | +4 [-13, +22] | 68.9% | +0.17 (z +0.75) ~ | +0.23% ±0.70% | -0.09% ±1.00% |
| 76 | `amenity-project-preemption` | In a severe empire-wide Amenity crisis, pause one repeatable project for the amenity repair chain and slot Liberalism. | — | off | +67 | +24 | +3 | 16.70% (n=93,495) | 16.64% (n=98,679) | 0.06% | +11 [-18, +40] | 77.2% | +0.05 (z +0.21) ~ | +0.18% ±0.65% | -0.27% ±0.99% |
| 77 | `coordinated-finish` | Let a force finish a defender together with a friendly volley, without the rest of the tactical-strategy bundle. | — | off | -17 | +41 | -22 | 16.70% (n=23,833) | 16.64% (n=29,057) | 0.05% | +5 [-51, +62] | 56.9% | -0.37 (z -1.63) ~ | +0.38% ±0.66% | +0.37% ±0.88% |
| 78 | `district-coverage` | Rank each district family by how much of the empire still lacks it, so Theater Squares get built. | — | off | +13 | -26 | +34 | 16.68% (n=93,412) | 16.66% (n=98,762) | 0.02% | +1 [-19, +20] | 53.3% | +0.44 (z +1.90) ~ | +0.22% ±0.68% | +0.30% ±0.99% |
| 79 | `blind-objective-units` | Price the enemy units remembered near an unseen objective instead of reading a fogged approach as empty. | — | off | +31 | +21 | -62 | 16.67% (n=93,299) | 16.66% (n=98,875) | 0.01% | +0 [-16, +17] | 51.5% | -0.39 (z -1.64) ~ | +0.54% ±0.65% | +0.22% ±0.87% |
| 80 | `shoot-and-scoot` | Let a ranged unit inside melee reach step to a safer firing tile and shoot the threatening body. | — | off | -15 | +11 | – | 16.67% (n=2,621) | 16.66% (n=7,843) | 0.01% | +0 [-82, +82] | 50.2% | -0.18 (z -0.83) ~ | -0.08% ±0.88% | -0.20% ±1.19% |
| 81 | `theology-for-founders` | Have a founder research Theology next, after its first government, so it can build a Temple. | — | **on** | -44 | -3 | -10 | 16.67% (n=77,233) | 16.67% (n=70,049) | 0.00% | -0 [-19, +18] | 48.6% | +0.07 (z +0.27) ~ | -0.00% ±0.74% | -0.09% ±1.02% |
| 82 | `settler-site-agreement` | Ask the walker's own loyalty verdict on the chosen site before building a Settler for it. | — | off | +34 | +43 | -24 | 16.67% (n=93,445) | 16.67% (n=98,729) | -0.00% | +3 [-21, +28] | 60.9% | -0.05 (z -0.24) ~ | -1.57% ±0.65% | -1.40% ±0.91% |
| 83 | `civilian-rescue` | Walk onto any capturable civilian within reach, and always take back a Settler the barbarians hold. | — | **on** | -6 | +49 | -5 | 16.66% (n=99,518) | 16.67% (n=92,656) | -0.01% | -1 [-17, +15] | 46.4% | -0.20 (z -0.75) ~ | +0.14% ±0.76% | +0.14% ±1.11% |
| 84 | `builder-barbarian-safety` | Make a Builder retreat from, and never step into, a tile a visible barbarian can capture next turn. | — | off | -67 | +11 | -23 | 16.65% (n=65,124) | 16.68% (n=70,158) | -0.03% | -9 [-43, +24] | 29.4% | -0.10 (z -0.40) ~ | -0.41% ±0.68% | -0.64% ±0.97% |
| 85 | `holy-site-where-the-threat-is` | Build a Holy Site in the city losing its religious majority so defenders can be bought there directly. | — | off | +7 | -15 | +4 | 16.64% (n=47,574) | 16.69% (n=52,560) | -0.04% | -4 [-26, +18] | 36.7% | -0.03 (z -0.11) ~ | -0.05% ±0.62% | +0.03% ±0.87% |
| 86 | `barbarian-capture-priority` | Capture a visible barbarian-held Settler or Scout within one-turn reach before healing, retreating or any other move. | — | off | +26 | +13 | -11 | 16.64% (n=47,271) | 16.69% (n=52,863) | -0.05% | -2 [-25, +20] | 41.7% | +0.16 (z +0.69) ~ | -0.12% ±0.70% | -0.23% ±1.02% |
| 87 | `religious-defence-scales` | Size the defensive Missionary corps by cities actually under conversion pressure, up to four, instead of two. | — | **on** | -38 | -12 | +20 | 16.64% (n=49,691) | 16.69% (n=50,443) | -0.05% | -3 [-26, +19] | 38.8% | +0.16 (z +0.74) ~ | +0.06% ±0.68% | +0.20% ±1.02% |
| 88 | `housing-research` | Aim research at the technology that raises the housing ceiling while housing is throttling growth. | — | off | -16 | -33 | +11 | 16.64% (n=93,424) | 16.69% (n=98,750) | -0.06% | -3 [-24, +18] | 38.3% | +0.21 (z +0.94) ~ | -0.74% ±0.66% | -0.79% ±0.95% |
| 89 | `siege-commitment` | Keep the campaign aimed at a city it has already damaged instead of re-targeting a fresh one each turn. | — | off | -33 | -8 | +22 | 16.64% (n=93,502) | 16.69% (n=98,672) | -0.06% | -3 [-19, +13] | 35.7% | +0.15 (z +0.64) ~ | -0.20% ±0.66% | -0.31% ±0.97% |
| 90 | `naval-recon` | Buy one ship for a fleetless empire with unexplored water off its coast and send it exploring. | — | off | +3 | -10 | -16 | 16.63% (n=93,713) | 16.70% (n=98,461) | -0.07% | -4 [-20, +12] | 31.2% | -0.11 (z -0.48) ~ | -0.23% ±0.69% | -0.75% ±1.02% |
| 91 | `one-shot-recovery` | Withdraw a unit that one enemy blow could kill to safe healing ground, and leave when threatened again. | — | off | +30 | -13 | -48 | 16.63% (n=47,438) | 16.70% (n=52,696) | -0.08% | -5 [-34, +23] | 35.3% | -0.62 (z -2.76) hurts * | -0.39% ±0.64% | -0.82% ±0.95% |
| 92 | `endgame-war-runway` | Refuse a fresh direct war declaration once the endgame reserve leaves too few turns to capture a city. | — | off | -8 | -3 | +14 | 16.63% (n=93,631) | 16.71% (n=98,543) | -0.08% | -5 [-20, +11] | 28.3% | -0.06 (z -0.27) ~ | -0.63% ±0.63% | -0.84% ±0.93% |
| 93 | `culture-coverage` | Pay a coverage bonus for a Theater Square in every city that lacks one, as the Campus already gets. | — | off | +26 | -2 | +3 | 16.62% (n=47,311) | 16.70% (n=52,823) | -0.08% | -5 [-27, +17] | 32.4% | -0.22 (z -0.94) ~ | -0.83% ±0.65% | -0.88% ±0.96% |
| 94 | `condemn-under-congress` | Condemn a heretic whose religion the World Congress condemned, not only one belonging to a war enemy. | — | off | +24 | -10 | +30 | 16.62% (n=47,636) | 16.71% (n=52,498) | -0.09% | -5 [-27, +17] | 33.5% | -0.01 (z -0.06) ~ | -1.43% ±0.58% | -1.83% ±0.79% |
| 95 | `joint-tactics` | Plan an engagement's attacks jointly across all units by search instead of one unit at a time in class order. | — | off | – | – | – | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 96 | `guru-heals-the-corps` | Let a founder defending its own cities buy one Guru, the only unit that heals religious units. | — | off | +8 | +36 | -68 | 16.61% (n=47,392) | 16.72% (n=52,742) | -0.11% | -5 [-58, +48] | 43.4% | -0.34 (z -1.50) ~ | -0.53% ±0.69% | -0.63% ±0.98% |
| 97 | `campus-finishes-first` | Scale the Campus coverage bonus by how complete the empire's existing Campuses are, so Labs come before new Campuses. | — | off | -10 | -17 | -1 | 16.60% (n=47,382) | 16.72% (n=52,752) | -0.12% | -5 [-28, +17] | 32.0% | +0.30 (z +1.26) ~ | +0.31% ±0.64% | +0.59% ±0.94% |
| 98 | `congress-banks-decided` | Cast the free vote on an already-decided resolution's winner to bank the Diplomatic Victory Point for predicting it. | — | off | +18 | +27 | -55 | 16.60% (n=47,559) | 16.73% (n=52,575) | -0.13% | -6 [-40, +27] | 35.9% | -0.45 (z -1.90) ~ | +0.48% ±0.58% | +0.38% ±0.85% |
| 99 | `siege-is-progress` | Count damage dealt to an enemy city or its walls as campaign progress, so a winning siege is never stalled. | — | off | -1 | +8 | -5 | 16.60% (n=93,497) | 16.73% (n=98,677) | -0.13% | -8 [-30, +14] | 24.4% | -0.37 (z -1.66) ~ | +0.36% ±0.64% | +0.23% ±0.95% |
| 100 | `fifteenth-citizen` | Credit growth in a Campus city near the Rationalism population gate with the science bonus crossing it unlocks. | — | off | -25 | +16 | +26 | 16.60% (n=47,623) | 16.73% (n=52,511) | -0.13% | -7 [-29, +15] | 26.7% | -0.04 (z -0.15) ~ | -0.12% ±0.63% | -0.31% ±0.85% |
| 101 | `lane-congress-ballot` | Score the World Congress ballot for the victory the empire is racing while its plan is still Expansion. | — | off | +10 | +11 | -23 | 16.59% (n=47,624) | 16.73% (n=52,510) | -0.14% | -6 [-28, +16] | 30.3% | -0.27 (z -1.24) ~ | +0.11% ±0.66% | -0.05% ±0.94% |
| 102 | `deals-at-the-ceiling` | Price a trade quote at the counterparty's walk-away point less two Gold, falling back to the midpoint if refused. | — | off | -5 | -1 | – | 16.55% (n=2,622) | 16.70% (n=7,842) | -0.15% | -8 [-90, +74] | 42.6% | +0.09 (z +0.42) ~ | -1.03% ±0.87% | -1.88% ±1.25% |
| 103 | `builder-worked-tile-priority` | Prefer Builder jobs on tiles citizens currently work, keeping luxury and strategic resource connections at full priority. | — | off | +12 | -8 | -34 | 16.59% (n=65,164) | 16.74% (n=70,118) | -0.16% | -7 [-31, +16] | 27.5% | -0.32 (z -1.35) ~ | -0.40% ±0.69% | -0.59% ±0.94% |
| 104 | `housing-districts` | Let the baseline governor build the Aqueduct and Neighborhood districts that raise the housing ceiling. | — | off | +47 | -11 | +26 | 16.58% (n=93,446) | 16.75% (n=98,728) | -0.17% | -7 [-27, +14] | 25.9% | +0.10 (z +0.42) ~ | -0.92% ±0.72% | -0.76% ±1.03% |
| 105 | `envoy-infrastructure` | Value the Consulate, Chancery and Diplomatic Quarter by the envoys their influence can produce before the turn limit. | — | off | +4 | -43 | -6 | 16.58% (n=47,570) | 16.75% (n=52,564) | -0.17% | -9 [-32, +14] | 22.4% | -0.11 (z -0.47) ~ | +0.64% ±0.67% | +0.68% ±0.97% |
| 106 | `city-campaign` | Appraise weaker neighbours, plan to take one to three holdable cities the army can afford, and launch when staged. | — | off | -3 | – | – | 16.53% (n=1,113) | 16.71% (n=3,363) | -0.18% | -9 [-136, +118] | 44.5% | +0.04 (z +0.15) ~ | +0.92% ±0.97% | +1.48% ±1.41% |
| 107 | `home-defense` | Let hostile units inside our own territory claim defenders before the offensive campaign takes them. | — | off | -3 | +11 | -22 | 16.57% (n=93,816) | 16.76% (n=98,358) | -0.18% | -10 [-25, +6] | 11.6% | -0.19 (z -0.83) ~ | +0.04% ±0.65% | +1.03% ±0.99% |
| 108 | `coupled-expansion` | Price a Settler as an investment, subtracting production, population, escort, route and safety costs from the site's payback. | — | off | -66 | +40 | -1 | 16.55% (n=24,010) | 16.77% (n=28,880) | -0.22% | -19 [-108, +70] | 33.9% | +0.22 (z +0.94) ~ | +0.26% ±0.63% | +0.86% ±0.88% |
| 109 | `lane-congress-favor` | Stake Favor behind a World Congress ballot for the victory the empire is racing while its plan is Expansion. | — | off | -27 | +22 | -25 | 16.54% (n=47,256) | 16.78% (n=52,878) | -0.24% | -13 [-35, +9] | 13.0% | -0.16 (z -0.68) ~ | -0.37% ±0.70% | -0.86% ±0.94% |
| 110 | `war-patience` | Keep fighting a war the empire overwhelmingly outweighs instead of offering peace because it reads as stalled. | — | off | -6 | -9 | -25 | 16.54% (n=93,552) | 16.79% (n=98,622) | -0.25% | -13 [-29, +3] | 5.5% | +0.09 (z +0.39) ~ | -0.30% ±0.69% | -0.21% ±0.98% |
| 111 | `priced-tile-purchase` | Buy a border plot only when its priced benefit clears its Gold cost by a margin. | — | off | -21 | -43 | +57 | 16.53% (n=65,109) | 16.80% (n=70,173) | -0.27% | -16 [-48, +17] | 17.0% | +0.38 (z +1.67) ~ | -1.29% ±0.71% | -0.69% ±0.99% |
| 112 | `spread-campaign-persists` | Keep a spread campaign on the offensive between waves once it has converted a foreign city. | — | off | -39 | +16 | +23 | 16.52% (n=47,476) | 16.80% (n=52,658) | -0.29% | -15 [-40, +10] | 12.6% | +0.04 (z +0.16) ~ | -0.94% ±0.63% | -0.66% ±0.93% |
| 113 | `congress-counter-votes` | Back the counter-victory ballot with every Favor the treasury can spare, since a losing vote is refunded. | — | off | +51 | -34 | -5 | 16.51% (n=47,304) | 16.80% (n=52,830) | -0.29% | -13 [-51, +24] | 24.2% | +0.23 (z +1.01) ~ | -0.32% ±0.67% | +0.10% ±0.95% |
| 114 | `district-building-chain` | Make every specialty district owe the buildings inside it, whatever victory lane the empire is playing. | — | off | -17 | -20 | -6 | 16.50% (n=47,295) | 16.82% (n=52,839) | -0.32% | -15 [-37, +7] | 9.3% | +0.08 (z +0.34) ~ | +0.20% ±0.64% | +0.93% ±0.94% |
| 115 | `tactical-strategy` | Assign explicit battlefield roles: counter cycle, ranged standoff, siege against walls, escorts and cavalry jobs. | — | off | +27 | -10 | -27 | 16.48% (n=23,908) | 16.82% (n=28,982) | -0.34% | -18 [-50, +15] | 14.1% | -0.60 (z -2.64) hurts * | -0.84% ±0.63% | -0.96% ±0.90% |
| 116 | `district-lookahead-settle` | Score a settle site by the districts the lane would actually build there, each on its own plot. | — | off | -46 | +6 | +21 | 16.48% (n=65,044) | 16.84% (n=70,238) | -0.36% | -18 [-48, +11] | 10.9% | +0.12 (z +0.54) ~ | +1.10% ±0.67% | +0.70% ±0.90% |
| 117 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one step away and clear it for the gold bounty. | — | off | -16 | +21 | +0 | 16.45% (n=65,016) | 16.87% (n=70,266) | -0.42% | -18 [-62, +27] | 21.8% | +0.05 (z +0.22) ~ | +0.23% ±0.69% | -0.08% ±0.96% |
| 118 | `escort-unstick-2` | Version 2 of escort-unstick: release a stalled settler's escort after two turns unless a visible barbarian raider can reach it. | 1 | off | +5 | -41 | +16 | v1 17.00% (n=95,565) · v2 16.33% (n=13,748) | v1 16.34% (n=96,609) · v2 16.79% (n=39,142) | -0.46% | -23 [-65, +19] | 13.7% | -0.12 (z -0.43) ~ | -0.63% ±0.89% | -0.91% ±1.35% |
| 119 | `deals-for-our-gain` | Pick the trade quote with the best net value to us instead of the most balanced exchange. | — | off | -32 | +6 | – | 16.27% (n=2,655) | 16.80% (n=7,809) | -0.53% | -30 [-127, +68] | 27.7% | -0.26 (z -1.17) ~ | -1.15% ±0.87% | -1.94% ±1.14% |
| 120 | `campaign-pillage` | Let a soldier at war pillage the tile it stands on with movement its march does not use. | — | off | -12 | – | – | 16.21% (n=1,178) | 16.83% (n=3,298) | -0.61% | -31 [-154, +92] | 31.2% | -0.29 (z -1.24) ~ | +1.83% ±1.02% | +2.83% ±1.41% |
| 121 | `settle-plan-ahead` | Rank a settle site by the future city sites it leaves room for as well as its own ground. | — | off | -19 | -17 | -1 | 16.32% (n=23,779) | 16.95% (n=29,111) | -0.63% | -33 [-65, -0] | 2.4% | -0.27 (z -1.20) ~ | +0.30% ±0.71% | +0.74% ±1.01% |
| 122 | `lane-commit` | From mid-game commit an adaptive seat to the victory lane it leads the field in, instead of re-picking each turn. | — | off | -21 | -6 | – | 16.17% (n=2,622) | 16.83% (n=7,842) | -0.66% | -34 [-116, +47] | 20.3% | +0.12 (z +0.56) ~ | -0.70% ±0.88% | -0.65% ±1.24% |
| 123 | `no-free-passage` | Stop bundling free one-way Open Borders into friendship and alliance proposals; sell passage through the quote lane. | — | off | +12 | -36 | – | 16.05% (n=2,641) | 16.87% (n=7,823) | -0.82% | -37 [-162, +88] | 28.3% | -0.38 (z -1.78) ~ | -0.09% ±0.87% | -0.21% ±1.18% |
| 124 | `builder-reward-survey` | Price a Builder by surveying the improvement jobs it would actually do, not by a city-count quota. | — | off | +13 | -21 | -94 | 16.10% (n=23,890) | 17.13% (n=29,000) | -1.03% | -64 [-136, +9] | 4.2% | -0.37 (z -1.62) ~ | +1.44% ±0.61% | +1.44% ±0.89% |
| 125 | `pass-picket` | Station an idle recon unit on the chokepoint tile of the land route toward a neighbour, or watch their border. | — | off | -20 | – | – | 15.82% (n=1,068) | 16.93% (n=3,408) | -1.11% | -55 [-181, +71] | 19.4% | +0.18 (z +0.76) ~ | +0.30% ±0.93% | +0.35% ±1.35% |
| 126 | `contact-posture` | Let a unit inside enemy reach choose to hold and heal, close on a shooter, or step out of range. | — | off | -72 | -17 | -34 | 16.03% (n=47,417) | 17.24% (n=52,717) | -1.21% | -64 [-92, -36] | 0.0% | -0.23 (z -1.00) ~ | -0.53% ±0.63% | -0.55% ±0.94% |
| 127 | `zoc-screen` | Stand an idle melee unit where its zone of control shields our shooters and wounded from the most enemy reaches. | — | off | -19 | -26 | – | 15.75% (n=2,641) | 16.98% (n=7,823) | -1.22% | -61 [-142, +19] | 6.8% | -0.36 (z -1.66) ~ | +1.52% ±0.87% | +2.33% ±1.21% |
| 128 | `naval-production-policy` | Slot the naval-production policy card while a coastal empire wants hulls it does not have. | — | off | -3 | -80 | -65 | 15.94% (n=23,829) | 17.26% (n=29,061) | -1.33% | -98 [-181, -14] | 1.1% | -0.59 (z -2.55) hurts * | -0.67% ±0.67% | -0.74% ±1.01% |
| 129 | `settler-screen` | Block a seen rival Settler with up to four nearby units standing on its likeliest paths to slow its founding. | — | off | -48 | – | – | 14.77% (n=1,124) | 17.30% (n=3,352) | -2.53% | -127 [-247, -6] | 2.0% | -0.24 (z -0.98) ~ | -0.84% ±0.98% | -0.73% ±1.45% |
| 130 | `fog-honest` | Plan the whole turn against a fog-redacted world and replay only the resulting orders on the real game. | 1 | off | -77 | -95 | -145 | v1 9.97% (n=2,296) · v2 3.35% (n=2,331) | v1 17.90% (n=12,434) · v2 19.17% (n=12,399) | -7.93% | -409 [-478, -339] | 0.0% | -1.27 (z -5.47) hurts * | -1.05% ±0.75% | -1.20% ±1.16% |
| 131 | `fog-honest-2` | Version 2 of fog-honest: the same redacted planning plus one re-plan from the real board when an order is refused. | 1 | off | -181 | -187 | -275 | v1 9.97% (n=2,296) · v2 3.35% (n=2,331) | v1 17.90% (n=12,434) · v2 19.17% (n=12,399) | -15.82% | -835 [-909, -760] | 0.0% | -4.32 (z -18.92) hurts * | +5.82% ±0.86% | +6.94% ±1.21% |

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
| `conversion-majority-alarm` | off (unmeasured) | Read a rival's religious clock from the cities it has converted rather than from whole civilizations already lost. | — |
| `culture-lane-forecast` | off (unmeasured) | Score the Culture lane by where the two tourist curves are when the clock stops. | — |
| `diplomatic-lane-forecast` | off (unmeasured) | Score the Diplomacy lane by when twenty Diplomatic Victory Points arrive along the Congress calendar, not by how many are banked. | — |
| `frontier-massing-alarm` | off (unmeasured) | Count a peacetime major's army massed near one of our cities toward that city's danger. | — |
| `naval-threat-triage` | off (unmeasured) | Ignore nearby barbarian ships that cannot land a meaningful blow, while still allowing ranged shots at them. | — |
| `science-chain-alarm` | off (unmeasured) | Read a rival's Science clock from the prerequisite chain it has climbed, not only from the launches it has made. | — |
| `science-victory-drive` | **on** (unmeasured) | When the empire leads the field in science, beeline the space-race chain, build launch-city production and race two pads early. | — |
| `surprise-war-mobilization` | off (unmeasured) | Convert the first six Standard-speed turns after a surprise war is declared against us into a bounded defensive mobilization. | — |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±/10k seats (last tracked measurement) | Win rate (on) | Win rate (off) | Source |
|---|---:|---:|---:|---|
| `governor-expansion-lane` | +69 | 17.36% | 15.97% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `research-floor-holds` | +60 | 17.27% | 16.08% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `science-payback-horizon` | +55 | 17.22% | 16.12% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `suzerain-cards` | +42 | 17.09% | 16.25% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `wonder-prereq-reach` | +29 | 16.96% | 16.38% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `chain-tech-lookahead` | +27 | 16.94% | 16.41% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `camp-reach` | +10 | 16.77% | 16.56% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-buildings` | +8 | 16.75% | 16.59% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `ranged-line-of-sight` | +4 | 16.71% | 16.63% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `recon-flight` | -1 | 16.66% | 16.67% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-cards` | -4 | 16.62% | 16.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `arrival-waves` | -7 | 16.59% | 16.74% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `idle-walkers-close-the-pipeline` | -10 | 16.56% | 16.77% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `muster-at-command-radius` | -12 | 16.55% | 16.79% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `barbarian-walls-one-tier` | -13 | 16.54% | 16.80% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `research-grants-first` | -20 | 16.46% | 16.87% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `siege-muster` | -26 | 16.41% | 16.93% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `siege-role` | -39 | 16.27% | 17.06% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `garrison-walls` | -54 | 16.12% | 17.21% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `loyalty-policy-defence` | -54 | 16.13% | 17.20% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `campus-every-city` | -94 | 15.73% | 17.60% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `stacked-escort` | -104 | 15.63% | 17.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `settler-stack-discipline` | -116 | 15.51% | 17.83% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `governor-every-lane` | -118 | 15.49% | 17.91% | `2026-08-24-standard-continuous-4266-total-seats.json` |
| `governor-victory-lanes` | -173 | 14.94% | 18.44% | `2026-08-24-standard-continuous-4266-total-seats.json` |

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

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 26,892 seats), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 2,000 seats), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 30,000 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 35,148 seats), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 14,400 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 47,244 seats), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 7,200 seats), `2026-08-24-standard-continuous-38160-total-seats.json` (standard, 38,160 seats). The fixed display batches are: `2026-08-25-standard-continuous-4476-total-seats.json` (4,476 seats), `2026-08-24-standard-continuous-5988-total-seats.json` (5,988 seats), `2026-08-24-standard-continuous-4266-total-seats.json` (4,266 seats). The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells are the operator's wins-per-ten-thousand-total-seat reporting view._
