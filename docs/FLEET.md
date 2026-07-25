# Fleet: improving the AI on whatever machines are up

`tools/civvis_fleet.py` runs the strategy league across every machine that
happens to be available, and — more importantly — keeps checking that the
games it is running still teach it something.

```bash
tools/civvis_fleet.py probe          # who is up, how many cores, which revision
tools/civvis_fleet.py deploy         # origin/main + release build everywhere
tools/civvis_fleet.py run            # keep a worker alive on every host
tools/civvis_fleet.py status         # is the league still learning anything?
tools/civvis_fleet.py stop
```

With no configuration this is one machine, which is a valid fleet. Add hosts
in `~/.civvis-fleet.json`:

```json
{
  "home": "local",
  "league_dir": "/Users/me/civvis-fleet/league",
  "rounds": 50, "games": 24, "players": 6, "turns": 250,
  "hosts": [
    {"name": "local", "transport": "local", "root": "/Users/me/civvis-fleet", "jobs": 6},
    {"name": "spark", "transport": "ssh", "ssh": "spark",
     "root": "/home/me/civvis-fleet", "jobs": 8}
  ]
}
```

## Why this is safe without a coordinator

`civvis league` already knows how to share one rating period between many
simulators (see [LEAGUE.md](LEAGUE.md)): a round is an immutable manifest,
jobs are claimed atomically, results are immutable and publish exactly once,
and finalization requires the complete result set and is deterministic — two
workers that both see a finished round compute byte-identical ratings. What it
assumes is a shared filesystem.

The fleet supplies the transport instead of a coordinator. It rsyncs the league
directory between hosts, which works precisely because of the properties above:
the only mutable file is `league.json`, and every host that can write it
computes the same bytes. A late duplicate is harmless, a partial round simply
does not finalize, and there is no lock to lose.

The consequences are the ones you want from a fleet:

- **A machine that is down costs one timeout.** It is skipped, and adopted
  again on the cycle after it comes back. Nothing wedges.
- **A machine that dies mid-game loses that game.** Its claims expire on the
  league's own lease and the jobs are replayed by whoever is left.
- **A machine can be rebuilt from nothing.** `deploy` clones the repository
  into a private detached `origin/main` worktree under the fleet's own root and
  builds it. It never touches a development checkout — the repository's rules
  forbid an automated build from mutating one, and this obeys that by only ever
  resetting a directory it created itself.

## The check that matters

A league is very good at looking healthy. It produces rounds, ratings,
leaderboards, breeding events and retirements whether or not its games decide
anything — and when a roster converges, they stop deciding anything while every
artifact keeps printing.

`fleet status` therefore does not report throughput. It asks
[`civvis rating`](RATING.md) how many nats per game the ratings actually knew
that guessing did not:

```
$ tools/civvis_fleet.py status
league /Users/me/civvis-fleet/league
  round 1072, 12 active strategies
  302 games scored, 6.0 seats on average
  best information per game: -0.0087 nats
  verdict: STALLED — no system beats guessing on these games: the roster has
  converged, seating is confounded, or games are ending on the turn cap.
  Fix the experiment, not the estimator (see docs/RATING.md)
```

That is a real reading, taken from the live exhibition league after a thousand
rounds. A freshly seeded league on the same machine reads `LEARNING +0.52`.
`status` exits non-zero when the league is stalled, so it works as a monitor.

The three causes it names are the three that have actually happened here:

1. **The roster converged.** Selection bred every entrant from the same few
   parents until the active pool was near-clones. Nine active genomes shared
   four identical values of `city_target` and `faith_builder`. A league of
   clones cannot rank anything. Keep the anchors, keep the niches, and widen
   mutation before adding more rounds.
2. **Seating is confounded.** Assigning each civ to whoever is rated best on
   it makes strategy and civilization the same variable — one strategy played
   Rome 200 games out of 200. Use `rating::rotate_seating`, or the league's own
   mirrored series, so every strategy draws every civ.
3. **Games end on the turn cap.** 62% of the audited games ended as score
   truncations at turn 250, where the winner is whoever is biggest rather than
   whoever played best. Raise `turns`, or read the victory mix in
   `matches.csv` before trusting the ratings.

## Adding a machine

1. Give it SSH access from home with a key (`BatchMode=yes` is used, so a
   password prompt reads as "down").
2. Install git and a Rust toolchain. `~/.cargo/bin` is added to `PATH` for
   every command the fleet runs, so a default `rustup` install is enough.
3. Add it to `hosts` and run `probe`, then `deploy`.

`jobs` defaults to the host's core count minus two, so a machine someone is
sitting in front of stays usable. Set it explicitly to be stricter.
