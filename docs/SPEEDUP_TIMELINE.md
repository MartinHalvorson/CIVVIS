# The speed timeline

How many times faster than a person a game of Civilization VI gets played.
The home page shows it over its videos: one number, and under it a line with
five ways of playing one game, each standing at its multiplier. Past them
stand two stops nothing has played yet:
[The stops past what was measured](#the-stops-past-what-was-measured).

The readings are `data/performance/speedup_timeline.json`. The page's figures
are worked out from them by `beta/speedup_timeline.py` when the site is
published. Nothing derived is committed.

The CIVVIS Performance page opens on the same readings as two tables, each
figure beside the sum that gives it. They are described under
[The tables on CIVVIS Performance](#the-tables-on-civvis-performance). A
third table there puts the same benchmarks on other processors:
[Processor by processor](#processor-by-processor). A fourth estimates what
renting the rest of the way to the goal would cost:
[The goals and the cloud's cost](#the-goals-and-the-clouds-cost). Under the
tables each stop is charted over time: [Each stop over time](#each-stop-over-time).

## The five stops

The project owner named them on 2026-11-24: "we should have: human player 5
hrs, Firaxis Computer control: 10-15mins but we should benchmark this with
our games, Firaxis self-play (test how long this take), CIVVIS self-play,
"Parallel games, M5 Max, all 18 cores"." The two Firaxis stops are the real
game. The two after them are the simulator.

The simulator's game is six players and nine city-states on a 74x46
Continents map at Online speed, played to a victory or to turn 250. The
verification games in Civilization VI are six players on Continents as well.
The first stop is the one every other is divided into.

| Stop | What plays the game | How it is timed |
| --- | --- | --- |
| Human player (`hand`) | One person, in Civilization VI. | The project owner's own figure for a game: five hours. |
| Firaxis computer control (`control`) | Civilization VI itself, Firaxis's game, with CIVVIS at the controls of one seat. | The middle game of our own complete Lv 8 games of the last seven days in `results.jsonl` on the `ledger` branch, each timed from the stamp in its tag to the stamp of its result: launching the game, setting it up, playing it and its ending. |
| Firaxis self-play (`firaxis_self_play`) | Civilization VI itself with Firaxis's own AI in every seat. | Not timed yet. The reading is worked out from our own games: the setup, the turns Firaxis's AI takes and the ending of a computer control game, with a sixth AI seat in place of ours. See [A reading that was worked out](#a-reading-that-was-worked-out). |
| CIVVIS self-play (`self_play`) | The simulator, one core: every seat a CIVVIS AI agent. | The mean of twelve games, `gene_screen --jobs 1` with its defaults, from each game's `secs`. |
| Parallel games, M5 Max, all 18 cores (`all_cores`) | CIVVIS self-play on every core of one machine at once. | Sixty seconds over the record of `data/performance/games_per_minute.json` (`docs/GAMES_PER_MINUTE.md`): the time from one finished game to the next. |

The last stop is a rate, not one game's time: eighteen games are in play at
once and one finishes every few seconds. It is the stop the page's number is
read from, because it is what one machine delivers.

One more stop is kept in the ledger and off the line (`"line": false`):
`deity_sim`, the simulator on one core with a CIVVIS seat against five Deity
rivals that play as Civilization VI's AI does. It stood on the line as
"Firaxis Civ 6 Deity sim" until the real game's self-play was named as the
stop. The Performance page's tables still show it, as "CIVVIS against Deity
rivals".

### A reading that was worked out

A reading with `"worked": true` was worked out from other measurements and
not timed itself. The home page draws its stop as a broken ring and says
"worked out, not yet timed", and the Performance page marks its row. A timed
reading added after it takes its place, and the mark goes with it.

Firaxis self-play is the one such reading. The control harness
(`tools/civ6_play.py`) always plays our seat, so it cannot start a game that
leaves every seat to the game's own autoplay, and the older grounding harness
(`tools/civ6_run.py`) can only start the game's default setup. Until one of
them can, the figure is this sum over our newest computer control games:

| Part of the middle game | Seconds |
| --- | ---: |
| Setup, from launching Civilization VI to the first turn | 47.8 |
| Our own seat's turns | 238.7 |
| The turns of Firaxis's AI: five seats, the city-states, the barbarians | 409.4 |
| The ending | 9.9 |

A game of the same length with a sixth Firaxis seat in place of ours is the
setup, the AI's turns and the ending: 467 s if the sixth seat cost nothing,
and 549 s if it cost a fifth more. The slower figure is the reading.

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

## The stops past what was measured

The project owner, 2026-11-22: "add to out speedup timeline: 10x theoretical
scale up from local hardware ... hardware scaleup is to the left of m5 max.
potential distributed RL fleet: up to 1000+x".

The ledger's `projected` are stops nothing has run at. Each is a measured
stop's multiplier times one figure, so it moves when that stop does.

| Stop | What it is | The sum |
| --- | --- | --- |
| Hardware scale-up (`scale_up`) | Larger hardware of our own than the one M5 Max. Theoretical. | `10 × the M5 Max's multiplier` |
| Distributed RL fleet (`fleet`) | Many machines playing self-play at once for reinforcement learning. A potential, and open at its top. | `1,000 × the M5 Max's multiplier`, and more |

Both figures are the project owner's and both are over the M5 Max's
every-core stop. No machine was sized or priced for either. Whether his
1,000 is over the one M5 Max or over the scaled-up hardware is this page's
reading of his sentence, and it reads the first.

On the home page they are rings in the line's own colour, under the line and
to the left of the M5 Max. The page's number is never one of them: it is the
fastest stop that was measured, and the line is drawn solid only as far as
that stop. A thousand M5 Maxes are past the 1,000,000x the line ends at, so
the fleet stands at the line's end as an arrow that points on from it
(`past` in the slot), with its own figure beside it.

A projected stop is an entry of `projected`:

- `key` is the name the page knows it by.
- `over` is the key of the measured stop it multiplies.
- `estimates` are dated, oldest first, and the newest is shown. Each has the
  day it was `taken`, the figure (`times`), `or_more` when the figure is open
  at its top (his "1000+", shown as `1,000x+`), and `from`, which says whose
  figure it is and what it stands for.

A changed figure is a new estimate after the ones there, as a new reading
is. `speedup_timeline.projected` works the rows out, and
`speedup_timeline.timeline` returns them as `projected`. The Performance
page's tables do not show them.

## The goals and the cloud's cost

The ledger's `goals` are the multipliers the line is marked with, each under
a `key` the pages name it by. There is one: 1,000,000x, a game every 18 ms,
which is 3,333 games a minute, and it is the line's left end. A billion
stood beside it from 2026-11-22 until the project owner took it off on
2026-11-24: "Change the timeline to just scale to 1,000,000x (drop the
1bX)." Two stops have goals of their own as well, a self-play game in 10 s
and a game a second on every core (18,000x).

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
- `scaling`, when it is `measured`, says a machine's cores are not taken to
  play as fast as one core does alone (see below).
- `from` says where each figure was read and what the estimate leaves out.

`speedup_timeline.rented` works the cost out, and the Performance page shows
each step with its sum:

| Step | The sum |
| --- | --- |
| One game on one rented core | `the one-core reading's seconds × the measured processor's single thread score ÷ the rented one's` |
| The share of its cores a machine delivers | `the every-core record's games a minute ÷ (its cores × 60 s ÷ the one-core reading's seconds)` |
| One rented machine | `cores × 18,000 s ÷ the seconds of one game on one of them × that share` |
| The rest of the way | `the goal − the fastest stop's multiplier` |
| Machines | `the rest ÷ one machine`, a whole machine for any part of one |
| Cost for an hour | `machines × the price for an hour` |
| Cost for a year | `the cost for an hour × 8,760 h` |

The first step is the sum the processors' table uses for a processor the
benchmarks were not run on.

The second step is what changed when the project owner asked for the cost to
be worked out again (2026-11-24: "reclauclate the cloud costs to make sure
they are accurate"). The first estimate took every core of a rented machine
to play as fast as one core alone. The only measurement of that is our own
every-core run, and it falls short: eighteen busy cores finish fewer games
than eighteen times one core. So the estimate of 2026-11-24 takes a rented
machine to reach the same share of its cores that the M5 Max reached of its
own. That may be hard on the rented machine, whose cores are all of one
speed and have nothing else to do, and it is the cautious side to err on. An
entry without `scaling` is still worked out the first way.

What was checked that day: the list price (unchanged), the spot price (two
cents higher), and the processor's single thread score, which still rests on
two samples. Two processors of the same generation with many samples, scaled
to the rented one's top speed, land within two parts in a hundred of it.

The estimate leaves out storage, network traffic and the work of running a
fleet, and it assumes the price holds for the machines it counts.

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

The two theoretical rows for self-play differ because of where and when
their speed was measured. The one-core reading is taken on the host as it
works, beside a live game and the tournament. A game alone has the machine
to itself, and eighteen times its speed is the ceiling a run is held
against. No machine reaches it whole: the run's workers had 14.7 of the 18
cores, and twelve of the eighteen are of the slower class
(`docs/GAMES_PER_MINUTE.md`). Deity self-play has a theoretical row and no
experimental one, because no run has played it on every core.

Each row carries the day of its reading, and the rows need not be of one
build. The one-core readings of 2026-11-24 are two days newer than the
every-core run and the game alone, and the simulator got faster in between,
so eighteen times the one-core reading stands above both and the share the
run reaches of it is short by that much. A new every-core run and a new game
alone on the same build put the three back in step. Both need the machine to
itself.

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

## Each stop over time

The project owner, 2026-11-24: "in civvis performance, we should have charts
studying the improvements rates of each of these over time".

The Performance page has a small chart for each stop after the baseline
(`speedup_timeline.history`, `python3 beta/speedup_timeline.py --history .`).
A chart draws every reading of its stop by the day it was taken, on a scale
of seconds that goes by tenfolds, so a steady pace of improvement is a
straight line. All the charts share one run of days. Each has its own scale,
because a 12-minute game and a 3-second one do not fit one.

Over each chart stands the pace its readings have kept
(`speedup_timeline.pace`): how many times faster the newest is than the
first, over how long, and from a week's span up the time in which a game's
time halves at that pace. Readings within two parts in a hundred of each
other are called steady.

| Stop | What its chart draws |
| --- | --- |
| Firaxis computer control, Firaxis self-play | The stop's readings in the ledger. |
| CIVVIS self-play | Its one-core benchmark readings, and beside them the self-play tournament's finished batches, the middle game of each on the day it ended (`data/performance/self_play_times.json`; `"tournament": true` on the stop). The tournament plays the same game seven or more at a time on a busy machine, so it reads slower, and it reaches back to October. |
| Parallel games | Every run of `data/performance/games_per_minute.json`. The pace is kept over the records among them. |

So a stop's chart grows by itself with every reading added to the ledger and
every run recorded.

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
# CIVVIS against Deity rivals (off the line): twelve games on one core.
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
