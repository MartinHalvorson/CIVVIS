# The heuristic gene ranking

**Deployment default:** operator-pinned (57 genes): retains the prior 36 selections and explicitly promotes `unit-cost-efficiency`, `unit-objective-memory`, `camp-party`, `slot-kind-tiebreak`, `promote-when-wounded`, `religion-sues-peace`, `lane-great-people`, `one-launch-pad`, `civilian-rescue`, `missionary-evades-raiders`, `district-planning`, `missionary-last-charge-explores`, `settlement-gap-target`, `religious-defence-scales`, `lane-policy-deck`, `science-multiplier-payoff`, `science-victory-drive`, `solvency-first-trade-slot`, `settler-factory-coordination`, `one-war-at-a-time`, `religious-veto-defence`. Screen columns, *Diff*, and posterior values are evidence only; new batches do not automatically change defaults.

| Rank | Gene | Description | Best version | Default | P(>0) | Wins ± /10k total seats — Last Batch (n=30,000 total seats) | Wins ± /10k total seats — Prior Batch (n=4,476 total seats) | Wins ± /10k total seats — Third Batch (n=5,988 total seats) | Total (on) Win rate | Total (off) Win rate | Diff | cost (compute) | cost (time) |
|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `solvency-first-trade-slot` | Reserve the first empty trade route slot ahead of ordinary production in any city that can start a safe route. | — | **on** | 100.0% | +125 | +144 | – | 21.77% (n=8,632) | 14.96% (n=25,844) | 6.80% | +2.60% ±1.00% | +2.66% ±1.40% |
| 2 | `air-surge` | Beeline Advanced Flight, build an Aerodrome and bombers, and take the appointed city with cavalry behind them. | — | **on** | 100.0% | +33 | +3 | +30 | 17.27% (n=82,562) | 15.52% (n=43,306) | 1.74% | +0.79% ±0.88% | +0.63% ±1.19% |
| 3 | `great-person-housing` | Reserve a city to build whatever unblocks an earned Great Person, selling duplicate works to make room. | — | **on** | 100.0% | +37 | +29 | +29 | 17.27% (n=100,192) | 15.67% (n=60,824) | 1.60% | +0.10% ±0.80% | +0.05% ±1.08% |
| 4 | `flip-nearby-city-states` | Add a city-state's proximity and hostile suzerain to the envoy score, amortised over envoys the flip needs. | — | off | 99.4% | +31 | +15 | -3 | 17.64% (n=10,059) | 16.35% (n=30,405) | 1.29% | +0.96% ±0.93% | +1.35% ±1.22% |
| 5 | `religious-veto-defence` | Scale religious defence with a rival's progress toward religious victory, and walk the Inquisitor to the heresy. | — | **on** | 99.7% | +24 | +31 | +8 | 17.56% (n=10,188) | 16.37% (n=30,276) | 1.19% | +1.25% ±0.87% | +2.13% ±1.12% |
| 6 | `wide-map-capacity` | Price the city ceiling off the passable land actually visible, at one city per 45 tiles, capped at twelve. | — | **on** | 100.0% | +16 | +16 | +41 | 17.14% (n=128,651) | 15.99% (n=89,257) | 1.15% | +0.39% ±0.85% | +0.90% ±1.14% |
| 7 | `opportunistic-war` | Open a surprise war on a neighbour whose Settlers, Builders or tiles lie exposed nearby, then sue for peace. | — | **on** | 100.0% | +46 | +16 | +35 | 17.10% (n=100,353) | 15.96% (n=60,663) | 1.14% | +0.82% ±0.88% | +0.80% ±1.17% |
| 8 | `engine-faith-price` | Read Faith purchase prices from the engine at the game's speed and discounts, instead of a Standard-speed literal. | — | **on** | 100.0% | +13 | +51 | +28 | 17.07% (n=49,347) | 15.99% (n=29,277) | 1.09% | -0.57% ±0.83% | -0.35% ±1.14% |
| 9 | `raid-pillage-prizes` | Count a neighbour's unpillaged improvements within reach as raid prizes and send raiders to pillage them. | — | **on** | 100.0% | +18 | +47 | +11 | 17.05% (n=100,249) | 16.04% (n=60,767) | 1.00% | +1.13% ±0.87% | +1.28% ±1.20% |
| 10 | `maintenance-aware-deck` | Subtract the unit-maintenance bill inside the policy counterfactual so maintenance-discount cards score above zero. | — | **on** | 98.9% | +23 | +15 | -22 | 17.03% (n=49,382) | 16.05% (n=29,242) | 0.98% | +0.57% ±0.86% | +1.03% ±1.17% |
| 11 | `recon-replacement` | Rebuild the recon arm when every scout is gone and unexplored ground remains to chart. | — | **on** | 99.9% | -11 | +3 | +35 | 16.98% (n=128,710) | 16.21% (n=89,198) | 0.77% | +0.87% ±0.88% | +0.65% ±1.18% |
| 12 | `one-war-at-a-time` | Fight one campaign front at a time, seeking peace with every other major and once the front turns against us. | — | **on** | 95.6% | +12 | +16 | +21 | 17.21% (n=10,138) | 16.48% (n=30,326) | 0.73% | +0.14% ±0.87% | +0.79% ±1.14% |
| 13 | `price-the-suzerainty` | Credit the envoy scorer with the resources, bonuses and points a suzerainty pays, amortised over envoys still needed. | — | **on** | 99.6% | +7 | +7 | -1 | 16.93% (n=49,460) | 16.22% (n=29,164) | 0.71% | -1.65% ±0.91% | -2.65% ±1.18% |
| 14 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns until the city flips rather than by its current loyalty level. | — | **on** | 100.0% | +4 | -3 | +18 | 16.94% (n=128,303) | 16.27% (n=89,605) | 0.67% | +0.78% ±0.83% | +1.17% ±1.11% |
| 15 | `settle-sooner` | Price a Settler's walk per turn, rising the longer it has walked, so it settles sooner. | — | **on** | 100.0% | +20 | +2 | -3 | 16.92% (n=100,411) | 16.25% (n=60,605) | 0.66% | -0.04% ±0.86% | -0.43% ±1.15% |
| 16 | `escort-unstick` | Release a settler's linked escort after two turns without progress so the settler marches on by itself. | — | **on** | 99.9% | +5 | +8 | +46 | 16.99% (n=106,857) | 16.36% (n=111,051) | 0.63% | +0.24% ±0.96% | -0.81% ±1.23% |
| 17 | `barbarian-scouts-are-scouts` | Stop pricing a barbarian Scout as a threat, since it never attacks or captures; settlers and scouts ignore it. | — | **on** | 100.0% | +16 | +40 | +25 | 16.91% (n=128,539) | 16.31% (n=89,369) | 0.60% | +1.13% ±0.84% | +0.76% ±1.15% |
| 18 | `buildings-before-projects` | Make a repeatable district project wait behind the science and production buildings the city can already build. | — | **on** | 99.7% | +5 | -12 | +4 | 16.87% (n=128,731) | 16.37% (n=89,177) | 0.50% | +0.59% ±0.83% | +0.58% ±1.13% |
| 19 | `idle-faith-patronage` | Let a seat with no religion and 600+ banked Faith patronize Great People whatever the points shortfall. | — | **on** | 100.0% | +17 | -18 | -7 | 16.86% (n=106,255) | 16.36% (n=66,761) | 0.49% | +0.67% ±0.96% | +1.05% ±1.29% |
| 20 | `bounded-recovery` | Let the defensive Recovery posture expire after a turn limit instead of trapping the empire in it permanently. | — | **on** | 99.9% | -7 | +13 | -14 | 16.85% (n=128,564) | 16.40% (n=89,344) | 0.45% | -1.35% ±0.89% | -1.71% ±1.22% |
| 21 | `holy-lane-parity` | Price the Religion lane's Holy Site at what the Culture lane pays for its Theater Square. | — | **on** | 89.4% | -11 | -15 | +18 | 16.84% (n=113,428) | 16.40% (n=73,988) | 0.44% | +0.34% ±0.83% | +0.88% ±1.11% |
| 22 | `science-victory-drive` | When the empire leads the field in science, beeline the space-race chain, build launch-city production and race two pads early. | — | **on** | 79.1% | +8 | – | – | 16.77% (n=22,407) | 16.37% (n=7,593) | 0.40% | -0.23% ±0.51% | -0.39% ±0.58% |
| 23 | `founder-temple` | Have a founder outside the Religion lane still build the Shrine and Temple an Apostle needs. | — | **on** | 99.5% | +14 | -15 | +36 | 16.82% (n=106,408) | 16.43% (n=66,608) | 0.39% | -1.21% ±0.84% | -1.43% ±1.10% |
| 24 | `war-economy` | Route an adaptive plan that switched to Conquest through the war production path instead of the basic governor. | — | **on** | 51.7% | -8 | -4 | -12 | 16.82% (n=128,882) | 16.44% (n=89,026) | 0.38% | +0.61% ±0.84% | +1.30% ±1.15% |
| 25 | `diplomatic-lane-forecast` | Score the Diplomacy lane by when twenty Diplomatic Victory Points arrive along the Congress calendar, not by how many are banked. | — | off | 77.4% | +7 | – | – | 16.95% (n=7,511) | 16.57% (n=22,489) | 0.38% | +0.73% ±0.53% | +0.76% ±0.60% |
| 26 | `peacetime-deterrence` | Let the strongest met major raise the army target in peacetime, so deterrence exists before any declaration. | — | **on** | 98.0% | +16 | -47 | +26 | 16.82% (n=128,521) | 16.45% (n=89,387) | 0.37% | -0.00% ±0.91% | +0.77% ±1.22% |
| 27 | `recorded-tactical-step` | Record each tactical step so a unit moved twice in one turn cannot return to the tile it just left. | — | **on** | 99.2% | +3 | +36 | +38 | 16.81% (n=128,453) | 16.45% (n=89,455) | 0.36% | -0.03% ±0.85% | -0.18% ±1.16% |
| 28 | `settler-factory-coordination` | Give early Settler pipeline slots to cities that finish fastest and hold distinct reachable claim sites. | — | **on** | 77.4% | +3 | +35 | – | 16.93% (n=8,741) | 16.58% (n=25,735) | 0.36% | +1.56% ±0.91% | +2.58% ±1.30% |
| 29 | `score-horizon` | Skip a space race or Manhattan Project that cannot finish before the turn limit ends the game. | — | **on** | 99.0% | +7 | +8 | +30 | 16.81% (n=128,526) | 16.46% (n=89,382) | 0.35% | +0.13% ±0.85% | +0.60% ±1.13% |
| 30 | `settler-threat-detour` | Send a Settler to the best safe alternative site when a visible threat blocks its route. | — | **on** | 97.6% | -4 | +19 | +5 | 16.80% (n=100,223) | 16.45% (n=60,793) | 0.35% | +1.77% ±0.92% | +2.05% ±1.16% |
| 31 | `settlement-gap-target` | Make the settlement-gap redirect and the Settler ranking read the same city target as the baseline cascade. | — | **on** | 90.9% | +8 | +17 | +11 | 16.81% (n=46,413) | 16.47% (n=32,211) | 0.34% | +0.71% ±0.87% | +1.71% ±1.18% |
| 32 | `competition-victory-points` | Price first place in a scored competition by the Diplomatic Victory Points it pays. | — | **on** | 93.8% | +4 | +17 | +26 | 16.80% (n=72,880) | 16.48% (n=52,988) | 0.33% | +0.48% ±0.93% | +1.02% ±1.26% |
| 33 | `camp-party` | In peacetime let the whole army answer home threats, ranking a nearby barbarian camp above countryside raiders. | — | **on** | 92.7% | -4 | -30 | +20 | 16.80% (n=119,029) | 16.50% (n=98,879) | 0.30% | +0.83% ±0.84% | +1.57% ±1.14% |
| 34 | `barbarian-bargain` | Price a fight against a barbarian below a fight against a major, since barbarians carry no war costs. | — | **on** | 95.0% | +1 | +3 | +24 | 16.78% (n=100,215) | 16.48% (n=60,801) | 0.30% | +1.01% ±0.91% | +1.11% ±1.21% |
| 35 | `strike-opening` | Credit a movement tile for the attack it opens next, not only charge it for the threat it accepts. | — | **on** | 96.8% | +8 | +16 | +22 | 16.78% (n=128,572) | 16.50% (n=89,336) | 0.28% | +0.11% ±0.84% | +0.39% ±1.16% |
| 36 | `naval-threat-triage` | Ignore nearby barbarian ships that cannot land a meaningful blow, while still allowing ranged shots at them. | — | off | 70.4% | +5 | – | – | 16.87% (n=7,374) | 16.60% (n=22,626) | 0.27% | +7.87% ±0.56% | +7.88% ±0.64% |
| 37 | `barbarian-ranged-answer` | Build a ranged defender, not a melee one, when the barbarian ring around a city is mostly shooters. | — | off | 94.6% | +9 | +34 | +15 | 16.80% (n=79,676) | 16.53% (n=81,340) | 0.27% | -0.75% ±0.91% | -1.54% ±1.24% |
| 38 | `deals-for-our-gain` | Pick the trade quote with the best net value to us instead of the most balanced exchange. | — | off | 56.5% | +10 | -32 | +6 | 16.86% (n=10,058) | 16.60% (n=30,406) | 0.26% | -1.15% ±0.87% | -1.94% ±1.14% |
| 39 | `missionary-last-charge-explores` | Let a Missionary on its last charge explore nearby fog for a few turns before spending it. | — | **on** | 74.8% | +3 | +3 | +20 | 16.75% (n=27,408) | 16.50% (n=13,056) | 0.25% | +1.09% ±0.83% | +1.31% ±1.10% |
| 40 | `culture-building-debt` | Make a Theater Square owe its Amphitheater, Museum and Broadcast Center the way a Campus owes its buildings. | — | **on** | 79.1% | -14 | +25 | -1 | 16.75% (n=82,601) | 16.51% (n=43,267) | 0.24% | -2.46% ±0.87% | -2.22% ±1.15% |
| 41 | `research-tier-premium` | Scale a missing Campus building's debt by its own science yield, so Universities and Labs outrank Libraries. | — | off | 84.9% | +9 | -4 | +10 | 16.80% (n=52,971) | 16.57% (n=72,897) | 0.23% | +0.04% ±0.90% | -0.10% ±1.24% |
| 42 | `slot-kind-tiebreak` | Break a production cost tie between museums by which great-work slots the empire can actually fill. | — | **on** | 92.7% | +6 | +10 | +17 | 16.77% (n=118,989) | 16.54% (n=98,919) | 0.23% | -0.32% ±0.94% | -0.92% ±1.19% |
| 43 | `lane-policy-deck` | Choose policy cards for the victory the empire is racing while its plan is still Expansion. | — | **on** | 85.0% | +8 | -16 | -11 | 16.76% (n=70,218) | 16.54% (n=55,650) | 0.22% | +0.45% ±0.88% | +0.59% ±1.20% |
| 44 | `religious-units-heal-first` | Let a wounded spreader in its own Holy Site's heal ring hold and heal instead of spending a weak charge. | — | **on** | 86.6% | +3 | +14 | +24 | 16.76% (n=72,841) | 16.54% (n=53,027) | 0.21% | +2.05% ±0.87% | +3.12% ±1.21% |
| 45 | `early-contact-window` | Buy the second and third Scout early, before Early Empire closes borders and city-states become unreachable. | — | **on** | 83.9% | +8 | -11 | +14 | 16.75% (n=72,935) | 16.55% (n=52,933) | 0.21% | +0.27% ±0.80% | +0.14% ±1.06% |
| 46 | `war-reinforcement` | Keep marching newly built rear units to the campaign objective after war is declared, not only before. | — | **on** | 80.7% | -6 | +15 | -37 | 16.75% (n=128,404) | 16.54% (n=89,504) | 0.21% | +1.42% ±0.84% | +1.06% ±1.12% |
| 47 | `stranded-settler-discount` | Discount a Settler that has stopped walking from the expansion gate, and found where it stands when stalled. | — | off | 90.1% | -2 | -3 | +19 | 16.78% (n=98,917) | 16.58% (n=118,991) | 0.20% | +0.71% ±0.87% | +1.08% ±1.16% |
| 48 | `pantheon-board` | Choose the pantheon by what it would pay on the tiles the empire owns, not from a fixed order. | — | off | 72.6% | -3 | -37 | +83 | 16.79% (n=29,023) | 16.59% (n=49,601) | 0.20% | +0.06% ±0.84% | +1.17% ±1.14% |
| 49 | `garrison-under-fire` | Treat a city that is losing hitpoints as besieged even when fog hides every attacker. | — | off | 79.2% | +4 | -29 | +3 | 16.77% (n=98,793) | 16.58% (n=119,115) | 0.20% | +0.15% ±0.82% | +0.34% ±1.06% |
| 50 | `lane-great-people` | Rank Great Person classes and project points by the victory the empire is racing, even during a war. | — | **on** | 80.4% | +2 | -11 | +1 | 16.75% (n=72,888) | 16.56% (n=52,980) | 0.19% | -0.76% ±0.92% | -0.57% ±1.22% |
| 51 | `army-target-weighs-enemy` | Raise the wartime army target when the enemy outweighs us, instead of counting only our own cities. | — | off | 78.9% | +19 | -35 | +30 | 16.76% (n=108,239) | 16.57% (n=109,669) | 0.19% | +2.08% ±0.85% | +2.96% ±1.12% |
| 52 | `amenity-district-path` | Price an amenity district by the building it will hold, and a regional amenity building by every city it reaches. | — | **on** | 88.4% | -4 | +17 | -3 | 16.74% (n=128,433) | 16.56% (n=89,475) | 0.17% | -1.62% ±0.86% | -2.44% ±1.15% |
| 53 | `one-launch-pad` | Let only one city at a time claim the 3,000-point first Spaceport bonus, instead of every city at once. | — | **on** | 87.6% | -7 | -12 | +21 | 16.74% (n=119,063) | 16.57% (n=98,845) | 0.17% | -0.91% ±0.91% | -1.50% ±1.20% |
| 54 | `settler-screen` | Block a seen rival Settler with up to four nearby units standing on its likeliest paths to slow its founding. | — | off | 30.7% | +11 | -48 | – | 16.79% (n=8,516) | 16.63% (n=25,960) | 0.17% | -0.84% ±0.98% | -0.73% ±1.45% |
| 55 | `whole-turn-backtrack-guard` | Refuse any step onto a tile the unit already stood on this turn, closing three-hop loops too. | — | off | 89.4% | -4 | +5 | +11 | 16.75% (n=108,388) | 16.59% (n=109,520) | 0.16% | -0.02% ±0.86% | -0.05% ±1.15% |
| 56 | `lane-culture-spending` | Run the Culture lane's Faith purchases, Naturalists and Rock Bands, for an empire racing Culture under an Expansion plan. | — | **on** | 79.3% | +0 | -7 | +25 | 16.73% (n=73,160) | 16.57% (n=52,708) | 0.16% | -1.22% ±0.84% | -1.72% ±1.09% |
| 57 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire needs rather than the largest number on the card. | — | **on** | 80.1% | -4 | -4 | +67 | 16.73% (n=128,736) | 16.57% (n=89,172) | 0.16% | +0.26% ±0.88% | -0.38% ±1.15% |
| 58 | `lane-space-race` | Open the Spaceport and launch pass for an empire racing Science even while its plan is still Expansion. | — | off | 78.5% | +6 | +53 | +8 | 16.76% (n=52,931) | 16.60% (n=72,937) | 0.16% | -0.31% ±0.91% | -0.22% ±1.26% |
| 59 | `enhancer-for-the-corps` | Choose the enhancer beliefs that multiply religious spread while the corps has a job to do. | — | off | 78.1% | +8 | -10 | +35 | 16.76% (n=52,755) | 16.60% (n=73,113) | 0.16% | +0.33% ±0.93% | +1.34% ±1.26% |
| 60 | `promote-when-wounded` | Defer a unit's promotion until it is wounded enough to use the promotion's heal instead of wasting it. | — | **on** | 84.7% | -10 | +49 | +38 | 16.73% (n=49,536) | 16.57% (n=29,088) | 0.16% | -0.40% ±0.88% | -1.20% ±1.13% |
| 61 | `strategic-wonders` | Price a wonder's effects in the victory lane's currency and build the ones that lane needs. | — | off | 85.5% | -2 | -13 | +32 | 16.75% (n=98,874) | 16.60% (n=119,034) | 0.15% | +0.08% ±0.88% | +0.79% ±1.16% |
| 62 | `unit-cost-efficiency` | Credit strength per production and the civilization's own unique unit when pricing military production. | — | **on** | 70.9% | -3 | +9 | +1 | 16.72% (n=49,336) | 16.57% (n=29,288) | 0.15% | -0.48% ±0.85% | -0.94% ±1.13% |
| 63 | `religion-sues-peace` | Have a Religion strategy offer peace to every at-war major so its missionaries can reach their cities. | — | **on** | 83.4% | +1 | +8 | +30 | 16.73% (n=118,925) | 16.59% (n=98,983) | 0.14% | +0.10% ±0.83% | -0.21% ±1.13% |
| 64 | `come-ashore` | Keep land units out of the water: no water exploration goals, and disembark units already at sea. | — | off | 82.9% | -4 | +26 | +38 | 16.74% (n=108,377) | 16.60% (n=109,531) | 0.14% | -0.33% ±0.90% | -0.45% ±1.21% |
| 65 | `unit-objective-memory` | Let a unit remember its campaign objective, dangerous approaches and a short retreat commitment across turns. | — | **on** | 69.5% | +3 | +16 | -18 | 16.72% (n=49,298) | 16.58% (n=29,326) | 0.13% | +0.56% ±0.93% | +0.41% ±1.22% |
| 66 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the ranking for several turns instead of re-picking it immediately. | — | off | 75.1% | +13 | -10 | +39 | 16.73% (n=98,720) | 16.61% (n=119,188) | 0.12% | +0.03% ±0.85% | -0.07% ±1.13% |
| 67 | `blind-objective-strength` | Price a fogged objective city from its last sighting instead of treating unseen ground as empty. | — | off | 68.9% | -9 | +68 | -41 | 16.73% (n=98,632) | 16.62% (n=119,276) | 0.11% | -1.62% ±0.78% | -1.95% ±1.06% |
| 68 | `settler-guard-holds` | Count a stacked guard as protection only when it can hold, and make it stay with its settler. | — | off | 72.0% | -4 | +22 | +36 | 16.72% (n=98,796) | 16.62% (n=119,112) | 0.10% | +0.29% ±0.93% | +0.48% ±1.27% |
| 69 | `relief-targets-the-siege` | Send a relief force at the besiegers actually damaging the city, not the enemy nearest the force. | — | **on** | 76.0% | -15 | +17 | +11 | 16.71% (n=128,675) | 16.61% (n=89,233) | 0.10% | +0.52% ±0.97% | +0.40% ±1.29% |
| 70 | `pass-picket` | Station an idle recon unit on the chokepoint tile of the land route toward a neighbour, or watch their border. | — | off | 57.7% | +5 | -20 | – | 16.74% (n=8,453) | 16.64% (n=26,023) | 0.10% | +0.30% ±0.93% | +0.35% ±1.35% |
| 71 | `amenity-project-preemption` | In a severe empire-wide Amenity crisis, pause one repeatable project for the amenity repair chain and slot Liberalism. | — | off | 80.8% | +7 | +67 | +24 | 16.72% (n=98,776) | 16.63% (n=119,132) | 0.09% | +2.66% ±0.98% | +2.24% ±1.27% |
| 72 | `campus-adjacency-threshold` | Credit a Campus plot that reaches the Rationalism adjacency threshold with the science bonus crossing it unlocks. | — | off | 69.6% | -5 | +34 | +33 | 16.70% (n=52,765) | 16.64% (n=73,103) | 0.06% | -0.84% ±0.81% | -0.47% ±1.12% |
| 73 | `guru-heals-the-corps` | Let a founder defending its own cities buy one Guru, the only unit that heals religious units. | — | off | 69.9% | +4 | +8 | +36 | 16.69% (n=52,750) | 16.65% (n=73,118) | 0.05% | +0.18% ±0.88% | +0.21% ±1.18% |
| 74 | `science-multiplier-payoff` | Credit a Campus building the science its city's multipliers will actually pay, not its raw spec yield. | — | **on** | 63.6% | -12 | +53 | +1 | 16.69% (n=70,053) | 16.64% (n=55,815) | 0.04% | +0.06% ±0.80% | +0.18% ±1.09% |
| 75 | `coordinated-finish` | Let a force finish a defender together with a friendly volley, without reopening the closed war-half bundle. | — | off | 59.1% | -4 | -17 | +41 | 16.68% (n=29,356) | 16.66% (n=49,268) | 0.03% | +1.25% ±0.89% | +1.17% ±1.19% |
| 76 | `missionary-evades-raiders` | Keep religious units out of every tile a visible barbarian raider can reach next turn. | — | **on** | 53.2% | +4 | -12 | -6 | 16.68% (n=27,279) | 16.65% (n=13,185) | 0.03% | -0.80% ±0.94% | -1.12% ±1.29% |
| 77 | `wonder-score-tally` | Let any civilization build wonders on merit by pricing the fifteen score points a finished wonder pays. | — | off | 52.3% | +0 | +1 | +3 | 16.68% (n=10,159) | 16.66% (n=30,305) | 0.02% | -0.70% ±0.86% | -1.01% ±1.16% |
| 78 | `inquisition-on-threat` | Let a founder under conversion pressure buy one Apostle to launch the Inquisition after its Missionaries. | — | **on** | 67.5% | -15 | -6 | +19 | 16.67% (n=106,271) | 16.65% (n=66,745) | 0.02% | -0.92% ±0.89% | -1.12% ±1.19% |
| 79 | `blind-objective-units` | Price the enemy units remembered near an unseen objective instead of reading a fogged approach as empty. | — | off | 52.5% | -8 | +31 | +21 | 16.67% (n=98,624) | 16.66% (n=119,284) | 0.01% | -0.27% ±0.87% | -0.37% ±1.16% |
| 80 | `power-the-laboratory` | Credit a power plant the powered yields it switches on, above all the Research Lab's extra science. | — | off | 47.7% | -6 | +10 | +12 | 16.67% (n=52,842) | 16.67% (n=73,026) | 0.00% | +1.17% ±0.90% | +1.73% ±1.20% |
| 81 | `district-planning` | Plan a city's districts, their plots and tile purchases jointly, reserving plots in rings one to three. | — | **on** | 48.8% | -5 | -30 | +8 | 16.66% (n=46,459) | 16.67% (n=32,165) | -0.01% | +0.33% ±0.91% | -0.49% ±1.22% |
| 82 | `no-free-passage` | Stop bundling free one-way Open Borders into friendship and alliance proposals; sell passage through the quote lane. | — | off | 36.8% | +5 | +12 | -36 | 16.66% (n=10,176) | 16.67% (n=30,288) | -0.01% | -0.09% ±0.87% | -0.21% ±1.18% |
| 83 | `naval-recon` | Buy one ship for a fleetless empire with unexplored water off its coast and send it exploring. | — | off | 40.9% | +5 | +3 | -10 | 16.66% (n=99,024) | 16.68% (n=118,884) | -0.02% | -0.46% ±0.87% | -0.72% ±1.15% |
| 84 | `religious-defence-scales` | Size the defensive Missionary corps by cities actually under conversion pressure, up to four, instead of two. | — | **on** | 41.2% | +4 | -38 | -12 | 16.65% (n=70,014) | 16.68% (n=55,854) | -0.03% | -2.64% ±0.83% | -3.12% ±1.12% |
| 85 | `surprise-war-mobilization` | Convert the first six Standard-speed turns after a surprise war is declared against us into a bounded defensive mobilization. | — | off | 47.1% | -1 | – | – | 16.64% (n=7,392) | 16.68% (n=22,608) | -0.04% | +0.27% ±0.54% | +0.10% ±0.61% |
| 86 | `holy-site-where-the-threat-is` | Build a Holy Site in the city losing its religious majority so defenders can be bought there directly. | — | off | 36.8% | +0 | +7 | -15 | 16.64% (n=53,073) | 16.68% (n=72,795) | -0.04% | +0.52% ±0.85% | +0.85% ±1.16% |
| 87 | `district-coverage` | Rank each district family by how much of the empire still lacks it, so Theater Squares get built. | — | off | 36.4% | -7 | +13 | -26 | 16.64% (n=98,794) | 16.69% (n=119,114) | -0.05% | +0.95% ±0.89% | +0.73% ±1.20% |
| 88 | `home-defense` | Let hostile units inside our own territory claim defenders before the offensive campaign takes them. | — | off | 29.1% | +15 | -3 | +11 | 16.63% (n=99,185) | 16.69% (n=118,723) | -0.06% | -0.52% ±0.82% | -0.56% ±1.06% |
| 89 | `builder-barbarian-safety` | Make a Builder retreat from, and never step into, a tile a visible barbarian can capture next turn. | — | off | 27.5% | -9 | -67 | +11 | 16.63% (n=70,555) | 16.70% (n=90,461) | -0.07% | +0.53% ±0.96% | +0.73% ±1.31% |
| 90 | `one-shot-recovery` | Withdraw a unit that one enemy blow could kill to safe healing ground, and leave when threatened again. | — | off | 32.4% | -8 | +30 | -13 | 16.62% (n=52,916) | 16.70% (n=72,952) | -0.08% | +0.57% ±0.85% | +1.12% ±1.13% |
| 91 | `siege-commitment` | Keep the campaign aimed at a city it has already damaged instead of re-targeting a fresh one each turn. | — | off | 29.7% | -2 | -33 | -8 | 16.62% (n=98,839) | 16.70% (n=119,069) | -0.08% | -0.77% ±0.80% | -0.71% ±1.06% |
| 92 | `housing-research` | Aim research at the technology that raises the housing ceiling while housing is throttling growth. | — | off | 31.3% | -5 | -16 | -33 | 16.62% (n=98,744) | 16.70% (n=119,164) | -0.08% | -0.25% ±0.88% | +0.20% ±1.15% |
| 93 | `theology-for-founders` | Have a founder research Theology next, after its first government, so it can build a Temple. | — | **on** | 28.0% | -14 | -44 | -3 | 16.63% (n=96,647) | 16.71% (n=76,369) | -0.08% | -0.03% ±0.88% | +0.47% ±1.21% |
| 94 | `civilian-rescue` | Walk onto any capturable civilian within reach, and always take back a Settler the barbarians hold. | — | **on** | 30.8% | -16 | -6 | +49 | 16.62% (n=118,712) | 16.72% (n=99,196) | -0.09% | +0.04% ±0.82% | +0.65% ±1.11% |
| 95 | `joint-tactics` | Plan an engagement's attacks jointly across all units by search instead of one unit at a time in class order. | — | off | 32.6% | – | – | – | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | +27.29% ±0.47% | +27.69% ±0.79% |
| 96 | `siege-is-progress` | Count damage dealt to an enemy city or its walls as campaign progress, so a winning siege is never stalled. | — | off | 22.1% | -4 | -1 | +8 | 16.59% (n=98,824) | 16.73% (n=119,084) | -0.13% | -0.19% ±0.83% | +0.18% ±1.15% |
| 97 | `city-campaign` | Appraise weaker neighbours, plan to take one to three holdable cities the army can afford, and launch when staged. | — | off | 30.9% | -4 | -3 | – | 16.49% (n=8,506) | 16.72% (n=25,970) | -0.23% | +0.92% ±0.97% | +1.48% ±1.41% |
| 98 | `deals-at-the-ceiling` | Price a trade quote at the counterparty's walk-away point less two Gold, falling back to the midpoint if refused. | — | off | 27.8% | -5 | -5 | -1 | 16.48% (n=10,061) | 16.73% (n=30,403) | -0.25% | -1.03% ±0.87% | -1.88% ±1.25% |
| 99 | `campaign-pillage` | Let a soldier at war pillage the tile it stands on with movement its march does not use. | — | off | 28.7% | -4 | -12 | – | 16.47% (n=8,524) | 16.73% (n=25,952) | -0.26% | +1.83% ±1.02% | +2.83% ±1.41% |
| 100 | `lane-commit` | From mid-game commit an adaptive seat to the victory lane it leads the field in, instead of re-picking each turn. | — | off | 21.4% | -4 | -21 | -6 | 16.42% (n=10,188) | 16.75% (n=30,276) | -0.33% | -0.70% ±0.88% | -0.65% ±1.24% |
| 101 | `spread-campaign-persists` | Keep a spread campaign on the offensive between waves once it has converted a foreign city. | — | off | 2.8% | -12 | -39 | +16 | 16.45% (n=52,884) | 16.83% (n=72,984) | -0.38% | -0.24% ±0.82% | -0.71% ±1.12% |
| 102 | `pillage-to-heal` | Let a unit at or below 65 health pillage a healing improvement on or beside its tile before retreating. | — | off | 38.7% | -12 | +36 | -23 | 16.35% (n=10,211) | 16.77% (n=30,253) | -0.42% | +0.75% ±0.90% | +1.68% ±1.19% |
| 103 | `shoot-and-scoot` | Let a ranged unit inside melee reach step to a safer firing tile and shoot the threatening body. | — | off | 13.7% | -12 | -15 | +11 | 16.32% (n=10,208) | 16.78% (n=30,256) | -0.46% | -0.08% ±0.88% | -0.20% ±1.19% |
| 104 | `zoc-screen` | Stand an idle melee unit where its zone of control shields our shooters and wounded from the most enemy reaches. | — | off | 10.9% | -5 | -19 | -26 | 16.28% (n=10,159) | 16.80% (n=30,305) | -0.51% | +1.52% ±0.87% | +2.33% ±1.21% |
| 105 | `fog-honest` | Plan the whole turn against a fog-redacted world and replay only the resulting orders on the real game. | 1 | off | 0.0% | -118 | -77 | -95 | v1 7.73% (n=4,993) · v2 1.96% (n=4,948) | v1 17.92% (n=35,471) · v2 18.72% (n=35,516) | -10.19% | +0.67% ±1.14% | +2.43% ±1.50% |
| 106 | `fog-honest-2` | Version 2 of fog-honest: the same redacted planning plus one re-plan from the real board when an order is refused. | 1 | off | 0.0% | -178 | -181 | -187 | v1 7.73% (n=4,993) · v2 1.96% (n=4,948) | v1 17.92% (n=35,471) · v2 18.72% (n=35,516) | -16.76% | +4.60% ±1.13% | +5.65% ±1.54% |

