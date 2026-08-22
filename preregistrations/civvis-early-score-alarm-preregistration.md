# Pre-registration — read the score race as a margin, not a clock

Written 2026-07-27 by the `loop-counter-leader` session, before the run.
PR #516. Third pre-registration of this loop. The first predicted the
ablation's null and was right; the second predicted the lane rewrite's shape
and was wrong in both directions and then refuted by its own confirmation.

## Why there is anything left to test

Every response-side change in `docs/COUNTERING_LEADERS.md` measured null:
deleting the whole response (dead heat at two map scales), organising a
coalition (one or two belligerents wins 4.4% and 10.7% of seats against a 16.7%
base), and changing what the alarm asks for (49.9% across 360 confirming maps).

The one thing that did not measure null is an *instrument*: at the deployment
profile the score leader is the eventual winner **62% of the time 200 turns
out** against a 16.7% base, and settles on them a median 135 turns before the
end, while `victory_threat` sits at or below the base rate at four of five
leads.

The shipped score term is a **clock, not an observation**: it fires only once
`turn*4 >= max_turns*3`, so every leader trips it at turn 300 of 400 alike, no
matter how far ahead. `early_score_alarm` reads the margin instead — 78 at 20%
ahead of the next empire, 100 at 50% ahead — from `standard_duration(60)`.

## Fires-check (16 maps, 4p 60×38, 6 city-states, seed 980000)

| arm | denial fires | first denial → win | follow-through | conquest | expansion | gold |
|---|---|---|---|---|---|---|
| ship | 13.9% | median 78 turns | 43% | 25% | 31% | 322 |
| **early** | **37.5%** | median **284** | **63%** | **35%** | 19% | 313 |
| **early_build** | 35.8% | median **143** | **33%** | **14%** | **52%** | 470 |

The instrument works: warning time goes 78 → 284 turns. **And on its own it is
a war engine** — an earlier alarm feeding the shipped Conquest counter pushes
plan mix from 25% to 35% conquest and follow-through from 43% to 63%. Pairing
it with `counter_in_lane` reverses that: conquest 14%, expansion 52%.

This is much the largest behavioural change any arm in this loop has produced —
21 points of plan mix against `counter_in_lane`'s 6.

## The runs

```
ai_eval advanced advanced_early_score_build \
  --players 4 --city-states 6 --width 60 --height 38 \
  --pairs 120 --turns 400 --seed 990000 --jobs 6
```

`advanced_early_score_alarm` (the instrument alone) is queued behind it.

## Predictions

1. **`advanced_early_score_build`: 48–54%, most likely null.** Ten iterations
   of this loop say the response layer does not move outcomes. The larger
   behavioural change raises the variance of that guess in both directions but
   does not change its centre.
2. **`advanced_early_score_alarm` alone: below 50%.** It adds war, and war at
   this map scale is measurably costly to the empire waging it.

If (1) comes back positive it is **not a result** until 360 pairs at a disjoint
seed reproduce it. That protocol already caught one false positive in this loop
— a 53.8% at p=0.0225 that regressed to 49.9% — and the discovery run here is
the same size that produced it.

## What would refute the whole line

`early_build` null. The instrument is the last untested thing that the
measurements point at; if reading score early and answering it by building also
changes nothing, then nothing available at this layer counters a leader in this
engine, and the honest conclusion is that the denial layer should be deleted
rather than improved.

---

# Confirmation — registered 2026-07-27, after both discovery runs

## Both registered predictions are now settled, and one was wrong

**Prediction 1 held.** `advanced_early_score_build` at seed 990000: 48.3%,
sign p=0.6271 on wins, p=0.3153 on terminal score. Inside the registered
48–54% band, null.

**Prediction 2 was wrong, in the opposite direction.** I registered
`advanced_early_score_alarm` alone as "below 50%, because it adds war". At
seed 991000 it scored **54.6%** for the treatment (`advanced` 45.4%,
Wilson 36.8–54.3, Elo-equivalent −32), win direction 13/83/24, sign
**p=0.0989**. Terminal score is dead flat — 50.5%, 61 vs 59, p=0.9273 with all
120 maps resolving — so whatever is happening is victory routing, not economy.

The arm that pushes plan mix from 25% to 35% conquest and follow-through from
43% to 63% is the one that scores *better*, not worse. That is the opposite of
what the dogpile table led me to expect.

## Why I do not believe it yet

- p=0.0989 is weaker than the p=0.0225 this loop has already watched evaporate,
  and wins rest on 37 of 120 maps.
- **The two arms disagree in a way noise explains better than mechanism.**
  Early alarm alone reads 54.6%; the same alarm with `counter_in_lane` reads
  48.3%. If the earlier warning were genuinely worth +32 Elo, changing what it
  asks for should not flip it 6 points the other way — unless the war *is* the
  mechanism, which the dogpile and ablation readings both argue against.
- Two 120-pair discovery runs in this loop have landed near 54%. One of them
  regressed to 49.9%.

## The run

```
ai_eval advanced advanced_early_score_alarm \
  --players 4 --city-states 6 --width 60 --height 38 \
  --pairs 360 --turns 400 --seed 992000 --jobs 6
```

## Prediction

**Regression to 49–52%, sign p > 0.05.** Same call I would have made about the
53.8%, and it was right then.

## What would refute it

The treatment holding at 53%+ with sign p < 0.05 on 360 maps. That would be the
first thing in this investigation to survive a confirmation, and it would mean
the earlier alarm is worth something *because* of the war it starts — which
would in turn mean the dogpile table's confound was hiding a real effect rather
than manufacturing one.
