# Future defending-district fire after turn reset

A healthy enemy fort that fired on its preceding turn still has a shot when its next turn begins. PR #4025 reused the current-turn `defending_district_can_strike` check in `DangerField::contributions_at_hp`. `speculative_clone` preserves the spent-shot flags, so a spent Encampment or Oppidum disappeared from the movement, strike-reach, and second-turn danger fields. The city-center forecast already ignores its spent budget.

The repair uses the independent fort's positive inner and outer health and unpillaged state for future eligibility. The district lookup also rejects a pillaged tile. Current-turn legal actions and their shot budgets keep their existing checks. No turn is advanced or budget reset in the live world to calculate danger.

## Unchanged-production reproduction

The branch started from actual squash `d918cfe4a1b67412741dd6953d085b9c4bb1bbf5`. Fixture-only checkpoint `608f4e13669edd7dbf135f02b17b2cc097aee5e0` ran every selected test without retries or fail-fast: 104 executed, 102 passed, two failed. Both failures were new spent-fort forecasts. All three fields returned zero instead of the healthy control's 14.029992810297276 expected damage. Both immediate-shot denial and real EndTurn reset followed by a legal damaging shot succeeded before the forecast comparison failed. The eight other new controls and 94 existing battle-planner/fort cases passed; all 647 registered inputs remained unchanged.

Two additional cases at fixture-only `8d8a3172028be09bf64e2ce633d526c021bfe84d` produce ordinary spent states by executing an actual preceding shot with no governor extra-shot allowance. Their complete unchanged-production run executed all 106 selected cases: 102 passed and four forecast cases failed, including both actual preceding-shot witnesses. No compiler errors or changed registered inputs occurred. The original assertions remain in place.

The fixture contains four major seats, with Gaul defending and Gran Colombia moving. Each source and victim stand beyond the parent City Center's range and beyond the other fort's range. Controls include independent inner-health depletion, zero outer health, state pillage, tile pillage, range, and peace. Tests exercise `DangerField::new`, `with_reach`, and `second_turn` and check the source game's health and spent flags after forecasting.

## Candidate validation

Validated implementation checkpoint: `1f97b833679b3493406ccd104f0467cbbff05a85`. All 647 registered Rust, data, build, Cargo, and control-Lua inputs stayed unchanged throughout validation. No retries, fail-fast, or QoS override were used locally. The only fixture cleanup replaces a redundant clone of a Copy fort state; every assertion from both baselines is preserved.

- Focused selection: **106 passed**, including all twelve new cases.
- Complete unfiltered Nextest: **4,834 passed, 51 existing skipped** (257.868 seconds). Documentation: zero failed, four existing ignored examples.
- Independent CI run [37979657730](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37979657730): **4,834 passed, 51 skipped**, four ignored documentation examples, and the isolated 71/21/22 regression selections passed. All eight PR checks passed.
- Exact-source CLI build and twelve-game integration soak: **12/12 completed without crashes**, all turn-cap draws. Four major seats were Gran Colombia, Gaul, Rome, and Scythia, with six city-states, Flat/Pangaea, Online speed, Emperor difficulty, and domination as the enabled victory. Six Ancient starts used seeds 402800–402805, and six Industrial starts used 402900–402905; each stopped at turn 60. Binary SHA-256: `d68c0c0d3905665a2611f687e2776d5b06a50ee203252f9726e8b87333481fb9`. These short engine games establish integration stability, with no win-rate credit.
- [Paired cost run 37979657744](https://github.com/MartinHalvorson/CIVVIS/actions/runs/37979657744): five pairs, 600 completed turns per arm; median −0.21%, pooled +0.04%, IQR 1.10 percentage points, resolution ±0.73%. The median is inside the ±1% noise floor. The gate passes; no speed improvement is established.

Logs and registrations are retained under `civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/future-district-fire-*`. Full local Nextest log SHA-256: `c61e02e71371e63fbac141ce0d22b1e7e7b8fee390164c7e0e9ecb3b63e62672`. Both unchanged baselines and their failures remain retained.

## Integration audit

The one pre-ready fetch/merge of main reported Already up to date on `d918cfe4a`; no additional main merge was needed. Shared-file neighbors #3791 and #3887 edit separate source ranges and test seams. Older withheld draft #3784 (`236d9f6665ec27aeb2ef8a90cfb9bf6db9ca79a8`) changes enemy-unit route handling. Against actual common base `3d8d41e43c982b92bd703c1d503c6e86e1a081e5`, the full branch and peer have no shared removed line or insertion seam in the shared file. Read-only `git merge-tree --write-tree` succeeds in both orders with identical tree `95a6e3c6cf835f155c277ccaf9fe18241aa2f3f7`, retaining the future fort health check and peer route-cache/restoration code. This is a context-only integration audit. No peer approval or deployment dependency is claimed, and no peer code is adopted into this branch.

## Scope

This is an engine forecast regression, not a measured native win-rate gain. No native candidate games, lane assignment, installed mod changes, or private-pin adoption have occurred. The exact merged #4025 revision passed all 4,822 trunk tests and is serving in the production spectator; that spectator deployment does not establish adoption by the separately pinned native seat.
