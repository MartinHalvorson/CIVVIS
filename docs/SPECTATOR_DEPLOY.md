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
5. When a winner appears it archives the result, keeps it on screen for the ten
   seconds the result screen counts down, then deals the next game on the
   freshest verified build.
6. Crash/stall recovery: active games are checkpointed every few seconds and
   resumed; a wedged game is nudged, then quarantined rather than looped on.

The supervisor also updates **itself**: when `tools/spectator_supervisor.py`
changes on `origin/main` it re-execs the canonical script under the live game
(`os.execv`), so a machine set up once tracks fleet development with no manual
step.

Watch any host with:  `tail -f <deploy-checkout>/spectator-supervisor.log`

## Keeping it up

`civvis_collab.py bootstrap` installs `com.civvis.spectator`, a launchd job with
`KeepAlive` running `tools/ops/civvis-spectator-runner.sh`. It is installed only
on a host that actually holds the source worktree below, because a service that
can only log a missing prerequisite is worse than an honest absence.

⚠ Unlike the Civilization VI ladder's supervisor, this one runs **directly**
under launchd. The ladder has to be started through Terminal, because installing
the control mod writes inside `Civ6.app` and macOS attributes that permission to
the responsible process. The exhibition drives no GUI — `--no-open`, build,
serve HTTP, play headless games — so it needs no such grant.

Before this existed the supervisor was started by hand from one operator's home
directory, and on 2026-08-18 it exited and the exhibition stayed down until
somebody looked.

## Deploy checkout

Run the supervisor from a **dedicated checkout pinned to `origin/main`**, not the
multi-session shared checkout:

```bash
git -C <shared-checkout> worktree add --detach <...>/civvis-spectator origin/main
```

`ROOT` (where it fetches and stages the runtime) then derives from that path;
the private build worktree is `<...>/civvis-spectator-spectator-src`.

## Windows (Task Scheduler)

The fleet is macOS-only today, so the Task Scheduler registrar
(`deploy/register_spectator_task.ps1`) was retired; it lives in git history if
a Windows spectator host ever returns. The supervisor itself is still
cross-platform: it gives every child `CREATE_NO_WINDOW` on Windows and holds a
per-port lock, so re-registering a scheduled task around it is all a future
Windows deploy needs.

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

## One complete frame per published turn boundary

**Martin-requested simulation requirement:** every turn the exhibition plays
must be shown in at least one complete frame before the simulation advances.
At **Blitz, Fast, Standard, and Slow** watch paces, “turn” here means each
living player's completed turn: major players in seat order, then city-states,
then Free Cities/barbarians. That makes each civilization's unit movement
visible before the next civilization acts. **Lightning** retains one frame at
the completed round boundary so its unlimited-throughput contract remains
intact. On a simultaneous world (a research regime — the product rolls only
sequential worlds, and no request can select the regime), Lightning also
plans each cycle's seats across a worker fleet sized to nine tenths of the
machine's cores, leaving the last tenth free for whatever else the host
runs; any actual pace — and every sequential world, whose seats each act on
the world the previous seat left — keeps the single-threaded cycle. The
fleet is an execution detail: the game it plays is byte-for-byte the game
one worker plays. Every frame carries the whole updated boundary from one snapshot: HUD, player
stats, victory tracker, world map, minimap, units, sidebars, controls,
overlays, and every other turn-bound surface. A partial update, a state merely
delivered to a socket, or two published boundaries composited into one visible
refresh does not satisfy the requirement.

The server enforces the requirement by holding a finished turn until every
active viewer acknowledges painting that exact snapshot — at **every** pace,
not only at Lightning. It accepts an acknowledgement only for a snapshot
previously delivered to that same viewer. A turn budget says how long a turn
lasts; it never promised that anybody managed to read it, and a page that
paints slower than the budget used to lose turns silently.

Only a page that paints holds the simulation to a turn. The browser reports the
frame it last drew on each `/state` poll
(`?painted=<turn>&world=<seed>&finished=<0-or-1>&frame=<sequence>`), so the keeper's refresh check
and any `curl` read the same state without dragging the exhibition down to their
own cadence. The sequence distinguishes player-turn frames that share the same
world turn; the terminal bit distinguishes a result decided without incrementing
that turn. Both must wake and paint as distinct frames rather than wait for the
long-poll timeout. A viewer that stops asking is dropped after a few seconds and
costs the unattended exhibition exactly one frame's delay.

