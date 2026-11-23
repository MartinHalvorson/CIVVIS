# The speed timeline

How many times faster than a person a game of Civilization VI gets played.
The home page shows it over its videos: one number, and under it a line with
five ways of playing one game, each standing at its multiplier.

The readings are `data/performance/speedup_timeline.json`. The page's figures
are worked out from them by `beta/speedup_timeline.py` when the site is
published. Nothing derived is committed.

The CIVVIS Performance page opens on the same readings as two tables, each
figure beside the sum that gives it. They are described under
[The tables on CIVVIS Performance](#the-tables-on-civvis-performance). A
third table there puts the same benchmarks on other processors:
[Processor by processor](#processor-by-processor). A fourth estimates what
renting the rest of the way to each goal would cost:
[The goals and the cloud's cost](#the-goals-and-the-clouds-cost).

## The five stops

The simulator's game is six players and nine city-states on a 74x46
Continents map at Online speed, played to a victory or to turn 250. The
verification games in Civilization VI are six players on Continents as well.
The first stop is the one every other is divided into.

| Stop | What plays the game | How it is timed |
| --- | --- | --- |
| Human player (`hand`) | One person, in Civilization VI. | The project owner's own figure for a game: five hours. |
| CIVVIS computer control (`control`) | CIVVIS, at the controls of the real Civilization VI, against its Deity AI. | The middle game of the complete Lv 8 verification games in `results.jsonl` on the `ledger` branch, each timed from the stamp in its tag to the stamp of its result. |
| Firaxis Civ 6 Deity sim (`deity_sim`) | The simulator, one core: one CIVVIS seat against five Deity rivals that play as Civilization VI's AI does (`--rivals firaxis-faithful`). | The mean of twelve games, `gene_screen --jobs 1` with the Deity field's flags, from each game's `secs`. |
| CIVVIS self-play (`self_play`) | The simulator, one core: every seat a CIVVIS AI agent. | The mean of twelve games, `gene_screen --jobs 1` with its defaults, from each game's `secs`. |
| M5 Max, all 18 cores (`all_cores`) | Self-play on every core of one machine. | Sixty seconds over the record of `data/performance/games_per_minute.json` (`docs/GAMES_PER_MINUTE.md`): the time from one finished game to the next. |

The last stop is a rate, not one game's time: eighteen games are in play at
once and one finishes every few seconds. It is the stop the page's number is
read from, because it is what one machine delivers.

## The number

A stop's multiplier is the first stop's seconds over its own. The page's
number is the largest. The line is a scale of multipliers by tenfolds, 1x at
its right end and the largest goal at its left, so a stop stands at
`log(multiplier) / log(largest goal)` of the way across. The marks from a
million up are worded (`1 million x`), so that ten of them fit along the line.

The line runs far past the fastest stop, so the stops stand in a short
stretch of it. Words that would meet their neighbour's on the same side of
the line stand a row further out, at the end of a rule from the stop's point
(`speedup_timeline.rows`). The line is drawn from a window 1120px wide, where
it is at its full length. A narrower window shows the stops as a list.

## The goals and the cloud's cost

The ledger's `goals` are the multipliers the line is marked with, each under
a `key` the pages name it by. There are two. 1,000,000x is a game every
18 ms, which is 3,333 games a minute. 1,000,000,000x is a game every 18 µs,
which is 3.3 million games a minute, and it is the line's left end. Two stops
have goals of their own as well, a self-play game in 10 s and a game a second
on every core (18,000x).

Each goal says what renting the rest of the way to it would cost. This is an
estimate and not a measurement: nothing has been run in a cloud. The pages
say so, and say who made it. The project owner asked for it as "a Fable 5.2
best estimate of the numbers", and the entry in the ledger was written by
that model. It is the newest entry of the ledger's `cloud`:

- `by` is who made the estimate and `taken` the day.
- `machine` is the machine rented and `cores` its cores.
- `of` is the one-core stop whose game those cores play.
- `single_thread` is the score of the machine's processor on the website of
  `data/performance/processors.json`.
- `dollars_an_hour` is its list price for an hour, and
  `spot_dollars_an_hour` its spot price, if it has one.
- `from` says where each figure was read and what the estimate leaves out.

`speedup_timeline.rented` works the cost out, and the Performance page shows
each step with its sum:

| Step | The sum |
| --- | --- |
| One game on one rented core | `the one-core reading's seconds × the measured processor's single thread score ÷ the rented one's` |
| One rented machine | `cores × 18,000 s ÷ the seconds of one game on one of them` |
| The rest of the way | `the goal − the fastest stop's multiplier` |
| Machines | `the rest ÷ one machine`, a whole machine for any part of one |
| Cost for an hour | `machines × the price for an hour` |
| Cost for a year | `the cost for an hour × 8,760 h` |

The first step is the sum the processors' table uses for a processor the
benchmarks were not run on. The second takes every core to play as fast as
one, which the every-core run on the M5 Max comes close to.

The estimate leaves out storage, network traffic and the work of running a
fleet. It assumes the price holds at any number of machines. The larger goal
needs tens of thousands of them, and no one has tested that such a number
can be rented at the list price, or at all.

To change the estimate, add a newer entry to `cloud` after the ones there:
another machine, a price read again, or a score with more samples behind it.
A goal is added to `goals` with a key, and an entry for it to the home
page's line (`beta/landing.html`, by `data-goal`).

## The tables on CIVVIS Performance

The Performance page (`beta/performance.html`) answers one question at its
top: how many times faster than a game played by hand. It names the three
simulator-side stops as the project owner does: Deity computer control
(`control`), Deity self-play (`deity_sim`) and CIVVIS self-play (`self_play`).
`speedup_timeline.worked` builds both tables from the ledger, and
`python3 beta/speedup_timeline.py --worked .` prints them.

**One game at a time** has a row for every stop with readings of its own. The
multiplier is the baseline's seconds over the row's, and the row shows that
sum:

| Row | The sum |
| --- | --- |
| By hand | `5 h = 18,000 s`, the baseline |
| Any other | `18,000 s ÷ the seconds one game takes` |

**One machine, every core** holds the experimental throughput against the
theoretical one, and then against the goals.

| Kind | What it is | The sum |
| --- | --- | --- |
| Experimental | A run: the every-core record (`all_cores`), games counted as they finish. | `games a minute × 60 × 5 h` |
| Theoretical, one core | A calculation: every core of that machine as fast as the stop's one-core reading. | `cores × 18,000 s ÷ the seconds one game takes on one core` |
| Theoretical, alone | The same, at the speed of one game that had the machine to itself, playing games as long as the run's. | `cores × (turns ÷ seconds, alone) × 18,000 s ÷ the run's turns a game` |
| Goal | The run's own goal, then each goal of the line. | `18,000 s ÷ the goal's seconds` |

Games an hour times the hours one game takes by hand is the multiplier, so
the two columns say the same thing. A theoretical row also gives the share
of it the measured run reaches.

The two theoretical rows for self-play differ because of where their speed
was measured. The one-core reading was taken on the host as it works, beside
a live game and the tournament, so eighteen times it comes out close to the
run. A game alone on the machine is about half again as fast, and eighteen
times that is the ceiling the run is held against. No machine reaches it
whole: the run's workers had 14.7 of the 18 cores, and twelve of the
eighteen are of the slower class (`docs/GAMES_PER_MINUTE.md`). Deity
self-play has a theoretical row and no experimental one, because no run has
played it on every core.

Three ledger keys are read by these tables alone:

- `cores: 1` on a stop marks the simulator on one core. Only such a stop gets
  theoretical rows.
- `of` on a stop that follows a benchmark names the one-core stop whose game
  the benchmark plays. The machine's core count, its chip and the turns its
  games ran are read from the record run.
- `alone` on a one-core stop is a list of readings of one game played with
  the machine to itself: `taken`, the `turns` it ran, the `seconds` it took,
  and `from`. The newest is used.

## Processor by processor

The third table on the Performance page lists the benchmarks across
processors: the one they were run on, and others they have not been run on.
The processors are `data/performance/processors.json`, and
`speedup_timeline.processors` works out the rows.

Each processor has two scores from PassMark's CPU benchmark charts
(cpubenchmark.net): the Single Thread Rating and the Multithread Rating of
its page there. A score is the website's average over the machines its users
tested, so the list keeps the number of samples and the margin for error the
page gives beside it.

A row is one of two kinds.

| Kind | What it is | The sums |
| --- | --- | --- |
| Experimental | The processor the benchmarks were run on. Its one-core games are the readings of the timeline, and its games a minute are the every-core record among the runs on its chip. | None: the figures are counted. |
| Theoretical | A processor the benchmarks were not run on. A calculation from the two scores, not a run. | One game on one core: `measured seconds × measured single thread score ÷ its own`. Every core: `measured games a minute × its multithread score ÷ the measured one`. |

The multiplier is the row's games a minute, times 60, times the hours one
game takes by hand, as in the table above it.

A theoretical row says what the scores predict. PassMark's tests are not
this simulator, and a score is an average over other people's machines, so
the row stands until the benchmark is run on that processor. When
`tools/games_per_minute.py --record` is run on a listed processor, its row's
every-core figure becomes the run's: the tool writes the chip's name into
the run, and a processor whose `chip` matches it shows the record among its
runs. Its one-core games stay worked out until the one-core readings are
taken there too.

The age in the last column is counted by the page on the day it is read,
from the oldest reading a row's figures rest on. A theoretical row is as old
as the measured readings it is worked out from.

The list's keys:

- `scores` names the website: `site`, `address`, what it calls each score,
  and `from`, how a score is read there.
- `measured` is the key of the processor the one-core readings were taken on.
- A processor is a `key`, the `name` the table shows, `cores`, `threads`, and
  `scores`: dated readings of its page, oldest first, the newest shown.
  A reading is the day it was `read`, `single_thread`, `multithread`,
  `samples`, `margin` and the `page`.
- `chip` is the name `tools/games_per_minute.py` gives the processor in a
  recorded run (`machine.chip`). `fleet` is the machine's ID in
  `docs/MACHINES.md` when the project has one.

To add a processor, add an entry with one reading of its page. To refresh a
score, add a newer reading after the ones there. The page of a processor is
found from `https://www.cpubenchmark.net/cpu.php?cpu=<its name there>`, with
`+` for each space.

## Adding a reading

A reading is one line in a stop's `readings`, after the ones already there:
the day it was `taken`, the `seconds` one game takes, and `from`, which says
how it was measured closely enough to measure it again. The newest reading is
the one shown. The older ones stay: they are the track of how far a stop has
been pushed, and the page says where a stop started once it has moved.

`all_cores` has no readings to add. Record a run with
`tools/games_per_minute.py --record` and the home page follows it when it is
a record.

The two simulator stops, on a build of `gene_screen` (`cargo build --release
--features developer-tools --bin gene_screen`):

```bash
# The Deity sim: twelve games on one core.
gene_screen --games 12 --target-games 12 --start-seed 9101 --jobs 1 \
  --players 6 --width 74 --height 46 --city-states 9 --map continents \
  --difficulty deity --rivals firaxis-faithful --handicap rivals \
  --rival-chairs 5 --rival-rung -2 --rival-pace --out deity.jsonl --quiet

# Self-play: twelve standard games on one core.
gene_screen --games 12 --target-games 12 --start-seed 20261122000000 --jobs 1 \
  --out self.jsonl --quiet
```

Each game writes one row per seat with the game's `secs`. The reading is the
mean over the games. Say in `from` what else the machine was doing: a game on
a busy host takes longer than the same game alone.

A game alone on the machine is one more reading, in the stop's `alone`: one
`gene_screen --jobs 1` game with nothing else running, its turns and its
seconds. One seed is a thin reading; the mean of several is a better one.

`python3 beta/speedup_timeline.py .` prints what the home page will show, and
`python3 beta/speedup_timeline.py --worked .` the Performance page's tables,
the processors among them.
