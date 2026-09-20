# Equip an active siege through economic replanning

Native run `civvis-20260920T080404Z`, pinned to `83886cee5`, knew Engineering
and offered a Catapult in Bogotá's host build menu on turn 64 (60 production,
five turns). Its siege of Qusqu still faced walls. The first immediate native
Catapult production order appeared on turn 78. Later three siege weapons
existed, but no siege-weapon city strike had been observed through turn 93.
This separates delayed equipment from the additional problem of reaching a
firing position; solving production alone does not prove a capture.

## Investigation and controlled test

`missing_domination_siege` previously applied only to the Conquest strategy.
Expansion and Recovery could retain the same at-war walled objective without
the composition exception. Full-governor tests reproduce that failure with an
otherwise full army. Removing the strategy restriction fixes those tests.
However, a 303-frame frozen native replay through turn 105/frame 0 yields ZERO
changed actionable orders for that edit alone. The army ceiling was not binding
on the recorded production decisions. Diagnostic reads at turns 64–65 confirm
the planner knows the domination target, war, walls and absence of siege units.
Ordinary infrastructure still outranks the first weapon.

The follow-up experiment values the first wall-breaking weapon on the same
400 raw-point reservation scale used for an opening conquest body, in addition to
the ordinary 95-point siege-role credit. Production time still discounts that
value. Existing and queued siege weapons close the exception; peace, absent
walls, absent targets and disabled domination retain the ordinary rules.
This is a priority for equipping one active siege, not a new permanent army
quota. The existing emergency-defense handling remains upstream.

## Reproduction

- Frozen source: `/tmp/civvis-native-siege-expansion/events.jsonl`.
- Driver: `/tmp/civvis-expansion-frame-replay.py`.
- Artifacts: `/tmp/civvis-siege-expansion-replay`.
- Baseline code: `f1c0da515`, including the previous safe-approach fix.
- Each binary processes a growing event prefix in one persistent serve session,
  using the verification batch's domination target and 19 forced genes.
- Compare actionable orders after excluding `order_failed`, `order_verified`
  and `turn_verified`; also compare `decision.native_actions`.
- `baseline` versus `patched` artifacts preserve the failed gate-only experiment.

The running native game is pinned to its own executable. Replayed orders do not
establish host acceptance, completed weapons, city captures, or victory rate.
No engine or game-rule changes are involved; an engine crash soak is inapplicable.

## Revised replay result

The reservation candidate (`priority` artifacts) and baseline both finish all
303 frames with exit 0. The first immediate `produce UNIT_CATAPULT` order moves
from turn 78/frame 0 (Cartagena de Indias) to turn 64/frame 0 (Guayaquil), then
Bogotá receives one on turn 65. This is 14 turns earlier at the order level.
Future `produce_next` hints are not counted as immediate starts.

Thirty-five frames change exported actionable orders after telemetry exclusions;
51 change the internal `decision.native_actions` stream. Changes include later
production and builder routing, not solely extra catapult orders. Repeated
orders on later recorded states do not mean multiple weapons were completed.
The candidate can defer economic construction; this is the intended cost of
supplying the active siege's first wall breaker, bounded by the existing/queued
weapon census. Actual city captures and campaign outcomes remain unverified.

Seven focused tests cover complete governor selection in Expansion and Recovery,
the new priority over routine growth, retention across replanning, and existing
army-ceiling controls. The original governor regressions fail before removing
the strategy gate; the infrastructure regression still fails with just that
gate removed (catapult score 15.32 versus granary 33.32).

Final `cargo test --profile ci --locked` passes: 3,662 library tests and 204
binary/integration tests; 49 library tests and four doc tests remain ignored.
`git diff --check` passes. Main was fetched and merged before validation
(already up to date). Logs are `/tmp/civvis-3609-{red,priority-red,priority-green,final-full}.log`.
