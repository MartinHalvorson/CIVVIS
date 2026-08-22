# Pre-registration — is the promoted search agent an improvement where it ships?

Written **before** the run, 2026-07-28. PR #519.

## Why this is the measurement that matters

`strategic` is the promoted macro-search agent and the strongest thing measured
in this repository. **It has never been evaluated at the deployment's profile.**
Everything known about it comes from 4p 24×16:

| | 4p 24×16 Standard | 4p 24×16 Online | 6p 74×46 Online |
|---|---|---|---|
| `strategic` v `advanced` | +117, gate PASS | +28, INCONCLUSIVE | **unmeasured** |
| `strategic_cheap` v `advanced` | — | +16 | **−63, p=0.0018** |

The cheap variant **reversed sign** between those last two columns. Three
estimates taken at 4p 24×16 have now failed to transfer — two magnitudes and one
sign. So the full agent's +28 cannot be assumed to survive either, and it is the
number the whole "more search is better" line rests on.

## Run

```
ai_eval strategic advanced --players 6 --width 74 --height 46 --city-states 6 \
  --pairs 60 --turns 250 --speed online --seed 8300000 --jobs 6
```

120 games at ~202 s each ≈ 67 minutes wall. Resolution ~±90 Elo — enough for an
effect the size of the cheap variant's −63, not enough for a small one.

## Prediction (mine, recorded before the run)

**I predict inconclusive, with the point estimate closer to zero than +28.**

My record this loop is poor — five mechanism stories refuted — so I hold it
loosely. The reasoning: every effect measured so far shrinks or reverses at the
deployment profile, and a bigger world with more seats is harder for a fixed
rollout budget to model, which is the same pressure that plausibly sank the
cheap variant.

**The most consequential outcome is a significant negative.** That would mean
the promoted search agent is not an improvement where it ships, and that the
+117/+45 results underpinning `strategic` and `strategic_deep` are properties of
the evaluator's map rather than of the game.

## Decision rule, fixed now

- No promotion or deployment claim either way from 120 maps; this sizes the
  effect, it does not gate it.
- A **significant negative** is reported as such and warrants the full 300-map
  overnight run before anything in `docs/SUPERHUMAN.md` is rewritten.
- A **positive** does not license deploying `strategic`: it is still ~200×
  `advanced` and 7× over the exhibition's time budget. It would instead make the
  cheap-configuration search worth re-attacking on a different knob.
- No seed re-rolls.
