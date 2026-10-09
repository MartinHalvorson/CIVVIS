# Native military Faith purchase budget

Native run `civvis-20261009T001708Z` exported 613 Faith at turn 132 and legal military Faith prices from 40 to 180 in Maracaibo. The game later verified a Pikeman purchase. Fresh-controller replay of that observation, after the earlier Faith sinks, retained 433 Faith but issued no military purchase with either the old or corrected Shrine rule. The separate entry threshold in `military_faith_spending` required 600 before it considered any army purchase.

The candidate permits an observed native standard-unit Faith quote below 600 when the action is legal, affordable above every protected reserve, and passes the existing gold upkeep guard. The existing action application and remaining-bank check enforce the price and reserve. Missing quotes, model-only games, and formations without an observed quote retain the original threshold.

The tests exercise an actual unit purchase at 433 Faith, reserve and upkeep refusal, the model-only threshold, and the exclusion of unquoted formations. Focused validation is pending. This is a decision-budget correction, not a change to native game settings. Native win-rate improvement is unproven.

The previous lake-only candidate finished 0/3 (two science defeats and one production-rank retirement). The first combined lake + Campus + Shrine-reserve candidate retired at turn 150, rank 3, with production 252 against rivals at 216, 381, and 318. Keep all retirements as non-wins. The native purchase establishes that the army path actuated; fresh-controller replay does not reproduce all original cross-turn memory or prove the Shrine correction caused that particular purchase.
