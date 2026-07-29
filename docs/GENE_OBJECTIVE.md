# Does the GA need combat score to evolve war?

## Pre-registration — 2026-07-29

`FitnessObservation::selection_value` ranks genomes by scaled Civilization
score share plus combat-achievement share. The combat term is real signal, but
on 120 finished games it changed the table leader in the wrong direction 13
times and the right direction twice (`docs/EVAL.md`). Removing it may align the
breeder more closely with winning. The unresolved objection is causal: it may
also be the only dense signal reached by military genes.

This experiment intervenes on that objection before changing the breeder. For
each of the 21 military genes, move the shipped champion to both ends of that
gene's declared bounds. Play the endpoint and unmodified champion on identical
map seeds, candidate seats, turn budgets, opponents, and speed through
`evolve::fitness_observations`. Compare the paired change in:

- the score component, `50 * players * score_share`;
- the combat component, `12 * players * combat_share`; and
- their sum, which is the production selection objective.

The primary run uses the deployment profile rather than the evaluator default:
six players, 74x46, Online speed, a 250-turn base budget, 24 common-seed games
per candidate, and seed 9,800,000. As in production evolution, every third game
gets twice the base turn budget. The embedded shipped champion is both the
intervention base and the non-anchor opponent.

### Fixed fires-check

For a gene, take whichever bound gives the larger mean absolute response. The
score-only channel remains a plausible breeding signal only if all three gates
pass:

1. score share changes on at least 25% of paired games for at least 16 of 21
   military genes (75% coverage);
2. the median across genes of `mean |score delta| / mean |full delta|` is at
   least 0.50; and
3. fewer than six genes (25%) are combat-only: the full objective changes on at
   least 25% of games while score changes on fewer than 10%.

These thresholds are fixed before the probe is compiled or run. Exact paired
changes establish reach, not strength. Passing the fires-check earns a second,
independent experiment comparing which candidates score-only and the production
objective select; it does **not** change `selection_value`. Failing retains the
combat term and directs work toward a win-aligned dense military statistic.

### Secondary diagnostics

The probe will also report signed paired means and standard errors, how often
the combat term reverses the score term's direction, and leave-one-map-out
selection outcomes. Those explain the primary result but cannot override its
gate. The selection diagnostic is exploratory because endpoint interventions
are not the mutation distribution and 24 held-out outcomes have low power.

## Result

Pending the pre-registered run.
