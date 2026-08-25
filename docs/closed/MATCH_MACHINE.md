# CIVVIS match machine (retired 2026-08-23)

> **Closed.** `tools/civvis_match_machine.py` and the league it rated into were removed on 2026-08-23 (#2357, operator: *"lets remove the league /
> elo work for now"*). The gene screen (`docs/GENE_SCREEN.md`) prices
> behaviours; nothing rates named agents against each other for now, and the
> deployment genome is the gene ledger's default-on set. A rating system for
> finished genomes is planned to return — see `docs/ROADMAP.md`. This document
> is kept as the record of how the retired instrument worked and what it
> measured.

`tools/civvis_match_machine.py` is the unattended evaluator for a bounded
operator window. It runs exactly one browser-visible match and then keeps up
to eight headless matches active. Every match is pinned to the requested stock
Civ VI contract:

- eight major civilizations, no teams;
- Standard map size (84x54, 12 city-states), Continents, flat with poles;
- Online speed and its natural 250-turn budget;
- Ancient start, stock leader pool, barbarians, disaster intensity 2, all
  victory conditions, and no optional game modes.

The server seats every civilization from the mutable league. Its normal
selection samples the top three eligible strategies at the table size — with
the entrants rotated across civilizations by Latin square over the league
round — and avoids duplicates until the active roster is exhausted. This
emphasizes proven players without collapsing the table to one strategy.

On top of that, each headless game pins one focus seat (`--force-strategy`)
from a coverage schedule the operator maintains: a full pass over every
active entrant ordered by **measurement need** — fewest games at the pinned
eight-seat table first, widest rating deviation breaking ties — followed by
the top eight by rating as the exploitation tail. Live selection now breeds
and retires from these very games, so the schedule rebuilds the moment the
active-name set changes: a newborn sorts to the front and is focused within
a few launches, and a retiree stops burning focus slots at the next one.
Rating drift alone never resets the cycle.

## Start a 24-hour run

Run the operator in the foreground and give it the PID of the terminal shell
that owns the tab:

```bash
python3 tools/civvis_match_machine.py \
  --watch-pid "$PPID" \
  --duration 86400 \
  --headless 8 \
  --max-processes 8 \
  --limit 70 \
  --speed online
```

`--speed` defaults to `online`, with the stock 250-turn Online limit. The
operator derives the natural turn limit for every supported Civ VI speed;
`--turns` is available only when an explicit non-stock cap is intended.

The default of eight total game processes means the visible game temporarily
occupies one slot; after it finishes, all eight slots are headless. Closing the
watched terminal stops every process group. On macOS, a matching `caffeinate`
assertion keeps the machine awake on AC power until the operator exits, which
supports clamshell operation without leaving an orphaned always-awake service.

## What stops it, and how to tell which one happened

⚠⚠ **Nothing on any machine restarts this.** There is no launchd job, no cron
entry and no workflow; the only launcher is the command below, typed into a
shell. `--watch-pid` then binds the run to that shell's life on purpose, so
whatever ends the shell ends the run.

That is not hypothetical. On **2026-08-15T08:59:11Z** the machine was killed
mid-window — it had 17 h 51 m of its 24-hour window left — because it had been
started as a background task of an agent session, and the session ended. Every
row of `AI_PLAYER_ELO_RANKINGS.md` then read "last played 2026-08-15" for eight
days, and **79 rated eight-player games sat unpublished** in
`target/match-machine/league/matches.csv` (rounds 4120–4198) the whole time.
The operator halt of 2026-08-19 is four days later and is not the cause; it
does gate the exhibition arm today.

Eight days is how long it took because the record could not tell a kill from a
finished window. `state.json` said

```json
"reason": "stopped", "deadline_utc": "2026-08-16T02:50:04+00:00"
```

— a completed window writes exactly that. Every other way the loop can end
already names itself in `events.jsonl` (`terminal_closed`,
`operator_window_ended`, `fatal`); the signal path was the one silent case, and
the signal handler discarded its argument. It no longer does. `reason` and the
`machine_stopped` event now carry a cause and the unspent time:

| `stop_cause` | what happened |
| --- | --- |
| `stopped:sigterm` / `stopped:sigint` | something killed it — read `seconds_unspent` |
| `stopped:window_ended` | `--duration` or `--deadline-utc` ran out; publish the results |
| `stopped:loop_exit` | the loop returned with time left and no signal |

Two smaller traps in the same launch, both observed: the tool dies with
`fatal: [Errno 2] ... 'cargo'` unless `~/.cargo/bin` is on `PATH`, and on a
crash `machine_stopped` is emitted from the `finally` block *before* `fatal`,
so the log reads backwards.

Publishing is a manual pull request either way — see `data/league/README.md`.
Results already on disk can be published without playing anything.

## Durable evidence

The runtime directory defaults to `target/match-machine/` and contains:

- `league/matches.csv`, `ratings.csv`, `calibration.csv`, and `league.json` —
  canonical CIVVIS match and Glicko evidence;
- `AI_PLAYER_ELO_RANKINGS.md` — regenerated after every result boundary;
- `events.jsonl` — revisions, builds, game seeds, outcomes, and failures;
- `resources.jsonl` — host-wide CPU, memory, disk, GPU, and thermal samples;
- `state.json` — an atomic, compact operator status;
- `logs/` — one server log per game.

The operator fetches `origin/main` every five minutes. A new revision closes
the launch gate, lets active games finish, resets only its private detached
build worktree, builds and validates HEAD, then starts new matches from the
promoted immutable binary. It never pulls, commits, or edits a development
worktree. The build timeout charges only unpaused execution: pressure pauses,
visible-game yields, and the CPU duty cycle stop the clock, so a busy host
slows a revision down without rejecting it.

CPU, memory, filesystem capacity, and Apple GPU utilization are measured
host-wide. At 70% the operator stops game process groups, preferring headless
games; it resumes one at a time only below 60%. Any macOS thermal or performance
warning is also a hard stop gate. The simulator is CPU-only; the single visible
browser is the only deliberate graphics consumer. Because of that, the early
shed and resume margins are measured against CPU, memory, and disk only:
another process rendering on the GPU below the limit slows nothing down, while
GPU at the limit itself remains a full stop.
