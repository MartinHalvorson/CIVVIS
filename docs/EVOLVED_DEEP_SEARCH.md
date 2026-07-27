# Deep-search budget on the evolved genome

## Question

`strategic_deep` was promoted because the 20-turn review cadence and 80-round
horizon beat the original 40x40 `strategic` budget. The deciding default-weight
run won 339-261 games over 300 mirrored maps, passed the promotion gate, and
completed a pooled 540-map record of 109 favorable directions to 32 adverse.

`ff083ea` then shipped the first evolved genome and correctly marked every
previous comparison that loaded `best.json` as superseded. The genome itself
has now transferred favorably into deep search: `docs/STRATEGIC_GENOME_TRANSFER.md`
records a 33-27 screen against frozen default weights. That does not establish
that the **extra search budget** still pays, because the evolved policy can
change branch resolution, priors, game length, and the value of another forty
rollout rounds.

Both existing entrants now load the same committed champion and the same
optional value-net path. They differ only in macro-search budget:

| agent | review cadence | horizon |
| --- | ---: | ---: |
| `strategic` | 40 turns | 40 rounds |
| `strategic_deep` | 20 turns | 80 rounds |

## Pre-registered evaluation

The shipped deep agent remains the top-rung incumbent. A 60-map confirmation
uses fresh seeds:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_deep strategic \
  --pairs 60 --players 4 --width 24 --height 16 \
  --turns 200 --seed 113000 --jobs 12
```

A deep-favorable or neutral win direction keeps the current ranking and stops.
Only a favorable `strategic` win direction earns a disjoint 240-map reversal
gate at seed 114000. Only `promotion gate: PASS` for the shallower challenger
can reverse the recommendation; terminal score and plan labels are diagnostic.
This is conservative because deep search already passed its original 300-map
gate and remains an explicit higher-compute choice rather than an inherited
default.

## Result

### Development screen

Across 60 fresh mirrored maps (120 games), the evolved deep agent lost 57-63:

- paired score 47.5%, 95% Wilson interval 35.4%-59.9%, and -17 Elo point
  estimate for deep;
- map directions seven deep-favorable, 43 neutral, and ten `strategic`-
  favorable (`p = 0.6291`);
- terminal score exactly 50.0%, with directions 27-4-29 (`p = 0.8939`);
- deep won 41 religious games to `strategic`'s 46, while aggregate score and
  economy were nearly equal.

The budget changed behavior substantially without improving the terminal
position. Deep switched plans 2.85 times per game versus 2.18 and reached 690
rollout reviews versus 454; `strategic` spent more observed turns committed to
Religion (33.1% versus 31.1%) and retained more faith (329.9 versus 301.6).

The shallow incumbent's win direction is favorable, although unresolved, so
it earns the pre-registered disjoint 240-map reversal gate at seed 114000.

### Disjoint reversal gate

Pending.
