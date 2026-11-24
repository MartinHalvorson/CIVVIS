# The self-play benchmark hour

What the self-play tournament on one machine finishes in one hour, taken
every six hours.

The project owner asked for it on 2026-11-24: "we should be benchmarking
civvis self-play tournament games as well. maybe every 6 hrs or so we can do
1 hr of civvis self-play games on this machine". The machine is the one that
plays the verification games in Civilization VI and the self-play tournament
both.

It is the self-play side of the
[computer control benchmark](CONTROL_GAME_TIME.md), and its readings go to
the same place: the `ledger` branch, where the site reads them when it is
published.

## What an hour is

`tools/self_play_hour.py run` watches the tournament that
`tools/continuous_batch_scheduler.py` plays
([how that is run](CONTINUOUS_BATCH_OPERATIONS.md)) for sixty minutes and
reads what it finished in that time.

It starts and stops nothing. The games are the tournament's own: six CIVVIS
agents and nine city-states on a 74x46 Continents map at Online speed, to a
victory or to turn 250. They count for the tournament as every game of it
does.

So a reading is what this machine delivered in that hour, beside everything
else it was doing. The scheduler's governor holds the tournament's workers
back while Civilization VI is playing and while other heavy work runs, and an
hour taken then has few games. The reading carries the conditions it was
taken under, so that an hour can be read for what it was.

## Which games an hour counts

A row of the tournament carries no finish time. But a batch's rows file,
`batches/<batch>/rows-continuous.jsonl`, is only ever added to, and a
finished game's six rows are written to it in one piece, in finishing order.

So the games of the hour are the rows between the file's length when the
hour began and its length when it ended. A game is a seed, and its first row
is the game.

| Case | What is read |
| --- | --- |
| One batch through the hour | The rows between the two lengths of its file. |
| The batch was cut during the hour | The rest of the first batch's file, any batch played whole in between, and the start of the last one's. |
| The hour began or ended while a game's rows were being written | Whole lines only. A line cut at the start is skipped, and one cut at the end is left for the next hour. |

## The reading

One JSON object on one line.

| Key | What it is |
| --- | --- |
| `taken` | The moment the hour began, UTC. |
| `minutes` | How long it ran by the wall clock. More than sixty if the machine slept. |
| `games`, `games_an_hour` | The games finished, and that count scaled to an hour. |
| `p` | A game's wall-clock seconds at the 10th, 50th and 90th percentile, nearest rank. They include time a worker was held by the governor, so they are long on a busy machine. |
| `cpu_a_turn`, `cpu_a_game` | The middle game's processor seconds for one turn and for the whole game, by the game thread's own clock (`cpu_secs`). The machine's load does not stretch these. |
| `turns` | The middle game's length. |
| `workers`, `running`, `cores` | The tournament's workers, how many were running, and the cores they were given. |
| `host_busy`, `load` | The share of the whole machine in use, in percent, and the one-minute load. |
| `civ6`, `battery` | The share of the hour Civilization VI was running, and the share on battery. |
| `reserved` | Whether the hour was set aside for self-play. `false` for an hour taken beside the machine's other work. |
| `build`, `batch` | The tournament's source commit and batch. |
| `host`, `machine`, `machine_cores` | The machine. |

`running`, `cores`, `host_busy`, `load`, `civ6` and `battery` are means over
samples taken every thirty seconds, from the scheduler's own `status.json`
and the system.

`cpu_a_turn` is the figure that follows the code. `games_an_hour` follows the
code and the machine's day together.

## Where a reading goes

1. **The journal.** Every reading is appended to `self-play-hours.jsonl` in
   the scheduler's state directory, before anything else.
2. **The ledger.** `run --record` then adds every line of the journal the
   branch lacks to `benchmarks/self_play_hours.jsonl` on the `ledger`
   branch, oldest first.

The push is the one `tools/civ6_ladder.py` publishes a run with: a commit on
the branch's tip, built in a scratch index, pushed as a fast forward. The
branch moves with every finished verification game. A push that loses that
race reads the tip again and rebuilds, three times at most, and is never
forced. A reading already on the branch is never rewritten.

A publish that fails leaves the journal as it is. The next hour's publish
sends both, and `self_play_hour.py publish --state-dir DIR` sends them at any
time.

## Running it

```bash
# One hour now, with its reading on the ledger:
python3 tools/self_play_hour.py run --state-dir ~/civvis-runs/<state> --record

# Every six hours, by launchd, at 02:00, 08:00, 14:00 and 20:00 local time:
python3 tools/self_play_hour.py install --state-dir ~/civvis-runs/<state>

python3 tools/self_play_hour.py show        # the readings on the ledger
python3 tools/self_play_hour.py uninstall
```

`install` writes `~/Library/LaunchAgents/com.civvis.self-play-hour.plist` and
loads it. The job only watches, so it runs as a background process. Its log
is `~/Library/Logs/civvis-self-play-hour.log`. Run it from the checkout that
the synchronization service keeps at `origin/main`, so the job follows the
tool as it changes.

## Not done yet: an hour set aside

The readings so far are hours taken beside the machine's other work. An hour
in which self-play has the machine to itself would be a cleaner benchmark,
and it would finish many times the games.

That needs two things this tool does not do, and both are the project
owner's decisions:

- **Holding the verification games for the hour.** Civilization VI fails
  rather than slows under load, so the lane has to be idle. Nothing today
  holds it between games without stopping the game in play.
- **Letting the tournament's workers run.** The governor's ceilings are fixed
  when the scheduler starts, and its launchd class gives the workers about
  five cores however many run
  ([measured](CONTINUOUS_BATCH_OPERATIONS.md)).

`reserved` is in every reading so that such hours can be told from these
when they exist.
