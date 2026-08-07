# Adjudication: measured, and not worth it

**Verdict (2026-08-07): do not adjudicate. The hypothesis that ending
"decided" games early recovers meaningful compute is refuted at every
threshold on both turn-budget regimes. The instrument stays; the mechanism
was never built.**

## The hypothesis

`docs/FLEET.md` records that 62% of an audited league history ended as score
truncations at turn 250 — games whose tails looked like compute spent on a
settled outcome. The engine already carries a calibrated live win-probability
model (`src/odds.rs`, Brier- and log-loss-checked at three phases of
completed games) whose only consumer was the spectator ribbon. The proposal:
from some mid-game turn, if the odds leader's `now` share crosses a high
threshold, declare it the winner and stop — projected at the time as a
30–40% cut in mean game cost.

That projection was never measured. This document is the measurement.

## The instrument

`civvis odds-audit` plays every game to its real end and records, per
threshold, the first world turn the odds leader crossed it. Nothing about
any outcome changes, so winner agreement and turns saved are exact rather
than estimated:

```
civvis odds-audit --games 60 --players 6 --jobs 10                # stock budget
civvis odds-audit --games 60 --players 6 --turns 250 --jobs 10    # capped
```

Options: `--adjudicate-start` (first sampled turn, default 100), `--every`
(sampling stride, default 5), `--thresholds` (default 0.90,0.95,0.98,0.995).
Seats get a flat 1500 prior — stock fleets, no roster — so only the board
terms and the clock separate the table. `score` endings are the turn cap's
truncation award; every other ending is a game the rules finished.

## Results — 60 seeds each, 6 players, Online speed, 2026-08-07

**Stock turn budget** (the league/exhibition regime; every game ended
naturally — 37 science, 12 diplomatic, 9 religious, 2 culture):

| threshold | crossed | agreement | mean turns saved | share of total compute |
|---|---|---|---|---|
| 0.90 | 31/60 | 93.5% (29/31) | 20.2 | **2.7%** |
| 0.95 | 23/60 | 91.3% (21/23) | 9.6 | 1.0% |
| 0.98 | 11/60 | 100% (11/11) | 4.6 | **0.2%** |
| 0.995 | 6/60 | 100% (6/6) | 2.7 | 0.1% |

**250-turn cap** (the truncation regime the hypothesis lived on; 59/60
games truncated on score):

| threshold | crossed | agreement | mean turns saved | share of total compute |
|---|---|---|---|---|
| 0.90 | 5/60 | 100% (5/5) | 21.2 | **0.7%** |
| 0.95 | 4/60 | 100% (4/4) | 20.2 | 0.5% |
| 0.98 | 4/60 | 100% (4/4) | 9.0 | 0.2% |
| 0.995 | 3/60 | 100% (3/3) | 0.3 | 0.0% |

## Why the hypothesis dies

Two independent mechanisms, one per regime:

- **Under the stock budget there is no tail to cut.** The model's clock
  multiplier deliberately discounts mid-game leads ("calibration must stay
  soft enough for ordinary late upsets"), so high confidence arrives only a
  handful of turns before the rules end the game anyway. And the thresholds
  safe enough to act on (100% agreement starts at 0.98) are exactly the ones
  that arrive latest.
- **Under a cap, truncated games are undecided games.** A board that reaches
  turn 250 without a natural victory is precisely a board where no seat holds
  a commanding position — the odds leader crossed 0.90 in only 5 of 60 capped
  games. The 62% truncation share measured in FLEET.md was never 62% of games
  with a settled outcome; it was 62% of games the *model itself* declines to
  call. The two regimes between them exhaust the design space: where games
  decide, they end; where they don't end, they aren't decided.

The league already took the real fix in this space: raising `max_turns` to
the stock budget so games end on victories rather than truncations
(`LeagueCfg` comment, `docs/EVAL.md`).

## Do not reopen unless

- a *deliberately aggressive* adjudication calibration is proposed — one
  that trades documented winner-agreement for savings. That is a different
  experiment: it changes recorded outcomes, so it needs the paired-map
  machinery and a pre-registered agreement floor (≥99% on ≥300 games at the
  deployment profile), and this instrument is how it would be screened; or
- the fleet returns to short-cap profiles for a bulk workload, in which case
  rerun both tables first — the result above says the savings still will not
  appear, but the instrument is one command.

Anything else re-proposing "stop decided games early via `odds.rs`" without
new mechanism should cite this document and stop.
