# Win-probability audit, 2026-11-09

The first failures were missing observations, rather than evidence for a new
set of coefficients. The live mirror retains public city totals separately
from located cities. `odds::table` previously read only located cities for
survival, the city-count term, and the no-city penalty. A living rival with six
publicly reported cities and no located city or Settler received exactly zero.
If it was the only rival, the local seat received 100% and could trigger a
false Mercy Rule result. The forecast now uses the public total, bounded below
by represented cities and explicit hidden-city existence. A confirmed defeat
still receives zero. The full-board simulation path has the same city counts
as before.

The replay exposed a second missing-data case: an unmet empire had no public
city row at all. Such a seat now retains its prior and receives neutral
standing evidence, with means computed from observed empires. Unknown city
status is distinct from an authoritative zero. Mercy abstains while a living
world major's city status is unknown. This prevents exploration coverage from
deciding a game.

Two other prior defects and a clock defect were corrected:

- A handicap-exempt seat still received credit for the difficulty's extra
  starting units and era boosts (and an exempt human for camp Gold). It now
  receives no handicap credit, matching the actual grants.
- `handicap_yield_pct` includes a scheduled late-science adjustment. Reading
  it for pregame strength changed the displayed **start** forecast when that
  schedule began. Pregame strength now reads the fixed setup yield bonus.
- An unlimited world's nominal horizon was always 500 turns. It now uses the
  chosen speed's stock horizon: 250 Online through 1,500 Marathon. Explicit
  caps retain their actual clocks.

## Measurement

An initial census of 84 configured, completed host games sampled opening
frames every 25 turns before the outcome. Of 1,787 rival observations with a
positive public city total, 1,637 exported fewer city locations than the public
total and 438 exported none. Mean public cities were 8.09 versus 3.18 exported
locations. This is a census of host exports, not a measurement of final
forecast accuracy: the reconstruction can retain map knowledge across frames.
The outcome replays below measure the actual estimator separately.

The final replay covers **66 completed six-player Deity Online games**, all
with a host cap of 500, including 53 Science and 13 Culture outcomes. Two of
the original 68 input directories became unavailable during the run; they
are named in the results file and excluded before scoring. No coefficients
were fitted to these outcomes.

| Forecast turn | Games still live | Brier before → after | Winner log loss before → after | Actual winners given 0%, before → after |
|---|---:|---:|---:|---:|
| 50 | 66 | 0.24787 → 0.13145 | 21.02848 → 1.58537 | 50 → 0 |
| 100 | 66 | 0.18217 → 0.12652 | 15.82888 → 1.51273 | 37 → 0 |
| 150 | 65 | 0.13280 → 0.11765 | 6.10540 → 1.36793 | 12 → 0 |
| 175 | 48 | 0.10427 → 0.08631 | 3.21331 → 0.97490 | 4 → 0 |

Lower is better for both errors. Game-bootstrap 95% intervals for the Brier
difference exclude zero at turns 50, 100 and 175; the turn-150 interval crosses
zero. Log-loss intervals exclude zero at all four turns. Winner log loss caps
zero probabilities at `1e-12`, so the old false exclusions dominate that
metric. Favorite accuracy does not improve at every phase: at turn 150 it
falls from 41.5% to 36.9%, with a wide interval spanning zero. Confidence
correction is the principal gain.

The compressed pre-outcome forecasts and exact paired report are committed
beside this note as `2026-11-09-win-probability.replays.jsonl.gz` and
`2026-11-09-win-probability.results.json`. The reference forecast agrees
bit-for-bit with the initial city-only replay's control on all 66 shared
games. These are fixed-outcome replays, not new gameplay wins.

The coefficients have not been promoted from an in-sample fit. The existing
four-player calibration does not establish accuracy for every controller,
board, difficulty, victory lane, or real-game adapter. Neither this audit nor
an improvement in prediction alone establishes a gameplay win-rate lift.
An exploratory 48-game Basic-AI cohort favored a flatter clock multiplier in
training, but its nine-game validation split had mixed phase results and wide
intervals. It does not justify promoting new global coefficients.

## Reproduce

`odds_export` is a developer tool, excluded from ordinary product builds:

```sh
cargo build --profile ci --locked --features developer-tools --bin odds_export
target/ci/odds_export --games 48 --jobs 4 --start-seed 940000 > odds.jsonl
python3 tools/odds_report.py odds.jsonl
```

Flags explicitly select the controller (`basic` or `advanced`), player count,
map dimensions, city-state count, map, speed, turn limit, difficulty, sample
stride and optional human seat. Civilizations are shuffled, mercy is off,
and every exported game is played to its real outcome. One JSONL record holds
every seat and sample of one game, so fitting and holdouts can split by whole
game. Reusing a seed across inputs is rejected.

For a configured, completed, stock free-for-all host attempt whose local seat
is zero:

```sh
target/ci/odds_export --replay /path/to/civvis-RUN --every 25 > replay.jsonl
python3 tools/odds_report.py replay.jsonl --baseline legacy_now --candidate now \
  --phases .1,.2,.3,.35
```

The replay uses the board and state at each selected turn, with the original
outcome held fixed. Its control selects the former located-city treatment,
including missing-rival exclusions, while leaving other observations intact.
It does not issue orders or alter the recorded game.
Free-for-all winner team IDs must match host player IDs; other team contracts
are outside this replay instrument.

The report scores all-seat mean Brier error and winner log loss, alongside
favorite accuracy, descriptive reliability bins, and winners assigned zero
probability. Comparisons use identical surviving games at each phase. Paired
95% intervals resample games, preserving the correlation among their seats.
Finished games are not carried forward with known outcomes. Compressed JSONL
evidence can be read directly.

## Regression coverage

The odds tests cover an entirely unlocated host empire through the real
`rebuild_from_state` adapter, false mercy, public totals, confirmed defeats,
missing exact counts, stale public zeros, handicap exemptions, immutable
scheduled-handicap priors, and speed-equivalent unlimited worlds.

The report tests check outcome scoring, chronological sampling, exclusion of
terminal observations, duplicate maps, missing outcomes, invalid probability
distributions, reliability endpoints, zero-probability winners, empty series,
and compressed evidence.
