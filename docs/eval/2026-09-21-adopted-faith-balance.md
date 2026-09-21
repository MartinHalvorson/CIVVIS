# Count useful counter-faith defenders

In native `civvis-20260921T152749Z`, purchased Orthodox Missionaries continued spreading after Orthodoxy became our majority. Byzantium later won Religion on turn 167. The separate change #3670 now holds those units while their faith is unsafe to spread, preserving their charges for a future reversal.

The existing defensive roster limit counted every Missionary. Two held units of the threatening faith could therefore prevent buying a Missionary of the safe counter-faith. This follow-up makes the count use the same faith eligibility as the purchase path: charged Missionaries whose faith differs from the current threat and passes `safe_adopted_counterfaith`. It applies only to non-founders explicitly targeting Domination with religious victory enabled. The existing two-unit limit and its threat-scaled extras are unchanged; other lanes retain their previous count.

A regression with two threatening-faith Missionaries and an affordable, legal counter-faith supplier fails on baseline: the roster stays at two instead of buying the defender. The fixed regression verifies that both original units hold their three charges under #3670, then a counter-faith unit is purchased. Further tests cover a mixed corps buying only its missing defender, two useful units still filling the limit, and unchanged other-lane/disabled-victory behavior.

Final integration uses main `2450248dd`, which contains both #3670 spread restraint and #3669 governor foundation. Validation passed: 4,069 Rust tests, 53 ignored; four focused balance regressions; 14 treatment append-point checks; eight four-player, 180-turn soak games; formatting and incremental Rust quality.

A paired full-history replay of all 469 completed decision frames through 167/2 changes zero exported orders and zero internal actions against that integrated baseline. This is a regression check on the recorded trajectory, not evidence of a changed native decision. The functional held-unit/purchase regression establishes the effect in the new state created by spread restraint. Artifacts: `/tmp/civvis-adopted-faith-count-replay/`, `/tmp/civvis-3672-comparison.json`, and `/tmp/civvis-3672-baseline-tests.log`. Both replay arms use the native summary's same 19 forced treatments.

 The original recorded future expended the Missionaries, so a fixed-state replay cannot establish the future effect of keeping them alive. No native victory or prevented loss is claimed.
