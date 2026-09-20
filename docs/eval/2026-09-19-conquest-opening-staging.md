# Connect the opening force to its staging objective

## Native evidence

In `civvis-20260920T043118Z` the early conquest opening named Russian Smolensk on turn 35, twelve tiles from the capital. The general plan simultaneously remained Expansion and named St. Petersburg. On turn 40 the opening expired without assembling.

The turn-35 host census already contained two Archers, a Slinger, and two Warriors, so adding production was not the missing step in this case. Barbarians claimed some of the force and must retain bounded home-defense priority. The opening's target and rally, however, are not used by the pre-war staging path: it reads only the general plan and refuses Expansion posture. The opening only pins the shared campaign after declaring war, while declaration itself requires assembly at the opening's rally.

A 40-turn frozen replay at `/tmp/civvis-late-contact-opening-replay/` reproduced the mismatch with both baseline `a65e42829` and the integrated production/campaign-memory candidate. Both emitted 40 decisions. This is diagnosis of orders, not evidence of counterfactual captures.

## Correction

Connect only the opening's reserved bodies to its own legal staging area before declaration. Preserve the existing military pipeline's combat, recovery, escort, and homeland-defense priority. A separate war or home Recovery must prevent opening assembly from claiming units. Unreserved units and disabled treatment retain ordinary staging behavior.

## Validation

The initial staging regression reproduced a missing order (`None` instead of movement) for a reserved warrior during Expansion. The fixed case moves toward its own opening, with the disabled treatment remaining unchanged. Three further tests cover the actual military dispatcher, eight exclusion cases (unreserved, threatened home, Recovery, existing war, wounded, civilian guard, deadline, already declared), and holding an assembled legal rally position. The hold test initially expected a true return; it was corrected to the mover's existing false-on-fortify contract and checks actual position and fortified state.

After integrating main `c5f89c6a`, `cargo test --profile ci --locked` passed 3629 library and 204 binary tests, with 49 library and four doctests ignored. No engine rule changed, so an engine-equivalence soak is not applicable.

Two frozen 40-turn native replays compared a source-identical `c5f89c6a` baseline to this change with the same 19 verification genes. All four CLI runs exited zero and emitted 40 decisions. In the earlier `civvis-20260920T033503Z` opening, actionable orders changed on 13 turns. At turn 11, Slinger 262146 at host (48,23) changes from FORTIFY to MOVE_TO (47,22), toward the opening objective. The late-contact `civvis-20260920T043118Z` replay emits identical orders in both arms; the change does not demonstrate an improvement on that fixture.

Artifacts are local at `/tmp/civvis-conquest-staging-replay/`. These replays retain actual host snapshots, so verification annotations about earlier alternative orders are not counterfactual host outcomes. They establish changed recommendations, not successful assembly, captures, or a domination win.
