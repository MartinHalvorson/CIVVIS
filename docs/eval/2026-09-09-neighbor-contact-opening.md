# Early-contact version two: opening diagnosis

The first implementation at `edfa3bab8` retained v1's city-state Scout incentive
and added an incentive worth two unmet city-states until the first major contact.
It capped that extra incentive at two Scouts and required nearby known land with
unexplored neighbors. **The confirmation did not support this approach.** More
explored tiles did not translate into earlier or more major contacts.

This is a mechanism diagnostic, not a win-rate screen or a deployment source.
Each map is played twice, with every major using the deployment genome plus v1,
then with every major using the same genome plus v2. Other players use the usual
Advanced controller. Six majors, nine city-states, 74x46 Continents, Online,
Emperor, shuffled civilizations and all victory conditions match the standard
world. Observation stops after turn 30 of a 250-turn game. The full-game family
screen remains a different, required check; none of these results changes defaults.

The four-map pilot used seeds 91093001..91093004 (24 seats per arm). It showed
more contacts by t30, but no improvement in average first-contact time. A fixed
24-map disjoint confirmation then used 91094001..91094024 (144 seats per arm):

| Metric | V1 | Initial V2 | V2 minus V1, paired-map bootstrap 95% interval |
|---|---:|---:|---:|
| First contact, capped at 31 for seats still unmet at t30 | 19.271 | 19.549 | +0.278 [-0.125, +0.799] turns |
| Major contacts by t30 | 1.333 | 1.264 | -0.069 [-0.208, +0.069] |
| Explored tiles by t30 | 242.208 | 247.118 | +4.910 [+0.563, +10.014] |
| Maximum simultaneously alive Scouts by t30 | 1.160 | 1.243 | +0.083 [0.000, +0.181] |

125/144 v1 seats had met a major by t30, versus 119/144 v2 seats. Seventeen seats
made contact earlier, 106 were unchanged, and 21 later (using the stated cap).
The intervals resample the 24 paired maps, not the correlated seats: 10,000
bootstrap samples with Python Random seed 91094, percentile indices 250 and 9749.
The first-contact result is a censored opening metric, not a claim about when
still-unmet seats would eventually discover someone. All-major treatment can
change both sides of a contact; this measures the world's exploration behavior,
not an individual seat's competitive advantage.

The original source, raw pilot and confirmation CSVs, and summary JSON are kept
beside this note. The harness links the `civvis` rlib from the clean developer-tools
build at `edfa3bab8` using `rustc --edition=2021 -O --extern civvis=<rlib>
-L dependency=<target/ci/deps>`. Its two arguments are starting seed and map count;
the confirmation was `91094001 24`. Fog-memory and war-ledger narration are disabled
in both arms; decisions and contact recording are unchanged.

The unmerged challenger is being revised to spread existing Scouts across known
land frontiers, preserving v1's production incentive. Its results must be measured
on new seeds. These files describe the rejected extra-Scout implementation only.
The original 12-game extra-Scout family screen was deliberately terminated before
any game completed when the mechanism was revised; it supplies no outcome evidence.
