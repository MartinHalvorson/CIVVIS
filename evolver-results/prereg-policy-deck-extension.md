# Pre-registration — extend the live-policy-deck prefix to the power it needs

Written 2026-07-31 **before any map beyond the recorded 300 was run**. Agent
`claude-evolver`.

## Why this and not a new mechanism

`docs/EVAL.md` records `advanced_policy_live_control` — stock `AdvancedAi` with
the existing counterfactual policy deck enabled, i.e. exactly the difference
between `Weights::default()`'s `PolicyDeck::Legacy` and the `Live` every
artifact genome already carries — at **300 maps per profile**:

| profile | score | Elo | interval |
|---|---|---|---|
| compact standard | 52.3% | +16 | 95% Wilson 46.7–57.9 |
| deployment online | **54.3%** | **+30** | 95% Wilson 48.7–59.9 |

Deployment direction was **92–51, p=0.0008**, and its anytime-valid evidence
**crossed the directional threshold at map 98**. The matrix stayed
`INCONCLUSIVE` for one reason only: the fixed-*n* Wilson lower bound sat just
below 50%.

That is the identical situation `docs/SUPERHUMAN.md` records for warm branches
— "Gate INCONCLUSIVE **only** on the fixed-n Wilson bound (48.3%); 54.6% needs
~450 maps to clear it arithmetically" — which was then resolved by a
pre-registered larger run and **PASSED**. The precedent for what to do here is
in the repository, and it is to buy the maps, not to weaken the gate.

Two independent corroborations of the direction exist beside the 300-map run:
`advanced_policy_live_control` v `advanced_v1` scored 53.7% (+26) at
deployment, and this session's deck-only artifact rung — `"weights": {}`, every
gene at the stock default, deck Live — scored **53.1% (+22)** against
`advanced` on 40 fresh deployment maps at seed 66,000,000, a different
construction path and a disjoint seed.

Arithmetically, 54.3% clears a 50% Wilson lower bound at roughly **520 maps**.

## The run, fixed now

`MATRIX_PROFILE_SEED_STRIDE` is constant and independent of `--pairs`
specifically so that a prefix can be extended without moving either profile
onto different maps. This therefore **extends the recorded prefix** rather than
selecting a new one:

```sh
ai_eval advanced_policy_live_control advanced --matrix --pairs 600 --jobs 4 \
  --seed 22051000
```

The first 300 maps of each profile are the recorded ones; 300 are new.

## The rule, fixed now

The **unmodified** matrix rule decides. Deployment must return
`promotion gate: PASS` and compact must not establish a regression. Nothing
else counts: not the direction test, not the anytime e-process on its own, not
the terminal-score share.

- **Full matrix PASS** → the deliverable is a one-line change of
  `Weights::default().policy_deck` from `Legacy` to `Live`, plus the
  `turn_cost` measurement of what the counterfactual valuation costs a
  game-turn at the deployment profile, because `f2d53cf` defaulted it to
  `Legacy` on compute grounds and that trade must be priced before it is
  reversed.
- **Anything else** → recorded as a second inconclusive at higher power, and
  the arm stays as it is. No third extension, no new seed, no threshold change.

No knob, seed, sample size or profile flag will be chosen after seeing this
result.
