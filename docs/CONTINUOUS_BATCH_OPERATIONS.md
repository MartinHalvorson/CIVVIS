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
| `--machine-cpu-ceiling PERCENT` | A governor thread measures the WHOLE host's CPU (`host_statistics` on macOS, `/proc/stat` on Linux) every two seconds and pauses the owned game process group with SIGSTOP/SIGCONT, in half-second slices, for as much of the next window as keeps the host at or under PERCENT. Overshoot is cut in the next window; headroom is retaken half the way per window. It governs a re-adopted segment after a daemon restart too, resumes the group on release, on segment exit and on SIGTERM, and never pauses when it cannot measure. |
| `--progress-minutes N` | One `progress` line every N minutes (default 5) in the service log: this batch's games and seats against the boundary, the totals over every batch the state directory has served, the rate and ETA, and the governor's last reading. |

`status` prints the governor's last reading (`cpu governor: host …% of ceiling
…%`) while the daemon runs, and `status --json` carries it as `governor` beside
`all_batches_games` / `all_batches_seats`.

Pausing stops every game in flight together, so it adds wall-clock noise to
the table's time-cost columns but cannot favour one genome over another. It
does not change a game's result: games are seeded and have no wall-clock rule.

Launch as a macOS `ProcessType` `Background` job (and a positive `Nice`) as
well: the kernel then keeps the games off the fastest cores and yields to
foreground work before the ceiling is ever reached. Example
(2026-09-30, `mbp-m5-max-128`, 18 logical cores): `--cpu-share 50
--machine-cpu-ceiling 90` gives nine workers and holds the host under 90%.

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
