# Elo for Civ AI Agents

A **Civ AI Agent** is one of our controllers frozen at a named genome and given
a gamer handle: `SiegeTheDay` comes for your capital, `MoonshotMaven` races to
the launch pad. Each one carries a measured Elo, and the scale is pinned to
Civilization VI's own difficulty levels, so somebody who beats that game at
Prince can pick a rival that plays at about Prince.

The registry is `strategies::ROSTER` (`src/strategies.rs`). The instrument is
`strategy_elo` (`src/bin/strategy_elo.rs`). The published numbers are
`data/strategy_elo.json`, which the build embeds; this document deliberately
repeats none of them.

## What an agent is

A row in the roster: an id, a handle, a tilt, a chassis, an optional victory
contract and a list of gene tags.

| Chassis | What it is |
| --- | --- |
| `Deployment` | `AdvancedAi` with the deployment genome |
| `Stock` | `AdvancedAi::new()`, the production genes with every opt-in off |
| `Legacy` | `AdvancedAi::legacy()`, the first hierarchical agent |
| `Basic` | `BasicAi`, the scripted baseline |

The tilt comes from the victory contract (`AdvancedAi::targeting`) and the
genes. A row is frozen: to change what an agent is, add a row under a new id.
Editing one in place leaves its logged games describing something else.

Freezing has a limit worth stating. The row pins the *genome*; the code under
each gene keeps moving with trunk. A rating therefore describes the agent as it
played when its tables were logged, and a large behaviour change calls for
fresh tables (`--log` to a new file).

## How a rating is measured

Every table is a full game: four majors on the Tiny map, Online speed, 250
turns, the gene screen's length of game. The winner is whoever the engine
declares, including its own score victory at the turn limit.

- **Pool tables** seat roster agents against each other at Prince, where the
  rules hand nobody anything. Each round shuffles the whole roster and deals it
  into tables, so every agent sits equally often.
