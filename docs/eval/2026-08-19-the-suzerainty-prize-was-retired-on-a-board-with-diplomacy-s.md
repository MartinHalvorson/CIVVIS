# the suzerainty prize was retired on a board with diplomacy switched off

_2026-08-19 · `f2e8111f`_

## What was asked

`price_the_suzerainty` ships off. Both 400-pair runs that decided that passed
`--victories science,culture,domination`, so **diplomatic victory — which is
what a suzerainty pays in — was disabled in both**. The entry recording them
says so itself: "this profile understates the mechanism by construction."

It then refuses, correctly, to re-read the arm on a friendlier profile after
the fact. This round does not do that. It asks a different question: the arm
has never been run on the *current* gate at all. Both existing runs are
hand-rolled `ai_eval` lines, and the promotion matrix changed on 2026-08-14 so
that `deployment-online` and `deployment-contested` run all six victories.

So: does enabling the victory the mechanism pays in change what it measures?

## How it was measured

The recorded profile, with exactly one variable changed — the victory set.

```
ai_eval advanced_price_suzerainty advanced --players 6 --width 74 --height 46
  --city-states 9 --turns 250 --speed online --map continents --shape planet
  --poles poles --randomize-civs --pairs 120 --seed 19200000
```

120 pairs / 240 games, all six victories (the default), fieldless — all six
seats are the paired entrants. Seed 19200000 is disjoint from the recorded
14900000 and 15400000.

A second run adds `--field live_target_diplomatic,live_target_culture`, the
`deployment-contested` field, on seed 19300000. The reason is recorded in
`ai_eval.rs` itself: fieldless, that shape produces **diplomatic 0 of 40**, so
a win-rate read there says nothing about this lane. The round
`2026-08-19-the-bare-ai-eval-default-is-not-the-recorded-profile` reaches the
same conclusion from the other direction — "the default profile never produced
a science or diplomatic victory (89 of 120 games religious), so win-rate reads
say nothing for those lanes".

## What it measured

### Fieldless, all six victories — the lane completes for the first time

| arm | culture | **diplomatic** | religious | science | score | wins |
|---|---:|---:|---:|---:|---:|---:|
| `advanced_price_suzerainty` | 13 | **2** | 54 | 19 | 34 | 122/720 (16.9%) |
| `advanced` | 11 | **0** | 59 | 12 | 36 | 118/720 (16.4%) |

Two diplomatic victories against zero. That is the first time the lane
completes in self-play on this shape, and it is the difference the recorded
runs could not have seen at any pair count, because they had the condition
switched off.

It is also **0.8% of games against the live ladder's 20.3%** (`EVAL_STATUS`:
47 of 232 terminal outcomes lost to a rival's diplomatic victory). The gap is
a factor of twenty-five.

And the win rate did not move: 122 against 118 is parity, consistent with the
recorded pooled 51.8%.

**The lane is still not adopted.** Over 146,975 observed player-turns the
treatment spends **0.2%** of them on Diplomacy — 0.0% in midgame — and the
strategy-transition census records `science->diplomacy` exactly **once** in
240 games. The suzerainties this arm buys are acquired as a side effect of
envoy placement, not pursued as a victory path.

### Contested field

<!-- CONTESTED -->

## What was decided

<!-- DECIDED -->
