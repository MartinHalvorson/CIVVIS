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

`deployment-contested`'s field, seed 19300000, 120 pairs / 240 games.
Entrants hold seats 0 and 1; the other four are
`live_target_diplomatic` and `live_target_culture`.

```
paired-map score 50.8% (95% betting CI 47.2%..54.6%)  Elo-equivalent +6 (CI -20..+32)
paired direction 15 for / 94 neutral / 11 against     sign p = 0.5572 INCONCLUSIVE
game-win share   59/240 (24.6%) against 55/240 (22.9%)
```

The mechanism fires, and harder than it did fieldless:

| | treatment | control |
|---|---:|---:|
| suzerainties | **1.47** | 0.90 |
| envoys placed | 18.4 | 17.9 |
| **diplomatic victory points** | **5.5** | **5.5** |

Envoy income is flat and suzerainties are up **63%** — a larger dose than
the 0.3 → 0.7 the fieldless fires-check recorded. And the diplomatic victory
points, which are what the lane is actually paid in, **do not move at all**.

### The board produces the lane. Our agents still never win it.

Over the 240 games: religious 108, culture 93, **diplomatic 29**, score 7,
science 3. So this board does produce the two lanes that beat us live.

The entrants' own wins do not:

| arm | religious | science | score | culture | diplomatic |
|---|---:|---:|---:|---:|---:|
| `advanced_price_suzerainty` | 54 | 3 | 2 | **0** | **0** |
| `advanced` | 54 | 0 | 1 | **0** | **0** |

**Every one of those 29 diplomatic and 93 culture victories was won by a
scripted field seat**, and a game the field wins is a draw for the pair. So
the lane is produced 29 times and can move the paired score by exactly
nothing.

That is a stronger statement than "the screen was underpowered". A win-rate
screen for a diplomacy treatment is not merely noisy on this board, it is
structurally incapable of registering one, because our agents do not win
that lane at all — on the board built to make them contest it.

## What was decided

**`price_the_suzerainty` stays off.** Nothing here supports promotion: the
contested screen reads 50.8% with the interval crossing parity and the sign
test at p=0.5572, and the fieldless run is at parity too. Two disjoint seeds
already retired it and this adds a third board that does not rescue it. The
earlier decision was right.

**But the reason it fails is now visible, and it is not the dose.** The arm
delivers what it promises — envoy income flat, suzerainties up 63% — and
converts none of it into the currency the lane needs. Diplomatic victory
points sit at 5.5 for both arms while one holds 63% more suzerainties, and
`Game::suzerain_diplomatic_favor_per_turn` says each of those should be
paying 1.0 favor a turn. **Where that favor goes is the open question this
round hands on**, and it is a better question than "should the prize be
bigger", which is where this arm's file has been stuck.

**Recorded about the instruments, not the arm:**

- Both 400-pair runs that retired this flag ran with diplomatic victory
  *disabled*. That is now on the record beside them.
- The arm was missing from `MINOR_DEPENDENT_ARMS`; at `--city-states 0` it
  is byte-identical to its control, and 12 pairs confirm it (0 favored / 12
  neutral / 0 against on wins *and* terminal score). Added.
- `ai_eval` now reports what a lane can move the paired score by *at most*,
  beside the interval it already prints. The fieldless run's `diplomatic 2`
  is bounded at 1.7 points against a ~5-point half-width; this round's
  `diplomatic 29` is bounded at **zero**, because no entrant won one. The
  first version of that check counted games rather than entrant wins and
  would have called this board adequate.

**Not decided here:** whether the composite `advanced_diplomacy_lane`
(#2185) behaves differently. It is registered and unscreened. On the
evidence above, screening it by win rate on either of these boards would
measure the board again — the useful screen is the mechanism one: does
giving the lane a reason to be entered convert suzerainties into DVP where
pricing alone did not.
