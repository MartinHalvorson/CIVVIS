# Continuous batch operations

How to drive the standard-screen tournament that runs on a fleet machine under
launchd, from any shell or agent session, without touching its state by hand.
The tool is `tools/continuous_batch_scheduler.py`; every command below takes
`--state-dir <root>` (for example
`~/civvis-runs/2026-09-01-standard-10000-games-20jobs-r3`).

## What is running

- A launchd service (`com.civvis.gene-screen.standard-*`) runs
  `continuous_batch_scheduler.py run --state-dir <root> --repo <checkout>
  --goal-games N --jobs J --no-publish ...`. It pins a detached `origin/main`
  source under `<root>/sources/<commit>/`, builds one release binary, reserves a
  seed window, and appends six `kind: game` rows per finished game to
  `<root>/batches/<id>/rows-continuous.jsonl`.
- `<root>/scheduler-state.json` is the only truth about phase, reservations,
  deadline and publication. `<root>/logs/scheduler.log` gets one line per
  lifecycle event (`segment_started`, `cut_request_adopted`, `frozen_deadline`,
  `awaiting_publication`, `published`, `rotated`).
- Never count games with `wc -l` on the rows file (six rows per game plus
  headers). Use `status`.

## The four commands

| Need | Command | Notes |
| --- | --- | --- |
| Progress | `status` (add `--json` for one machine-readable object) | Read-only. Works while the service holds the lock. Shows games, remaining, games/hour and boundary ETA (once a segment started under this tool version), whether the daemon is running, and any pending cut request. |
| Refresh the table before the boundary | `cut` (`--in MINUTES` or `--at 2026-09-01T18:00:00Z`, default now; `--note` optional) | Writes `<root>/cut-request.json`. The running daemon adopts it within a second, stops the games at that instant, snapshots the validated prefix into `rows-deadline-cutoff.jsonl`, runs the analyzer, and exits `awaiting_publication` (with `--no-publish`). No kill, no restart, no state edit. A request that cannot apply is moved to `cut-request.rejected-*.json` with its reason. |
| Publish a frozen batch | `publish --repo <checkout> --goal-games N --machine <id> --publisher-agent <agent>` | One publication pass: claims a fleet task, regenerates the table, validates, commits, opens the PR, arms auto-merge and waits. Only valid in phase `frozen`. Re-run it if it stops early; every stage is idempotent. |
| Start the next batch | restart the service: `launchctl kickstart -k gui/$(id -u)/<label>` | The service loads the same arguments, sees `published`, rotates onto the merge commit and starts the next batch at the original goal. |

Pass the same `--goal-games` the service uses. A cut deadline is persisted by
the daemon itself, so the unchanged service arguments keep loading the frozen
state; no `--deadline-at` is ever needed for a cut.

## Timeline of one refresh (observed 2026-09-01)

1. `cut` → adopted within 1 s → games stopped, prefix frozen, analysis written
   in about a minute.
2. `publish` → PR open in about a minute; CI plus auto-merge took 35–40 minutes.
3. `kickstart` → new batch running on the merge commit within two minutes
   (release build is cached per commit).

## Starting a brand-new run

Only when a new state directory is wanted (new goal, new jobs count, new naming).
Copy the newest `com.civvis.gene-screen.standard-*.plist` in
`~/Library/LaunchAgents`, change the label, state dir, log paths and
`--seed-floor` (any value at or above the previous run's `next_seed` shown by
`status`), then `launchctl bootstrap gui/$(id -u) <plist>`. Otherwise reuse the
running state directory: rotation is what it is for.

## Sharing the machine

A host that also runs a live Civilization VI seat, builds or interactive work
should not give the tournament 85% of its cores. Three `run` options bound it:

| Option | Effect |
| --- | --- |
| `--cpu-share PERCENT` | Game workers = floor(PERCENT% of logical cores), never above the 85% cap; an alternative to `--jobs`. The pinned release build uses the same worker count as its `cargo --jobs`. |
| `--machine-cpu-ceiling PERCENT` | A governor thread measures the WHOLE host's CPU (`host_statistics` on macOS, `/proc/stat` on Linux) every two seconds and gives the owned games what is left under PERCENT. **Whole workers:** a segment played by several one-thread `gene_screen` processes runs that many of them and stops the others outright (SIGSTOP to the process, never to the group), so a running worker keeps its core, its cache and its clock speed. A cut is the whole overshoot in one window; room is retaken one worker a window, and only when a whole core is free. Stopped and running workers trade places every two minutes by CPU time, so every worker's seed window ends at about the same time. A worker another controller parked in darwin-background is stopped, and lifted out of that band when it resumes. **Slices:** a segment that is one process (the stock `--jobs N` segment) is paused with SIGSTOP/SIGCONT, in half-second slices, for as much of the next window as keeps the host at or under PERCENT; headroom is retaken half the way per window. There the games' demand is never taken below their worker count: a low-priority worker resumed on a saturated host is barely scheduled, and a duty sized from that starved reading would let every worker in as soon as the other work paused. Nor is it taken above the worker count plus one core, and a window counts the run the games really got (this thread wakes late on a saturated host): an uncapped mis-timed reading once latched seven workers at a 0.02-0.08 duty for six days (`mbp-m5-max-128`, 2026-10-09..15, demand 144.6 cores on 18). Either way it governs a re-adopted segment after a daemon restart too, resumes the group on release, on segment exit and on SIGTERM, and never pauses when it cannot measure. |
| `--beside NAME=PERCENT` | While a process with exactly that executable name runs (as `pgrep -x`), the ceiling is PERCENT instead. Repeatable; the lowest that applies wins, and none can raise the ceiling. For work that needs free cores rather than a share: a live Civilization VI game (`Civ6_Exe_Child`), a compiler (`rustc`). The progress line then reads `ceiling=…%(NAME)`. |
| `--pause-on-battery` | Pause the games (duty 0) while the host draws from its battery, or is plugged in but still discharging because the adapter cannot keep up (`pmset -g ps`, re-read every 10 s; unknown elsewhere, which never pauses). Logs each transition and marks the progress line `PAUSED_ON_BATTERY`. Turns the governor on at a 100% ceiling when no ceiling is given. Measured why on 2026-09-30: `mbp-m5-max-128` ran on battery from 82% to 1% in one hour under the host's whole load, hibernated for two hours, and took a live Civilization VI game down with it. |
| `--progress-minutes N` | One `progress` line every N minutes (default 5) in the service log: this batch's games and seats against the boundary, the totals over every batch the state directory has served, the rate and ETA, and the governor's last reading. |

