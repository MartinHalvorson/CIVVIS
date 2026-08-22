# Pre-registration — disjoint-seed confirmation of the r3 PASS

Written 2026-07-31 **immediately on reading the 300-map PASS and before any map
at the confirmation seed was run**. Agent `claude-evolver`.

## What passed

`ai_eval advanced_evolved advanced --matrix --pairs 300 --jobs 5 --seed 67000000`,
`r3` staged at `evolved/best.json`:

| profile | paired score | Elo-equiv | directions | sign p | anytime-valid | verdict |
|---|---|---:|---:|---:|---|---|
| compact standard | 55.6% (95% Wilson 49.9–61.1) | +39 | 96–39 | 0.0000 | e=7.8e3, crossed at map 159 | INCONCLUSIVE → **ACCEPT** |
| deployment online | **57.8%** (95% Wilson **52.1**–63.2) | **+54** | 120–44 | 0.0000 | e=1.9e6, crossed at map 27 | **PASS** |

`multi-profile promotion gate: PASS — advanced_evolved cleared every required
profile.`

## Why a confirmation is still owed

Two independent reasons, and neither is doubt about the verdict.

1. **The 300-map run extended the same prefix after an inconclusive 120-map
   read on those same maps.** The anytime-valid e-process is valid under
   optional continuation, which is exactly why the matrix computes it, and the
   seed stride exists so a prefix can be extended. But the *decision to extend*
   was taken after seeing a promising result, so the reported **+54 is a
   discovery estimate and is biased upward**. `docs/EVAL_INTEGRITY.md` §4 is
   about precisely this, and the repository has a recorded instance — a +207
   that re-measured to +86.
2. **Warm branches, the closest precedent, confirmed on a fresh seed** before
   merging, and that is the practice worth copying.

## The run, fixed now

```sh
ai_eval advanced_evolved advanced --matrix --pairs 300 --jobs 5 --seed 70000000
```

Seed 70,000,000 is disjoint from every seed in this line — 61/62/63/66/67/68
million — and its deployment child at 71,000,000 is likewise untouched.

## The rule, fixed now

- The **unmodified** matrix rule decides again: deployment must return
  `promotion gate: PASS`, compact must not establish a regression.
- **PASS on this disjoint seed → the effect size may be quoted as the pooled
  reading of two independent 300-map matrices**, and the artifact swap is
  proposed on that basis.
- **Anything else → the artifact is NOT swapped**, and what stands is a single
  gate-passing run on one prefix, recorded as such with its bias named. No
  third seed, no pooling of a failed confirmation with the passing run.
- Either way the *number* from seed 67,000,000 alone is never quoted as the
  expected gain.

## What ships if it holds

`data/evolved/best.json` replaced by the gen-14 champion with eleven genes —
`docs/GENOME.md`'s economy (7) plus expansion (4) — reverted to
`Weights::default()`, written with all forty explicit. The file is
`include_str!`'d, so one file updates the artifact and the embedded fallback.
⚠ It is resolved by 38 evaluator arms, the league seeding and that fallback, so
every strength number measured against `advanced_evolved` before the swap is
measured against a different agent after it.
