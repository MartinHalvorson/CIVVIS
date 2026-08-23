# The heuristic gene ranking

**Default on:** both newest columns >0; or avg >+15 with neither <−10; sole reading >+20; pooled *Diff* <0 vetoes. These batch columns do not change this deployed default.

| Rank | Gene | Description | Default | Wins ± /10k total seats — Last Batch (n=10,002 total seats) | Wins ± /10k total seats — Prior Batch (n=47,244 total seats) | Wins ± /10k total seats — Third Batch (n=35,148 total seats) | Total (on) Win rate | Total (off) Win rate | Diff | Posterior (95% CI) | P(>0) | Share Δpp (z) | cost (compute) | cost (time) |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | `engine-faith-price` | THE FAITH PRICE THE AI READS IS THE STANDARD-SPEED ONE. | off | +58 | – | – | 17.84% (n=4,989) | 15.50% (n=5,013) | 2.34% | +117 [+44, +190] | 99.9% | +0.38 (z +2.44) helps * | -0.71% ±0.35% | -1.39% ±0.62% |
| 2 | `air-surge` | Beeline Advanced Flight from three technologies out, raise an Aerodrome and a bomber wing, and take the appointed city with the cavalry behind it. | **on** | +43 | +54 | – | 17.62% (n=31,200) | 15.53% (n=26,046) | 2.09% | +109 [+80, +137] | 100.0% | +0.70 (z +3.86) helps * | +0.24% ±0.40% | -0.76% ±0.69% |
| 3 | `builder-worked-tile-priority` | Prefer existing Builder work that pays on a tile a citizen currently works, while preserving luxury and strategic connections. | off | +41 | -4 | +12 | 16.81% (n=46,229) | 16.53% (n=46,165) | 0.28% | +22 [-19, +63] | 85.5% | +0.25 (z +1.63) ~ | +0.28% ±0.38% | +0.40% ±0.69% |
| 4 | `great-person-housing` | A class earned and blocked reserves a city for the slot building, district, wonder or soldier that lifts the block, and a due cultural person sells duplicate works to make room. | **on** | +41 | +47 | +39 | 17.49% (n=48,707) | 15.75% (n=43,687) | 1.73% | +89 [+66, +111] | 100.0% | +0.64 (z +3.66) helps * | +0.50% ±0.42% | +0.59% ±0.75% |
| 5 | `recon-replacement` | Rebuild the recon arm when it is gone and there is ground left to chart. | **on** | +39 | +15 | +24 | 17.13% (n=77,153) | 16.17% (n=72,133) | 0.95% | +51 [+27, +75] | 100.0% | +0.18 (z +1.01) ~ | +0.79% ±0.42% | +0.90% ±0.77% |
| 6 | `bounded-recovery` | Stop the defensive-war posture from becoming permanent. | **on** | +37 | +14 | +9 | 16.97% (n=77,089) | 16.34% (n=72,197) | 0.63% | +32 [+14, +49] | 100.0% | +0.57 (z +3.26) helps * | -0.22% ±0.42% | -0.61% ±0.75% |
| 7 | `war-economy` | Send an adaptive Conquest plan through the war production path. | **on** | +37 | +59 | +19 | 16.84% (n=77,189) | 16.48% (n=72,097) | 0.36% | +13 [-95, +122] | 59.5% | +1.06 (z +5.92) helps * | +0.49% ±0.43% | +0.36% ±0.74% |
| 8 | `garrison-under-fire` | A city losing hitpoints is besieged, whatever the fog says. | off | +33 | -14 | -3 | 16.77% (n=74,610) | 16.56% (n=74,676) | 0.21% | +19 [-18, +56] | 84.4% | +0.41 (z +2.64) helps * | -0.90% ±0.36% | -1.71% ±0.62% |
| 9 | `wide-map-capacity` | Price the city ceiling off uncontested land. | **on** | +32 | +45 | +18 | 17.18% (n=77,117) | 16.11% (n=72,169) | 1.07% | +53 [+24, +81] | 100.0% | +0.67 (z +3.98) helps * | +0.45% ±0.45% | -0.07% ±0.81% |
| 10 | `idle-faith-patronage` | A seat with no religion and 600+ Faith patronizes Great People with it whatever the shortfall. | **on** | +30 | +19 | +18 | 17.03% (n=54,681) | 16.27% (n=49,713) | 0.76% | +31 [+16, +46] | 100.0% | +0.37 (z +2.06) helps * | -0.35% ±0.42% | -0.82% ±0.73% |
| 11 | `religious-defence-scales` | Size the defensive Missionary corps by the number of cities actually under conversion pressure instead of the shipped constant 2. | off | +30 | -4 | – | 16.70% (n=28,619) | 16.63% (n=28,627) | 0.07% | +17 [-48, +83] | 69.9% | +0.23 (z +1.53) ~ | -0.21% ±0.39% | -0.22% ±0.73% |
| 12 | `opportunistic-war` | Open a surprise war on a neighbour whose unescorted Settlers, Builders or unpillaged tiles lie within a short march of our soldiers, take them, and sue for peace. | **on** | +27 | +25 | +11 | 17.04% (n=48,718) | 16.25% (n=43,676) | 0.80% | +41 [+18, +63] | 100.0% | +0.58 (z +3.41) helps * | +0.14% ±0.43% | +0.52% ±0.79% |
| 13 | `barbarian-scouts-are-scouts` | Stop pricing a Firaxis barbarian scout as a threat. | **on** | +26 | +6 | +12 | 17.00% (n=77,204) | 16.31% (n=72,082) | 0.70% | +37 [+15, +59] | 100.0% | +0.12 (z +0.67) ~ | +0.41% ±0.43% | +0.56% ±0.76% |
| 14 | `apostle-promotion-by-role` | Promote an Apostle for the job the empire has rather than for the largest number on the card. | **on** | +24 | +8 | +7 | 16.73% (n=77,131) | 16.60% (n=72,155) | 0.13% | +6 [-19, +32] | 69.2% | +0.14 (z +0.79) ~ | -0.19% ±0.45% | -0.57% ±0.79% |
| 15 | `tactical-strategy` | Enable explicit battlefield roles: the land-unit counter cycle, safe ranged standoff, wall-focused siege/support, and cavalry job priority. | off | +23 | – | – | 17.14% (n=4,959) | 16.20% (n=5,043) | 0.94% | +47 [-26, +120] | 89.8% | +0.11 (z +0.71) ~ | +0.10% ±0.37% | +0.24% ±0.66% |
| 16 | `lane-policy-deck` | Choose the policy cards for the victory the empire is actually racing while its plan is still Expansion. | off | +22 | +0 | – | 16.74% (n=28,591) | 16.59% (n=28,655) | 0.15% | +9 [-26, +44] | 69.8% | +0.20 (z +1.29) ~ | -0.42% ±0.37% | -0.41% ±0.66% |
| 17 | `raid-pillage-prizes` | Count a neighbour's unpillaged tiles within reach as raid prizes and send raiding soldiers to them. | **on** | +20 | +26 | +15 | 17.07% (n=48,804) | 16.21% (n=43,590) | 0.86% | +44 [+22, +66] | 100.0% | +0.46 (z +2.56) helps * | -0.17% ±0.43% | +0.14% ±0.75% |
| 18 | `research-grants-first` | A finished research city pays more for its own district's project. | off | +20 | -10 | – | 16.57% (n=28,605) | 16.76% (n=28,641) | -0.19% | +1 [-56, +58] | 51.0% | +0.05 (z +0.34) ~ | -0.56% ±0.39% | -0.67% ±0.70% |
| 19 | `spread-campaign-persists` | Keep a spread campaign that has already converted a foreign city on the offensive between waves, instead of dropping the posture the turn its last charge is spent. | off | +19 | -8 | – | 16.60% (n=28,682) | 16.73% (n=28,564) | -0.13% | +0 [-47, +48] | 50.8% | -0.23 (z -1.47) ~ | +0.44% ±0.37% | +0.86% ±0.67% |
| 20 | `barbarian-ranged-answer` | Answer a ring of shooters with a shooter. | **on** | +18 | +8 | +7 | 16.83% (n=48,716) | 16.48% (n=43,678) | 0.35% | +18 [-5, +40] | 94.1% | +0.14 (z +0.78) ~ | +0.04% ±0.48% | +0.13% ±0.87% |
| 21 | `buildings-before-projects` | A district project waits behind the science and production buildings the city can already build. | **on** | +18 | +31 | -1 | 16.96% (n=77,048) | 16.35% (n=72,238) | 0.61% | +30 [+5, +55] | 99.0% | +0.16 (z +0.93) ~ | +0.68% ±0.43% | +1.09% ±0.77% |
| 22 | `lane-culture-spending` | Run the Culture lane's Faith pass — the Naturalist that founds a National Park, the touring Rock Bands — and size its reserve, for an empire racing Culture whose plan has not named the lane. | off | +18 | +4 | – | 16.80% (n=28,680) | 16.54% (n=28,566) | 0.26% | +12 [-16, +41] | 79.9% | +0.13 (z +0.89) ~ | -0.36% ±0.38% | -1.27% ±0.66% |
| 23 | `enhancer-for-the-corps` | Evangelize the beliefs that multiply a religious corps while the corps has a job, instead of the victory lane's worship building. | off | +16 | +4 | – | 16.79% (n=28,655) | 16.55% (n=28,591) | 0.24% | +11 [-17, +40] | 78.3% | +0.09 (z +0.57) ~ | -0.04% ±0.41% | -0.13% ±0.73% |
| 24 | `guru-heals-the-corps` | Let a founder that is defending its own cities hold one Guru, the only field heal a religious corps has. | off | +16 | -15 | – | 16.48% (n=28,631) | 16.85% (n=28,615) | -0.37% | -8 [-65, +50] | 39.5% | +0.11 (z +0.69) ~ | +0.15% ±0.37% | +0.30% ±0.66% |
| 25 | `religious-units-heal-first` | Let a wounded spreader standing in its own Holy Site's heal ring hold instead of spending a charge at a fraction of its strength. | off | +16 | +6 | – | 16.82% (n=28,528) | 16.51% (n=28,718) | 0.31% | +15 [-13, +43] | 85.4% | -0.00 (z -0.02) ~ | +0.23% ±0.38% | +0.72% ±0.70% |
| 26 | `war-reinforcement` | March rear units to the campaign objective while the war is on. | **on** | +16 | +17 | +1 | 16.87% (n=77,142) | 16.45% (n=72,144) | 0.43% | +21 [+0, +42] | 97.6% | +0.35 (z +1.95) ~ | +0.83% ±0.44% | +1.23% ±0.75% |
| 27 | `come-ashore` | Keep the land army out of the water. | **on** | +15 | +2 | +6 | 16.77% (n=77,144) | 16.56% (n=72,142) | 0.21% | +11 [-7, +28] | 87.8% | +0.03 (z +0.16) ~ | -0.70% ±0.42% | -0.97% ±0.73% |
| 28 | `priced-tile-purchase` | A border plot is bought only when its priced benefit clears its Gold by a margin. | off | +15 | -6 | -15 | 16.52% (n=46,183) | 16.81% (n=46,211) | -0.29% | -15 [-39, +9] | 11.3% | -0.04 (z -0.27) ~ | -0.59% ±0.38% | -0.89% ±0.67% |
| 29 | `settle-sooner` | Price a Settler's walk in turns, each turn dearer the longer the Settler has already been walking, so expansion founds sooner without giving up a site good enough to pay for its walk. | **on** | +15 | +14 | +20 | 16.98% (n=48,657) | 16.32% (n=43,737) | 0.67% | +34 [+12, +56] | 99.9% | +0.10 (z +0.61) ~ | -0.23% ±0.42% | -0.53% ±0.73% |
| 30 | `settler-target-hysteresis` | Keep a settler target dropped for danger out of the next picks for a few turns. | off | +15 | -9 | +7 | 16.65% (n=74,660) | 16.68% (n=74,626) | -0.03% | -2 [-19, +16] | 41.8% | -0.11 (z -0.72) ~ | -0.73% ±0.39% | -0.82% ±0.71% |
| 31 | `congress-counter-votes` | Back a ballot aimed at the empire closest to a victory with everything the treasury can spare — a losing vote is refunded in full, so an opposition that fails costs no Favor. | off | +14 | -6 | – | 16.61% (n=28,684) | 16.72% (n=28,562) | -0.11% | -6 [-36, +24] | 35.2% | +0.12 (z +0.78) ~ | +0.41% ±0.38% | +0.60% ±0.70% |
| 32 | `district-coverage` | Rank district families by how much of the empire still lacks them. | off | +13 | -11 | +16 | 16.65% (n=74,748) | 16.68% (n=74,538) | -0.03% | -1 [-23, +21] | 47.2% | +0.19 (z +1.25) ~ | +0.38% ±0.36% | +0.50% ±0.64% |
| 33 | `settler-site-agreement` | THE ORDER AND THE MARCH MUST AGREE ON THE GROUND. | off | +13 | -11 | +12 | 16.68% (n=74,639) | 16.66% (n=74,647) | 0.02% | +3 [-21, +26] | 58.5% | +0.18 (z +1.20) ~ | -0.05% ±0.39% | -0.26% ±0.71% |
| 34 | `fortify-idle-units` | Fortify units the planner gave nothing to do. | off | +12 | – | – | 16.90% (n=5,082) | 16.42% (n=4,920) | 0.48% | +24 [-49, +97] | 74.1% | -0.11 (z -0.67) ~ | -0.01% ±0.40% | +0.21% ±0.74% |
| 35 | `religion-sues-peace` | A Religion strategy offers peace to unblock its spread lane. | off | +12 | -9 | +3 | 16.74% (n=74,598) | 16.59% (n=74,688) | 0.15% | +8 [-11, +28] | 80.4% | -0.00 (z -0.02) ~ | +0.21% ±0.36% | +0.47% ±0.64% |
| 36 | `settler-guard-holds` | A stacked guard holds with its settler, and only a guard that can hold counts as protection. | off | +12 | -4 | -1 | 16.67% (n=74,688) | 16.67% (n=74,598) | 0.00% | -0 [-18, +17] | 49.3% | -0.02 (z -0.12) ~ | +0.07% ±0.38% | -0.55% ±0.68% |
| 37 | `wonder-ring-settle-value` | Price a revealed natural wonder's ring into the settle scorer. | **on** | +12 | +11 | +3 | 16.78% (n=77,106) | 16.55% (n=72,180) | 0.23% | +12 [-6, +29] | 90.0% | +0.11 (z +0.68) ~ | -0.10% ±0.43% | -0.11% ±0.77% |
| 38 | `governor-expansion-lane` | The other half: the governor under Expansion only. | off | +11 | -14 | -15 | 16.44% (n=46,162) | 16.90% (n=46,232) | -0.46% | -24 [-46, -2] | 1.8% | -0.00 (z -0.01) ~ | +0.19% ±0.37% | +0.46% ±0.64% |
| 39 | `campus-adjacency-threshold` | A Campus plot that clears the multiplier's adjacency threshold is credited what crossing it unlocks. | off | +10 | -9 | – | 16.56% (n=28,576) | 16.78% (n=28,670) | -0.22% | -12 [-40, +16] | 19.9% | +0.24 (z +1.58) ~ | +0.02% ±0.36% | -0.36% ±0.64% |
| 40 | `competition-victory-points` | Price a scored competition's first place by the Diplomatic Victory Points it pays, at the rate `strategic_wonder_value` already pays a wonder's. | off | +9 | +0 | – | 16.70% (n=28,546) | 16.64% (n=28,700) | 0.06% | +3 [-26, +31] | 56.9% | +0.01 (z +0.06) ~ | -0.25% ±0.37% | -0.21% ±0.66% |
| 41 | `maintenance-aware-deck` | Let the deck counterfactual see the unit-maintenance bill. | off | +9 | – | – | 16.86% (n=4,929) | 16.48% (n=5,073) | 0.38% | +19 [-53, +91] | 69.7% | +0.36 (z +2.45) helps * | -0.26% ±0.37% | -0.67% ±0.66% |
| 42 | `relief-targets-the-siege` | Send a relief force at the units actually besieging the city rather than the nearest one to itself. | **on** | +9 | +3 | +7 | 16.72% (n=77,143) | 16.61% (n=72,143) | 0.11% | +6 [-12, +23] | 73.9% | +0.15 (z +0.83) ~ | -0.25% ±0.45% | -0.51% ±0.78% |
| 43 | `fifteenth-citizen` | A Campus city within reach of the Population gate credits growth with what crossing it unlocks. | off | +8 | -6 | – | 16.60% (n=28,550) | 16.74% (n=28,696) | -0.14% | -8 [-36, +20] | 29.2% | -0.07 (z -0.46) ~ | +0.48% ±0.38% | +1.26% ±0.68% |
| 44 | `one-launch-pad` | Give the 3,000-point first-pad rung to one city at a time. | off | +8 | -5 | +8 | 16.77% (n=74,614) | 16.56% (n=74,672) | 0.21% | +10 [-7, +28] | 87.4% | -0.04 (z -0.25) ~ | +0.36% ±0.38% | +0.63% ±0.67% |
| 45 | `builder-barbarian-safety` | Keep Builders from entering a visible Barbarian-capture envelope. | off | +7 | -5 | +6 | 16.68% (n=46,197) | 16.65% (n=46,197) | 0.03% | +1 [-21, +23] | 53.8% | -0.19 (z -1.26) ~ | -0.02% ±0.40% | +0.12% ±0.69% |
| 46 | `lane-congress-favor` | Stake the Favor behind a World Congress ballot for the victory the empire is actually racing. | off | +7 | -7 | – | 16.58% (n=28,601) | 16.76% (n=28,645) | -0.18% | -10 [-38, +18] | 24.8% | +0.09 (z +0.57) ~ | -0.23% ±0.37% | -0.53% ±0.65% |
| 47 | `power-the-laboratory` | A power plant is credited the yields it switches on in its city. | off | +7 | -4 | – | 16.62% (n=28,563) | 16.71% (n=28,683) | -0.09% | -5 [-33, +23] | 36.1% | +0.22 (z +1.41) ~ | -0.25% ±0.37% | -0.34% ±0.67% |
| 48 | `coupled-expansion` | Enable the evaluator-only paid expansion treatment. | off | +6 | – | – | 16.79% (n=4,950) | 16.55% (n=5,052) | 0.24% | +12 [-61, +85] | 62.7% | +0.12 (z +0.75) ~ | -0.44% ±0.37% | -0.41% ±0.66% |
| 49 | `unit-cost-efficiency` | Credit strength-per-production and the civ's own unique unit in the military production arm. | off | +6 | – | – | 16.80% (n=4,965) | 16.54% (n=5,037) | 0.26% | +13 [-61, +87] | 63.5% | +0.06 (z +0.40) ~ | +0.05% ±0.38% | -0.22% ±0.65% |
| 50 | `housing-districts` | Let the baseline governor raise the housing ceiling. | off | +5 | -18 | +2 | 16.51% (n=74,583) | 16.82% (n=74,703) | -0.30% | -16 [-33, +2] | 4.1% | +0.09 (z +0.60) ~ | -0.33% ±0.38% | -0.60% ±0.67% |
| 51 | `civilian-rescue` | Walk onto a capturable civilian within reach, and never decline a settler held by the barbarians. | off | +4 | -1 | -3 | 16.63% (n=74,633) | 16.70% (n=74,653) | -0.06% | -3 [-21, +14] | 35.7% | -0.03 (z -0.23) ~ | +0.20% ±0.37% | +0.69% ±0.67% |
| 52 | `lane-congress-ballot` | Score the World Congress ballot — which outcome and target this seat names — for the victory the empire is actually racing rather than for an expansion posture that has no lane. | off | +4 | +4 | – | 16.75% (n=28,594) | 16.59% (n=28,652) | 0.16% | +8 [-20, +36] | 71.4% | +0.14 (z +0.93) ~ | -0.60% ±0.36% | -1.31% ±0.64% |
| 53 | `theology-for-founders` | A founder researches Theology next. | off | +4 | +11 | -8 | 16.72% (n=52,183) | 16.62% (n=52,211) | 0.10% | +4 [-16, +24] | 65.6% | -0.05 (z -0.33) ~ | +0.80% ±0.38% | +1.19% ±0.67% |
| 54 | `whole-turn-backtrack-guard` | Refuse a step onto any tile this unit has already stood on this turn. | **on** | +4 | +3 | +9 | 16.84% (n=77,103) | 16.48% (n=72,183) | 0.37% | +18 [+1, +36] | 97.9% | -0.23 (z -1.31) ~ | -0.21% ±0.43% | -0.67% ±0.75% |
| 55 | `blind-objective-strength` | Stop a fogged objective city from reading as an empty tile when the army decides whether it is strong enough to engage. | off | +3 | +0 | +15 | 16.84% (n=74,677) | 16.49% (n=74,609) | 0.35% | +17 [-0, +34] | 97.2% | -0.05 (z -0.36) ~ | +0.50% ±0.39% | +1.02% ±0.70% |
| 56 | `holy-site-where-the-threat-is` | Put a Holy Site in the city that is actually losing its majority, so its defender can be bought there instead of walking from the Holy City. | off | +3 | -9 | – | 16.52% (n=28,576) | 16.81% (n=28,670) | -0.28% | -15 [-43, +13] | 14.8% | -0.22 (z -1.44) ~ | -0.03% ±0.37% | -0.02% ±0.66% |
| 57 | `recorded-tactical-step` | Record tactical steps so a unit stepped twice in one turn cannot walk back onto the tile it just left. | **on** | +3 | +9 | +15 | 16.81% (n=77,158) | 16.51% (n=72,128) | 0.30% | +15 [-2, +33] | 95.6% | +0.13 (z +0.72) ~ | -0.48% ±0.45% | -0.72% ±0.81% |
| 58 | `camp-party` | The peacetime camp party. | off | +2 | -8 | +6 | 16.83% (n=74,662) | 16.50% (n=74,624) | 0.34% | +19 [-8, +47] | 91.6% | -0.07 (z -0.47) ~ | +0.39% ±0.39% | +0.81% ±0.70% |
| 59 | `joint-tactics` | Plan each engagement's attacks as one joint problem instead of one unit at a time in a fixed class order. | off | – | – | +2 | 16.61% (n=46,020) | 16.72% (n=46,020) | -0.10% | -5 [-28, +17] | 32.6% | +0.25 (z +3.84) helps * | +27.29% ±0.47% | +27.69% ±0.79% |
| 60 | `loyalty-rate-alarm` | Rank loyalty emergencies by turns-to-flip instead of by level. | **on** | +2 | +19 | +20 | 17.08% (n=77,219) | 16.23% (n=72,067) | 0.85% | +44 [+26, +61] | 100.0% | +0.18 (z +1.02) ~ | +0.33% ±0.45% | +0.49% ±0.81% |
| 61 | `price-the-suzerainty` | Let the envoy scorer see the suzerainty it is walking toward. | off | +2 | – | – | 16.71% (n=5,046) | 16.63% (n=4,956) | 0.08% | +4 [-69, +77] | 54.3% | +0.17 (z +1.09) ~ | -0.30% ±0.38% | -0.43% ±0.70% |
| 62 | `settler-threat-detour` | Let a Settler switch to the best safe alternate when a visible threat blocks the next step toward an otherwise sound settlement site. | **on** | +2 | +9 | +25 | 16.94% (n=48,726) | 16.36% (n=43,668) | 0.58% | +30 [+6, +53] | 99.4% | -0.20 (z -1.12) ~ | +0.61% ±0.43% | +0.79% ±0.77% |
| 63 | `research-tier-premium` | A Campus building's debt is scaled by its own Science against the chain's first rung. | off | +1 | -3 | – | 16.62% (n=28,618) | 16.71% (n=28,628) | -0.09% | -5 [-33, +24] | 37.4% | -0.02 (z -0.10) ~ | +0.42% ±0.39% | +0.69% ±0.67% |
| 64 | `settlement-gap-target` | Make the settlement-gap redirect and the Settler ranking honour the same city target the cascade settles toward. | off | +1 | – | – | 16.70% (n=5,037) | 16.64% (n=4,965) | 0.06% | +3 [-70, +76] | 53.2% | +0.15 (z +0.97) ~ | -0.12% ±0.38% | -0.61% ±0.68% |
| 65 | `stranded-settler-discount` | Stop a Settler that has stopped walking from holding the expansion gate shut. | off | +1 | -4 | +7 | 16.72% (n=74,635) | 16.61% (n=74,651) | 0.11% | +6 [-12, +23] | 73.3% | +0.11 (z +0.68) ~ | -0.34% ±0.36% | -0.59% ±0.65% |
| 66 | `strategic-wonders` | Build the wonders the chosen victory actually needs. | off | +1 | +6 | -3 | 16.77% (n=74,693) | 16.56% (n=74,593) | 0.21% | +10 [-7, +28] | 87.9% | -0.24 (z -1.54) ~ | -0.11% ±0.36% | -0.34% ±0.63% |
| 67 | `culture-coverage` | Pay for the Theater Square the empire has not got. | off | +0 | -7 | – | 16.55% (n=28,585) | 16.78% (n=28,661) | -0.23% | -12 [-40, +16] | 20.0% | +0.00 (z +0.01) ~ | +0.43% ±0.38% | +0.85% ±0.67% |
| 68 | `escort-unstick` | Release an escort that is not walking its settler. | **on** | +0 | +18 | +36 | 16.95% (n=77,181) | 16.36% (n=72,105) | 0.59% | +27 [-1, +56] | 97.0% | +0.02 (z +0.09) ~ | -0.39% ±0.45% | -0.63% ±0.79% |
| 69 | `amenity-project-preemption` | When host-observed Amenity deficits have crossed a severe empire-wide threshold, pause one repeatable project for the concrete repair chain and let the policy deck use its direct empire-wide repair. | off | -1 | -17 | -7 | 16.58% (n=74,693) | 16.76% (n=74,593) | -0.18% | -7 [-31, +16] | 26.9% | -0.02 (z -0.12) ~ | +0.54% ±0.37% | +0.78% ±0.66% |
| 70 | `culture-building-debt` | Make the Theater Square owe its buildings. | **on** | -1 | +12 | – | 16.85% (n=31,123) | 16.45% (n=26,123) | 0.39% | +21 [-8, +50] | 92.0% | +0.03 (z +0.15) ~ | +0.63% ±0.43% | +1.10% ±0.77% |
| 71 | `slot-kind-tiebreak` | Break a production cost tie by which great-work slots can be filled. | off | -1 | +0 | -6 | 16.72% (n=74,592) | 16.61% (n=74,694) | 0.11% | +5 [-12, +23] | 72.5% | -0.21 (z -1.37) ~ | -0.45% ±0.39% | -0.60% ±0.70% |
| 72 | `contact-posture` | A unit already inside a hostile's next-turn reach picks a posture: stand and heal where the melee exchange favours holding, close on a shooter it cannot answer, or step out of that shooter's envelope. | off | -2 | -27 | – | 16.21% (n=28,674) | 17.13% (n=28,572) | -0.92% | -41 [-85, +3] | 3.5% | -0.05 (z -0.36) ~ | +0.15% ±0.39% | +0.73% ±0.69% |
| 73 | `housing-research` | Aim research at the housing ceiling when the empire is paying it. | off | -2 | -9 | +7 | 16.67% (n=74,711) | 16.66% (n=74,575) | 0.01% | +1 [-23, +26] | 54.6% | -0.13 (z -0.82) ~ | +0.62% ±0.38% | +1.25% ±0.69% |
| 74 | `peacetime-deterrence` | Let the strongest met major weigh on the army target while at peace, so deterrence exists before a declaration. | **on** | -2 | +10 | +1 | 16.81% (n=77,095) | 16.52% (n=72,191) | 0.29% | +15 [-3, +33] | 94.7% | +0.01 (z +0.07) ~ | +0.08% ±0.44% | +0.18% ±0.80% |
| 75 | `strike-opening` | Let movement credit the attack a tile opens. | **on** | -3 | +2 | +8 | 16.79% (n=77,096) | 16.54% (n=72,190) | 0.25% | +13 [-5, +30] | 92.5% | -0.02 (z -0.13) ~ | -0.76% ±0.43% | -0.87% ±0.76% |
| 76 | `amenity-district-path` | Price an amenity district by the building it will host and a regional amenity building by every city it reaches. | **on** | -4 | +8 | +6 | 16.76% (n=77,135) | 16.57% (n=72,151) | 0.18% | +10 [-8, +27] | 85.6% | -0.19 (z -1.11) ~ | -0.49% ±0.41% | -0.95% ±0.73% |
| 77 | `promote-when-wounded` |  | off | -4 | – | – | 16.59% (n=5,069) | 16.74% (n=4,933) | -0.15% | -8 [-81, +66] | 41.9% | +0.05 (z +0.30) ~ | +0.00% ±0.37% | +0.06% ±0.67% |
| 78 | `barbarian-bargain` | Price a raider's life below a major's. | **on** | -5 | +5 | +2 | 16.72% (n=48,683) | 16.60% (n=43,711) | 0.12% | +6 [-16, +29] | 70.9% | +0.02 (z +0.10) ~ | -0.56% ±0.41% | -1.03% ±0.72% |
| 79 | `inquisition-on-threat` | A founder under conversion pressure may hold one Apostle for the Inquisition, bought after its Missionaries when the bank covers it. | **on** | -5 | +1 | +1 | 16.71% (n=54,664) | 16.61% (n=49,730) | 0.10% | +8 [-12, +28] | 78.6% | -0.04 (z -0.21) ~ | +0.67% ±0.45% | +1.04% ±0.79% |
| 80 | `science-multiplier-payoff` | Credit a Campus building the beakers its city's multipliers will actually pay it. | off | -5 | -4 | – | 16.59% (n=28,649) | 16.75% (n=28,597) | -0.16% | -8 [-36, +20] | 28.9% | -0.04 (z -0.27) ~ | +0.38% ±0.36% | +0.17% ±0.62% |
| 81 | `unit-objective-memory` | Let a unit retain its campaign objective and a short, threat-driven retreat across turns. | off | -5 | – | – | 16.57% (n=4,968) | 16.77% (n=5,034) | -0.20% | -10 [-84, +64] | 39.6% | -0.06 (z -0.42) ~ | -0.08% ±0.38% | -0.58% ±0.67% |
| 82 | `congress-banks-decided` | Answer a World Congress resolution that is already decided with the one free vote on its settled winner, taking the Diplomatic Victory Point for an exact prediction and staking nothing. | off | -6 | -4 | – | 16.58% (n=28,691) | 16.76% (n=28,555) | -0.18% | -9 [-37, +19] | 26.6% | +0.08 (z +0.53) ~ | +0.40% ±0.40% | +0.54% ±0.69% |
| 83 | `district-lookahead-settle` | A settler scores a site by the districts the plan would build there, each on its own plot. | off | -6 | -21 | -11 | 16.36% (n=46,164) | 16.97% (n=46,230) | -0.61% | -31 [-53, -9] | 0.3% | -0.30 (z -1.93) ~ | +0.29% ±0.40% | -0.05% ±0.73% |
| 84 | `holy-lane-parity` | The Religion lane pays for its Holy Site what the Culture lane pays for its Theater Square. | **on** | -6 | +10 | +32 | 17.00% (n=61,837) | 16.31% (n=56,957) | 0.69% | +31 [-12, +74] | 92.0% | -0.21 (z -1.23) ~ | -0.18% ±0.45% | -0.27% ±0.80% |
| 85 | `condemn-under-congress` | Condemn a heretic the World Congress has condemned, not only one this seat is at war with. | off | -7 | -4 | – | 16.58% (n=28,654) | 16.75% (n=28,592) | -0.17% | -8 [-37, +20] | 27.8% | +0.01 (z +0.08) ~ | +0.39% ±0.37% | +0.60% ±0.66% |
| 86 | `siege-commitment` | Keep a live campaign pointed at its chosen city. | off | -7 | +3 | +0 | 16.61% (n=74,668) | 16.72% (n=74,618) | -0.11% | -5 [-23, +12] | 28.3% | -0.01 (z -0.07) ~ | +0.11% ±0.38% | +0.03% ±0.71% |
| 87 | `one-shot-recovery` | A unit one enemy blow from death withdraws to safe healing ground, and leaves that ground again the moment an enemy can strike it. | off | -8 | -4 | – | 16.57% (n=28,631) | 16.76% (n=28,615) | -0.19% | -10 [-38, +19] | 25.3% | -0.18 (z -1.13) ~ | +0.48% ±0.36% | +0.94% ±0.63% |
| 88 | `campus-finishes-first` | The Campus coverage term is scaled by how finished the empire's standing Campuses are. | off | -9 | +2 | – | 16.66% (n=28,581) | 16.67% (n=28,665) | -0.00% | +0 [-28, +29] | 51.2% | +0.13 (z +0.83) ~ | +0.10% ±0.36% | +0.06% ±0.64% |
| 89 | `lane-great-people` | Rank Great Person classes, and the Great Person points a project earns, by the victory the empire is actually racing rather than by a war it is fighting. | off | -9 | -1 | – | 16.61% (n=28,747) | 16.72% (n=28,499) | -0.11% | -5 [-33, +23] | 35.8% | -0.18 (z -1.18) ~ | +0.45% ±0.37% | +0.57% ±0.67% |
| 90 | `lane-space-race` | Treat an empire racing Science as a Science seat throughout the space race: the pad count, the city a launch project may claim and the city a pad may be sited in all read the race rather than an explicitly assigned target, and the pass opens at all. | off | -9 | +5 | – | 16.72% (n=28,556) | 16.61% (n=28,690) | 0.11% | +6 [-22, +35] | 66.1% | -0.11 (z -0.72) ~ | -0.47% ±0.39% | -0.57% ±0.67% |
| 91 | `barbarian-capture-priority` | Take a visible Barbarian Settler or Scout in exact one-turn reach before healing, retreat, or any ordinary tactical choice. | off | -10 | +1 | – | 16.64% (n=28,590) | 16.69% (n=28,656) | -0.05% | -2 [-30, +26] | 44.5% | -0.08 (z -0.53) ~ | -0.20% ±0.37% | -0.45% ±0.66% |
| 92 | `army-target-weighs-enemy` | Let the army target account for the enemy it has to beat. | **on** | -13 | +9 | +15 | 16.71% (n=77,100) | 16.62% (n=72,186) | 0.09% | +3 [-21, +27] | 58.5% | -0.06 (z -0.35) ~ | +0.32% ±0.44% | +0.35% ±0.76% |
| 93 | `endgame-war-runway` | Keep a fresh direct declaration out of the final campaign reserve. | off | -13 | -8 | -3 | 16.57% (n=74,606) | 16.76% (n=74,680) | -0.19% | -10 [-27, +8] | 14.0% | -0.05 (z -0.34) ~ | +0.66% ±0.36% | +0.93% ±0.67% |
| 94 | `naval-recon` | Buy one ship for an empire that has none while unexplored water lies off its coast, and send it exploring. | off | -14 | -10 | +8 | 16.57% (n=74,565) | 16.76% (n=74,721) | -0.18% | -9 [-26, +8] | 15.7% | -0.20 (z -1.31) ~ | +0.08% ±0.37% | -0.10% ±0.64% |
| 95 | `founder-temple` | A founder outside the Religion lane still builds its Shrine and Temple. | **on** | -15 | +4 | +7 | 16.77% (n=54,784) | 16.55% (n=49,610) | 0.22% | +15 [-9, +40] | 89.0% | +0.05 (z +0.26) ~ | +0.62% ±0.42% | +0.77% ±0.76% |
| 96 | `builder-reward-survey` | Price Builder production by a survey of the work it would do. | off | -17 | – | – | 16.34% (n=5,050) | 17.00% (n=4,952) | -0.67% | -33 [-106, +39] | 18.3% | +0.10 (z +0.66) ~ | +1.47% ±0.37% | +1.88% ±0.63% |
| 97 | `home-defense` | Let a raider standing in our own territory claim a unit before the offensive does. | off | -17 | -9 | -7 | 16.51% (n=74,665) | 16.83% (n=74,621) | -0.32% | -16 [-33, +2] | 3.9% | +0.02 (z +0.12) ~ | -0.57% ±0.37% | -0.78% ±0.65% |
| 98 | `science-payback-horizon` | Price the science economy on whether it can still repay rather than on how much of the game is left. | off | -17 | -5 | – | 16.52% (n=28,563) | 16.81% (n=28,683) | -0.30% | -14 [-42, +14] | 16.0% | -0.05 (z -0.33) ~ | -0.58% ±0.36% | -0.52% ±0.64% |
| 99 | `barbarian-hunt` | Walk onto a visible, undefended barbarian camp one legal step away — the clear IS the move, so no attack scan ever offers it, and without this a unit ends its turn beside a free 50-gold clear until the camp spawns the archer that kills it. | off | -20 | +5 | -43 | 16.35% (n=46,215) | 16.99% (n=46,179) | -0.64% | -38 [-108, +31] | 13.9% | -0.30 (z -1.94) ~ | -0.97% ±0.37% | -1.25% ±0.65% |
| 100 | `chain-tech-lookahead` | The research goal aims at a Campus rung the empire can BUILD, not only one it has already built. | off | -25 | -12 | – | 16.38% (n=28,597) | 16.96% (n=28,649) | -0.58% | -28 [-57, +0] | 2.5% | -0.19 (z -1.24) ~ | -0.33% ±0.38% | -0.39% ±0.69% |
| 101 | `coordinated-finish` | Admit the friendly-volley extension without the rest of the closed war-half bundle. | off | -26 | – | – | 16.15% (n=4,966) | 17.18% (n=5,036) | -1.03% | -51 [-125, +22] | 8.6% | -0.09 (z -0.60) ~ | -0.19% ±0.37% | -0.06% ±0.67% |
| 102 | `research-floor-holds` | The citizen tilt and the beaker floor hold while the research can still pay. | off | -26 | -9 | – | 16.42% (n=28,628) | 16.91% (n=28,618) | -0.49% | -24 [-52, +5] | 5.0% | -0.31 (z -2.07) hurts * | -0.39% ±0.38% | -0.86% ±0.67% |
| 103 | `siege-is-progress` | A SIEGE THAT IS WINNING IS NOT A STALLED WAR. | off | -26 | -3 | -8 | 16.49% (n=74,701) | 16.85% (n=74,585) | -0.36% | -20 [-47, +7] | 6.9% | -0.05 (z -0.33) ~ | +0.10% ±0.38% | +0.12% ±0.68% |
| 104 | `envoy-infrastructure` | Value the infrastructure that produces city-state influence: the Consulate and Chancery's per-turn influence becomes the envoys it can produce before the turn limit, and a first Diplomatic Quarter sees part of the Consulate stream it unlocks. | off | -29 | -3 | – | 16.52% (n=28,626) | 16.82% (n=28,620) | -0.30% | -21 [-68, +25] | 18.4% | -0.09 (z -0.61) ~ | -0.08% ±0.38% | -0.45% ±0.67% |
| 105 | `pantheon-board` | Choose the pantheon from the land the empire holds rather than from a fixed order. | off | -31 | – | – | 16.03% (n=4,922) | 17.28% (n=5,080) | -1.25% | -63 [-136, +10] | 4.6% | -0.30 (z -1.94) ~ | -0.24% ±0.38% | -0.35% ±0.67% |
| 106 | `score-horizon` | Skip a space race or a bomb that cannot finish before the turn limit. | **on** | -31 | +9 | +12 | 16.79% (n=77,194) | 16.53% (n=72,092) | 0.26% | +11 [-13, +36] | 82.0% | -0.28 (z -1.54) ~ | -0.06% ±0.43% | -0.05% ±0.76% |
| 107 | `siege-tracks-wall` | Size the siege train by the wall it has to breach. | off | -33 | +4 | -2 | 16.77% (n=74,643) | 16.56% (n=74,643) | 0.21% | +9 [-18, +36] | 74.3% | -0.14 (z -0.94) ~ | +0.64% ±0.37% | +0.77% ±0.66% |
| 108 | `early-contact-window` | Buy the second and third Scout while the world's borders are still open — after Early Empire a city-state cannot be met by land at all. | off | -34 | +1 | – | 16.56% (n=28,679) | 16.77% (n=28,567) | -0.21% | -25 [-91, +41] | 23.0% | -0.31 (z -2.11) hurts * | +0.10% ±0.37% | -0.04% ±0.67% |
| 109 | `blind-objective-units` | Let the army price the enemy units it REMEMBERS around an objective it cannot currently see, instead of reading an unseen approach as empty. | off | -36 | +0 | -3 | 16.62% (n=74,654) | 16.71% (n=74,632) | -0.09% | -4 [-21, +13] | 32.1% | -0.32 (z -2.07) hurts * | +0.29% ±0.38% | +0.39% ±0.70% |
| 110 | `naval-production-policy` | Reach for the naval-production discount while hulls are wanted. | off | -42 | – | – | 15.79% (n=4,825) | 17.48% (n=5,177) | -1.69% | -84 [-159, -10] | 1.3% | -0.67 (z -4.42) hurts * | -0.26% ±0.38% | +0.09% ±0.67% |
| 111 | `district-building-chain` | Make every specialty district owe its own buildings, whatever the lane. | off | -44 | +0 | – | 16.50% (n=28,525) | 16.83% (n=28,721) | -0.32% | -38 [-123, +47] | 19.1% | -0.31 (z -2.07) hurts * | -0.20% ±0.35% | +0.33% ±0.63% |
| 112 | `war-patience` | Keep prosecuting a war the empire overwhelmingly outweighs instead of suing it out as stalled. | off | -64 | -14 | -10 | 16.49% (n=74,722) | 16.84% (n=74,564) | -0.34% | -22 [-56, +12] | 10.2% | -0.50 (z -3.17) hurts * | +0.19% ±0.38% | +0.42% ±0.65% |
| 113 | `governor-victory-lanes` | Half the composite: the governor under the four victory lanes only. | off | -82 | -118 | +23 | 15.37% (n=49,837) | 17.97% (n=49,757) | -2.60% | -148 [-312, +16] | 3.9% | -1.41 (z -9.27) hurts * | +0.08% ±0.39% | +0.35% ±0.67% |
| 114 | `governor-every-lane` | Run the strategic governor under every lane. | off | -108 | -117 | +7 | 15.69% (n=74,655) | 17.64% (n=74,631) | -1.94% | -99 [-210, +11] | 3.9% | -1.42 (z -9.25) hurts * | +0.06% ±0.36% | +0.56% ±0.64% |

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

