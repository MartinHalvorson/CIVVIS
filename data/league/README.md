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
- `strategic`, carrying the same zero-delta prior and RD 350 as a non-retiring
  search anchor.

Neither row imports games, wins, leader ratings, or a head-to-head Elo claim.
Their uncertainty is intentional: future league and recorded exhibition games
must determine their ratings. The snapshot remains at round 60 until such games
are actually recorded.

To refresh it, run a league (`civvis league --rounds N --dir league`) and copy
`league/league.json` here. The repo-root `league/` directory stays gitignored
runtime state; only this snapshot is committed.
