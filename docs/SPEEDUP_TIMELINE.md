# The speed timeline

How many times faster than a person a game of Civilization VI gets played.
The home page shows it over its videos: one number, and under it a line with
five ways of playing one game, each standing at its multiplier.

The readings are `data/performance/speedup_timeline.json`. The page's figures
are worked out from them by `beta/speedup_timeline.py` when the site is
published. Nothing derived is committed.

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
its right end and the goal at its left, so a stop stands at
`log(multiplier) / log(goal)` of the way across.

The goal is 1,000,000x: a game every 18 ms, which is 3,333 games a minute on
one machine. Two stops have goals of their own, a self-play game in 10 s and
a game a second on every core (18,000x).

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

`python3 beta/speedup_timeline.py .` prints what the page will show.
