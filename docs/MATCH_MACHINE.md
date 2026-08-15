# CIVVIS match machine

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
