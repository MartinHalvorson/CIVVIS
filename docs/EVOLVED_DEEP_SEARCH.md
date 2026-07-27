# Deep-search budget on the evolved genome

## Question

`strategic_deep` was promoted because the 20-turn review cadence and 80-round
horizon beat the original 40x40 `strategic` budget. The deciding default-weight
run won 339-261 games over 300 mirrored maps, passed the promotion gate, and
completed a pooled 540-map record of 109 favorable directions to 32 adverse.

`ff083ea` then shipped the first evolved genome and correctly marked every
previous comparison that loaded `best.json` as superseded. Generation 2 itself
transferred favorably into deep search: `docs/STRATEGIC_GENOME_TRANSFER.md`
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

## Generation-2 pre-registered evaluation

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

## Generation-2 result

### Generation-2 development screen

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

### Generation-2 disjoint reversal gate

The exact gate command reversed entrant order so the shallower agent was the
challenger whose PASS could change the ranking:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic strategic_deep \
  --pairs 240 --players 4 --width 24 --height 16 \
  --turns 200 --seed 114000 --jobs 12
```

The fresh 240-map result decisively reversed the development screen:

- `strategic` lost 215-265 games; its paired score was 44.8%, with a
  38.6%-51.1% Wilson interval and -36 Elo point estimate;
- map directions were 13 shallow-favorable, 189 neutral, and 38 deep-
  favorable; exact sign-test `p = 0.0006`;
- anytime evidence favored deep with peak `e = 521.7`, crossing at map 71;
- terminal score was 49.7% for shallow, with directions 109-9-122
  (`p = 0.4299`);
- the formal challenger gate was **INCONCLUSIVE**, so no reversal was earned.

The win mechanism is victory conversion backed by a larger empire. Deep won
182 religious games to shallow's 136 while both won exactly 70 score games.
It averaged 144.0 score to 139.3, 2.54 cities to 2.41, 16.0 population to
14.9, 181.6 military strength to 164.6, and 15.6 science to 14.2. Despite
spending a slightly smaller share of observed turns labeled Religion (30.1%
versus 31.1%), it converted 46 more religious victories.

Across the screen and gate, 300 disjoint generation-2 maps split 322-278 games
for deep and **45 map directions to 23** (pooled exact `p = 0.0103`). The
60-map screen was ordinary small-sample inversion; the powered fresh run
restored the original ordering on that policy population.

## Generation-14 upstream confirmation

Before this audit was finalized, `0a5afd5` replaced generation 2 with the
generation-14 champion. The completed generation-2 screen and gate remain
valid evidence for that frozen population, but they cannot decide the search
budget on a materially different genome.

Before observing any generation-14 games, this audit therefore registers a
fresh 60-map confirmation with otherwise identical settings:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_deep strategic \
  --pairs 60 --players 4 --width 24 --height 16 \
  --turns 200 --seed 115000 --jobs 12
```

A deep-favorable or neutral win direction keeps the current ranking and stops.
Only a favorable `strategic` win direction earns a disjoint 240-map reversal
gate at seed 116000, with `strategic` first so that only its formal
`promotion gate: PASS` can reverse the recommendation. This confirmation is a
new-policy safeguard, not an extension selected in response to the
generation-2 outcome.

### Generation-14 result

Across the 60 fresh mirrored maps (120 games), deep won 61-59:

- paired score 50.8%, 95% Wilson interval 38.5%-63.0%, and +6 Elo point
  estimate for deep;
- map directions five deep-favorable, 51 neutral, and four `strategic`-
  favorable (`p = 1.0000`);
- terminal score 48.8% for deep, with directions 22-3-35 (`p = 0.1112`);
- the formal gate was **INCONCLUSIVE**, with no anytime-valid boundary
  crossing in either direction.

The diagnostics again separate victory routing from economic development.
Deep won 48 religious games to shallow's 46, with each side taking 12 score
games and one domination game. Shallow averaged 142.6 score to 133.3, 2.63
cities to 2.42, and 15.9 population to 14.5. Deep reached 675 rollout reviews
versus 450 and switched plans 2.52 times per game versus 2.06.

The observed win direction is deep-favorable, so the pre-registered stopping
rule applies: the shallow challenger does not earn the seed-116000 reversal
gate. Generation 14 therefore provides no evidence to overturn the powered
generation-2 result or the original default-genome promotion.

`strategic_deep` remains the evidence-backed top rung. This audit changes no
agent behavior; it confirms that the evolved genome did not make the cheaper
40x40 budget a demonstrated replacement for the 20x80 incumbent.
