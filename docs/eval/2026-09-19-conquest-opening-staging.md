# Connect the opening force to its staging objective

## Native evidence

In `civvis-20260920T043118Z` the early conquest opening named Russian Smolensk on turn 35, twelve tiles from the capital. The general plan simultaneously remained Expansion and named St. Petersburg. On turn 40 the opening expired without assembling.

The turn-35 host census already contained two Archers, a Slinger, and two Warriors, so adding production was not the missing step in this case. Barbarians claimed some of the force and must retain bounded home-defense priority. The opening's target and rally, however, are not used by the pre-war staging path: it reads only the general plan and refuses Expansion posture. The opening only pins the shared campaign after declaring war, while declaration itself requires assembly at the opening's rally.

A 40-turn frozen replay at `/tmp/civvis-late-contact-opening-replay/` reproduced the mismatch with both baseline `a65e42829` and the integrated production/campaign-memory candidate. Both emitted 40 decisions. This is diagnosis of orders, not evidence of counterfactual captures.

## Intended correction

Connect only the opening's reserved bodies to its own legal staging area before declaration. Preserve the existing military pipeline's combat, recovery, escort, and homeland-defense priority. A separate war or home Recovery must prevent opening assembly from claiming units. Unreserved units and disabled treatment retain ordinary staging behavior.

Validation is in progress; this draft does not yet establish the correction or a domination win.
