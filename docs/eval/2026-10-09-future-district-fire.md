# Future defending-district fire after turn reset

A healthy enemy fort that fired on its preceding turn still has a shot when its next turn begins. PR #4025 reused the current-turn `defending_district_can_strike` check in `DangerField::contributions_at_hp`. `speculative_clone` preserves the spent-shot flags, so a spent Encampment or Oppidum disappeared from the movement, strike-reach, and second-turn danger fields. The city-center forecast already ignores its spent budget.

The repair uses the independent fort's positive inner and outer health and unpillaged state for future eligibility. The district lookup also rejects a pillaged tile. Current-turn legal actions and their shot budgets keep their existing checks. No turn is advanced or budget reset in the live world to calculate danger.

## Unchanged-production reproduction

The branch started from actual squash `d918cfe4a1b67412741dd6953d085b9c4bb1bbf5`. Fixture-only checkpoint `608f4e13669edd7dbf135f02b17b2cc097aee5e0` ran every selected test without retries or fail-fast: 104 executed, 102 passed, two failed. Both failures were new spent-fort forecasts. All three fields returned zero instead of the healthy control's 14.029992810297276 expected damage. Both immediate-shot denial and real EndTurn reset followed by a legal damaging shot succeeded before the forecast comparison failed. The eight other new controls and 94 existing battle-planner/fort cases passed; all 647 registered inputs remained unchanged.

Two additional cases at fixture-only `8d8a3172028be09bf64e2ce633d526c021bfe84d` produce ordinary spent states by executing an actual preceding shot with no governor extra-shot allowance. Their complete unchanged-production run executed all 106 selected cases: 102 passed and four forecast cases failed, including both actual preceding-shot witnesses. No compiler errors or changed registered inputs occurred. The original assertions remain in place.

The fixture contains four major seats, with Gaul defending and Gran Colombia moving. Each source and victim stand beyond the parent City Center's range and beyond the other fort's range. Controls include independent inner-health depletion, zero outer health, state pillage, tile pillage, range, and peace. Tests exercise `DangerField::new`, `with_reach`, and `second_turn` and check the source game's health and spent flags after forecasting.

## Candidate validation

The health-only forecast repair is implemented. Candidate validation is pending. The only fixture cleanup replaces a redundant clone of a Copy fort state; every assertion from both baselines is preserved.

## Scope

This is an engine forecast regression, not a measured native win-rate gain. No native candidate games, lane assignment, installed mod changes, or private-pin adoption have occurred. The exact merged #4025 revision passed all 4,822 trunk tests and is serving in the production spectator; that spectator deployment does not establish adoption by the separately pinned native seat.
