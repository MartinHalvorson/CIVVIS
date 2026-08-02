# Committed league snapshot

`league.json` is a Glicko-2 strategy-league state (see `docs/LEAGUE.md`)
committed so every checkout can seat rated strategies and show elo and
usernames in the spectator HUD without first running a league locally.
`tools/spectator_supervisor.py` picks it up automatically (`--league auto`).
Each AI strategy is a named player, and `leader_elo` stores that player's
separate rating for every observed leader/civilization combination. Each exact
rating also records `last_played`, the UTC `YYYY-MM-DD` date of the latest game
credited to it; this makes the published Elo table's recency explicit.

Provenance: the founding rated rows are the result of 60 rounds x 16 games at
`--turns 250 --seed 7`, run 2026-07-23. This snapshot advances through round
150 with 91 additional rated games run on 2026-08-01 by the match machine at
8 players, 84 x 54 Standard, 12 city-states, Standard speed, Continents, no
teams, stock Civ 6 defaults, and a 500-turn limit. The live scheduler draws
from the managed roster with extra weight on each civilization's strongest
proven strategies. Two zero-game entrants were admitted after the founding run
so the existing league can measure agent families its original population
could never produce:

- `advanced_evolved`, carrying the `advanced` anchor's point rating with RD 350;
- `strategic`, starting at the ordinary 1500 / RD 350 new-player prior as a
  non-retiring search anchor.

Neither row imports games, wins, leader ratings, or a head-to-head Elo claim.
Their initial uncertainty was intentional: future league games determine their
ratings, with recorded exhibition games contributing only for live-eligible
entrants.

Pair ratings inherited from the founding run have `last_played` backfilled to
2026-07-23. Ratings touched by the match machine record 2026-08-01, and future
live and distributed games retain the UTC day on which each game finishes.

`strategic` is `league_only`: offline rounds schedule every active entrant and
can therefore rate it from the clean prior, while the live exhibition excludes
it until a separate current-profile turn-cost gate establishes that one search
seat fits the viewer-facing pace. The supervisor marks its `--league auto`
runtime copy as managed. On an idle load, required controller families missing
from an older managed copy are appended at fresh-entrant uncertainty; existing
ratings, records, lineage, and roster order are preserved. A pending distributed
round defers reconciliation until its immutable manifest has finalized.

To refresh it, run a league (`civvis league --rounds N --dir league`) and copy
`league/league.json` here. The repo-root `league/` directory stays gitignored
runtime state; only this snapshot is committed.
