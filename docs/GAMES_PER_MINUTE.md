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

That last pair overlaps, and on 2026-11-24 it went wrong. The harness
stopped the three tournament workers that were running and continued them
four minutes later. In between, the tournament's CPU governor had cut its own
count to nought and marked those three as stopped by itself. It signals only
when its count changes, so after the harness continued them they ran outside
its count, beside the live game, for fifty-two minutes until another session
noticed. The governor now reads each worker's state from the process table
every window and stops again any worker it holds that it finds running
(`CpuGovernor._apply_workers` in `tools/continuous_batch_scheduler.py`). A
tournament whose scheduler was started before that change does not have it:
after a `--quiesce` run beside one, compare the workers that are running
(`ps -Ao pid,pgid,stat`, the scheduler's process group) with
`governor.workers_running` in its `status.json`, and stop any beyond that
count.

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

**24 games per minute on an M5 Max** (MacBook Pro, 18 cores, 128 GB, mains
power), build `d301d68d0c`, main of 2026-11-24. One run that day,
`--quiesce --minutes 3`, 18 workers. Before it the record was 18, from two
runs on 2026-11-22 on build `a57ffc68c6`, the build the self-play tournament
was playing then.

| Taken (UTC) | Build | Games per minute | Measured minutes | Best minute | First minute | Turns a second | Cores the workers had |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| 11-22 05:32 | `a57ffc68c6` | 18.33 | 17, 18, 20 | 20 | 8 | 59.5 | about 14.7 (sampled) |
| 11-22 05:37 | `a57ffc68c6` | 18.00 | 17, 18, 19 | 19 | 6 | 58.3 | 14.18 |
| 11-24 15:58 | `d301d68d0c` | 24.33 | 26, 25, 22 | 26 | 19 | 80.9 | 15.3 |

What the run of 2026-11-24 says:

- **The simulator got faster, and the record with it, by less.** One game on
  one core went from 57.1 s to 34.5 s between the two builds, 1.66 times
  faster. The record went from 18.33 to 24.33, 1.33 times. Eighteen cores
  times one core's 34.5 s would be 31.3 games a minute, so the run reaches
  78 % of that, where the first record reached 97 % of its own. The cloud
  cost on the Performance page takes a rented machine to reach the same
  share.
- **A worker finishes a game in about 42 s** (the median on its worker; the
  games averaged 199.5 turns), and the cold first minute already finished 19.
- **The workers had 15.3 of the 18 cores.** The held Civilization VI window,
  the mirror page and the rest took about 1.6.
- It is one run. The second and third measured minutes fell (26, 25, 22), so
  another run could move the headline by a game a minute either way.

What the two runs of 2026-11-22 said:

- **It repeats.** The 60 games finished in both runs ended on the same turn
  with the same victory in each. Run 2 played each game a median 3.8 % slower
  (it started at load 21.8, run 1 at 16.1), which moved the headline by 0.33.
  Treat a difference under about half a game a minute as noise until more
  runs say otherwise.
- **A worker finishes about one game a minute.** The games averaged 194 turns
  and a game took a median 62 to 64 s on its worker.
- **The workers had 14.2 of the 18 cores.** The rest went to what stays up
  around a live game: the mirror page in Chrome, the held Civilization VI
  window, WindowServer and many small processes. A machine with nothing else
  open would score higher; this is the figure for this host as it runs.
- **Eighteen games at once run slower than one alone.** Alone on this machine
  the same build played a 180-turn game in 37 s, 4.9 turns a second (one seed,
  measured earlier the same day). Here a worker made a median 3.2 turns a
  second, about two thirds of that. Having 14.2 cores for 18 workers accounts
  for most of the gap (0.79 x 4.9 = 3.9). The remaining sixth is not
  attributed yet; a `--workers 6` run would show whether it is the twelve
  cores of the slower class.
- The cold first minute finished 6 to 8 games, a third of the steady rate.
- Each run held the live game for about four minutes, and the game played on
  afterwards (held at turn 154 the first time and at turn 169 the second).
