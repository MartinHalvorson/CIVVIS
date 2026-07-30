# Committed league snapshot

`league.json` is a Glicko-2 strategy-league state (see `docs/LEAGUE.md`)
committed so every checkout can seat rated strategies and show elo and
usernames in the spectator HUD without first running a league locally.
`tools/spectator_supervisor.py` picks it up automatically (`--league auto`).
Each AI strategy is a named player, and `leader_elo` stores that player's
separate rating for every observed leader/civilization combination.

Provenance: the rated rows are the result of 60 rounds x 16 games at
`--turns 250 --seed 7`, run 2026-07-23. Two zero-game entrants were admitted
after that run so the existing league can measure agent families its original
population could never produce:

- `advanced_evolved`, carrying the `advanced` anchor's point rating with RD 350;
- `strategic`, starting at the ordinary 1500 / RD 350 new-player prior as a
  non-retiring search anchor.

Neither row imports games, wins, leader ratings, or a head-to-head Elo claim.
Their uncertainty is intentional: future league games must determine their
ratings, with recorded exhibition games contributing only for live-eligible
entrants. The snapshot remains at round 60 until such games are actually
recorded.

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