- **Rung tables** measure a Civilization VI difficulty level. The reference
  agent (`strategies::CIV6_REFERENCE`) sits alone in one seat with the rung's
  shipped AI handicap from `data/difficulties.json`, and is rated as the
  entrant `civ6:<rung>`. A hand of roster agents dealt from a shuffle
  (`strategies::dealt`) takes every other seat, each on the terms a person
  gets at that rung. [Why a rung sits alone](#why-a-rung-sits-alone) has the
  reason.

The fit (`strategies::fit`) is the model `elo::win_shares` already states: a
seat wins its table with probability proportional to `10^(elo / 400)`. With two
seats that is the ordinary Elo expectation. It is fitted by maximum likelihood
over the whole log rather than by a running K-factor update, so the result does
not depend on the order the games were played in. Each entrant also gets two
imaginary head-to-head games against an average player, one won and one lost,
so an entrant that has never won still has a finite rating.

The scale is then shifted so that Prince is `strategies::PRINCE_ELO`. At Prince
the reference agent plays with nothing added, so its own rating *is* Prince.
The `sd` column is a bootstrap over tables, relative to that anchor. An agent
with fewer than `strategies::PROVISIONAL_TABLES` tables is shown with a `~`.

Two things keep the published numbers from saying more than the tables do:

- **Bounds.** An entrant that has never lost a table is rated "at least"
  (`bound: "lower"`, shown with a trailing `+`), and one that has never won is
  rated "at most" (`bound: "upper"`, a leading `≤`). The fit can only place such an entrant from one side.
- **The ladder climbs.** Each rung's handicap contains the one below it, so a
  higher rung cannot truly be easier. Rungs that measure out of order are
  pooled to their weighted mean (`strategies::monotone_ladder`, with Prince
  held at the pin), and the file's `ladder` is what a person is shown. Rungs
  the tables cannot tell apart show the same Elo. The raw fit stays in
  `ratings`.

```bash
cargo run --features developer-tools --profile ci --bin strategy_elo -- \
  --games 240 --rung-games 40 --jobs 4
```

`--games` and `--rung-games` are totals for the log to reach, not amounts to
add. A killed run resumes, and a later run with larger totals only plays the
difference. `--fit-only` refits without playing. Finished tables are appended
to `data/strategy_elo_games.jsonl`.

### Re-rating after the genome moves

The Deployment agents play the default genome, so every defaults update moves
them. Ratings are therefore kept current by **rating runs**: each run adds a
fresh batch of tables under its own label, and the fit uses a window of the
newest runs.

```bash
strategy_elo --run 2026-11-23-15125c4 --add-games 480 --add-rung-games 120 \
  --keep-runs 4 --jobs 3
```

- `--run LABEL` stamps every table the run plays (`"run"` in the log). The
  label starts with the run's date; the scheduled runs add the revision.
- `--add-games N` and `--add-rung-games M` add N pool tables and M tables at
  each rung, numbered on after the newest table of each series
  (`strategies::added_indices`), so they are new deals and seeds. A killed run
  resumes under its label rather than adding twice.
- `--keep-runs K` drops every table outside the newest K runs
  (`strategies::kept_runs`) before fitting. Tables logged before runs were
  labelled count as one run, the oldest.

The published file lists the runs it was fitted on (`runs`) and gives each
rating a `margin95`: the 95% margin of error, `strategies::MARGIN_Z` (1.96)
times the bootstrap deviation. ELO Rankings lists every agent with that
margin (`1333 ±27`), and so did Win Analytics until 2026-11-23.

**Why a window:** a run's tables describe the genome of its day. A window of K
runs trades freshness for precision: the margin shrinks with the square root
of the tables in it (an agent's was about ±75 at 1,320 tables, ±34 at 4,560
and ±28 at 9,120), while a genome change older than K runs ages out entirely. The
scheduled job keeps K = 4 runs, one every two days, so no table in the fit is
older than about eight days. To narrow the margin further, raise the tables
per run before raising K. (Since 2026-11-24 the runs follow the self-play
batches instead, smaller and more often: [Fresh ratings](#fresh-ratings).)

**Schedule:** on the tournament host a launchd job starts a run every two
days: 960 pool tables, 240 at each rung, twice that at Immortal and four
times at Deity, about an hour and a half on six cores. It borrows six of the
gene screen tournament's nine worker slots for the build and the run, gives
them back, and ships the new ratings as a PR the same way the defaults
updates ship.

The first re-rate, on 2026-11-21, replaced the 11-11 log outright: 1,320
tables (`--games 480 --rung-games 120`) on the 242-gene defaults, replacing
the 660 tables of 2026-11-11 played on about 200. Those 1,320 tables are the
window's first, unlabelled run. A table now costs about 1.6 CPU-seconds (it
was about 35).

| Entrant | 2026-11-11 | 2026-11-21 |
|---|---|---|
| `tourist` TouristTrap | 1178 | 1291 |
| `holy` HolyRoller | 1128 | 1247 |
| `jack` JackOfAllTrades | 1211 | 1143 |
| `forward` ForwardSettle | 1202 | 1138 |
| `moonshot` MoonshotMaven | 1164 | 1095 |
| `siege` SiegeTheDay | 1137 | 1073 |
| `swing` SwingVote | 1192 | 1037 |
| Emperor (rung) | 1461 | 1287 |
| Immortal (rung) | ≥1660 | 1515 |
| Deity (rung) | ≥1660 | ≥1762 |

The bootstrap deviation is about 35-40 for each agent (a 95% margin of about
±70-80). The upper rungs came down: at Emperor the challengers won 12 of 120
(2 of 60 before), at Immortal 3 of 120 (0 of 60), at Deity none.

### Why a rung sits alone

Until 2026-11-22 a rung table was the other way round: one roster agent as
the challenger, and the rung's rival in every other seat. Two things were
wrong with it.

**It could not see the top of the ladder.** A table says most about a rung
when either side might win it. Over the 240 tables each rung then had, the
lone challenger won 21 at Emperor, 4 at Immortal and none at Deity, so Deity
was published as "at least" and Immortal with a margin of ±158.

**It was not the table the scale is pinned at.** Prince is the reference
agent's own rating: one seat of it, nothing added, among roster agents. Every
agent is rated the same way, one seat among others. A table that holds three
copies of one entrant is a different table, and the fit only joins the two if
a seat's strength does not depend on who sits beside it.

The fit itself is neutral about that
(`the_fit_reads_one_distance_from_either_shape_of_table`), so the games were
asked. On 2026-11-22 both shapes were played side by side and each was fitted
on its own, over the same pool tables:

| Rung | One challenger, the rival in every other seat | The rival alone | Difference |
|---|---|---|---|
| Settler | 873 ±48 (challenger won 147 of 303) | 998 ±46 (rival won 77 of 480) | +126 (3.7 sd) |
| Chieftain | 858 ±54 (challenger won 152 of 303) | 974 ±44 (rival won 68 of 480) | +116 (3.3 sd) |
| Warlord | 910 ±55 (challenger won 135 of 303) | 1012 ±47 (rival won 80 of 480) | +103 (2.8 sd) |
| King | 1008 ±48 (challenger won 102 of 303) | 1188 ±40 (rival won 167 of 480) | +180 (5.7 sd) |
| Emperor | 1329 ±85 (challenger won 21 of 240) | 1507 ±43 (rival won 363 of 480) | +177 (3.6 sd) |
| Immortal | 1610 ±161 (challenger won 4 of 240) | 1660 ±43 (rival won 842 of 960) | +50 (0.6 sd) |
| Deity | ≥1895 (challenger won 0 of 240) | 1954 ±48 (rival won 2,806 of 2,880) | |

The older shape read every rung it could place between about 100 and 180 Elo
low. Three copies of one rival are easier on the fourth seat than the fit
reads from one copy. The likely cause is that three seats running one plan
contest the same prizes; nothing here proves the cause, only the gap.
Immortal and Deity do not disagree, because the older shape could barely
place them.

The same gap had the bottom of the ladder wrong. The older shape put Settler,
Chieftain and Warlord 90 to 140 below Prince and King level with Prince.
Prince was never measured in that shape, so the 90 to 140 was the shape, not
the rung. Measured alone, the three lower rungs sit at Prince within their
margins: what a person is handed there (Combat Strength, experience, gold
from camps) moves these agents little. King's handicap is worth about 190.

So a rung is now measured the way Prince is: its rival alone, among agents.
That makes "this agent rates level with Emperor" a like-for-like statement,
since both numbers come from one seat at a mixed table, and it is a table
either side can win at every rung.

The older tables stay in `data/strategy_elo_games.jsonl` as `"kind": "rung"`
until their runs leave the window. They are not fitted, and a rung table is
logged as `"kind": "lone"`.

The first run in the new shape, `2026-11-22-ca8754f`: 1,920 pool tables, 480
at each rung, 960 at Immortal and 2,880 at Deity (`--rungs`, below).

| Level | Rung | 2026-11-21, older shape | 2026-11-22, alone |
|---|---|---|---|
| 1 | Settler | 880 ±65 | 998 ±46 |
| 2 | Chieftain | 865 ±60 | 974 ±44 |
| 3 | Warlord | 887 ±57 | 1012 ±47 |
| 4 | Prince | 1000 (the pin) | 1000 (the pin) |
| 5 | King | 998 ±63 | 1188 ±40 |
| 6 | Emperor | 1306 ±84 | 1507 ±43 |
| 7 | Immortal | 1584 ±158 | 1660 ±43 |
| 8 | Deity | ≥1868 | 1954 ±48 |

The agents kept their order, and their margins fell from about ±50 to ±28: a
rung table seats three agents where the older one seated one. Most moved by
less than 25. HolyRoller moved furthest, 1258 to 1333, and the shape of the
rung tables is not why (it rates 1324 with the older tables fitted instead).
It won 63% of its pool tables in this run, against 48% and 57% in the two
runs before: each run plays the defaults of its day, and they moved its way.

**A rung that wins nearly every table** learns little from each one. Deity's
rival won 2,806 of 2,880 even alone, so it took six times its neighbours'
tables to bring its margin level with theirs. `--rungs deity,immortal` plays
rung tables at the named rungs only, to add tables where the margin is:

```bash
strategy_elo --run 2026-11-22-ca8754f --rungs deity --add-rung-games 960
```

It adds under the run's label, up to that many tables at each named rung.

### Fresh ratings

Operator 2026-11-23: "we should have our benchmarking games update our elo.
as well as our civvis self-play games should update elo too. so elo is always
staying fresh."

**The self-play batches.** The gene-screen tournament's games cannot rate an
agent themselves: every seat there plays a random genome (each default gene
kept with a chance of three in four, each opt-in taken with one in four), so
no seat is a roster agent and a table of them says nothing about one. What
the batches do is move the agents: each published batch may promote genes
into the defaults, and every Deployment agent plays the defaults. So a rating
run now follows them. The host's cadence (`elo_cadence.py`) starts a run once
a continuous batch has been published on `main` since its last run, no sooner
than six hours after it, and every 48 hours whatever happens; each run is
smaller than the 48-hour runs were and the window keeps more of them, so the
window is about as large and its newest tables are hours old, not days. The
run plays its tables in worker slots borrowed from the same tournament, on
that batch's build, which is as near to self-play updating the ratings as the
tables can come.

**The benchmark games.** The tournaments against Civilization VI's own AI
(the Indefinite Benchmarking Tourney and the ones before it) rate the
Firaxis levels themselves, measured on the real AI rather than on our
stand-in. A finished game is a table of six: our seat and five Firaxis AIs
at the game's level. It is fitted on the model above, a seat winning with
probability proportional to `10^(elo / 400)`, so a game against five AIs
rated `r` is our seat's with probability `s / (s + 5 * 10^(r / 400))`.

Our seat is the live decider playing one victory lane, and it is rated as
its twin, the roster agent that plays that lane in the simulator
(`elo_page.TWINS`: a Science game is MoonshotMaven's, a Religious one
HolyRoller's, a game with no lane JackOfAllTrades'), at the rating published
when the game began (`data/strategy_elo_history.json`). That is the one
assumption joining the two scales: the twin plays inside Civilization VI as
it plays in the simulator. Each Firaxis level meets only our seat, so the
joint fit of these tables with the simulator's reduces to one equation per
level, which `beta/elo_page.py` solves (`fit_level`): the level's rating is
the one at which our seat's expected wins equal its wins, with the pair of
imaginary games every entrant gets, so a level we have always beaten reads
"at most" (`≤`). Its margin is the fit's own and the twins' margins added in
quadrature.

It is fitted when the site is built, off the run ledger, so the levels move
with every finished game and never wait for a rating run: over the last 7
days for the rating shown, and once a day for the line over time.

The first reading (2026-11-24, 326 games): Lv 8 Deity 1198 ±103, Lv 7
Immortal 1153 ±165, Lv 6 Emperor 824 ±124, Lv 5 King 747 ±179, Lv 4 Prince
at most 397. The simulator's Difficulty dial puts the same levels at 1954,
1660, 1507, 1188 and 1000. Two readings of different things, and the gap is
the finding: our stand-in, `stock` with Deity's handicap, is a far stronger
rival in the simulator than Civilization VI's own AI with that handicap is in
the game. Our seat wins 14.5% of its Deity games, where an equal seat at a
table of six would win 16.7%. Two cautions: the table of one seat against
five copies of a rival is the shape that read our own rungs 100 to 180 low
([Why a rung sits alone](#why-a-rung-sits-alone)), and the games pooled the
Diplomatic victory on and off as the benchmark deals them. They no longer do:
[Diplomatic victory on and off](#diplomatic-victory-on-and-off).

**Elo over time.** Every published fit is a point in
`data/strategy_elo_history.json` (`tools/strategy_elo_history.py`: the
rating run appends one before it commits, `backfill` rebuilt the fits since
2026-11-11 from git, and `tools/test_strategy_elo_history.py` fails a
revision whose newest point is not the published file). ELO Rankings
(`beta/elo.html`, under Charts/) draws the top ten agents from fit to fit
and the five Firaxis levels day by day on one axis.

### Diplomatic victory on and off

Operator 2026-11-24: "add a second elo for games with diplo off. we should
maintain this because diplo on or off makes a big difference."

Every rating is kept twice: for games with every victory on (the stock game,
everything above) and for games with the Diplomatic victory switched off.

- **The agents.** `strategy_elo --no-diplo` plays every table with
  `victory_conditions.diplomatic` off and the other five on, and keeps its own
  log, fit and history: `data/strategy_elo_no_diplo_games.jsonl`,
  `data/strategy_elo_no_diplo.json` (`"diplomatic": false`) and
  `data/strategy_elo_no_diplo_history.json`. Each logged table says which game
  it was (`"diplomatic"`; a line without it is from before the switch, a stock
  game), and a fit reads only the tables played its way, so the two can never
  mix. The recipe is the stock one, pool and rung tables alike, so the two
  ratings of an agent differ only in the switch.
- **The Firaxis levels.** The benchmark games are split by their own run's
  switch, the split Win Analytics' two win-rate tables make
  (`research_page.switches`). A diplo-off game is rated with our seat at its
  twin's diplo-off rating, a diplo-on game at the stock one. A game whose run
  does not say how it was set is in neither reading.
- **Kept fresh.** The host's cadence (`elo_cadence.py`, `no_diplo` in
  `elo.json`) plays the same recipe a second time with `--no-diplo` in every
  scheduled run and ships both fits in the run's one PR;
  `tools/strategy_elo_history.py append` appends each published fit to its own
  history, and `check` fails a revision whose diplo-off history does not end at
  its published fit, as it does for the stock one.

ELO Rankings shows both side by side: Player strategies has a column for
each (its head sorts the list), the levels table each reading's benchmark and
simulator figures, and the graph a switch between the two.

## What the Civilization VI mapping does and does not prove

It measures, in our engine, what each rung's *handicap* is worth: the yield
percentages, Combat Strength, free units, era boosts and starting buildings the
real game gives its AI above Prince, and the Combat Strength and experience it
gives a person below it. Those tables are Firaxis's own rows.

It does not measure Firaxis's AI. The rival at every rung is our reference
agent, and the claim "Prince is 1000" is the assumption that the reference
agent, unassisted, plays about as well as theirs. The spacing between rungs is
a measurement; where the whole ladder sits against our agents is that one
assumption.

The reference is `stock` (`ByTheBook`), the planner with no promoted extras,
chosen by what it predicts at the top of the ladder. Calibration record,
2026-11-11:

- With the scripted baseline (`wheels`) as the rivals, challengers won 12 of
  the first 14 rung tables, including 2 of 4 at Immortal and Deity. Our real
  seat has won a handful of several hundred games at Deity
  (`docs/CIV6_LADDER.md`), so that reference is far too weak. Those 14 tables
  are still in the log, marked `"reference": "wheels"`, and are not fitted.
- With `stock`, challengers won 0 of 120 tables at Immortal and Deity, 2 of 60
  at Emperor and 15 of 60 at King. That is the shape of the real record.
  (These were tables of the shape used until 2026-11-22: one challenger
  against the rung in every other seat.)

Pass `--reference <id>` with its own `--out` to try another.

The live ladder is the check on it. `docs/CIV6_LADDER.md` records our seat's
real games against the real AI at each level, and a clean win rate there gives
that level's Elo directly: with `p` the win rate at a table of `n`,
`rung − agent = 400·log10((1 − p) / (p·(n − 1)))`. That is the fit's reading
of a table of `n − 1` equal rivals, the shape
[that read our own rungs low](#why-a-rung-sits-alone), so expect it to
understate the rung. Since 2026-11-24 the benchmark's games are that check,
fitted level by level on every build of the site
([Fresh ratings](#fresh-ratings)), and they put every level from Prince to
Deity far under the stand-in's rung. Moving `CIV6_REFERENCE` to the roster
agent that agrees with them is still open.

`src/odds.rs` carries hand-set handicap constants (`HANDICAP_YIELD_ELO` and its
neighbours) for the win-odds annotation. The rung ratings here are the
measurement those constants were waiting for; reconciling them is open.

## The watched table

A world nobody is playing, which is what civvis.ai opens on, is a contest
between the agents. `Session::deal_watched_table` takes as many of them off the
top of the published ranking as the world has majors (`strategies::ranked`,
strongest first) and deals them round the seats by the world's seed
(`strategies::table`), so each agent holds one civilization and none of them
always moves first. A table larger than the roster seats the ranking again from
the top. A battlefield keeps the house fleet: the ratings were measured on whole
games.

Each agent plays exactly as its roster row is written, because that is what its
rating describes. The site's experiment list (`SITE_EXPERIMENT_GENES`) therefore
reaches a watched table only through a row that names the gene.

What the page shows of it:

- The standings carry **PLAYER** (the handle) and **ELO** (the published
  rating) after Watch as and before the start odds. ELO exists only at a table
  with a rated seat.
- A table whose every major is rated sits down at those ratings, with each
  one's bootstrap deviation, so the **START** odds differ by seat. A table with
  one unrated seat (a person, the house fleet) keeps the flat prior for
  everybody: the published ratings share one scale and nothing else is on it.
- **CIVVIS Arena Stats → Strategies** keeps a win–loss record per handle in
  the viewer's own browser. It is a tally of the games that browser watched, not a
  rating: it moves no number in `data/strategy_elo.json`.

A mirrored game of Civilization VI (`civvis play --mirror`, the page beside a
validation game) is not a watched table and is dealt nobody. PLAYER there says
who is really at each seat: every rival reads **8 Deity AI Firaxis**, the
number and name of whatever level the host reports (`strategies::civ6_ai_seat_json`), and our own seat reads
the lane the live decider is playing, **CIVVIS Diplomatic** for
`--victory diplomatic` (`strategies::live_seat_json`). `tools/follow.py` reads
the lane from the decider's own first record in `why.log` and sends it with
each frame as `mirror_strategy`. Neither is a roster agent, so neither carries
a rating and the table has no ELO column: the rung ratings above were measured
on our stand-in for Civilization VI's AI, not on that AI.
- The result card names the agent that won.

## Where a person sees it

- The setup panel's **Rival Civ AI Agent** select, under Difficulty, seats every
  rival major with one agent. Each option reads Elo first, then the handle
  (`1291 - TouristTrap`), then the nearest Civilization VI level and the
  tilt; the note under it is the whole ladder in Elo. `House mix` is the
  default fleet and changes nothing.
- The auto-play bar's **Civ AI** select hands your own seat to an agent. It
  lists the agents the same way, `<Elo> - <handle>`.
- Every list of agents is in Elo order, the highest first; an agent nothing
  has rated yet follows the rated ones under its handle alone.
- `/rules` publishes `strategies`, `civ6_ladder` and `rival_strategy`; `/new`
  accepts `rival_strategy` by id or handle and refuses a name nobody registered.
- **ELO Rankings** on the site (Charts/, `beta/elo.html`) lists the agents by
  Elo with the Firaxis levels the benchmark measures among them, each level's
  two readings side by side, and the Elo of the top ten agents and of Lv 4 to
  Lv 8 over time, every figure with the Diplomatic victory on and with it off.

Difficulty and rival are separate dials. Difficulty is what the rules hand the
rivals; the agent is how well they play. As measured on 2026-11-22 the
strongest unhandicapped agents rate between King and Emperor and none
reaches Emperor, so a harder game than that is a strong agent *and* a higher
Difficulty, and it will be harder than the rung's own number, which was
measured with the reference agent as the rival.
