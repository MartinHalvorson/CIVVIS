# Victory clocks that expect the economy to grow, 2026-11-09

The operator asked that the per-victory estimates account for how a game
accelerates, a little conservatively. The standings' "Closest victory" column
read the planner's clock at today's pace, so research and launch production were
priced as if no empire would ever grow again.

## What changed

Only the published clock (`AdvancedAi::victory_clocks`, read by `victory_eta`).
`victory_forecasts` and the planner keep today's pace, so play does not move: all
72 simulated games ended on the same turn, by the same victory, for the same
winner on both builds.

| Lane | Change |
|---|---|
| Science | Research and production rise as a power of the turn, exponent 1.25. Flight is 0.7 of distance over speed. |
| Culture | The gap to each defender closes on the same curve, exponent 1.25. |
| Religious | None. See below. |
| Diplomatic, Domination, Score | None. Their clocks have no income term to grow. |

A power of the turn fits better than a fixed percentage, because the percentage
falls through the game (winners' science: about 4% a turn at t25, 1.7% at
t100 to t150). The turn also cancels out of the ratio, so one exponent serves
every speed. The curve is anchored no earlier than 10% of the speed's stock clock.

## How conservative

Across the 66 audited host replays in
`2026-11-09-win-probability.replays.jsonl.gz`, over 50-turn windows, the exponent
for science had median 1.66 and lower quartile 1.10, and production 1.71 and 1.10.
1.25 is about three quarters of the median. Flights in progress took a median
0.63 of distance over current speed in 43 Science wins, and 0.7 is used.

## Results

Science wins on six-player Deity Online host games, the winner's own clock, each
game counted once per phase. "Actual / clock" under 1 means the clock read long.
Intervals are game-level bootstraps of the change in mean absolute error.

All 43 Science wins:

| Phase | Games | Actual / clock, before → after | Mean error in turns, before → after | Change [95%] | After: clock no earlier than the real finish |
|---|---:|---|---|---|---:|
| t25–50 | 6 | 0.15 → 0.78 | 880 → 47 | −1039 to −645 | 100% |
| t50–100 | 15 | 0.38 → 0.79 | 187 → 27 | −179 to −142 | 93% |
| t100–125 | 21 | 0.49 → 0.76 | 104 → 35 | −91 to −52 | 95% |
| t125–150 | 26 | 0.53 → 0.69 | 55 → 26 | −41 to −21 | 92% |
| t150–175 | 37 | 0.49 → 0.62 | 35 → 20 | −21 to −10 | 92% |
| t175–end | 32 | 0.44 → 0.63 | 16 → 11 | −7 to −4 | 84% |

The newest 16 of those, split off by run id:

| Phase | Games | Actual / clock, before → after | Mean error in turns, before → after | Change [95%] |
|---|---:|---|---|---|
| t50–100 | 6 | 0.39 → 0.83 | 184 → 25 | −191 to −132 |
| t100–125 | 9 | 0.50 → 0.78 | 93 → 30 | −80 to −46 |
| t125–150 | 10 | 0.54 → 0.70 | 54 → 25 | −37 to −22 |
| t150–175 | 15 | 0.56 → 0.70 | 37 → 21 | −30 to −7 |
| t175–end | 11 | 0.48 → 0.68 | 12 → 7 | −6 to −3 |

Among seats that did not win, the share whose soonest clock landed before the
game really ended rose from 0.04% to 0.28% of 2465 host readings at t75 or later.
In 72 simulated games it stayed at 17.42%. Across the growth grid, which excludes
the flight share, the seat with the soonest clock was the eventual winner just as
often (host 43.0% at every exponent, simulated 56.1% to 56.0%), so this should not
be read as a better call of who wins.

## Limits

- **There is no clean out-of-sample test.** The newest 16 games were split off,
  but their rows were on screen beside the older games when the flight share was
  chosen, and the first grid scored all 52 together. The exponent came from yield
  growth rather than clock error, on replays that may overlap these games. Only
  one game finished after the constants were fixed, too few to score. The second
  table shows stability across time, not held-out performance. Re-score newer
  games with the recipe below.
- **These are the eventual winners' clocks.** Winners grow faster than the field
  (science exponent 1.94 against 1.66), so a non-winner's clock is less tested.
- **Culture is barely measured.** In all 131 readings of the 9 host Culture wins
  the winner's clock was blank (`unmet_rivals`), and only 3 simulated games have
  one. There, even with exponent 2 the clock read about nine times too long, so
  the direction is safe, and the size is unvalidated. The exponent is the Science
  value, below culture yield's lower quartile of 1.40.
- **Religious clocks are unchanged on purpose.** They read short at t50 to t150
  in simulated Religious wins (actual / clock 1.41, 1.28, 1.20), and faith growth
  moved them slightly further the wrong way. All 4992 readings are identical.
- **Domination is untested,** as no recorded or simulated game ended that way.
- **Clocks still read long late.** At t175 and after, a median clock is about 1.6
  times the real time left, and growth barely moves it. One unmeasured suspect is
  that the clock prices a single launch city.

## Reproduce

```
cargo build --profile ci --locked --features developer-tools --bin odds_export
target/ci/odds_export --replay <run dir> --every 10 --clock-growth 0,1,1.25,1.5
target/ci/odds_export --games 72 --jobs 4 --start-seed 9600000 --every 10 --clock-growth 0
```

Each seat's `clocks` holds one entry per exponent, without the flight share, and
`published` holds what the standings show. Figures are in the `.results.json`
beside this note, where `host_science_holdout` is the newest 16 described above.
