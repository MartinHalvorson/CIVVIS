# The computer control benchmark

How long one game takes with CIVVIS at the controls of Civilization VI
itself, end to end.

The project owner asked for it on 2026-11-24: "we should be benchmarking our
civvis computer control games time end to end and that number show flow
through to a performance chart (civvis comptuer control games speed over
time) and be a top chart for civvis performance".

It is the first part of the
[CIVVIS Performance page](https://civvis.ai/charts/civvis-performance/): four
tiles and a chart of every day's games.

## What is timed

One run of the harness, `tools/civ6_play.py`, from the moment it starts to
the moment it writes its summary. That is everything the machine spends on
the game:

| Part | From | To |
| --- | --- | --- |
| Setup | The harness starts. Civilization VI is not yet running. | The game is hosted, set up and checked ("in a configured game"). |
| Play | The first turn. | The watch loop ends: a victory, a defeat, or a stop. |
| Ending | The final screen is held through the game's own victory film. | Civilization VI has quit and the summary is written. |

Two things are outside it. Publishing the result to the `ledger` branch
happens after the summary is written. The pause before the lane launches its
next game belongs to no game.

## The clock

`civ6_play.RUN_CLOCK` is one monotonic clock for a run. `play` starts it as
its first statement. Two marks split it: `configured`, where setup ends, and
`ended`, where play does. `run_timing()` reads it as the summary is built.

The summary carries the reading as `timing`, in seconds to a tenth, and the
moment the clock started as `started_utc`:

```json
"started_utc": "2026-11-24T15:00:36Z",
"timing": {"seconds": 755.2, "setup_seconds": 47.9, "play_seconds": 679.0, "end_seconds": 28.3}
```

A run stopped before a mark has only the parts it reached. A run stopped
during setup has `seconds` alone. A run that attached to a game somebody else
started has no reading, because it did not time that game from its start.

`civ6_ladder.result_row` copies `seconds`, `setup_seconds` and `play_seconds`
onto the run's line of `results.jsonl` on the `ledger` branch. The ending is
what is left of the whole. A run from before the clock keeps the line it had.

`tools/test_civ6_play_timing.py` holds the clock, and that `play`, the two
marks and both summaries are wired to it.

## From a run to a game

A game is the lines of `results.jsonl` that share a `game`. Most games are
one run. A game reloaded from its autosave is several.

`beta/control_times.py` gives a game a time in one of two ways:

- **By the clock.** Every run of the game has `seconds`, and the game is
  their sum. This holds for a reloaded game too.
- **By two stamps.** A run from before the clock has none, so its time runs
  from the stamp in its tag, which is when it was launched, to the stamp of
  its result. This holds for a game of one run only. A continuation's tag
  carries the first run's stamp, so a reloaded game from before the clock has
  no time and is left out. The page says how many.

On a game of one run the two agree to about a second, so the chart does not
step where the clock begins. The lane stamps a tag as it launches the
harness, and the harness takes the game lock inside `play` about a second
later: run `civvis-t7-20261124T152452Z` held it from 15:24:53Z. Both ways end
on the same moment, the summary's `finished_utc`.

## Which games count

The games played to a victory, ours or a rival's. A game we were eliminated
from, one the harness conceded early and one somebody stopped all end sooner
than a game does, and would read as speed.

Every Civilization VI level and every setup counts. The games were four
players on Pangaea for some weeks and six on Continents for others, and the
level has climbed from Settler to Deity. The chart's other measure, the time
of one turn, is a game's seconds over the turn it ended on. It does not move
with how long the games happened to run.

## The figures

A day is the UTC day a game ended on.

| Figure | What it is |
| --- | --- |
| A day's point | The middle game of the day, and the times one game in ten beats and one in ten runs past. A day needs three games to be drawn. |
| The reading | The same three over the last seven days, back from the newest game. This is the benchmark's number. |
| Faster than by hand | The seconds of a game played by hand (`data/performance/speedup_timeline.json`, its first stop) over the reading's middle game. |
| When the record began | The same three over the first seven days of the record. |
| Where the time goes | The middle setup, play and ending of the last seven days' clocked games, shown from three of them. While fewer games are clocked than the seven days hold, the line says how many it is drawn from (`split_games`): the parts of a few games do not add up to the middle game of them all. |

Percentiles are nearest rank on the games themselves, as the page's
self-play times are.

## How it reaches the page

Nothing is committed. `beta/publish.sh` fills the page with
`beta/performance_page.py`, which calls `control_times.control` on the run
ledger as the clone last fetched it. A finished game is on the chart with the
next publish of the site.

```bash
python3 beta/control_times.py .      # the record, as JSON
```

A clone that never fetched the `ledger` branch has no record, and the page is
published without this part. So is a ledger the script cannot read: that is
said on stderr and costs the page this part alone.
