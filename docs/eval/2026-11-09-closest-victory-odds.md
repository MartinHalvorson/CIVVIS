# Closest-victory win odds, 2026-11-09

The operator asked that the viewer's "Win odds · now" column partly account for
each seat's closest victory and how likely it is to be reached. The forecast
already carried the raw meter of the closest race. It missed two things:

- **The Science road was invisible.** Science's meter reads 0% until the first
  launch, which came around turn 125 to 150 in these games, yet Science decides
  most Deity host games. A seat's tech lead against the living field now counts while Science is enabled:
  450 Elo per natural-log ratio, with 5 techs added to both sides.
- **Every seat began with free Domination progress.** Holding your own capital
  is where the game starts, and it set a floor of 1/n under every seat's closest
  race. That share is now removed, and the race still reads 100 when finished.

`victory_races`, which the AI reads, is unchanged. Only the way `odds.rs` prices
its output changed. The displayed and exported race percentage still shows the
raw meter.

## What was measured

Lower is better for both scores. Log loss is on the actual winner. Intervals are
game-level paired bootstraps, each game counted once however many snapshots it
has. Baseline and candidate read the same recorded boards, and every input other
than the estimate matched exactly. All 60 held-out simulated games played out
identically under both builds, so the comparison is paired game by game.

| Set | Games | Window | Log loss before → after | Change [95%] | Brier change [95%] |
|---|---:|---|---|---|---|
| Host, audited (chosen on) | 51 | t50+ | 1.430 → 1.400 | −0.030 [−0.046, −0.016] | −0.0022 [−0.0036, −0.0010] |
| Host, audited (chosen on) | 51 | t125+ | 1.295 → 1.249 | −0.046 [−0.063, −0.029] | −0.0034 [−0.0050, −0.0017] |
| Host, unseen | 19 | t50+ | 1.167 → 1.120 | −0.047 [−0.088, −0.007] | −0.0052 [−0.0096, −0.0009] |
| Host, unseen | 19 | t125+ | 1.056 → 1.000 | −0.056 [−0.096, −0.016] | −0.0061 [−0.0111, −0.0013] |
| Simulated, held out | 60 | t20+ | 1.051 → 1.043 | −0.008 [−0.031, +0.016] | −0.0001 [−0.0036, +0.0036] |
| Simulated, held out | 60 | last 30 turns | 0.638 → 0.620 | −0.018 [−0.049, +0.017] | −0.0017 [−0.0068, +0.0038] |

Held-out simulated games, by how the game was won:

| Winner by | Games | Log loss change, t20+ | Log loss change, last 30 turns |
|---|---:|---|---|
| Religious | 39 | −0.017 [−0.043, +0.012] | −0.041 [−0.073, −0.009] |
| Diplomatic | 14 | +0.044 [−0.004, +0.094] | +0.078 [−0.006, +0.174] |
| Culture | 4 | −0.082 [−0.127, −0.035] | −0.057 [−0.086, −0.023] |
| Score | 3 | −0.030 [−0.055, +0.013] | −0.104 [−0.197, +0.028] |

The host games are six-player Deity Online. Across 70 of them, 56 were Science
and 14 Culture wins. No host game ended by Religion, Diplomacy or Domination,
so those lanes could only be tested in simulation: four seats, Prince, the
basic AI controller, a 32×22 Tennis Ball map. No simulated game ended by Science
or Domination, so Domination is untested in both sources.

`*.results.json` beside this note has every figure above and the rejected
variants.

## What was rejected, and why it matters

The first version also squared the Religious and Diplomatic meters and removed
the own-faith share. It looked right on host games, where neither race ever
finished and their meters barely separated the winners. It scored better there
than what shipped (t125+ log loss −0.118 against −0.047).

Simulation exposed it. On 36 simulated games, 26 won by Religion, the last 30
turns of Religious wins went from log loss 0.552 to 0.832 (change +0.281,
95% interval +0.138 to +0.453). The eventual winner's final reading fell from
79% to 59%. A seat three-quarters of the way through that race is usually about
to finish it. Keeping the curve on Diplomatic alone failed the same way
(+0.553, 95% interval +0.015 to +1.219, five games).

The lesson generalizes. A discount fitted where a race never finishes reads as
"that meter overstates", and the same discount is wrong wherever the race does
finish. The host data cannot tell those apart, so both meters are read as they
stand.

An earlier form blended a separate "someone finishes" probability with the Elo
softmax. It was worse held-out (log loss 1.3937 → 1.4142) and was dropped.

## Limits

- **The coefficients are not free of the audited games.** The tech term and its
  size were chosen after viewing them. The grid optimum sat at the edge (1000);
  450 is deliberately conservative because about 80% of that sample is Science.
  The 19 unseen host games and 60 simulated seeds were not used to choose
  between the variants, and they carry the out-of-sample claim.
- **Many unseen host games are short continuation runs,** with few snapshots.
  At single turns (t50 through t175) those games show no detectable change; the
  significant result comes from pooling each game's snapshots.
- **Diplomatic wins get slightly worse in simulation.** The tech lead pulls some
  probability toward seats that are not the eventual diplomat. On 14 held-out
  games the Brier change at t20+ is +0.0088, with an interval that excludes zero
  (+0.0011 to +0.0172), and the log loss interval just includes zero. Across all
  60 held-out simulated games the favorite was right 2.9 points less often at
  t20+ (interval −5.6 to −0.2), though overall log loss and Brier did not move.
  These are small, and the tech term stays because Science decides most games
  on the difficulty this viewer mostly shows.
- **The Science term only reads techs.** It does not see rockets, Spaceports or
  whether a rival is racing the same projects, so it is a proxy for the road
  before launches begin.
- **Run directories get pruned while they are read,** so counts differ between
  stages. 70 host games were available for the final build.

## Mercy Rule

`odds::mercy_leader` uses the same table, so the moment a seat crosses the
Mercy threshold can shift. Mercy is off by default, and all arithmetic stays on
`libm`, so outcomes remain deterministic.

On the 60 held-out simulated games, read every 10 turns with Mercy disabled:

| Threshold | Games crossing, before → after | Crossing leader won, before → after |
|---:|---|---|
| 90% | 27 → 27 | 26 of 27 → 25 of 27 |
| 95% | 8 → 16 | 8 of 8 → 15 of 16 |
| 97% | 6 → 7 | 6 of 6 → 7 of 7 |
| 99% | 1 → 3 | 1 of 1 → 3 of 3 |

The 95% rung fires about twice as often, and made one wrong call in this sample.
The calibration tables in `docs/ADJUDICATION.md` were measured on the earlier
formula and were not rerun.

## Reproduce

```
cargo build --profile ci --locked --features developer-tools --bin odds_export
target/ci/odds_export --replay <run dir> --every 25
target/ci/odds_export --games 60 --jobs 3 --start-seed 9500000 --every 10
python3 tools/odds_report.py <joined.jsonl> --baseline shipped_now --candidate now --phases .1,.2,.3,.35
```

The joined file adds each seat's `shipped_now` from the baseline build's export
of the same game. The 66 audited host replays are kept in
`2026-11-09-win-probability.replays.jsonl.gz`.
