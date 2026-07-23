# Committed league snapshot

`league.json` is a Glicko-2 strategy-league state (see `docs/LEAGUE.md`)
committed so every checkout can seat rated strategies and show elo and
usernames in the spectator HUD without first running a league locally.
`tools/spectator_supervisor.py` picks it up automatically (`--league auto`).

Provenance: 60 rounds x 16 games at `--turns 250 --seed 7`, run 2026-07-23.

To refresh it, run a league (`civvis league --rounds N --dir league`) and copy
`league/league.json` here. The repo-root `league/` directory stays gitignored
runtime state; only this snapshot is committed.
