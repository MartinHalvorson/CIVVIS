# Static doctrine challengers

The strategic search previously tried `Doctrine` as a second decision axis at
each macro review. That experiment rarely switched doctrine, which left two
different explanations: the policies might be equivalent, or the per-review
rollout might be too noisy to select among policies whose effects accumulate
over a whole game.

This experiment separates those explanations. Three evaluator-only agents
apply one bounded doctrine to the base genome for the entire game and retain
the strongest measured macro-search budget (`review_every = 20`, `horizon =
80`):

- `strategic_deep_expand`
- `strategic_deep_consolidate`
- `strategic_deep_militarize`

They are complete player agents, not rollout branches. Each has the same
runtime search cost as `strategic_deep`; only the static genome differs. The
perturbations are clamped through `Weights::bounds`, and provenance reports a
missing optional champion or value artifact instead of silently collapsing a
challenger into its control.

## Pre-registered screen

All three challengers played the same 30 mirrored maps against
`strategic_deep`:

```sh
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_expand strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 102000 --jobs 4

cargo run --profile ci --bin ai_eval -- \
  strategic_deep_consolidate strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 102000 --jobs 4

cargo run --profile ci --bin ai_eval -- \
  strategic_deep_militarize strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 102000 --jobs 4
```

| challenger | game wins | directional maps | paired score | terminal-score diagnostic | verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| Expand | 30-30 | 1-1 | 50.0% | 49.3% | inconclusive, no edge |
| Consolidate | 29-31 | 3-4 | 48.3% | 49.5% | inconclusive, no edge |
| Militarize | 27-33 | 1-4 | 45.0% | 51.8% | inconclusive, adverse point estimate |

Every sign test was inconclusive (`p = 1.0`, `1.0`, and `0.375`). No treatment
earned the pre-registered disjoint 120-map gate, so none is promoted.

## What the null says

Static policy choice does not rescue the doctrine axis on the current
`strategic_deep` agent. Expand was exactly neutral. Consolidate produced more
builders but less population, military, gold, and wins. Militarize produced
more cities, units, military strength, food, and production, yet won three
fewer games than its control.

That last result is a compact reproduction of the genome-fitness problem:
Militarize improved the dense development proxy while worsening the objective
that deploys an agent. It is therefore evidence against promoting score-rich
genomes without an independent win gate, not evidence that the gate needs to
be weakened.

The named controls remain available so future genome or engine changes can
re-run this comparison. They stay out of `BUILTIN_AIS`, persistent ratings,
and the production default.