## Evidence for future operator selections

The deployment genome is explicitly operator-pinned. The win columns, pooled *Diff*, posterior, and score-share readings below remain useful evidence, but a new source does not promote or demote a gene automatically. To change a default, update the pinned list with an explicit operator decision and regenerate this ledger.

*Posterior (95% CI)* is a random-effects (DerSimonian–Laird) inverse-variance pool of every screen's on−off difference on the win column's scale. It weights each screen by its standard error and carries between-screen disagreement in the interval; `P(>0)` makes the resulting precision visible.

### What the posterior resolves

Of 81 priced genes the interval clears zero for **21 upward** and **0 downward**; **60 straddle zero**. Those are evidence states, not automatic deployment calls.

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

## The two shapes, apart

`τ` (tau) is the between-screen standard deviation the random-effects pool estimates. It is the statistic that answers *“is 'both columns positive' two confirmations?”*: when screens agree to within their errors it is zero and the pool is the ordinary inverse-variance one; when they do not, it widens the interval instead of averaging two worlds into a confident wrong answer. `POSTERIOR_SHAPES` in `tools/genes.py` says which shapes the published pool admits and is currently `standard, legacy`.

| Shape | Sources | Player seats | Genes priced |
|---|---:|---:|---:|
| standard | 3 | 92,604 | 80 |
| legacy | 7 | 132,440 | 52 |

