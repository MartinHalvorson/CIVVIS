# Barbarian research prices the field

Unpriced opt-in `barbarian-research-prices-the-field` changes the research alarm, not the production or upgrade-spending alarms. The default remains off.

The old alarm compares the best land unit's full-health tier against nearby barbarians, including siege bombard strength. The alternative applies the existing engine wound penalty and prices bombard strength against land units. With battlefront observation, Science research requires an actual visible barbarian City-Center attack opportunity and the field-quality gap, rather than a second requirement that the attacker exceed the city's strength by 25%. Other research priorities, active research, resource requirements and upgrade-goal selection retain their rules. No goal is latched after a threat leaves.

The shipped normal-game rules are `Base/Assets/Gameplay/Data/GlobalParameters.xml:134`, `COMBAT_BOMBARD_VS_UNIT_STRENGTH_MODIFIER=17`, and line220, `COMBAT_WOUNDED_DAMAGE_MULTIPLIER=10`. The existing engine implements these in `Game::ranged_strike_strengths` and `effective_strength`. A test compares the Catapult's field yardstick with the engine's shot against a land defender. This is a tier alarm, not a terrain/promotion/formation matchup forecast.

## Recorded mechanism

Native Indonesia/Gitarja Deity Online Science game `civvis-20261106T061004Z` lost to rival Science on192, with city losses64/87/91. Its Horseman remained at11HP on70,75,79 and84 while Men-at-Arms and Crossbowmen threatened home. Research had followed Machinery prerequisites through Engineering, but began Shipbuilding on70; Machinery began84. These observations motivate the hypothesis. They do not prove which alternative would save a city or win.

The committed archive uses exactly the last captured native read for70/0 as a constant input:9,875,775bytes, SHA256 `28281986514bb8c67843e8a4ea9fc3210b89483d6050dd9fbcab9e2aa63bb49c`. It is not the original ordered multi-read sequence or persistent native AI history. The new worktree binary uses the default deployment controller with explicit Indonesia/Science and no common force flags, not the native game's419-force profile.

Source `b0866224939a8c961ce53afac238ed3a01db6c01` produced two fresh queries per arm. All four processes exited cleanly; every query's17 actual reads matched the same frozen bytes. Cold/played full raw genomes, sole candidate-bit argv/genome differences and repeated payloads are independently checked. Off emits `TECH_SHIPBUILDING`; on emits `TECH_MACHINERY`.

Run the archive verifier:

```sh
python3 docs/eval/2026-11-06-barbarian-research-prices-the-field.verify.py \
  docs/eval/2026-11-06-barbarian-research-prices-the-field.action-pairs.tar.xz
```

The matching action-pairs artifact is also checked by `tools/gene_fires.py --max 0`. It establishes emitted proposal activation only. Native application, prevented losses, saved turns and victory strength remain unmeasured. No native game, harness, lock waiter, force-file, configuration or UI change was used.

## Validation and tradeoffs

Eight focused tests cover default-off registration, the wounded Horseman/Catapult case, healthy defenders, a wounded raider, the engine bombard penalty, camp guards/distance, the complete research chooser and a visible raider outside City-Center attack reach. The73 barbarian-related library tests pass locally with `CARGO_PROFILE_CI_OPT_LEVEL=0`; this is explicitly an unoptimized local profile, not the optimized CI suite.

The opt-in can divert research before walls are overwhelmed. That may improve field parity or delay science unnecessarily if the existing army would recover and clear the threat. The quality ratio, home radius and legal upgrade paths are unchanged; terrain, promotions, tactical exchanges and recovery time are not forecast by this alarm. Native outcome pricing is required before considering any default promotion.
