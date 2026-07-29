# Computer control for Civilization VI

`docs/GROUNDING.md` measures CIVVIS against Civilization VI while the game
plays *itself*: the engine's autoplay manager runs every seat with the shipped
AI and a mod writes down what happened. That answers "does this rule behave the
same way". It cannot answer "is this strategy any good", because nothing in
that setup is under our control — the shipped AI is playing both sides, and the
difficulty setting, which only gives its bonuses to a *human* seat, is inert.

This document covers the other half: **occupying a seat and playing it**. The
deliverable is a controller that can be pointed at a difficulty and left alone,
and the milestone ladder is the measurement — Settler first, then each rung up
to Deity, each claimed only by a game the controller actually won.

## What this build gives you, measured

Everything below was established on this install (1.0.12.54, macOS, Gathering
Storm), not read from documentation. Several of them contradict the obvious
guess, and each one produced a *working-looking* failure before it was found.

### The game binary takes arguments — but only the child does

```
Civ6.app/Contents/MacOS/Civ6_Exe          <- a stub; spawns the child and exits
Civ6.app/Contents/MacOS/Civ6_Exe_Child    <- the game
```

Running `Civ6_Exe_Child` directly, with the Steam client running and
`SteamAppId=289070` in the environment, starts the game and reaches the main
menu in about three minutes. This removes the Aspyr LaunchPad and its PLAY
button from the launch path entirely, which was the single most fragile step in
the old harness: the launcher's window position is not fixed, so a hard-coded
click lands on whatever window happens to be there.

Arguments handed to the *stub* do not reach the child, and the stub refuses to
start while a copy is running — which reads as "the launch did nothing".

Steam's own launcher may pop up "An error occurred while launching this game:
Game configuration unavailable" when it notices a process it did not start.
That popup is cosmetic; the game runs.

### The automation system has a command-line front door

`Src/App/Scripting/AppAutomation.cpp` in the shipped binary defines three
flags — `-autoscript`, `-autoparams`, `-autojson` — and reads a parameter set
named `Startup` with keys `Scripts` and `Tests`. That is the same machinery
`Assets/Base/Assets/UI/Automation/Automation_StandardTests.lua` reads through
`Automation.GetStartupParameter("RunTests")`, and its `PlayGame` test accepts
`RuleSet`, `MapScript`, `MapSize`, `Handicap`/`Difficulty`, `GameSpeed`,
`MapSeed`, `GameSeed`, `StartEra`, `MaxTurns`, `Turns` and `HumanPlayers`.

`-autojson <file>` is accepted — `Logs/Startup.log` records the command line —
and with `{"Scripts": ["Automation_StandardTests.lua"], "Tests": [...]}` the
automation log opens and closes within five seconds without running a test,
which is what `Automation_StandardTestSupport.lua` does when the `Tests` global
is absent. With `Scripts` as a semicolon-separated *string* instead, no
automation log is written at all. **The accepted shape is not yet known**; the
variants still to try are in the history of `tools/civ6_control/`. Nothing
depends on it — the mod route below does the same job — but it is the
officially supported path and worth finishing.

### FireTuner listens, and its framing is known

With `EnableTuner 1` in the **nested** user directory's `AppOptions.txt` (see
`tools/civ6_env.py` for why there are two), the game listens on 127.0.0.1 ports
4318 and 4319 (`Src/App/TunerSupport/Tuner2Listener.cpp`). Measured:

- One connection at a time per port; a second is refused.
- Messages are `<uint32 length><uint32 type><payload>`, little-endian. The
  server acknowledges every message with an empty reply of the same type, and
  emits `length=0, type=0xffffffff` as a keepalive once a client has sent
  anything.
- The listener's verbs appear in the binary next to the source path: `TREE`,
  `STRACKERS`, `START_TRACKER`, `STOP_TRACKER`, `ARTDEF`, `HELP`, `HELPT`,
  `KILLQRY`, `TABLE`, `CUSTOM_TABLE`, `CUSTOM_TABLE_SEL`, `LIST`, `LISTSEL`,
  `MLIST`, with errors `ERR:Invalid Lua State`, `ERR:Bad lua state`,
  `ERR:No Query Found`.