`status` prints the governor's last reading (`cpu governor: host …% of ceiling
…%`) while the daemon runs, and `status --json` carries it as `governor` beside
`all_batches_games` / `all_batches_seats`.

Pausing cannot favour one genome over another: what is stopped is chosen by
the host's load and a worker's CPU time, never by what it is playing. It does
not change a game's result either: games are seeded and have no wall-clock
rule. It does stretch wall-clock time, and under whole workers it stretches
one game a great deal and its neighbour not at all, so a wall-clock time-cost
column gets noisier.

Why whole workers (measured 2026-11-22 on `mbp-m5-max-128`, build
`a57ffc68c6`): nine workers sliced together at a 0.05-0.7 duty spent 350 ms
of CPU a game turn over 429 games; the same binary spends 243 ms with
eighteen workers running flat out and about 200 ms alone. A sliced worker
restarts on a cold cache and a slowed core twice a second.

Launch the launchd job as `ProcessType` `Standard` with a positive `Nice`
(15 on `mbp-m5-max-128`), **not** `Background`. Measured 2026-09-30 on that
host (18 logical cores: 6 Super + 12 Performance): under `Background` every
game worker ran at scheduler priority 4 on the lower cluster only, and nine
workers got at most 2.5 cores while the host was 70% busy with five cores idle.
The same nine workers at `Standard` + Nice 15 (priority 20) burn 9.4–9.7 cores
whenever the governor lets them run; Nice keeps them behind other work, and the
ceiling keeps the host's total down. A thread's QoS is fixed when it is
created, so `taskpolicy -B -p <pid>` does not rescue a running segment:
changing the class takes a new segment (stop the game group, restart the
service; its unplayed seeds are retired, never replayed).

**Beside a live Civilization VI seat, choose `Background` anyway.** The live
game is the one workload on the host that fails rather than slows under load
(it froze in "PLEASE WAIT" three times at load 23–30 on 2026-09-30, and its
screenshot/OCR path is already documented to fail silently under load), so the
live lane asked for the lowest class and got it: `ProcessType` `Background` +
`Nice` 19 in the plist. Here the trade goes the other way: a slower tournament
beats a lost live game. The opposite direction does work in place:
`taskpolicy -b -p <pid>` moves a RUNNING game process to darwin background
(its workers went from priority 20 to 4 at once), and `renice -n 19 -g <pgid>`
lowers it too (on macOS `-n` is an increment, capped at 20). Do not pause the
tournament with `kill -STOP`: the governor resumes what it does not hold.

Do not pin the workers to the lower cluster with a QoS clamp to make them
"efficiency core work". Measured 2026-11-22 on that host, one 90-turn game,
three runs each: plain or Nice 15, 4.3-5.8 s at 3.8-4.3 GHz; `taskpolicy -c
utility`, 9.7-11.7 s at 2.8-3.1 GHz; `taskpolicy -c background` or `-b`,
19-36 s. The lower cluster under a clamp runs this workload at under half
speed. At the default class the same cores cost far less: eighteen workers
flat out, twelve of them necessarily on the lower cluster, spent 243 ms of CPU
a turn against about 200 ms for one worker alone. Nice already puts any
ordinary thread ahead of a worker.

Example (same host): `--cpu-share 80 --machine-cpu-ceiling 80 --beside
Civ6_Exe_Child=70 --beside rustc=60` gives fourteen one-thread workers, runs
as many as keep the host at 80%, stands back to 70% while a live game is up
and to 60% while anything compiles. The 60 is not arbitrary on that host:
`~/bin/civvis-cpu-headroom` parks batch work and compilers when under 20% of
the host is idle beside a live game and lifts them only above 40%, so a
tournament that refilled the host past 60% would hold a parked compiler down.

## Do not

- Do not SIGINT or kill the daemon to stop a batch. Use `cut`; the kill path
  burns the reserved seed window and leaves no deadline record.
- Do not edit `scheduler-state.json` by hand.
- Do not delete `cut-request*.json` files; they are the audit trail.
- Do not start a second `run` against a live state directory; the lock refuses it.

## Upgrading the tool under a live run

`launchctl kickstart -k gui/$(id -u)/<label>` restarts only the daemon. The
game process runs in its own session, so it survives; the new daemon reads the
live reservation, sees its PID alive and re-adopts it (`active_segment`). Pull
the checkout named by the service's `--repo` first, then kickstart. `cut` and
the rate/ETA in `status` need the daemon (and, for the ETA, the segment) to
have started under the new code.
