# A verification host, from the tracked tree

The live Civ VI verification lane — a Mac that plays the deployment genome
against Firaxis's game, indefinitely, on every fresh `origin/main` — is mostly
tracked here already: `tools/ops/civvis-game-supervisor.sh` runs the cycle,
`civvis-interactive-host.sh` owns it, `civvis-ladder-terminal-launcher.sh`
hosts it in Terminal, `ladder_watchdog.py` restarts it, and
`civvis_collab.py bootstrap` installs those as launchd services.

What was **not** tracked until 2026-08-28 was the layer the operator actually
touches: the switch that turns the lanes on and off, the wrapper that pins
every game to the operator's policy, the retention job that keeps the disk
alive, and the two launchd agents that run them. They lived in one machine's
home directory (`~/bin/civvis-games`, `~/civvis-verified-head-launcher.zsh`,
`~/.local/bin/civvis_run_prune.sh`, two hand-written plists and a block of
`export`s in `~/.zprofile`), named that machine's home directory outright, and
had never been seen by a CI run. This page is how any Mac gets the same loop.

## The pieces

| tracked file | installed as | job |
| --- | --- | --- |
| `tools/ops/civvis-games.sh` | `~/bin/civvis-games` (symlink) | `on` / `retire` / `off` / `status` / `wins` / `ensure` — the lane switch and explicit verification authorization |
| `tools/ops/civvis-verified-head-launcher.sh` | `~/civvis-verification-launch.command` (symlink) | the entry point every start goes through: fresh `origin/main`, deployment genome, this host's policy, nothing inherited from the window |
| `tools/ops/civvis-run-prune.sh` | — | removes every run directory older than 24 h; never the ledgers or an open run |
| `deploy/com.civvis.keepplaying.plist` | `~/Library/LaunchAgents/…` | `civvis-games ensure` every 5 min |
| `deploy/com.civvis.run-prune.plist` | `~/Library/LaunchAgents/…` | `civvis-run-prune.sh` hourly |
| `tools/ops/civvis-install-host-automation.sh` | — | wires all of the above, idempotently |

Symlinks, never copies: a home copy of a tracked script is exactly what
`tools/test_ops_portability.py` exists to stop, because a fix landed in
`tools/ops/` then reaches a file nothing runs.

## Install on a new Mac

1. **The game.** Steam, Civilization VI with the expansions the ladder plays,
   and the Terminal grants the harness needs (Screen Recording, Accessibility,
   App Management — `docs/CIV6_COMPUTER_CONTROL.md`). The loop must start
   *through Terminal*; launchd cannot install the control mod
   (`tools/ops/ladder_watchdog.py` explains why).
2. **Two trees.** One clone whose `main` branch the freshness service keeps
   current (the tree you run the installer from), and one tree the games run
   in — the supervisor `git checkout --detach origin/main`s it every cycle, so
   it cannot be the worktree holding `main`. A second clone or a detached
   worktree of the first is fine.

   ```bash
   git clone https://github.com/MartinHalvorson/CIVVIS.git ~/civvis-main
   git -C ~/civvis-main worktree add --detach ~/CIVVIS origin/main
   ```
3. **The managed services.** `python3 ~/civvis-main/tools/civvis_collab.py
   bootstrap` installs the ladder keeper, the memory guard and, where the host
   serves it, the exhibition — see `docs/SPECTATOR_DEPLOY.md`.
4. **This layer.**

   ```bash
   ~/civvis-main/tools/ops/civvis-install-host-automation.sh --head-repo ~/CIVVIS
   ```

   It links `~/bin/civvis-games` and `~/civvis-verification-launch.command`
   to the tracked scripts, renders and loads the two launchd agents, writes
   `~/.civvis-verification-policy` once (seeded from any `CIVVIS_*` exports in
   the shell you ran it from, so a `.zprofile`-style setup migrates itself),
   and re-renders the keeper so a recovery starts the wrapper rather than the
   stock launcher. `--dry-run` shows the plan; rerun it after every pull that
   touches these files (the symlinks make that a no-op for the scripts).
5. **Policy, pin, on.**

   ```bash
   $EDITOR ~/.civvis-verification-policy     # the rung, at least
   printf 'head\n' > ~/.civvis-play-pin       # games track origin/main
   civvis-games on
   tail -f ~/Library/Logs/civvis-ladder.log
   ```

## The policy file

`~/.civvis-verification-policy` is the only place a host's verification policy
lives. `KEY=VALUE` lines, `#` comments, and only these keys are honoured —
anything else is logged and ignored, an invalid value refuses the launch (and
says so in `civvis-ladder.log`, because the Terminal window is not a log):