- **No message type executes Lua.** Types 0–255 were swept on both ports with
  four payload shapes, using a Lua statement with a logged side effect as the
  oracle; nothing ran. Either the exec type is outside that range, the payload
  is a structure rather than a string, or a handshake is required first.

This would be a useful accelerator — it would replace a three-minute restart
per question with a round trip — but it is not required, and the sweep above is
where a future attempt should resume rather than starting over.

### The mod is the control channel

Two Lua contexts, installed by `tools/civ6_control/install.py`:

| context | file | job |
|---|---|---|
| FrontEnd | `CivvisControlSetup.lua` | write the exact game settings into `GameConfiguration`/`MapConfiguration` and call `Network.HostGame` |
| InGame | `CivvisControlAgent.lua` | hold the seat: issue every order, answer every prompt, end every turn |

Both are installed into the **install's `DLC` tree**. No user `Mods` directory
is scanned on this build, so a mod placed in one is never discovered and
nothing logs why. `Mods.sqlite` is an mtime index, so it is dropped on install
or the new folder is not noticed.

The run's settings are **prepended** to each script rather than `include`d: a
file listed under `<Files>` in a `.modinfo` is not on the include path unless an
`ImportFiles` action puts it there, so the include fails silently and every
setting falls back to its default — for a difficulty ladder, a run that reports
Settler and plays Prince.

### Out is `Automation.log`, and only that

This build writes no `Lua.log`, so `print` from a mod goes nowhere.
`Automation.Log` is the channel that survives, and it lands in the nested user
directory's `Logs/Automation.log`. Both contexts write one JSON object per
event there, prefixed `CIVVISJSON`; `tools/civ6_control/watch.py` tails it.

The file is empty while the game sits at the main menu, which makes a FrontEnd
context that never loaded and one that loaded and threw indistinguishable —
both are a game at the menu with an empty log. So the setup context writes its
outcome into `GameConfiguration` under `CIVVIS_SETUP` and hosts the game
*whether or not* configuring succeeded; the agent reads that value back on its
first turn. Three outcomes, all distinguishable: no game at all (the context
never ran), a game marked `ok` (it ran and configured), a game carrying an
error (it ran, failed to configure, and hosted anyway).

## How the controller plays

The turn loop is built around the game's own **end-turn blockers** rather than
a checklist of things a player might want to do. Civilization VI already knows
every decision it is waiting on and publishes it through
`NotificationManager.GetFirstEndTurnBlocking`. The loop is:

1. On a new turn, run one decision pass — research, civic, city production,
   unit orders.
2. Ask what is blocking. If something is, answer that specific blocker and ask
   again.
3. When nothing is blocking, `UI.RequestAction(ActionTypes.ACTION_ENDTURN)`.

This is smaller than enumerating decisions ourselves and it cannot silently
skip a decision type this build has and the code does not know about: an
unrecognised blocker is reported by name. A blocker that survives
`MaxBlockedAttempts` is dismissed through `NotificationManager.Dismiss` and
logged as forfeited — a far better trade than a run that stops dead, because a
stopped run reports nothing at all.

### Two things about the in-game context that fail silently

- **`ContextPtr:SetUpdate` does not tick in a script-only in-game context.**
  The first controller drove its turn loop from a per-frame update, exactly as
  the shipped `MainMenu.lua` does. The result was a game that founded its
  capital, set its research, and then sat on turn 1 forever having emitted one
  turn record and no errors. The loop now runs from
  `Events.GameCoreEventPublishComplete` and friends, which also fixes a second
  problem: orders resolve over several frames, so a single pass at the bottom
  of the turn could not end a turn even if the update did fire.
- **`UnitOperationTypes` is not the list of unit operations.** It is a
  convenience table and it is incomplete: on this build it has no `SKIP_TURN`,
  no `SLEEP` and no `AUTOMATE_EXPLORE`, while the database defines all three.
  Reading a missing name off the enum yields `nil`, the guarded call refuses
  the order, and a unit that could have been told to skip blocks the end of the
  turn instead. Operations and commands are looked up through
  `GameInfo.UnitOperations` / `GameInfo.UnitCommands` and their resolution is
  reported at startup.

## The ladder

`tools/civ6_ladder.py` holds the record. A rung is claimed only by a victory
event naming the controller's own team, with the run's event log kept.

See `docs/CIV6_LADDER.md` for the current standing.
