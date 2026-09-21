# Fund standing Domination upgrades

Native run `civvis-20260921T120510Z` had three legal Archer-to-Crossbow offers at 125 Gold each and two Trebuchet-to-Bombard offers at 85 Gold each on turn 125, with 156 Gold. It was at peace. Its strongest land unit was 55 and the strongest observed barbarian was 65, below the quality alarm's 73.75 threshold. The generic peacetime reserve of 120 blocked those purchases. Military Training was available by turn 100, but Mercenaries was not completed until 204 despite being boosted since 45. Feudalism, another prerequisite, completed on 99; Mercenaries was not directly researchable on 45. The empire lost to religion on 207.

The delay is not universal. `civvis-20260921T123956Z-cont4` completed Mercenaries on 141 and lost to culture on 211. The current `civvis-20260921T134621Z`, pinned to `294f13d76`, was researching Mercenaries by 131. These are reasons to respond to the actual standing army, rather than prescribe a fixed turn for every map.

## Decision changes

The explicit Domination lane connects civic research, policy choice, and upgrade funding:

- At least two existing ground combat units with unlocked, stronger direct successors and sufficient strategic material create a Mercenaries research goal after Political Philosophy. Earlier special commitments, including Byzantium's Divine Right, retain priority. Recon, naval and air units do not create this goal.
- Professional Army, or Force Modernization when available, may replace an ordinary military card. Maintenance, loyalty and Culture-defense cards remain protected. The priority remains through the final unit of the cohort. Available-policy queries exclude held cards, so retention checks the held deck too.
- A named major offensive funds authoritative legal offers before discretionary purchases, retaining 30 Gold plus one turn of any current deficit. City-saving and surprise-defense purchases go first; appointed war packages keep their existing budget. Healthy combat gain per Gold selects among affordable bodies.
- If the discount is newly available, the policy assessment runs before this funding pass. A native upgrade still uses the host's quoted price until a later state reports a different one; the policy swap does not manufacture a confirmed discount.

No upgrade rules, native runtime settings or victory conditions change.

## Validation

`cargo test --profile ci --locked` passed 4,052 tests, with 53 ignored. Nine focused tests cover the peacetime reserve failure, deficits and spent actions, health priority, other-lane and peaceful controls, the actual Military Training-to-Mercenaries research path, policy replacement followed by discounted Game upgrades, held-card retention and release, host quotes and refusals, locked successors and recon exclusion, and appointed war budgets.

`cargo fmt --all -- --check`, the local Rust quality check, and all 14 treatment append-point tests passed. The eight-game soak completed seeds 0–7 with four players and a 180-turn cap. This is stability validation, not evidence of a native Domination victory.

The full paired replay uses all 614 decision frames of `civvis-20260921T120510Z` against the preserved `294f13d76` baseline. Both binaries completed all frames. Actionable exports changed on 95 frames; 116 frames changed when historical verification receipts are included. Internal action lists differed on 133 frames. Exported upgrade orders rose from 14 to 43 (35 additions and six removals), including repeated hypothetical offers across frozen frames. Fifteen civic decisions redirected toward Military Tradition or Military Training. Movement, combat, production and policy decks also changed; this is not an upgrade-only replay diff. Gold/Faith purchase, technology-research and diplomacy exports did not change. On 125/0, the baseline does not upgrade; the candidate orders the healthy Archer `1966098` to upgrade for the authoritative 125-Gold quote, leaving 31 Gold in its planning board. Subsequent frozen frames offer other upgrades because the original history did not execute that hypothetical purchase. They must not be summed as a causal sequence of affordable purchases.

Replay receipt failures compare new orders against old host facts. They are not new native failures. The replay cannot complete an earlier Mercenaries research order, demonstrate the host's resulting discounted price, or establish victory. No native Domination victory is claimed.