Genes priced at both shapes. **A row whose two intervals do not overlap is not a gene with one number; it is two instruments disagreeing**, and the pooled column beside it should be read as a warning rather than an answer.

| Gene | legacy | standard | pooled | τ | overlap |
|---|---:|---:|---:|---:|---|
| `amenity-district-path` | +7 [-15, +29] | +17 [-8, +42] | +12 [-5, +28] | 0 | yes |
| `amenity-project-preemption` | +3 [-24, +31] | -5 [-63, +53] | -1 [-26, +25] | 22 | yes |
| `apostle-promotion-by-role` | -4 [-38, +30] | +19 [-6, +44] | +6 [-15, +27] | 15 | yes |
| `army-target-weighs-enemy` | -1 [-37, +35] | +12 [-14, +37] | +5 [-16, +25] | 13 | yes |
| `barbarian-bargain` | +5 [-32, +41] | +24 [-9, +56] | +16 [-5, +38] | 5 | yes |
| `barbarian-ranged-answer` | +14 [-22, +50] | +9 [-16, +34] | +11 [-10, +32] | 0 | yes |
| `barbarian-scouts-are-scouts` | +45 [+21, +68] | +9 [-16, +34] | +30 [+8, +51] | 16 | yes |
| `blind-objective-strength` | +27 [+5, +50] | -9 [-33, +14] | +11 [-10, +31] | 14 | yes |
| `blind-objective-units` | +0 [-22, +22] | +0 [-23, +24] | +0 [-16, +16] | 0 | yes |
| `bounded-recovery` | +28 [+6, +51] | +34 [+10, +59] | +31 [+14, +48] | 0 | yes |
| `builder-barbarian-safety` | +13 [-23, +48] | -1 [-24, +23] | +3 [-16, +23] | 0 | yes |
| `buildings-before-projects` | +14 [-8, +36] | +48 [+14, +81] | +28 [+5, +51] | 18 | yes |
| `camp-party` | +35 [+10, +60] | +5 [-37, +48] | +22 [-3, +47] | 21 | yes |
| `civilian-rescue` | -5 [-28, +17] | -1 [-25, +23] | -3 [-20, +13] | 0 | yes |
| `come-ashore` | +11 [-18, +40] | +1 [-24, +27] | +7 [-10, +24] | 0 | yes |
| `district-coverage` | +5 [-23, +34] | -6 [-41, +29] | +0 [-20, +20] | 14 | yes |
| `escort-unstick` | +27 [-20, +74] | +37 [+13, +61] | +32 [+8, +57] | 21 | yes |
| `founder-temple` | +29 [-4, +62] | +12 [-13, +38] | +19 [+1, +38] | 0 | yes |
| `garrison-under-fire` | +26 [-16, +68] | +1 [-56, +58] | +15 [-17, +48] | 32 | yes |
| `great-person-housing` | +78 [+42, +114] | +87 [+62, +112] | +84 [+64, +105] | 0 | yes |
| `holy-lane-parity` | +45 [-24, +114] | +16 [-9, +41] | +32 [-6, +71] | 39 | yes |
| `home-defense` | -13 [-35, +10] | -6 [-34, +22] | -10 [-26, +6] | 0 | yes |
| `housing-research` | +10 [-26, +45] | -12 [-36, +11] | +0 [-22, +23] | 17 | yes |
| `idle-faith-patronage` | +26 [+9, +44] | +21 [-19, +61] | +26 [+11, +40] | 0 | yes |
| `inquisition-on-threat` | +16 [-16, +47] | +5 [-21, +30] | +9 [-9, +28] | 0 | yes |
| `loyalty-rate-alarm` | +49 [+25, +73] | +28 [+2, +54] | +40 [+22, +58] | 9 | yes |
| `naval-recon` | -1 [-23, +21] | -3 [-40, +34] | -3 [-19, +13] | 0 | yes |
| `one-launch-pad` | +20 [-2, +42] | +2 [-26, +30] | +11 [-5, +28] | 0 | yes |
| `opportunistic-war` | +23 [-14, +59] | +59 [+33, +85] | +48 [+20, +76] | 17 | yes |
| `peacetime-deterrence` | +13 [-12, +38] | +24 [-1, +49] | +18 [+1, +35] | 0 | yes |
| `raid-pillage-prizes` | +30 [-6, +65] | +65 [+35, +96] | +54 [+25, +83] | 18 | yes |
| `recon-replacement` | +53 [+25, +82] | +49 [+7, +90] | +51 [+30, +72] | 14 | yes |
| `recorded-tactical-step` | +15 [-8, +37] | +19 [-6, +44] | +17 [+0, +33] | 0 | yes |
| `relief-targets-the-siege` | +4 [-18, +27] | +17 [-11, +45] | +10 [-7, +26] | 0 | yes |
| `religion-sues-peace` | +19 [-3, +41] | -9 [-33, +15] | +7 [-11, +24] | 7 | yes |
| `score-horizon` | +18 [-4, +41] | +15 [-10, +40] | +17 [+0, +34] | 0 | yes |
| `settle-sooner` | +41 [+5, +76] | +31 [+7, +56] | +35 [+14, +55] | 0 | yes |
| `settler-guard-holds` | +2 [-20, +24] | +6 [-25, +38] | +3 [-13, +19] | 0 | yes |
| `settler-target-hysteresis` | +4 [-18, +26] | -2 [-37, +32] | +0 [-16, +16] | 0 | yes |
| `settler-threat-detour` | +50 [+14, +86] | +12 [-13, +38] | +24 [-2, +51] | 14 | yes |
| `siege-commitment` | -11 [-37, +15] | +7 [-16, +31] | -2 [-18, +14] | 0 | yes |
| `siege-is-progress` | -21 [-64, +21] | +6 [-21, +34] | -9 [-36, +18] | 25 | yes |
| `slot-kind-tiebreak` | +10 [-14, +33] | +10 [-15, +35] | +9 [-7, +26] | 0 | yes |
| `stranded-settler-discount` | +13 [-9, +36] | +11 [-30, +51] | +11 [-5, +27] | 0 | yes |
| `strategic-wonders` | +11 [-12, +33] | +7 [-17, +30] | +9 [-7, +25] | 0 | yes |
| `strike-opening` | +19 [-3, +41] | +4 [-21, +29] | +12 [-4, +29] | 0 | yes |
| `theology-for-founders` | -12 [-40, +16] | +14 [-9, +38] | +3 [-15, +22] | 0 | yes |
| `war-economy` | -48 [-185, +88] | +109 [+84, +134] | +13 [-89, +115] | 115 | yes |
| `war-reinforcement` | +14 [-17, +46] | +21 [-12, +53] | +17 [-4, +37] | 13 | yes |
| `whole-turn-backtrack-guard` | +25 [+3, +47] | -7 [-41, +26] | +12 [-8, +31] | 11 | yes |
| `wide-map-capacity` | +33 [+11, +55] | +98 [+73, +122] | +61 [+28, +93] | 32 | **no** |

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
| `pantheon-board` | +5 [-32, +43] | 61.0% | off | +7.5 | 728,917 |
| `come-ashore` | +7 [-10, +24] | 79.3% | off | +7.0 | 353,593 |
| `guru-heals-the-corps` | -4 [-56, +49] | 44.7% | off | +5.7 | 1,648,811 |
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
| `one-shot-recovery` | -2 [-25, +22] | 43.7% | off | +1.1 | 5,846,162 |
| `blind-objective-units` | +0 [-16, +16] | 51.4% | off | +1.1 | 245,127,417 |
| `unit-objective-memory` | +10 [-28, +47] | 69.7% | on | +1.1 | 201,764 |
| `settler-target-hysteresis` | +0 [-16, +16] | 50.7% | off | +1.0 | 1,036,197,883 |
| `science-multiplier-payoff` | +6 [-24, +35] | 64.5% | on | +0.9 | 639,181 |
| `district-planning` | +12 [-25, +49] | 74.1% | on | +0.7 | 125,973 |
| `settlement-gap-target` | +14 [-23, +52] | 77.5% | on | +0.5 | 87,669 |
| `lane-great-people` | +13 [-22, +48] | 77.0% | on | +0.4 | 103,314 |
| `siege-commitment` | -2 [-18, +14] | 40.5% | off | +0.3 | 5,199,392 |
| `unit-cost-efficiency` | +17 [-20, +55] | 81.7% | on | +0.3 | 55,043 |
| `joint-tactics` | -5 [-28, +17] | 32.6% | off | +0.3 | 755,823 |
| `competition-victory-points` | +16 [-19, +50] | 81.3% | on | +0.2 | 67,501 |
| `siege-is-progress` | -9 [-36, +18] | 25.3% | off | +0.2 | 222,389 |
| `theology-for-founders` | +3 [-15, +22] | 64.7% | on | +0.2 | 1,662,178 |
| `early-contact-window` | +8 [-16, +31] | 73.8% | on | +0.1 | 318,670 |
| `naval-recon` | -3 [-19, +13] | 35.6% | off | +0.1 | 2,174,265 |
| `apostle-promotion-by-role` | +6 [-15, +27] | 71.4% | on | +0.1 | 520,461 |
| `lane-policy-deck` | +13 [-16, +42] | 80.9% | on | +0.1 | 103,406 |
| `lane-culture-spending` | +9 [-15, +32] | 75.9% | on | +0.1 | 250,826 |
| `religious-units-heal-first` | +9 [-14, +33] | 78.0% | on | +0.0 | 208,321 |
| `holy-lane-parity` | +32 [-6, +71] | 95.0% | on | +0.0 | 5,971 |
| `religion-sues-peace` | +7 [-11, +24] | 77.2% | on | +0.0 | 421,343 |
| `inquisition-on-threat` | +9 [-9, +28] | 83.2% | on | +0.0 | 189,468 |
| `spread-campaign-persists` | -17 [-41, +7] | 8.0% | off | +0.0 | 35,582 |
| `relief-targets-the-siege` | +10 [-7, +26] | 87.0% | on | +0.0 | 153,392 |
| `settler-threat-detour` | +24 [-2, +51] | 96.4% | on | +0.0 | 5,630 |
| `barbarian-bargain` | +16 [-5, +38] | 93.3% | on | +0.0 | 32,522 |
| `slot-kind-tiebreak` | +9 [-7, +26] | 87.4% | on | +0.0 | 156,019 |
| `camp-party` | +22 [-3, +47] | 96.1% | on | +0.0 | 8,154 |
| `home-defense` | -10 [-26, +6] | 11.7% | off | +0.0 | 137,225 |
| `war-reinforcement` | +17 [-4, +37] | 94.7% | on | +0.0 | 24,014 |
| `amenity-district-path` | +12 [-5, +28] | 91.2% | on | +0.0 | 83,886 |
| `one-launch-pad` | +11 [-5, +28] | 91.6% | on | +0.0 | 81,761 |
| `strike-opening` | +12 [-4, +29] | 92.5% | on | +0.0 | 65,525 |