```
CIVVIS_HEAD_REPO=/Users/you/CIVVIS      # the game tree; default: the wrapper's own tree
CIVVIS_DIFFICULTY=DIFFICULTY_KING       # absent: the read-only ladder policy picks the rung
#CIVVIS_VICTORY=                        # a victory lane, if the operator wants one
#CIVVIS_PLAY_ATTEMPTS=1                 # games per cycle; 1 = every game builds a fresh origin/main
#CIVVIS_RESTART_BELOW_LEADER_RATIO=     # absent: the harness default (0.60 after t150); 0 plays every game out
#CIVVIS_PLAY_TIMEOUT=10800              # seconds per game
#CIVVIS_PLAY_TIMEOUT_CEILING=14400
```

The wrapper unsets every other `CIVVIS_*` variable that would change what the
seat plays — a labelled experiment, a retired strategy, an alternate host, an
old restart policy — before it hands over. Change the file, then restart the
chain at a boundary (`civvis-games off` … `civvis-games on`); the environment is
fixed at process start, so an edit alone reaches nothing.

Forced genome arms are a different file (`~/.civvis-live-force-on`, read per
batch) and the revision pin is another (`~/.civvis-play-pin`, read per cycle);
`civvis-games status` reports all three beside the revision the next batch
will build.

## Day to day

```
civvis-games status     the switch, the services, the entry point, what will build, what is playing, recent wins
civvis-games on         clear any halt, record intent=running, start the chain through Terminal, reset the pin to head
civvis-games retire     request Civ VI's native Retire action for exactly one active, already-playing harness; record operator_retired and keep the lane on for its replacement
civvis-games off        halt AND tear the live chain down, youngest first (TERM only on the Civ VI core)
civvis-games wins 20    the last twenty live-game wins from the ladder ledger
```

`ensure` is what `com.civvis.keepplaying` runs: it recovers a missing chain only
while the intent file says `running` and no explicit halt is present. It never
clears a halt or turns a missing intent into permission; only `civvis-games on`
records `running` and clears the halt. `off` writes `stopped`, so nothing
restarts until the operator says `on`. A raw `gamelock.py --resume` clears only
the low-level halt and does not authorize or start verification. To end just the
current game while continuing the indefinite lane, use `retire`, not `off`: it refuses setup/no-turn and
ambiguous harnesses, asks the installed control mod to invoke Civilization
VI's native Retire action, waits for its `retired` acknowledgement, and leaves
a durable request/status/result sidecar in that run.

## The "App Background Activity" alert

Every new LaunchAgent label a Mac registers makes Background Task Management
post a persistent Notification Center alert — on macOS 26: *"'zsh' can run in
the background. You can manage background activity in Login Items &
Extensions."* It sits top-right over the game being recorded, and a click on
it opens System Settings on that pane in front of everything. It cannot be
pre-approved without MDM, and a verification host keeps registering labels
(each new clone that runs `bootstrap` installs its own
`com.civvis.freshness.<hash>` agent).

`tools/ops/civvis-foreground-guard.sh` dismisses exactly that alert and closes
exactly that Settings pane, the latter only while a game lane is up. The
wrapper starts it, because it has to be Terminal-descended — driving
Notification Center is an Apple Event to System Events, and macOS grants
Automation to the responsible process, which for the whole chain is Terminal;
a launchd job holds nothing. `--once` runs a single pass by hand;
`touch ~/.civvis-foreground-guard-off` stands it down; `CIVVIS_FOREGROUND_GUARD=0`
in the wrapper's environment skips it. Log: `~/Library/Logs/civvis-foreground-guard.log`.

## What stayed out of the repository, and why

- **`~/.zprofile` exports** (`CIVVIS_DIFFICULTY`, `CIVVIS_PLAY_ATTEMPTS`,
  `CIVVIS_RESTART_BELOW_LEADER_RATIO`, the timeouts). Superseded by the policy
  file: the wrapper unsets them. A host may keep them for interactive use;
  they no longer reach a game.
- **The `screencapture` shim** (`~/bin/screencapture`,
  `~/civvis-safe-screencapture.py`) that routed the harness's screenshots
  through CoreGraphics to avoid the Screen Recording consent sheet. #2678
  moved that into `civ6_play.screenshot()` itself, so the shim is dead code.
- **The promising-game handoff scripts** (`~/civvis-promising-game-*.zsh`,
  `~/civvis-post-promising-handoff.zsh`, `~/civvis-promising-popup-guard.py`).
  One-off recoveries for a single run tag, written against one host's process
  table; what they were for is in the harness now (#2676, #2681).
