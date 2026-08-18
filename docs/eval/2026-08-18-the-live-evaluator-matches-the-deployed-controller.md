# The live evaluator matches the deployed controller

_2026-08-18 · `a9e54f9b`_

## What was asked

Does the evaluator's `live` arm construct the same `AdvancedAi` controller as
the Civilization VI order binary deploys? In particular, can every setting in
that controller be identified in its public treatment stamp and withheld by a
matching evaluator arm?

## How it was measured

This was a deterministic construction audit at `a9e54f9b`, not a gameplay
screen. It compared the `AdvancedAi` configuration in `civvis_orders` with the
`live` branch in `elo`, then checked the bundle, public provenance tags, and
`live_without_*` table as one ordered contract. A startup smoke run against an
empty mirror read the binary's emitted controller identity.

No games, seeds, or outcome comparison were run: a strength screen would have
answered a different question before the two sides agreed on which controller
it was screening.

## What it measured

The deployed binary applied five settings after `enable_live_bridge`:
`parallel_settlers`, `host_settler_pop`, `explore_dead_targets`,
`explore_commit`, and `bank_envoys`. The evaluator invoked only the helper, so
its prior `live` stamp contained 69 treatments while the deployed controller
contained 74; the five absent settings also had no withholding controls.

After the repair, the shared bundle, startup identity, and evaluator all expose
the same 74 ordered treatments, with one `live_without_*` arm for each. The
startup smoke emitted all five moved settings. There is no win-rate, Elo, or
confidence interval here because the measurement is structural rather than a
sampled game outcome.

## What was decided

Ship the construction repair and treat `live` results from this revision onward
as a new, 74-treatment controller definition. Keep the five settings enabled
in deployment exactly as before; only their shared construction, provenance,
and evaluator controls change.

Do not pool prior 69-treatment `live` screens or ablations with future results
that include these controls. Any promotion decision involving `live` must be
re-run on disjoint seeds under the new stamp; this audit makes no performance
claim about the five mechanisms.