The top 7 that one batch could actually resolve (≤ 60,000 seat pairs each), as an argument list:

```sh
gene_screen --genes unit-cost-efficiency,holy-lane-parity,spread-campaign-persists,settler-threat-detour,barbarian-bargain,camp-party,war-reinforcement
```

`python3 tools/genes.py boundary` prints this list on its own, with `--arm-pairs` and `--max-arm-pairs` to size it.

## Lane genes and the share axis

At the standing 250-turn Online clock a **science or congress gene cannot pay through the win axis at all**: science and diplomatic victories land at median t283 and t285, past the clock, so they are 1–2% of endings and `docs/VICTORY_GENES.md` records **science 0/8** and **diplomacy 1/8** for exactly that reason. The seat a lane gene would have carried to a science victory shows up as a score win or a score loss instead. The decision axis stays WINS — `docs/GENOME.md` records what happened the one time selection ran on a correlate — so the share reading is a **pre-registered secondary**, fixed in `docs/GENE_SCREEN.md` before the next screen rather than chosen after it.

The set is discovered from the code: every gene whose flag field `src/ai/advanced/victory_lane.rs` reads. A gene joins it by being a lane gene, not by being listed here.

| Lane gene | Default | ± Wins / 10k seats | Share Δpp (z) | Posterior (95% CI) | Status |
|---|---|---:|---|---:|---|
| `lane-great-people` | **on** | +33 | +0.10 (z +1.26) ~ | +13 [-22, +48] | unresolved |
| `lane-policy-deck` | **on** | +29 | +0.08 (z +1.07) ~ | +13 [-16, +42] | unresolved |
| `lane-culture-spending` | **on** | +9 | -0.01 (z -0.13) ~ | +9 [-15, +32] | unresolved |
| `lane-space-race` | off | -12 | -0.10 (z -1.32) ~ | +1 [-22, +25] | unresolved |
| `competition-victory-points` | **on** | +35 | +0.04 (z +0.46) ~ | +16 [-19, +50] | unresolved |

