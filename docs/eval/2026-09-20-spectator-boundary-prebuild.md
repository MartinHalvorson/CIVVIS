# Preserve an active spectator build at a game boundary

While deploying `fb0acb06f7455a8158868eb0067387d4c5c9a03e` on the MacBook Pro,
the spectator began compiling that revision during an active game. The game
ended in a turn-250 draw at 11:53:02 UTC. The supervisor stopped the active
prebuild and immediately started a second build of the same revision. Cargo
PID 92960 was replaced by PID 4615. The revision eventually went live near
12:00 UTC. This observation concerns the separate spectator service, not a
native Civilization VI victory or defeat.

The cancellation was intentional: `prepare_latest_once` refreshes the shared
private source worktree, so an old compiler must not keep reading it during
that refresh. However, cancelling before checking whether it can finish loses
work even when the source revision is unchanged.

At a finished-game boundary, the supervisor now gives a running prebuild up to
600 seconds from the first finished observation to finish. It returns to the
normal polling loop between checks, so operator halt and player takeover still
interrupt the wait. A completed or failed worker proceeds to the existing
canonical refresh and runtime verification; an expired grace period stops the
worker before that refresh. A successful prebuild never substitutes for the
fresh canonical-head check. The previous source-isolation and fallback rules
are preserved.

Repeated polls of the same finished world do not download its full state
again. The first observation still supplies the archive and successor settings;
subsequent polls retain lock-free staged settings and user-action monitoring.
This preserves the one-full-observation-per-finished-game behavior while waiting.

## Validation

The new whole-supervisor-loop regression failed against baseline because the
boundary refreshed before the active compile completed. After the change,
115 supervisor tests passed, including five new loop scenarios: completion,
failed build, grace expiry, operator halt, and player takeover. They verify
that canonical preparation occurs after completion/expiry and before retiring
the incumbent server, and that halt/takeover can prevent successor launch.
The existing zero-grace boundary case also continues to pass.

No engine rules or AI decisions change. These tests exercise the supervisory
control flow; no native victory or measured build-time saving is claimed.
The live incident and before/after test logs are retained in
`~/civvis-climb-logs/spectator-prebuild-boundary-investigation.json` and
`/tmp/civvis-prebuild-*.log`.
