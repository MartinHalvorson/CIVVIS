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
- **Rung tables** measure a Civilization VI difficulty level. One roster agent
  is the challenger and sits on the terms a person gets at that rung. Every
  other seat is the reference agent (`strategies::CIV6_REFERENCE`) with the
  rung's shipped AI handicap from `data/difficulties.json`. The three rivals
  are rated as one entrant, `civ6:<rung>`.

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

## What the Civilization VI mapping does and does not prove

It measures, in our engine, what each rung's *handicap* is worth: the yield
percentages, Combat Strength, free units, era boosts and starting buildings the
real game gives its AI above Prince, and the Combat Strength and experience it
gives a person below it. Those tables are Firaxis's own rows.

It does not measure Firaxis's AI. The rivals at every rung are our reference
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

Pass `--reference <id>` with its own `--out` to try another.

The live ladder is the check on it. `docs/CIV6_LADDER.md` records our seat's
real games against the real AI at each level, and a clean win rate there gives
that level's Elo directly: with `p` the win rate at a table of `n`,
`rung − agent = 400·log10((1 − p) / (p·(n − 1)))`. Most recorded attempts were
stopped or abandoned rather than played out, so they are not yet a win rate.
When a rung has enough finished games, compare, and move `CIV6_REFERENCE` to
the roster agent that agrees with them.

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
- **Arena Stats → Strategies** keeps a win–loss record per handle in the
  viewer's own browser. It is a tally of the games that browser watched, not a
  rating: it moves no number in `data/strategy_elo.json`.
- The result card names the agent that won.

## Where a person sees it

- The setup panel's **Rival Civ AI Agent** select, under Difficulty, seats every
  rival major with one agent. Each option shows the handle, the tilt, the Elo
  and the nearest Civilization VI level; the note under it is the whole ladder
  in Elo. `House mix` is the default fleet and changes nothing.
- The auto-play bar's **Civ AI** select hands your own seat to an agent.
- `/rules` publishes `strategies`, `civ6_ladder` and `rival_strategy`; `/new`
  accepts `rival_strategy` by id or handle and refuses a name nobody registered.

Difficulty and rival are separate dials. Difficulty is what the rules hand the
rivals; the agent is how well they play. As measured on 2026-11-11 no
unhandicapped agent of ours rates nearer Emperor than King, so a harder game than that is a strong agent *and* a higher
Difficulty, and it will be harder than the rung's own number, which was
measured with the reference agent in the rival seats.