| Shape | Sources | Player seats | Genes priced |
|---|---:|---:|---:|
| standard | 2 | 54,444 | 99 |
| legacy | 7 | 132,440 | 65 |

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
| `district-planning` | off (unmeasured) | The city plans its districts, sites and tile buys together: wished districts get jointly assigned, reserved plots over rings 1-3, and the tile a very valuable site needs is bought. |
| `escort-unstick-2` | off (unmeasured) | Version 2 of `escort_unstick`: the same two-turn release, refused while a visible barbarian raider can reach the settler's tile. |
| `settle-plan-ahead` | off (unmeasured) | Rank a settle site by the cities it leaves room for as well as its own ground, so a Settler stops taking the one plot in a pocket that would have held two. |

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

Every screenable heuristic gene on the Advanced controller, ranked most beneficial to least by the latest fixed batch. Each batch header carries its actual player-seat count once; cells show the enabled arm's excess projected to 10,000 **total** player seats, where a six-player chance expectation is 1,667 wins. A dash means that batch did not screen the gene. The *Total* win-rate columns pool the displayed observations and retain their real per-gene on/off seat counts in every row. *Diff* is that display total's on rate minus off rate, in percentage points. The report-only latest 10k batch updates these display statistics but does **not** change the deployment genome: *Default* remains the existing ledger call in `docs/gene_ledger.json` until an explicit rules decision records the batch as an authoritative source. Screenable genes awaiting every displayed measurement are listed separately below without a rank.

