# Rival-lane rollout modeling

## Question

Strategic preserves the searching seat's live `AdvancedAi` state in every
branch, but still reconstructs each rival as a fresh adaptive planner. A
plausible fidelity repair is to infer a rival's victory lane from public state
and keep that lane fixed through the projection. This experiment tests that
mechanism without changing the searching policy, branch set, horizon, genome,
or evaluator.

The treatment is `StrategicAi::model_rival_lanes`, exposed only through the
evaluator names `strategic_rivals` and `strategic_deep_rivals`. It remains off
by default.

## Public inference

The inference mirrors the public components of Advanced's adaptive victory
focus:

- technology-tree and space-project progress for Science;
- foreign/domestic tourism progress for Culture;
- a founded religion and civilization conversions for Religion;
- diplomatic-victory points and suzerainties for Diplomacy.

It abstains before Strategic's first review, below 30 progress, or when the top
two lanes are separated by fewer than five points. An abstention keeps the
rival adaptive. No hidden AI plan, fogged map state, production queue, or
private intent is read.

## Fires-check

`rival_probe` warms paired positions with the production evolved genome, keeps
only reviews that reach the rollout evaluator, then reads the exact same
position twice. The baseline uses adaptive rivals and the treatment changes
only `model_rival_lanes`.

```text
target/ci/rival_probe \
  --players 4 --maps 24 --warmup 60 --turns 200 \
  --seed 94000 --jobs 12
```

Eleven of 24 positions reached rollouts. The model targeted 19 of 29 living
rivals (65.5%): ten Science and nine Culture. It changed 61 of 77 branch
values across nine of eleven positions, with mean absolute movement 0.02225
and maximum 0.60938. Three of eleven emitted lane decisions flipped. The
mechanism therefore acts substantially on the exact values Strategic consumes;
it is not an inert A/B.

## Mirrored evaluation

The first disjoint screen was encouraging but sparse:

```text
target/ci/ai_eval strategic_rivals strategic \
  --pairs 30 --players 4 --turns 200 --seed 95000 --jobs 12
```

`strategic_rivals` won 32 games to 28, +23 Elo-equivalent. Only two of 30 maps
had a non-neutral win direction, both favoring the treatment (exact sign
`p=0.5000`). Terminal score was 50.1%. The promotion gate was inconclusive.

A larger, separately seeded confirmation did not reproduce that direction:

```text
target/ci/ai_eval strategic_rivals strategic \
  --pairs 120 --players 4 --turns 200 --seed 96000 --jobs 12
```

The treatment won 115 games to 125: 47.9% paired-map score, 95% Wilson interval
39.2%–56.8%, and -14 Elo-equivalent. Five maps favored the treatment, ten
favored the control, and 105 were neutral (`p=0.3018`). Terminal score was
50.0%, with 52 treatment-favored directions, 43 control-favored, and 25
neutral. The promotion gate was again inconclusive and the predeclared
positive direction failed to replicate.

Across both disjoint blocks the treatment scored 147 wins to 153, with seven
favorable map directions and ten adverse. It is not promoted and
`strategic_deep_rivals` does not earn an evaluation.

## Diagnosis and decision

The treatment's reachable distribution explains the miss. Reviews with a
concrete religious or endgame threat are commonly intercepted by Strategic's
duel, irreversible-Prophet, or urgent-counter priors before economic rollouts
run. Among the ordinary roots that do reach the evaluator, the inferred rivals
in the artifact-backed probe were exclusively Science or Culture. Technology
progress is partly generic development rather than evidence of a durable
Science commitment, and a civilization prior is weaker still. Fixing those
guesses for a whole branch can make the synthetic opponent less faithful than
the adaptive reconstruction it replaces.

The useful result is therefore a boundary, not a new production agent:

- public rival lanes are inferable often enough to change search decisions;
- fixed-lane reconstruction does not improve the production controller;
- stronger public commitments mostly occur where existing priors already
  bypass the rollout evaluator;
- future opponent modeling should preserve actual rival planner state or
  target unresolved behavior, rather than hardening a weak lane argmax.

The implementation and probe remain as an off-by-default negative control so
the mechanism can be reproduced without being mistaken for a promoted feature.
