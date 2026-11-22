# Games per minute

How many complete self-play games one machine finishes in a minute with every
core playing. One number for the whole simulator, on a named machine:
"N games per minute on an M5 Max".

The harness is `tools/games_per_minute.py`. The recorded runs are
`data/performance/games_per_minute.json`, newest last.

## The test

1. **One worker per core.** Each worker is a `gene_screen --jobs 1` process
   that plays one game after another on one thread. Eighteen cores, eighteen
   workers, eighteen games in play at any moment. This is the shape the
   self-play tournament already uses (one process per game, no shared state),
   with the worker count raised from nine to every core.
2. **The standard game.** The gene screen's bare defaults, which is the game
   the tournament plays and the Performance page times: six CIVVIS seats, each
   with its own drawn genome, 74x46 Continents, nine city-states, Online speed,
   250 turns, all six victories. A game ends on a victory or on the turn clock.
3. **Fixed seeds.** Worker `i` starts at seed `20261122000000 + 100000 * i` and
   counts up. The same binary plays the same games in the same order on every
   run.
4. **A game counts when it is finished.** The binary writes a game's rows when
   the game ends. The harness reads each worker's rows file every 20 ms and
   stamps a game with the time its rows appeared.
5. **The clock.** The first minute starts cold: every worker begins a new game
   at second 0. Then come the measured minutes (two by default). When the last
   one ends the workers are killed, and games still in play count for nothing.

## The three figures

| Figure | What it is |
| --- | --- |
| `games_per_minute` | Mean games finished per measured minute. The headline. |
| `best_minute` | The best single measured minute. |
| `first_minute` | Games finished in the first 60 s from a cold start. |

`first_minute` is the literal stopwatch reading, and it is always the lowest
of the three: no game can finish before the shortest game is over, and every
game the bell cuts off is lost. After the first minute the workers are out of
step with one another, so each minute inherits about as many part-played games
as it leaves behind. That is the rate a long batch actually gets, so it is the
headline.

A minute holds a few dozen games, so one minute is a noisy reading. Compare
runs on `games_per_minute` over two or three minutes, not on `best_minute`.

## Reading a change

The figure counts games, not work. A build can raise it in two ways: by
playing a turn faster, or by ending its games in fewer turns. Both are real
throughput, but only the first is simulator speed. The report prints
`turns_per_second` and the mean turns per game beside the headline, so the two
can be told apart. To price a single change, `tools/speed_ab.py` (paired,
per completed turn) is the instrument. This one answers a different question:
what the whole machine delivers.

The report also shows how many cores the workers actually had and what else
was running. A run where the workers had 14 of 18 cores is not a record.

## A quiet machine

A maximum needs free cores, and this host is rarely idle: a live Civilization
VI game, the nine-worker self-play tournament and other sessions' compilers
share it. `--quiesce` frees the machine for the length of the run and puts
everything back when the run ends, however it ends:

- **The live game is held**, through the operator's own Pause
  (`tools/civ6_control/live_pause.py`). The harness waits until the game
  reports it is held at our turn before any worker starts, and releases it
  afterwards. A held game has no rival turn in flight, so nothing can stall. A
  pause somebody else set is left alone.
- **The CPU-headroom guard is switched off** (`~/.civvis-headroom-off`). That
  guard parks every batch process when the host has under 20 % idle, which is
  exactly what this test does on purpose.
- **Other batch work is stopped** with SIGSTOP (compilers, other simulator
  batches) and continued afterwards. Only what the harness stopped is
  continued. The tournament's scheduler also pauses its own games while the
  host is over its 60 % ceiling.

Each step is written to `quiesce.json` in the run directory before it is
taken. If the harness itself is killed, this undoes it:

    tools/games_per_minute.py --release <run directory>/quiesce.json

A two-minute measurement holds the live game for a little over three minutes.

## Running it

    cargo build --release --features developer-tools --bin gene_screen
    tools/games_per_minute.py --binary target/release/gene_screen
    tools/games_per_minute.py --binary B --quiesce --record --minutes 3

`--workers N` sets the worker count (default: every core). Arguments after
`--` go to `gene_screen` and make the run a probe (a smaller map, a shorter
clock); a probe cannot be recorded.

A record run is: `--quiesce`, mains power, a clean build of a commit on
`main`, the default worker count and the standard game.

## The record

<!-- record -->