**Every viewer is owed every published frame, and each paint is waited for on its own.** Two tabs
on one exhibition used to take alternate turns: the stepper released a turn as
soon as either had been handed it, so each saw half the game — and the audit,
reading the same single cursor, called it perfect, because between them they had
seen it all. A page names itself with `?viewer=<id>` and gets a seat of its own.
The cost is that a turn now waits for the slowest tab watching to acknowledge
its completed same-snapshot render, which is the promise working rather than
failing.

**A frame reaches the glass, not just the socket.** The page draws at most one
published state per animation frame. Two player turns drawn inside one display
refresh are composited into one, so a frame drawn faster than the screen can
show it is still a frame nobody saw. Running on the display's clock also removed the fixed 100ms
between polls, which was a ceiling of ten turns a second however fast the world
could be played.

**The map is sent as a patch.** A page says which tile array it is holding
(`&have=<world>:<turn>:<finished>:<sequence>`) and is sent only the tiles that changed —
about a dozen of 2252 on a standard turn, so a poll costs ~157 KB instead of
~1.36 MB, an 89% saving. Saying so is also what parks the poll until there *is*
a next frame, so a finished turn or same-turn victory is written to a socket
the moment it exists rather than at the page's next tick. A page that reports
no baseline, including every health check, is answered immediately with the
whole map.

That report is also what makes the promise auditable while it runs. `/status`
carries:

- `frames_missed` — published frames that some attached viewer never drew. **Zero**
  on a healthy exhibition, and kept for the server's whole run rather than reset
  when the tab that missed them closes.
- `frames_painted` — the last turn a viewer reported drawing, or `null` when
  nobody is watching. Zero misses with nothing painted means nobody was there,
  which is not the same as the promise being kept.
- `frame_sequence` / `frames_painted_sequence` — the exact published and last
  painted frame counters. Their difference is visible even when several Blitz
  player turns share one value of `turn`.
- `viewers` — how many pages that promise is being kept to, and so how many
  paints a turn costs before the next one starts.
- `autoplay_turns` — turns `POST /autoplay` has simulated, which is the count
  auto-play's share of `frames_missed` is out of. Zero here means auto-play was
  never used, not that it behaved.

**Auto-play is measured separately, because for a long time it was not measured
at all.** The three numbers above are all built out of a viewer's
`/state?painted=` acknowledgements. A single-player game is not stepped by the
exhibition loop and its page never sends those — it advances by posting to
`/autoplay` — so nothing above could see it. It was batching up to ten turns
into one request, returning only the state after the last, and reporting a
perfectly clean `frames_missed: 0` while nine turns in ten reached no screen.
A response carries one state, so `autoplayed - 1` is the shortfall exactly, and
that is now charged to `frames_missed` where an operator will see it.

```bash
python3 tools/civvis_frames.py watch --port 8766          # read a live exhibition
python3 tools/civvis_frames.py probe --port 8766 --render-ms 400   # be a slow viewer
python3 tools/civvis_frames.py autoplay --port 8912 --turns 30     # audit a human game
```

`probe` polls the way the page does and names every turn that never arrived.
Give `--render-ms` a cost a loaded machine would really spend on a repaint: a
fast synthetic viewer proves much less than the browser it stands in for.

`autoplay` hands a human seat to an agent one turn per request and checks each
turn came back as its own state. It plays the game it is pointed at, so point it
at a scratch `civvis play` server rather than one somebody is using. `--batch`
above 1 reproduces the old behaviour deliberately, which is the quickest way to
confirm the audit still has teeth.

A viewer slower than the pace now sets the pace, by design — the alternative is
turns nobody sees. A full-size map costs the page about 55ms a frame, well
inside the 0.5s Blitz budget, so a foreground tab does not slow the exhibition.

A page that is not being presented — a backgrounded tab, a headless browser with
nobody watching — gets no animation frames, so it stops asking, is dropped after
the staleness window, and the exhibition runs flat out unattended. It picks up
again by itself when the tab comes back. That is the intended behaviour: a page
that cannot put a frame on a screen should not be holding a turn open waiting to.

## Tuning

`--players --width --height --city-states --turns --map --speed` size the game.
A finished result stays on screen for **ten seconds**, and that is not a
setting: it is the countdown the server shows the viewer, so anything able to
disagree with it is a way for the screen to be wrong. `--cooldown` is still
accepted for launchers that pass it, and ignored (the supervisor logs that it
did). `--port` defaults to 8766; the fleet's watched
exhibition runs on **8765**. Shorter games rotate builds onto the screen more
often, which makes a better production heartbeat.