## Awaiting measurement

These screenable genes have no on/off result, so they receive no rank or promotion from this table. Their deployment state remains explicit while a screen is pending.

| Gene | Default | Description | Best version |
|---|---|---|---:|
| `builder-tries-the-next-tile` | off (unmeasured) | Let a Builder whose nearest improvable tile cannot be routed to try the next-nearest instead of standing still for the rest of the game. | — |
| `congress-counter-leader` | off (unmeasured) | Point the three targeted World Congress penalties at the empire the denial layer names. | — |
| `conversion-majority-alarm` | off (unmeasured) | Read a rival's religious clock from the cities it has converted rather than from whole civilizations already lost. | — |
| `culture-lane-forecast` | off (unmeasured) | Score the Culture lane by where the two tourist curves are when the clock stops. | — |
| `defensible-sites` | off (unmeasured) | Weigh whether a settle site can be held — barbarian exposure and distance from our own cities — not only what it yields. | — |
| `domination-city-count` | off (unmeasured) | Read a rival's conquests from the cities it has taken rather than only from the capitals. | — |
| `elective-war-in-reach` | off (unmeasured) | Measure the elective war against the weakest rival we can reach rather than the weakest on the board. | — |
| `expansion-pays-back` | off (unmeasured) | Shut the settler window on whether the city would pay the settler back before the game ends, rather than on a deadline. | — |
| `expansion-schedule` | off (unmeasured) | Open the settler pipeline by the shortfall while the opening is behind the four-cities-by-turn-sixty pace every recorded win came from. | — |
| `frontier-massing-alarm` | off (unmeasured) | Count a peacetime major's army massed near one of our cities toward that city's danger. | — |
| `growth-to-settle` | off (unmeasured) | Work food while the opening is behind the pace and no city has reached the population a Settler needs. | — |
| `order-retry` | off (unmeasured) | Fall through to the next-best candidate the planner already ranked when an order is refused, instead of losing the turn. | — |
| `recovery-reads-the-war` | off (unmeasured) | Measure the Recovery power gap against the war we are actually fighting. | — |
| `rival-suzerainty-alarm` | off (unmeasured) | Count a rival's city-state suzerainties toward the Diplomatic threat it presents. | — |
| `science-chain-alarm` | off (unmeasured) | Read a rival's Science clock from the prerequisite chain it has climbed, not only from the launches it has made. | — |
| `unchosen-war-keeps-the-lane` | off (unmeasured) | Stop a war we did not declare from taking the grand strategy while our own victory lane is live. | — |