**Reading the table.** A six-player seat wins 1-in-6 by chance, so the expected count is 1,667 wins per 10,000 total seats. The batch cells are the enabled arm's excess over that chance rate, scaled from actual completed seats; they do not invent games or seats. The independent latest batch can have unequal on/off arms, which is why the pooled *Total (on)* and *Total (off)* cells retain their own `n` on every row.

**Batch provenance.** The newest displayed batch is the completed current-standard 6-major Continents screen (74×46, nine city-states, Online speed through turn 250, all six victory lanes, shuffled civilizations and best-genome baseline). Older displayed batches remain visible for trend context. The deployment ledger's sources and default state remain intact while this report-only result is reviewed.

**What each screen resolves.** The median gene’s column standard error times 2.8 — a two-sided 5% test at 80% power. Judge a column against the band of the screen named beside it, never against a single number for the instrument: these differ by more than three to one.

*Pairing gain* is how far a screen’s error per pair sits below the unpaired baseline, and it is what separates them. A foldover cancels only to the extent its two arms play a similar game, so the gain reads on the **genes**, not the design — a gene that rarely fires leaves most pairs identical and cancels almost everything, while a whole-genome screen flips every gene between arms and cancels almost nothing. ⚠ Gene count is not the driver, though the rows below invite that reading — the falsifier is in them. `h1` carries **one** gene over **14,400 player seats** and resolves ±68 at a 1.28× gain, *wider* than four-gene `s6` over 12,000 seats. Its gene changes nearly every game; `s7`'s rarely fires. That, not the count, is the difference.

