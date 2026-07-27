# Terminal-tempo deep-search challenger

## Hypothesis

`strategic_deep` projects every enabled victory lane for 80 rounds. That depth
wins, but it also resolves every branch in about 56% of reviews. The terminal
evaluator then maps every projected win to `1.0` and every projected loss to
`0.0`, so a review that paid enough to reveal seven outcomes discards the only
remaining distinction between equal outcomes: when they happened.

`strategic_deep_tempo` keeps three disjoint value bands:

| projected result | value |
|---|---:|
| win | `3 - turn / max_turns` |
| unresolved | `1 + position_value` |
| loss | `turn / max_turns` |

This is lexicographic. Every projected win beats every unresolved position;
every unresolved position beats every projected loss. The treatment cannot
exchange a simulated win for development score. It only prefers faster wins
among wins and later losses among losses.

The mechanism targets conversion rather than resource accumulation. It costs
no additional branches or turns and leaves the existing agent bit-identical
unless the evaluator-only name is selected.

## Pre-registered evaluation

Control: `strategic_deep`. Treatment: `strategic_deep_tempo`. Both use the same
genome, review every 20 turns, project 80 rounds, and inherit the promoted warm
branch state.

The development screen is 30 fresh mirrored maps:

```text
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_tempo strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 104000 --jobs 12
```

Only a favorable development direction earns a disjoint 120-map gate at seed
105000. Only the existing win-based promotion gate may change the production
agent; terminal score and tempo are diagnostics, never promotion inputs.

## Result

Pending.
