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

For a gene, take whichever bound gives the larger mean absolute response in the
**current full objective**. This gives the incumbent objective first choice of
the intervention and prevents the score-only arm from cherry-picking the more
favorable bound. The score-only channel remains a plausible breeding signal
only if all three gates pass:

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

### Integrity rejection before inference

The first execution completed all 1,008 games and printed a nominal three-gate
PASS. It is **invalid and contributes no evidence**. Every endpoint changed
score on all 24 maps, and several unrelated genes produced byte-identical
rows. The suspicious result led to a round-trip test, which failed:

```text
load_champion("evolved").policy_deck               Live
Weights::from_vec(champion.to_vec()).policy_deck   Legacy
```

The committed generation-14 artifact predates the non-gene `policy_deck`
field. `Weights` explicitly defaults to the measured production deck,
`Legacy`, but the enum independently derived `Default` as `Live`. Serde uses
the enum default for the missing field, while gene-vector reconstruction uses
`Weights::default`. Therefore all 41 purported single-gene interventions also
switched the candidate from Live to Legacy. Their mean score deltas of roughly
10--16 objective points measured that shared deck change, not military-gene
reach.

This is a production evolution defect as well as an instrument defect:
`evolve::mutate` and `crossover` reconstruct gene vectors, so the first child
of an old loaded champion silently changes its non-gene policy. The enum
default now matches `Weights::default`, with tests pinning both legacy Serde
restoration and champion gene-vector round trips.

No threshold or seed was inspected or changed. The exact pre-registered command
must be rerun after the integrity test passes; only that run can populate the
result.

### Valid primary run

The corrected run used the exact pre-registered command, thresholds and seed.
All candidates now preserve the production Legacy deck, and the result changed
qualitatively:

| fires-check | required | observed | verdict |
|---|---:|---:|---|
| score-responsive military genes | at least 16/21 | **17/21** | PASS |
| median `mean |score delta| / mean |full delta|` | at least 0.50 | **0.831** | PASS |
| combat-only genes | fewer than 6 | **0/21** | PASS |

The four genes score could not reach were `war_ratio`, `war_margin`,
`peace_ratio`, and `war_min_turn`. Both bounds of all four produced zero score
change **and zero full-objective change on all 24 maps**. The combat term does
not rescue them; at this champion and deployment profile, the war-declaration
block is unreachable by either breeding objective.

The genes that did fire did not merely perturb combat. Score changed in 15--24
of 24 maps for the incumbent-favorable endpoint, with a median 83.1% of the
full objective's mean absolute response. The largest effects were army size,
attack threshold, movement support/threat, withdrawal, and rejoining. Removing
combat would reduce dense signal but would not strand any observed military
gene.

The predeclared exploratory leave-one-map-out comparison selected the same
endpoint for 17/24 held-out maps. Where the objectives differed, score-only
selection won 8 games against 7 for the full objective (two discordances for,
one against) and improved held-out score share by **+0.01278 +/- 0.00507**.
This is directionally consistent with the earlier independent finding that the
combat term reduced agreement with the win gate, but it was explicitly not a
gate and does not change production.

## Confirmation pre-registration — 2026-07-29

Replicate the complete endpoint grid and leave-one-map-out selection procedure
on 24 disjoint deployment-profile maps at seed 9,810,000. All candidates,
thresholds, game options, and the fixed Legacy-deck integrity tests remain
unchanged.

The hypothesis is that removing the combat term improves the generalization of
selection because combat achievement is a distinct, partly anti-aligned target,
while score retains most of its useful causal response. Score-only earns a
production change only if all three confirmation gates pass:

1. the two objectives select different candidates on at least four held-out
   maps, so the A/B actually fires;
2. the mean held-out score-share difference, score-only minus full, is positive
   by at least two paired standard errors; and
3. score-only wins at least as many held-out games as the full objective.

If the confirmation passes, production selection becomes
`62 * players * score_share`: the factor 62 preserves the current objective's
parity value and therefore the separately calibrated 65-point screen, while
not affecting genome rankings. Champion promotion remains outcome-only and is
unchanged. Any failure retains the current combat term.

### Confirmation result — combat retained

The disjoint seed-9,810,000 grid reproduced the causal mechanism almost
exactly: 17/21 score-responsive genes, median response retention **0.837**, no
combat-only genes, and the same four entirely unreachable war-declaration
genes. The objective-selection result did not reproduce at the required power:

| confirmation gate | required | observed | verdict |
|---|---:|---:|---|
| objectives choose differently | at least 4/24 maps | **7/24** | PASS |
| held-out score-share gain | positive by at least 2 SE | **+0.00315 +/- 0.00351** | FAIL |
| held-out wins | score-only at least full | **4 vs 4** | PASS |

The direction remained positive but was smaller than its uncertainty. Per the
pre-registration, `FitnessObservation::selection_value` is unchanged and the
combat term remains. The supported conclusion is about representation, not a
new objective: score alone reaches the active military genome, while the four
war-declaration parameters are dead under both dense signals on two disjoint
deployment samples.

## Champion policy-default repair

The integrity failure changes runtime behavior, not only the probe. The
generation-14 artifact omits `policy_deck`, so before this repair every
`advanced_evolved` or Strategic agent loaded the off-by-default Live deck.
`Weights::default`, every vector-reconstructed child, and the documented
production controller use Legacy. The shipped champion and its first child
therefore differed on a non-gene dimension before mutation touched anything.

The fix makes the Serde/enum default agree with `Weights::default` at Legacy.
This is the deck already measured against Live over 120 small-profile mirrored
maps: 18 directions for Live, 15 for Legacy, sign p=0.7283, terminal score flat.
The repair removes a measured-null evaluator and restores mutation integrity,
but that old result is not a deployment-profile result.

### Deployment A/B pre-registration — 2026-07-29

Run the existing symmetric policy evaluator with the embedded champion as both
arms' base, changing only Legacy (treatment, repaired behavior) versus Live
(control, pre-repair behavior): 120 maps, two directions, six players, 74x46,
six city-states, Online speed, 250 turns, seed 9,820,000, 12 jobs.

Legacy is retained unless the paired map-direction sign test is significant
against it at p < 0.05. That asymmetric rule is deliberate and fixed before
the run: Legacy is the documented default, preserves vector mutation exactly,
and removes policy-evaluation cost; Live needs evidence of strength to justify
silently re-entering through an old artifact. Wins decide, with terminal score
reported only as diagnosis.