| Screen | Shape | Genes | Player seats | 1 SE | ±80% power | Pairing gain |
|---|---|---:|---:|---:|---:|---:|
| `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` | standard | 1 | 7,200 | 39.1 | ±109 | 1.12× |
| `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` | standard | 99 | 47,244 | 15.6 | ±44 | 1.10× |
| `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` | legacy | 1 | 14,400 | 24.3 | ±68 | 1.28× |
| `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` | legacy | 75 | 35,148 | 18.3 | ±51 | 1.09× |
| `2026-08-21-p7-native-6p-allseats-15000-pairs.json` | legacy | 57 | 30,000 | 19.9 | ±56 | 1.08× |
| `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` | legacy | 1 | 12,000 | 10.3 | ±29 | 3.32× |
| `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` | legacy | 4 | 12,000 | 22.9 | ±64 | 1.49× |
| `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` | legacy | 1 | 2,000 | 36.1 | ±101 | 2.68× |
| `2026-08-20-p4-native-6p-allseats-13446-pairs.json` | legacy | 64 | 26,892 | 21.5 | ±60 | 1.06× |

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

_Generated by `tools/genes.py` from the ledger's sources: `2026-08-20-p4-native-6p-allseats-13446-pairs.json` (legacy, 26,892 seats), `2026-08-20-s2-step-and-reassess-native-4p-1000-pairs.json` (legacy, 2,000 seats), `2026-08-21-s6-religion-genes-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-s7-idle-faith-patronage-native-6p-allseats-6000-pairs.json` (legacy, 12,000 seats), `2026-08-21-p7-native-6p-allseats-15000-pairs.json` (legacy, 30,000 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (legacy, 35,148 seats), `2026-08-22-h1-holy-lane-parity-direct-6p-allseats-1200-pairs.json` (legacy, 14,400 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (standard, 47,244 seats), `2026-08-23-g1-governor-victory-lanes-direct-6p-allseats-3600-pairs.json` (standard, 7,200 seats). The fixed display batches are: `2026-08-23-standard-gene-screen-10000-total-seats.json` (10,002 seats), `2026-08-22-standard-10k-6p-allseats-23622-pairs.json` (47,244 seats), `2026-08-22-p10-native-6p-allseats-17574-pairs-ended-early.json` (35,148 seats). The deployment verdicts live in `docs/gene_ledger.json`; the table's batch cells are the operator's wins-per-ten-thousand-total-seat reporting view._
