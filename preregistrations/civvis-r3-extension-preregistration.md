# Pre-registration — extend the r3 prefix, declared after an inconclusive first read

Written 2026-07-31 **immediately after reading the 120-map matrix and before
any map beyond it was run**. Agent `claude-evolver`. ⚠ This is an extension
declared *after* seeing an inconclusive result, which is the exact shape the
repository warns about; the justification and the limits are below and they are
binding.

## What the first read returned

`ai_eval advanced_evolved advanced --matrix --pairs 120 --jobs 5 --seed 67000000`,
pre-registered before the screen that nominated `r3` was read:

| profile | paired score | Elo-equiv | directions | sign p | anytime | verdict |
|---|---|---|---|---|---|---|
| compact standard | 55.2% (95% Wilson 46.3–63.8) | +36 | 42–18 | **0.0027** | e=6.62, not crossed | INCONCLUSIVE → **ACCEPT** |
| deployment online | 56.9% (95% Wilson 47.9–65.4) | +48 | 44–17 | **0.0007** | **e=485.8, p≤0.0021, crossed at map 27** | INCONCLUSIVE → **REJECT** |

`multi-profile promotion gate: RETAIN advanced — cleared 1/2 required profiles.`
**`r3` is not promoted and this document does not promote it.**

The deployment profile failed on **one thing only**: the fixed-*n* Wilson lower
bound sits at 47.9%, below parity. Its direction test is significant at
p=0.0007 and its anytime-valid evidence crossed at **map 27**. Positive on both
profiles, significant direction on both, and short of the interval on one.

## Why an extension is the precedented move and not gate-weakening

`docs/SUPERHUMAN.md` records the identical situation for warm branches — *"Gate
INCONCLUSIVE **only** on the fixed-n Wilson bound (48.3%); 54.6% needs ~450
maps to clear it arithmetically"* — resolved by a pre-registered larger run that
then **PASSED under the unmodified gate**. `docs/EVAL.md` records the same for
`advanced_policy_live_control` at 300 maps and says explicitly that this is
*"evidence for another targeted treatment, not permission to weaken the gate"*.

Nothing here weakens the gate. The rule is unchanged; only the sample grows.
`MATRIX_PROFILE_SEED_STRIDE` is a constant independent of `--pairs` precisely so
a prefix can be extended without moving either profile onto different maps, so
the first 120 maps of each profile are the ones already run.

Arithmetic: at 56.9% the Wilson half-width must fall from 9.0 to under 6.9
points, which needs roughly **205 maps**. 300 gives margin without inviting a
third bite.

## The run, fixed now

```sh
ai_eval advanced_evolved advanced --matrix --pairs 300 --jobs 5 --seed 67000000
```

from the working directory staging `r3` at `evolved/best.json`.

## The rule, fixed now, and the limit

- The **unmodified** matrix rule decides. Deployment must return
  `promotion gate: PASS`; compact must not establish a regression.
- **A full matrix PASS is the only outcome that nominates `r3` for the shipped
  artifact.** Anything else — including another positive-but-inconclusive read —
  ends this line. **There is no third extension**, no new seed, no pooling with
  the 66,000,000 screen maps, and no change to the candidate.
- ⚠ Because this extension was chosen after an inconclusive read, a PASS here is
  a **discovery estimate and biased upward**: the effect size it reports must not
  be quoted as the expected gain. Only the verdict travels; the number needs a
  disjoint confirmation seed before anyone cites it.
