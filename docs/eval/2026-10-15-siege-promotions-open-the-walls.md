# Siege promotions for a known walled campaign

The default-off `siege-promotions-open-the-walls` experiment promotes siege guns toward Shells when the current Conquest plan targets a living major enemy's known, standing walls. It prefers an offered Expert Crew, then Shells, then Crew Weapons. The existing promotion healing gate and ordinary scorer remain in place for other cases. The command phase passes the current strategic target, including wars that have no prewar campaign object. Fogged targets use matching remembered wall information.

This is a promotion experiment, with no default, Native pin, forcing set, verification policy, engine rule, or promotion data change. Its full registry index is 661 and screen index is 626; all earlier indices are preserved. `siege_wall_promotions` counts accepted promotions that differ from the legacy choice, separately from assignment to the ON arm.

## Why test this

In the completed Native run `civvis-20261015T120650Z` (source `1d770be4c8e0d2b8c660a6ff648e8740c05b47da`), Bombard 7667725 received Grape Shot at 178 and Shrapnel at 185; Bombard 8912896 received Grape Shot at 180 and Shrapnel at 188. Subsequent Native state exports verified all four promotions. That world ended with Canada's Science victory at 209.

The installed Gathering Storm database and `Base/Assets/Gameplay/Data/UnitPromotions.xml:351–352,610–629` give Crew Weapons +7 defense, Shells +10 against districts with Crew Weapons as prerequisite, Grape Shot +7 against land units, and Shrapnel +10 against land units. The district branch has an opportunity cost: 28 historical `RANGE_ATTACK` commands from these two guns targeted no city tile known in the corresponding opening-state export. These are command classifications, not a claim that all shots executed or that unseen tiles were empty. The policy therefore remains an opt-in experiment.

## Fixed headless cohort

All 60 registered worlds and 360 seat rows completed. ON: 92 seats, 11 wins; OFF: 268 seats, 49 wins. The analyzer estimates a win difference of -6.3271 percentage points (SE 4.1053) and share difference -0.3160 points (SE 0.7898). Actual changed promotion choices: 0 across 0 ON seats; 0 OFF. World endings: culture: 3, science: 57. Treatment assignment alone is not evidence of a changed promotion. This screen does not establish a Native Domination or strength improvement; the policy remains default off. **No seat recorded a changed promotion, so this cohort did not exercise the behavior being tested. The group estimate is assignment imbalance and cannot be attributed to the promotion branch.** The fixed cohort is closed; it was not extended or filtered to obtain exposure.

Registered before games: seeds 40700001–40700060, six seats per world, one independently drawn policy bit with the standard 0.25 ON probability, all other 626 screen bits held fixed. The command uses headless Deity for both seats and barbarians, 74×46 Continents, nine city states, Online speed, a 250-turn cap, balanced assigned victory lanes, and all six victory conditions. These are 60 worlds with 360 seat rows, not 360 independent games. They are all-AI headless games with the model's handicap and do not establish Native strength against Firaxis opponents.

The original registration mistakenly called the topology “Planet/default.” The actual command never overrode `GameOptions`' Flat default. The header and source were checked and the correction recorded before outcome review. All 60 original worlds were retained; no seed, arm, command, or cohort length was changed. The original registration and correction are both included.

A descriptive audit after all games ended found that this standard headless baseline held 141 of the other 626 screen bits on; it is a different configuration from the 283-flag Native proposal profile. In particular, `siege-train` and `siege-target-needs-a-road` were off. End turns ranged from 116 to 176 (median 143.5), and all campaign-plan, campaign-war, campaign-capture, and changed-promotion counters were zero. These facts describe the closed cohort and do not supply a policy comparison.

The compiled source was `c0fe12f90d7b002f3990c57b8e3e581689a424d2`; source bytes and HEAD stayed frozen through tests, proposals, all worlds, and analysis. Normal background activity included a separately recorded fixed Native replay profiling run. No performance cost estimate is claimed from the headless screen.

## Native proposal and model checks

The same C0 CI decider, with 283 common current Native flags, proposed Grape Shot OFF and Crew Weapons ON on the exact captured opening inputs at 178 and 180. The unrelated Charge promotion at 178 was unchanged. Both arms' actual captured reads matched the original byte prefixes. The separately verified FF ARM64 baseline also proposed Grape Shot on both inputs. These are outbound proposal checks; the alternative orders were never executed in Native.

Nine focused controls passed, covering healing, peace, unknown and stale walls, remembered wall information, other unit classes, legal offered promotions, current wartime context, and a dead or non-major target. An executed model city shot compared the two earned promotion paths on the same valid 400-wall city and RNG state: 21 wall damage through Grape Shot/Shrapnel and 31 through Crew Weapons/Shells. Both shots were below the 100-damage cap. This fixture demonstrates the model's district modifier; it is not a general Native damage or victory gain.

## Validation and artifacts

Local serial `cargo test --profile ci --locked --no-fail-fast` passed 6,115 tests across targets, with the existing 51 library ignores and four documentation ignores. Developer-tools screen tests, all-target checks, changed Rust quality, and the exact-input Native proposal checks passed. The new controls first produced a valid old-scorer failure; invalid compiler/fixture attempts were retained separately and were never treated as behavioral evidence.

- [2026-10-15-siege-promotions-open-the-walls.json](../gene_screens/fires/2026-10-15-siege-promotions-open-the-walls.json)
- [2026-10-15-siege-promotions-open-the-walls.rows.jsonl.gz](../gene_screens/fires/2026-10-15-siege-promotions-open-the-walls.rows.jsonl.gz)
- [2026-10-15-siege-promotions-open-the-walls.native-input.events.jsonl.gz](../gene_screens/fires/2026-10-15-siege-promotions-open-the-walls.native-input.events.jsonl.gz)
- [2026-10-15-siege-promotions-open-the-walls.native-proof.json.gz](../gene_screens/fires/2026-10-15-siege-promotions-open-the-walls.native-proof.json.gz)
- [2026-10-15-siege-promotions-open-the-walls.provenance.json.gz](../gene_screens/fires/2026-10-15-siege-promotions-open-the-walls.provenance.json.gz)

Integration validation and final GitHub checks are recorded in the pull request.
