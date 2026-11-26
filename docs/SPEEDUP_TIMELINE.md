# The speed timeline

How many times faster than a person a game of Civilization VI gets played.
CIVVIS Performance shows it in the middle of the page, under its tables of
sums and over its charts of each stop over time, under the title the project
owner gave it on 2026-11-24, "Civ Game Speedup - Synthetic Data Pipeline":
one number, and under it a line with five ways of playing one game, each
standing at its multiplier. It stood on the home page, over the videos, until
later that day ("move the civvis performance timeline thing from home/ to the
civvis performance section. middle of the page or so"). Among and past them stand four stops nothing has
played yet:
[The stops past what was measured](#the-stops-past-what-was-measured).
Left of CIVVIS self-play the games play many at once, and a time there is
amortized: [Amortized games](#amortized-games).

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
| Human Player (me) (`hand`) | One person, in Civilization VI: the project owner. | His own figure for a game: five hours. The stop says so under its figures, in his words: "Some are faster, some are slower. I'm a bit on the slower side but 5h seems reasonable." |
| CIVVIS → Firaxis computer control (`control`) | Civilization VI itself, Firaxis's game, with CIVVIS at the controls of one seat. | The computer control benchmark (`beta/control_times.py`, [`docs/CONTROL_GAME_TIME.md`](CONTROL_GAME_TIME.md)): the middle game of the games played to a victory in the last seven days, each timed end to end, from launching the game to its ending. The stop names it (`"benchmark": "control"`), so a finished game moves the stop with the next publish. A clone without the run ledger shows the newest reading written in the ledger instead. |
| Firaxis self-play (`firaxis_self_play`) | Civilization VI itself with Firaxis's own AI in every seat. | One game played by the verification lane between two of its own (`tools/civ6_play.py --firaxis-self-play`): the lane's setup at Lv 8, our seat handed to the game's autoplay manager on its first turn. Timed end to end by the harness's run clock, the clock the computer control games are timed by. See [Timing a Firaxis self-play game](#timing-a-firaxis-self-play-game). |
| CIVVIS self-play (`self_play`) | The simulator, one core: every seat a CIVVIS AI agent. | The mean of twelve games, `gene_screen --jobs 1` with its defaults, from each game's `secs`. |
| Parallel games, M5 Max, all 18 cores (`all_cores`) | CIVVIS self-play on every core of one machine at once. | Sixty seconds over the record of `data/performance/games_per_minute.json` (`docs/GAMES_PER_MINUTE.md`): the time from one finished game to the next. |

The last stop is a rate, not one game's time: eighteen games are in play at
once and one finishes every few seconds. It is the stop the page's number is
read from, because it is what one machine delivers.

The second stop's name is the project owner's of later that day: "lets also
call Firaxis computer Control inside CIVVIS->Firaxis computer control". His
arrow is drawn as one, on the line and in the tables over it.

## Amortized games

The project owner, 2026-11-24: "starting left of CIVVIS self-play (but not
including that), we should note that estimates are amortized games". And the
same day: "on the left side of the timeline, at more than 1 game / second, we
should also provide the games per second as the standard measure and (time
per game amoritized as the second measure)".

Up to CIVVIS self-play a stop's time is how long one game takes. Left of it
the games play many at once, and a stop's time is the stopwatch's time over
the games finished: the time from one finished game to the next. No single
game is that fast. The ledger marks those entries `amortized`: the every-core
stop, the 1,000,000x goal, and a projected stop that plays many games at once.
A projected stop that multiplies an amortized stop is amortized with it.

An amortized entry has two measures, its games per second (the multiplier
over the baseline's seconds) and its time per game. It leads with its games
per second, and its time per game stands in brackets under them with the word
amortized: `4.1 games/s`, `(247 ms a game, amortized)`. That holds under a
game a second as well, so the M5 Max as it runs today reads `0.41 games/s`,
`(2.5 s a game, amortized)`, and its goal is worded the same way, `goal 1
game/s`. Until 2026-11-24 an entry under a game a second led with its time;
the project owner asked for the M5 Max's stop to lead with its games that
day: "lest have m5 max blurb list amortiized games per second as the primary
unit. as this is really amortiization at play".

`speedup_timeline.amortize` works both out (`rate`, and `lead`, which says the
games lead). Under the line the page says once what the word means. The irreducible
game, below, is left of CIVVIS self-play and is not amortized: it is one game
on one core.

One more stop is kept in the ledger and off the line (`"line": false`):
`deity_sim`, the simulator on one core with a CIVVIS seat against five Deity
rivals that play as Civilization VI's AI does. It stood on the line as
"Firaxis Civ 6 Deity sim" until the real game's self-play was named as the
stop. The Performance page's tables still show it, as "CIVVIS against Deity
rivals".

### A reading that was worked out

A reading with `"worked": true` was worked out from other measurements and
not timed itself. The line draws its stop as a broken ring and says
"worked out, not yet timed", and the tables over it mark its row. A timed
reading added after it takes its place, and the mark goes with it.

Firaxis self-play began as such a reading on 2026-11-24, 549 s, worked out
from the turns Firaxis's AI takes in our own computer control games. The same
day the lane played one for real, and the timed reading took its place:
331.9 s. The sum had been too slow by two thirds. With no seat of ours in the
game, a turn took 1.61 s, where the AI's share of a turn in our games is
2.1 s.

### Timing a Firaxis self-play game

The control harness plays one when asked (`--firaxis-self-play`, #5795): the
lane's own setup, with our seat handed to the game's autoplay manager on its
first turn and no order issued. The lane starts it from a one-shot marker:

```bash
touch ~/.civvis-firaxis-self-play-once
```

At its next game boundary the lane removes the file, plays one self-play
game, and goes on to its ordinary game. The run is kept beside the lane's
own, in `~/civvis-civ6-runs/firaxis-selfplay/<tag>/summary.json`, under
`firaxis_self_play`: whether the autoplay took the seat, how the game ended
and on which turn, `play_seconds` from the first turn to the victory,
`run_seconds` for the whole run, and the machine's load at both ends. It is
not a game of ours, so it writes no ladder row and nothing to the run ledger.

The reading in the ledger is `run_seconds`, the whole run, so that it stands
beside the computer control games' end-to-end time. The first game, run
`firaxis-selfplay-20261124T174852Z`:

| Part | Seconds |
| --- | ---: |
| Setup, from launching Civilization VI to the first turn | 48.0 |
| Play, 171 turns to a science victory | 275.2 |
| The ending | 8.7 |
| The whole run | 331.9 |

It is one game. A game's length moves with the turn it is won on (our own
Lv 8 games end between turn 135 and turn 219), so a few more games would
give a middle one.

## Why CIVVIS self-play is faster than Firaxis self-play

The project owner, 2026-11-24: "what is the main reason civvis is so much
faster than firaxis civ 6? please try to explain that gap in the
chart/timeline".

The two stops stand 9.6 times apart: 331.9 s for a real game with Firaxis's
AI in every seat, 34.5 s for a simulator game. On the line the stretch
between them is drawn in gold with that figure on it, and the figure is a
link to the part under the line, where the two games are bars to one scale.
The real game's bar is in three parts. Each accounts for a factor of the gap,
and the three multiply to it:

| Part of the real game | Seconds | Left without it | Factor |
| --- | ---: | ---: | ---: |
| Launching Civilization VI, its menus, loading the map, and the ending | 56.7 | 275.2 | 1.2x |
| The play, its game thread off the processor | 130.0 | 145.2 | 1.9x |
| The play, its game thread on a processor | 145.2 | the simulator's 34.5 | 4.2x |

So the main reason is the turn itself. Firaxis's game thread is on a
processor for 0.85 s of an average 1.61 s turn, and the simulator's whole
turn, six AI seats and the rules, takes 0.178 s. The second reason is that
the same thread is off the processor for nearly half of the play: it is one
thread of an application, parked until the rest of it wakes it. The third is
that a real game has to be launched, set up and loaded, and a simulator game
begins at once.

Three pairs of figures stand under the bars:

| | Firaxis self-play | CIVVIS self-play |
| --- | ---: | ---: |
| One turn | 1.61 s | 178 ms |
| Processors kept busy while it plays | 3.2 | 1 |
| Game thread on a processor | 53% of the play | 99% |

The second is why the last stop of the line is so far from this one: a
simulator game needs one core, so eighteen play at once on the M5 Max, where
one real game keeps more than three busy.

### How it is read

The real game's source is not ours, so its time is read from outside the
process as it plays, by `tools/civ6_thread_time.py`:

```bash
touch ~/.civvis-firaxis-self-play-once                  # the lane plays one at its next game boundary
tools/civ6_thread_time.py record PID > threads.jsonl    # PID of Civ6_Exe_Child, as soon as it is up
sample PID 5 1 -file stacks-1.txt                       # a few times while it plays
tools/civ6_thread_time.py read threads.jsonl ~/civvis-civ6-runs/firaxis-selfplay/<tag>
tools/civ6_thread_time.py stacks stacks-*.txt
```

`record` asks the kernel once a second how much processor time each thread
has used so far. Those are running totals, exact over any stretch, and
reading them does not stop the game. `read` takes the stretch from the first
turn the recording covers to the last. The game names its own threads:
`Game Core` plays the rules and the AI, `WinMain` is the window's, which
draws the frame and runs the interface, and the `TBB` threads are workers
either hands jobs to. `stacks` reads macOS's `sample` and says in how many
samples a thread was parked until woken and in how many it was passing its
turn while it waited on a job. The second is counted as processor time and
is not work.

The reading is the ledger's `gap.threads`, from the run
`firaxis-selfplay-20261125T034803Z` (2026-11-25, the lane's setup as for the
first game, a science victory on turn 188), over turns 30 to 188:

| Over 491.1 s of play | Processor seconds | Of one processor |
| --- | ---: | ---: |
| The whole process | 1,592.9 | 324% |
| Game Core | 259.1 | 53% |
| WinMain | 407.4 | 83% |
| Four TBB workers | 739.2 | 151% |

In three five-second samples of the stacks, on turns 46, 102 and 179, the
game thread was parked until woken in 1,338 of 4,489 samples (30%) and
passing its turn in 238 (5%). The window's thread was never parked and was
passing its turn in 2,457 of its 4,489 (55%), so half of its 83% is waiting
on jobs too.

What the page draws from it (`speedup_timeline.gap`, the record's
`timeline.gap`): the slower stop's newest reading gives the times (its
`parts`: 48.0 s to the first turn, 275.2 s of play, 8.7 s of ending, and its
`turns`), and `gap.threads` gives the game thread's share of the play,
259.1 / 491.1. The simulator's reading gives its `turns` and its
`cpu_seconds`. No share and no factor is written in the ledger.

### What it does not say

- **Why Firaxis's turn takes that much more work.** The two are not the same
  program doing the same sums: the simulator is its own engine with its own
  agents, following the game's rules. The measurement says how much
  processor time each spends on a turn and not what Firaxis's is spent on.
- **The share on a quiet host.** The second game was not a timing. The host
  was busier than for the first (load 22 to 25, where the first had 13) and
  the game was sampled as it played: it took 600.3 s, 63.5 s to its first
  turn, 529.0 s of play at 2.81 s a turn and 7.8 s of ending. A busy host
  also keeps a thread off a processor, so some of the 47% is waiting for a
  core and not for the application. The stacks put the waiting to be woken
  at 30%. With that in place of 47% the factors would be 1.2x, 1.4x and
  5.6x: the turn itself is the main reason either way.
- **The first 29 turns.** The recording began on turn 30.

A newer reading of either stop moves the bars with it. A new thread reading
replaces `gap.threads`.

## The number

A stop's multiplier is the first stop's seconds over its own. The page's
number is the largest. The line is a scale of multipliers by tenfolds, 1x at
its right end and its end at its left, so a stop stands at
`log(multiplier) / log(end)` of the way across. The marks from a
million up are worded (`1 million x`), so that ten of them fit along the line.

The line's end is the largest goal, or the ledger's `line_to` where that is
further out (`speedup_timeline.end`). It is 10,000,000x, a tenfold past the
one goal, since the project owner asked on 2026-11-24: "maybe extend timeline
out to 10,000,000x". `line_to` is no goal: nothing is aimed at it and nothing
is costed to it. It is seven tenfolds of line where there were six, so every
entry stands a seventh nearer its neighbours.

The line runs far past the fastest stop, so the stops stand in a short
stretch of it. Words that would meet their neighbour's on the same side of
the line stand a row further out, at the end of a rule from the stop's point
(`speedup_timeline.rows`). How far out that is, and how much room each side
of the line takes, the page measures from the words once they are written in
(`fit` in `beta/performance.html`): a name is one line or three, and a stop may
have a second measure and a goal under its figures. The line is drawn from a window 1120px wide, where
the panel it stands in is 1000px or more inside, the length the words were
given their room on (`speedup_timeline.ROOM`). A narrower window shows the
stops as a list.

## The stops past what was measured

The project owner, 2026-11-22: "add to out speedup timeline: 10x theoretical
scale up from local hardware ... hardware scaleup is to the left of m5 max.
potential distributed RL fleet: up to 1000+x".

And on 2026-11-24: "somewhere to the left of CIVVIS self-play, through down
Civ 6 Computationl Irreducability - ??? not sure where this is. Maybe just
guess a 10x improvement? Maybe through down a "M5 Max, Theoretical Max Civ 6
Throughput" somewhere like 10x to the left of Civ 6 Computational Irr...".
The same day he renamed the scale-up: "lets change hardware scale-up to
"Reasonable Local Hardware Scale-up Theoretical". And on 2026-11-26 he moved
it onto the M5 Max's theoretical most: "reasonable local hardware scale-up
theoretical should be 10x left of m5 max theoretical max civ 6 thorugh".

The ledger's `projected` are stops nothing has run at. Each is another stop's
multiplier times one figure, so it moves when that stop does. The stop it
multiplies (`over`) is a measured one, or a projected one listed before it.

| Stop | What it is | The sum |
| --- | --- | --- |
| Civ 6 Computational Irreducibility (`irreducible`) | One game on one core with nothing left in it that the game does not need. A guess: nobody knows where it is. | `10 × CIVVIS self-play's multiplier` |
| M5 Max, Theoretical Max Civ 6 Throughput (`m5_max`) | The most games one M5 Max could finish: the irreducible game on every core at once. A guess on a guess, and amortized. | `10 × the irreducible game's multiplier` |
| Reasonable Local Hardware Scale-up Theoretical (`scale_up`) | Hardware of our own, scaled up within reason past the one M5 Max. Theoretical. | `10 × the M5 Max theoretical's multiplier` |
| Distributed RL fleet (`fleet`) | Many machines playing self-play at once for reinforcement learning. A potential, and open at its top. | `1,000 × the M5 Max's multiplier`, and more |

Every figure is the project owner's. The first two are his guesses and stand
on CIVVIS self-play, so the M5 Max's theoretical most is a hundred times one
self-play game, whatever the M5 Max measures today. The scale-up is ten times
that theoretical most (until 2026-11-26 it was ten times the M5 Max as
measured, which left it close over the theoretical stop). The fleet is over
the M5 Max's every-core stop as it was measured. No machine was sized or
priced for either. Whether his 1,000 is over the one M5 Max or over the
scaled-up hardware is this page's reading of his sentence, and it reads the
first.
What a game takes now, in steps and instructions, and the plan for taking
steps out of it, is [`docs/PATH_STEPS.md`](PATH_STEPS.md).

On the line they are rings in the line's own colour. Each entry of the
line, a stop, a projected stop or a goal, takes its side of the line in turn
(`speedup_timeline.sides`), so two that stand close together go one over the
line and one under it. The page's number is never a projected stop: it is the
fastest stop that was measured, and the line is drawn solid only as far as
that stop. A thousand M5 Maxes are past the 1,000,000x goal and short of the
10,000,000x the line runs out to, so the fleet stands on the line at its own
place, left of the goal. Until the line was extended it stood at the line's
end as an arrow that pointed on from it, and a stop past the end of the line
still does (`past` in the record), with its own figure beside it.

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
which is 3,333 games a minute. It was the line's left end until the line was
run out a tenfold past it (see The number), and stands six sevenths of the
way along now. A billion
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
A goal is added to `goals` with a key, and an entry for it to the
page's line (`beta/performance.html`, by `data-goal`).

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
straight line. Each chart has its own run of days and its own scale: one
stop has readings since the summer and another since yesterday, and a
12-minute game and a 3-second one do not fit one scale.

Over each chart stands the pace its readings have kept
(`speedup_timeline.pace`): how many times faster the newest is than the
first, over how long, and from a week's span up the time in which a game's
time halves at that pace. Readings within two parts in a hundred of each
other are called steady.

| Stop | What its chart draws |
| --- | --- |
| CIVVIS → Firaxis computer control | The middle game of every day of the computer control benchmark's record (a day is drawn from three games up). Its pace is from the middle game of the record's first seven days to that of its last seven, because one day is a noisy thing to measure a pace by. A clone without the record draws the readings in the ledger. |
| Firaxis self-play | The stop's readings in the ledger. |
| CIVVIS self-play | Its one-core benchmark readings, and beside them the self-play tournament's finished batches, the middle game of each on the day it ended (`data/performance/self_play_times.json`; `"tournament": true` on the stop). The tournament plays the same game seven or more at a time on a busy machine, so it reads slower, and it reaches back to October. A third line is the self-play benchmark hours ([`docs/SELF_PLAY_HOUR.md`](SELF_PLAY_HOUR.md)), each as the processor seconds one game took. |
| Parallel games | Every run of `data/performance/games_per_minute.json`. The pace is kept over the records among them. A second line is the same benchmark hours as the time from one finished game to the next, which is the rate the machine keeps with the live game and everything else running beside the tournament. |

So a stop's chart grows by itself with every reading added to the ledger and
every run recorded.

## Adding a reading

A reading is one line in a stop's `readings`, after the ones already there:
the day it was `taken`, the `seconds` one game takes, and `from`, which says
how it was measured closely enough to measure it again. The newest reading is
the one shown. The older ones stay: they are the track of how far a stop has
been pushed, and the page says where a stop started once it has moved.

`all_cores` has no readings to add. Record a run with
`tools/games_per_minute.py --record` and the page follows it when it is
a record.

`control` needs none either: every computer control game is timed, and the
stop reads the benchmark's record when the site is published. Its readings
in the ledger are the fallback for a clone without the run ledger, so add
one now and then to keep the fallback near the truth.

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

`python3 beta/speedup_timeline.py .` prints what the line will show, and
`python3 beta/speedup_timeline.py --worked .` the tables over it,
the processors among them.