## Removed from the code

Genes whose code has left the repository (operator directive: the bottom of the table leaves the code), listed from their last measurement:

| Gene | Wins ±/10k seats (last tracked measurement) | Win rate (on) | Win rate (off) | Source |
|---|---:|---:|---:|---|
| `settler-site-agreement` | +178 | 18.45% | 16.09% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `coupled-expansion` | +156 | 18.23% | 16.13% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `chain-tech-lookahead` | +153 | 18.20% | 16.16% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `congress-banks-decided` | +107 | 17.74% | 16.31% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `lane-congress-favor` | +91 | 17.57% | 16.37% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `governor-expansion-lane` | +88 | 17.55% | 16.36% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `barbarian-hunt` | +85 | 17.52% | 16.38% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `fortify-idle-units` | +68 | 17.35% | 16.44% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `fifteenth-citizen` | +62 | 17.29% | 16.46% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `barbarian-capture-priority` | +53 | 17.20% | 16.49% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `lane-congress-ballot` | +45 | 17.11% | 16.52% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `suzerain-cards` | +42 | 17.09% | 16.25% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `wonder-prereq-reach` | +29 | 16.96% | 16.38% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `district-lookahead-settle` | +24 | 16.91% | 16.58% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `research-grants-first` | +16 | 16.83% | 16.61% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `camp-reach` | +10 | 16.77% | 16.56% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-buildings` | +8 | 16.75% | 16.59% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `ranged-line-of-sight` | +4 | 16.71% | 16.63% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `recon-flight` | -1 | 16.66% | 16.67% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `housing-cards` | -4 | 16.62% | 16.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `siege-tracks-wall` | -4 | 16.62% | 16.68% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `arrival-waves` | -7 | 16.59% | 16.74% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `culture-coverage` | -9 | 16.58% | 16.70% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `idle-walkers-close-the-pipeline` | -10 | 16.56% | 16.77% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `endgame-war-runway` | -11 | 16.56% | 16.70% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `muster-at-command-radius` | -12 | 16.55% | 16.79% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `barbarian-walls-one-tier` | -13 | 16.54% | 16.80% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `siege-muster` | -26 | 16.41% | 16.93% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `step-and-reassess` | -27 | 16.40% | 16.93% | `2026-08-21-p7-native-6p-allseats-15000-pairs.json` |
| `wonder-ring-settle-value` | -28 | 16.39% | 16.76% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `builder-worked-tile-priority` | -31 | 16.36% | 16.77% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `war-patience` | -38 | 16.28% | 16.79% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `siege-role` | -39 | 16.27% | 17.06% | `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` |
| `condemn-under-congress` | -40 | 16.27% | 16.80% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `tactical-strategy` | -40 | 16.27% | 16.80% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `housing-districts` | -43 | 16.24% | 16.81% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `garrison-walls` | -54 | 16.12% | 17.21% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `loyalty-policy-defence` | -54 | 16.13% | 17.20% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `contact-posture` | -67 | 16.00% | 16.90% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `settle-plan-ahead` | -67 | 15.99% | 16.90% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `campus-finishes-first` | -69 | 15.98% | 16.89% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `district-building-chain` | -80 | 15.87% | 16.93% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `builder-reward-survey` | -85 | 15.82% | 16.95% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `research-floor-holds` | -89 | 15.78% | 16.96% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `science-payback-horizon` | -90 | 15.77% | 16.97% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `campus-every-city` | -94 | 15.73% | 17.60% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `stacked-escort` | -104 | 15.63% | 17.71% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `settler-stack-discipline` | -116 | 15.51% | 17.83% | `2026-08-20-p4-native-6p-allseats-13446-pairs.json` |
| `escort-unstick-2` | -131 | 15.36% | 17.26% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `congress-counter-votes` | -141 | 15.26% | 17.11% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `envoy-infrastructure` | -167 | 15.00% | 17.25% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `priced-tile-purchase` | -177 | 14.90% | 17.24% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `naval-production-policy` | -319 | 13.48% | 17.73% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `governor-victory-lanes` | -359 | 13.08% | 17.85% | `2026-08-24-standard-continuous-5988-total-seats.json` |
| `governor-every-lane` | -442 | 12.24% | 18.17% | `2026-08-24-standard-continuous-5988-total-seats.json` |

## How to read this

Every screenable heuristic gene on the Advanced controller, ranked by the displayed pooled *Diff* from highest to lowest (alphabetically by tag on a tie). Each batch header carries its actual player-seat count once; cells show the enabled arm's excess projected to 10,000 **total** player seats, where a six-player chance expectation is 1,667 wins. A dash means that batch did not screen the gene. The *Total* win-rate columns pool the displayed observations and retain their real per-gene on/off seat counts in every row. *Diff* is that display total's on rate minus off rate, in percentage points. *Default* is the explicit operator-pinned deployment selection: sources and display batches update evidence, not defaults. Screenable genes awaiting every displayed measurement are listed separately below without a rank.

**Versioned genes.** An improvement to a gene is a new gene `<base>-<n>` (`docs/GENE_SCREEN.md`, *Versioning a gene*), priced on its own row: a version's *on* is the seats that played that version, and every other seat — off, or a sibling version on — is its *off*. *Best version* names the family's head (`1` is the original) on every row of the family: the priced version with the highest tracked wins (pooled *Diff*), ties to the higher version — and a pinned family ships its head, so *Default* is **on** on the head's row. A versioned row's *Total (on)* and *Total (off)* cells show the best two versions' rates side by side, best first, each with its own `n`; `—` marks a gene with no versions. A family holds at most three versions; before a fourth is added the third-best by tracked wins leaves the code (`python3 tools/genes.py versions`).

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

**P(>0).** The probability that the gene's pooled effect on the win column is positive. The pool is a random-effects (DerSimonian–Laird) inverse-variance pool of **every** screen that priced the gene: each screen's on−off difference weighted by its own standard error, with the between-screen disagreement (τ) carried in the pool instead of assumed away, read as Φ(effect / se). It is the answer to two things the win columns cannot express — that the same +24 means different things from a ±29 screen and a ±64 one, and that two positive columns from screens differing in baseline, build and shape are not two confirmations (#2283/#2284 measured that: five of seven lane genes changed sign on disjoint seeds). The pooled point and its 95% interval are printed per gene in the evidence sections below; the newest screen's score-share contrast (*Share Δpp (z)*) is printed in the lane table, where a lane gene that cannot pay on the win axis at 250 turns shows its evidence. **P(>0) does not automatically decide a default**; it is evidence for a later explicit operator selection.

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

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 26,892 seats), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 2,000 seats), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 30,000 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 35,148 seats), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 14,400 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 47,244 seats), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 7,200 seats), `2026-08-24-standard-continuous-38160-total-seats.json` (standard, 38,160 seats). The fixed display batches are: `2026-08-25-standard-continuous-30000-total-seats-20260825T022230Z-d349.json` (30,000 seats), `2026-08-25-standard-continuous-4476-total-seats.json` (4,476 seats), `2026-08-24-standard-continuous-5988-total-seats.json` (5,988 seats). The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells are the operator's wins-per-ten-thousand-total-seat reporting view._
