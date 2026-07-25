# Running the spectator loop as an always-on visual test

`tools/spectator_supervisor.py` is the production visual test: it plays the
latest `origin/main` build on screen, one full game after another, and rebuilds
the newest code *while the current game is still running*. As soon as that
binary is verified, it checkpoints the active game and resumes it on the new
image; deployment does not wait for a potentially hour-long victory boundary.
It is one cross-platform program; only the way you keep it alive differs per OS.

## What the loop does

1. Builds `origin/main` in a **private worktree** (never the shared checkout, so
   a session's uncommitted edits never reach the screen) and promotes a
   known-good binary.
2. Serves that binary in `--spectate --supervised` mode; the game auto-steps to
   a decision.
3. **During** the game it fetches `origin/main` and, if it moved, compiles it in
   the background. It identifies deployments by the promoted binary hash, not
   only by the runtime-input hash, so every newly stamped commit reaches the
   live `/status` endpoint even when its functional inputs are unchanged.
4. A verified update captures a safe checkpoint, replaces the server, and
   resumes the same active match on the fresh binary. If the build or checkpoint
   is not ready, the last verified server stays live and the supervisor retries;
   the loop never stalls on slow or broken source.
5. When a winner appears it archives the result, keeps it on screen for
   `--cooldown` seconds (default 5), then deals the next game on the freshest
   verified build.
6. Crash/stall recovery: active games are checkpointed every few seconds and
   resumed; a wedged game is nudged, then quarantined rather than looped on.

The supervisor also updates **itself**: when `tools/spectator_supervisor.py`
changes on `origin/main` it re-execs the canonical script under the live game
(`os.execv`), so a machine set up once tracks fleet development with no manual
step.

Watch any host with:  `tail -f <deploy-checkout>/spectator-supervisor.log`

## Deploy checkout

Run the supervisor from a **dedicated checkout pinned to `origin/main`**, not the
multi-session shared checkout:

```bash
git -C <shared-checkout> worktree add --detach <...>/civvis-spectator origin/main
```

`ROOT` (where it fetches and stages the runtime) then derives from that path;
the private build worktree is `<...>/civvis-spectator-spectator-src`.

## Windows (Task Scheduler)

```powershell
powershell -ExecutionPolicy Bypass -File deploy\register_spectator_task.ps1 `
    -DeployRoot C:\Users\<you>\PycharmProjects\civvis-spectator
Start-ScheduledTask -TaskPath '\Martbot\' -TaskName 'Civvis Spectator'
```

The task runs `pythonw.exe` (windowless); the supervisor gives every child
process `CREATE_NO_WINDOW`, so nothing ever flashes a console — the operator
terminal policy is honored. A per-port lock inside the supervisor guarantees a
single instance even across a self-update, so the logon + 5-minute recovery
triggers can fire freely.

## macOS / Linux (launchd)

```bash
cp deploy/com.civvis.spectator.plist ~/Library/LaunchAgents/
#  edit the four __PLACEHOLDER__ paths (python3, deploy checkout, home)
launchctl load -w ~/Library/LaunchAgents/com.civvis.spectator.plist
```

`KeepAlive` relaunches on exit; `RunAtLoad` starts it at login. On macOS
`os.execv` keeps the same PID, so self-updates are invisible to launchd.
On a bare Linux host, `nohup python3 tools/spectator_supervisor.py … &` under a
systemd unit or a `while` wrapper works the same way.

## One complete frame per turn

Every turn the exhibition plays is owed at least one frame, and that frame has
to carry the whole turn: player stats, victory tracker, map and units, all out
of the same snapshot. The server keeps the first half by holding a finished
turn until an active viewer has been handed it — at **every** pace, not only at
Lightning. A turn budget says how long a turn lasts; it never promised that
anybody managed to read it, and a page that paints slower than the budget used
to lose turns silently.

Only a page that paints holds the simulation to a turn. The browser reports the
turn it last drew on each `/state` poll (`?painted=<turn>&world=<seed>`), so the
keeper's refresh check and any `curl` read the same state without dragging the
exhibition down to their own cadence. A viewer that stops asking is dropped
after a few seconds and costs the unattended exhibition exactly one turn's
delay.

**Every viewer is owed every turn, and each is waited for on its own.** Two tabs
on one exhibition used to take alternate turns: the stepper released a turn as
soon as either had been handed it, so each saw half the game — and the audit,
reading the same single cursor, called it perfect, because between them they had
seen it all. A page names itself with `?viewer=<id>` and gets a seat of its own.
The cost is that a turn now waits for the slowest tab watching, which is the
promise working rather than failing.

**A turn reaches the glass, not just the socket.** The page draws at most one
turn per animation frame. Two turns drawn inside one display refresh are
composited into one, so a turn drawn faster than the screen can show it is still
a turn nobody saw. Running on the display's clock also removed the fixed 100ms
between polls, which was a ceiling of ten turns a second however fast the world
could be played.

**The map is sent as a patch.** A page says which tile array it is holding
(`&have=<world>:<turn>`) and is sent only the tiles that changed — about a dozen
of 2252 on a standard turn, so a poll costs ~157 KB instead of ~1.36 MB, an 89%
saving. Saying so is also what parks the poll until there *is* a next turn, so a
finished turn is written to a socket the moment it exists rather than at the
page's next tick. A page that reports no baseline, including every health check,
is answered immediately with the whole map.

That report is also what makes the promise auditable while it runs. `/status`
carries:

- `frames_missed` — turns simulated that some attached viewer never drew. **Zero**
  on a healthy exhibition, and kept for the server's whole run rather than reset
  when the tab that missed them closes.
- `frames_painted` — the last turn a viewer reported drawing, or `null` when
  nobody is watching. Zero misses with nothing painted means nobody was there,
  which is not the same as the promise being kept.
- `viewers` — how many pages that promise is being kept to, and so how many
  paints a turn costs before the next one starts.

```bash
python3 tools/civvis_frames.py watch --port 8766          # read a live exhibition
python3 tools/civvis_frames.py probe --port 8766 --render-ms 400   # be a slow viewer
```

`probe` polls the way the page does and names every turn that never arrived.
Give `--render-ms` a cost a loaded machine would really spend on a repaint: a
fast synthetic viewer proves much less than the browser it stands in for.

A viewer slower than the pace now sets the pace, by design — the alternative is
turns nobody sees. A full-size map costs the page about 55ms a frame, well
inside the 1s Blitz budget, so a foreground tab does not slow the exhibition.

A page that is not being presented — a backgrounded tab, a headless browser with
nobody watching — gets no animation frames, so it stops asking, is dropped after
the staleness window, and the exhibition runs flat out unattended. It picks up
again by itself when the tab comes back. That is the intended behaviour: a page
that cannot put a frame on a screen should not be holding a turn open waiting to.

## Tuning

`--players --width --height --city-states --turns --map --speed` size the game;
`--cooldown` is the seconds the finished result stays on screen before the next
game (the "~5–10s between games"). `--port` defaults to 8766; the fleet's watched
exhibition runs on **8765**. Shorter games rotate builds onto the screen more
often, which makes a better production heartbeat.
