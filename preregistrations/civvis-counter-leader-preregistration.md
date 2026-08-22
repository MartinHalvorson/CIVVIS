# Pre-registration — what is the counter-leader response worth?

Written 2026-07-27, before the run, by the `loop-counter-leader` session.
PR #516. Registered *before* any result was read.

## The question

`leader_census` (PR #516) measured the AI's denial layer as a near-perfect
predictor of the winner and no deterrent at all:

- 85.6% of every empire the layer ever names goes on to win (95 of 111 at 4p,
  base rate 25%); 82.8% at 6p against a 16.7% base.
- 78.7% still win when war actually followed the alarm (48 of 61).
- The alarm arrives a median 16 turns before the win, in a 400-turn game.
- At 6 seats the response is *twice* as large — the winner is at war with a
  major 21% of turns before the alarm and 53% after — and no more effective.

None of that is causal. It is all conditional on reaching high victory
pressure, which is itself caused by being about to win. So the ablation.

## The run

```
ai_eval advanced advanced_blind_to_leaders \
  --players 6 --pairs 120 --turns 400 --seed 930000 --jobs 6
```

`advanced_blind_to_leaders` is identical to `advanced` except that
`victory_denial` is silent and `urgent_victory_threat` always returns false:
it still fights, expands and races, it just never does any of it *because*
somebody else is about to win. Mirrored maps, so the comparison is paired.

Smoke-tested at 4 pairs before registering: the arms differ on production
(35.8 vs 33.6), military (4.8 vs 4.6) and settlers (0.29 vs 0.25), so the
treatment is not a silent no-op.

## Prediction

**Null: `advanced` scores 48–52%, sign p > 0.05.** The census says the layer
detects reliably and stops nothing, so removing it should cost nothing. If
that is right, the whole counter-leader response is bounded at ~zero and
"respond harder" is retired the way the goal layer was.

## What would refute it

`advanced` clearly above 50% with sign p < 0.05 across map directions. That
would mean the response *does* work and only its timing is wrong — which
would promote the early-warning tier (score-keyed, ~70–100 turns out) from
speculation to the obvious next build.

A result *below* 50% for `advanced` — the denial layer actively costing
strength — is the third possibility and would be the most interesting: an
empire that abandons its own race to chase a leader it cannot catch.

## Not to be read as

Strength of anything else. One ablation, one seed, one table size.
