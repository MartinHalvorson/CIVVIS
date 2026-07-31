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

### ★★★★★ The inbound channel EXISTS: `DB.Query` can ATTACH a database we own

⚠ THIS DOCUMENT USED TO SAY THERE WAS NO INBOUND CHANNEL, and that claim shaped the
whole architecture — a settle plan baked in at install, and a hand-written Lua
heuristic for every other decision. It is false.

From the InGame gameplay context:

```lua
DB.Query("ATTACH DATABASE '/Users/martin/civvis-civ6-runs/orders.sqlite' AS civvis")
DB.Query("SELECT turn, payload FROM civvis.orders WHERE id = 1")
```

Measured on run `sqlprobe-20260730T103836Z` over 29 turns: ATTACH returned no error and
the SELECT returned **23 distinct payloads** tracking a nonce an outside process
rewrote every second. The value FOLLOWED the external writer — that is the check that
matters, not that the name resolves.

So CIVVIS decides per turn and this mod actuates. See `tools/civ6_brain.py`,
`src/bin/civvis_orders.rs`, and `tools/civ6_civvis_status.py` for the fires-check.

**Measured DEAD on this build — do not spend time on these again:**

- `ModUserData` — nil. Does not exist; zero hits in the shipped UI either.
- `io`, `loadfile`, `dofile` — nil. The sandbox claim is real.
- `UIManager` — exists, but only `SetClipboardString`. There is no getter, so the
  clipboard is useless inbound.
- `Options.GetAppOption` / `GetUserOption`, `GameConfiguration.GetValue`,
  `UserConfiguration.GetValue` — all nil for a custom key, even with a `[Civvis]`
  section written into `AppOptions.txt`/`UserOptions.txt` on disk.

`tools/civ6_control/probe_channel.py` is the harness that found it: it writes a
CHANGING nonce into every candidate sink, and the mod emits what each candidate
answers. Existence is not a channel.

⚠⚠ **The outbound leg drops its last line.** `Automation.Log` does not terminate its
record — the log's final byte was `}` — and `watch.py` holds the unterminated tail as
`partial`, so the most recent event is never delivered until the next one flushes it.
Harmless while the mod only reports; fatal once it WAITS, because the last line written
before the wait is the `state` export the brain must answer. Two runs deadlocked on
turn 2 with the game spinning at 139% CPU. Fixed at the source with
`Automation.Log(line .. "\n")`.

⚠ And do not busy-wait on the channel from inside the mod:
`GameCoreEventPublishComplete` fires thousands of times per turn, so a `DB.Query` per
tick starves the very log flush the brain depends on. Poll every ~30 ticks, and count
the wait in POLLS rather than ticks.

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

### One run at a time, and the game must stay in front

Two constraints on *running* this, both found by a run that looked broken and
was not:

- **The installation is a single writer resource, and nothing enforced it.**
  Two sessions ran the ladder against this install at once. The second one's
  mod install landed between the first one's turns, so the first was reading
  events from a mod it had not installed, under a run tag it had never used;
  its games exited without warning because the other harness stopped them. It
  reads exactly like a flaky game. `tools/civ6_control/gamelock.py` now holds a
  lock for the duration of a run and refuses to start rather than interleave.
  A lock whose holder is no longer running is treated as stale and taken over.
- **macOS throttles a background application to almost no frames.** The turn
  loop runs off game-core events, which are tied to frames, so a browser taking
  focus stops the game dead -- a run sat on turn 15 for ten minutes with
  nothing wrong in any log. `civ6_play.py` raises the game window every few
  seconds for the whole run.

### The controller shares the game's frame budget

Every pass this makes runs *instead of* the game advancing, and
`Events.GameCoreEventPublishComplete` fires many times per frame. Acting on all
of them, with a settle-site search that reads a 15x15 block of plots and a
policy pass that walks the whole policy table per open slot, took a turn from
about three seconds to over ten minutes. Measured at turn 20 of one game: 2,720
event batches arrived and the controller acted on 170.

So: the tick is throttled, each expensive pass has a per-turn budget, and
settle sites are found once per settler per turn and remembered. The turn
record carries `ticks_taken/ticks_seen` so this stays measurable rather than
becoming folklore.

Moves are checked for a route before they are issued. Ordering a unit to a plot
it cannot reach does not fail -- the engine accepts it and prints its no-path
sentinel, `Distance: 2147483647`, once per attempt, forever. A settler aiming
across water and an army aiming at a capital on another continent both do it.

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

### The announcement screens close themselves

End-turn blockers are not the only thing that stops a turn. Civilization VI
also stops for its own **announcements** — a wonder finishing, an era ending
with its era score, a technology completing, a Eureka — and those are not
blockers at all. Each is a full-screen UI context that takes the popup lock,
and `ExclusivePopupManager:Lock` calls `UI.ReferenceCurrentEvent()`, which
holds an engine event until the player proceeds. With this controller in the
seat there is no player to proceed. The announcement stays up over the map,
which costs twice: the run stops, and whoever is watching the game loses the
thing they were watching.

`CivvisControlAutoClose.lua` gives each of those screens a stopwatch instead of
a person. The screen opens, stays up for `AnnouncementSeconds` (2 by default —
long enough to read, short enough not to sit through), and then ends exactly
the way its own close button ends it. Nothing about the announcement changes:
what it says, when it fires, what it locks. Only who finishes it.

Three things about how it is wired are worth keeping:

- **One file serves every screen.** Each `ReplaceUIScript` in
  `CivvisControl.modinfo` points a context at the same replacement. The
  replacement asks the context for its own name with `ContextPtr:GetID()`,
  `include`s the shipped script of that name, and adds the stopwatch on top. No
  shipped code is copied, so nothing here goes stale when the game updates —
  and a `LuaReplace` file must also be named in an `ImportFiles` action or it
  is not in the virtual file system at all, which is the same trap that
  silently emptied the settings prelude.
- **`ContextPtr:SetUpdate` *does* tick here**, unlike the script-only context
  below. These are shipped popup contexts with controls and a layout, which is
  the difference; the delta it is handed is in seconds, the same clock the
  shipped `IntroScreen.lua` counts its legally-required logo delay on.
- **Screens that ask something are deliberately not on the list**: a
  dedication, a great person, a promotion. Closing a question answers it, and
  those answers belong to the blocker loop. The one screen on the boundary is
  the era review, which both reports the era score and leads into the
  dedication choice — it is *continued* rather than closed, because its Close
  button skips the dedication and its Continue button raises it.

**This is not a cosmetic problem, and it was measured rather than reasoned
about.** Ladder run `lad-4` stopped at turn 16 on 2026-07-29 and stayed stopped
for seventeen minutes. It did not look like a hang: the process was running at
about a third of a core, and the machine was loaded enough that "it is only
slow" was the obvious explanation. What ruled that out was the log directory —
every *game* log froze at the same second while `FiraxisLive.log` kept being
written, which is a game core that has stopped, not a game core that is
behind. The screen on top of the map was `NaturalDisasterPopup`: **NATURAL
DISASTER OCCURRING — MAJOR FLOOD**, waiting for a click that was never coming.
One Escape into the window closed it and the run moved again within seconds,
which is the whole causal claim tested end to end. The next run, `lad-5`, was
stopped on the same screen at turn 12 twenty minutes later, so this is not a
freak: on Gathering Storm it is the ordinary way an unattended run dies, and it
dies looking like a slow machine.

That screen also settled a question worth writing down. Gathering Storm's
`NaturalDisasterPopup` is already replaced by `GranColombia_Maya` on the
criterion `RuleSetInUse RULESET_EXPANSION_2`, which is true of every run here,
so pointing a second `ReplaceUIScript` at the same context is a race. The
replacement it declares is fourteen lines, and it opens with
`include("NaturalDisasterPopup")` — Firaxis using exactly the pattern above,
which is the best evidence available that the pattern is right.

**The race is real, and the later mod wins it.** The first attempt chained
through their file and hoped the race did not matter. It did: a verification
game on 2026-07-29 armed eight screens and not that one, with no
`autoclose_unarmed` line to go with it — the replacement had simply never been
loaded, because `GranColombia_Maya` sorts after `CivvisControl`. The fix is a
`<References>` entry naming that mod, which is soft (it orders the load when
the mod is present and is ignored when it is not) and is the same tag
`GranColombia_Maya` itself uses to sit after Expansion2. Loading last also
means the chained include picks up their comet-strike label rather than
discarding it.

**One more screen is a different shape.** Not every stopper is a popup
context: `PopupDialogInGame` sends every generic dialog through
`LuaEvents.OnRaisePopupInGame` to the `InGamePopup` context, which calls
`UIManager:PushModal` and whose input handler eats all input ("popups are
blocking!"). That context renders both **UNIT CAPTURED**, which has one button
and asks nothing, and **raze or keep this city**, which has two and asks
everything. So `InGamePopup` alone arms per *dialog* rather than per context:
the open is wrapped, the buttons are counted, and the stopwatch runs only when
there is exactly one. The wrapper has to be re-registered with `LuaEvents`
rather than merely assigned, because the shipped `Initialize` already handed
the event its own function; if that swap cannot be made, the dialog is never
armed and the screen behaves exactly as it ships. Closing goes through the
shipped Escape handler rather than a bare close, so the one button's own
callback runs.

A run's `Automation.log` says which screens armed (`autoclose_armed`, one line
per screen as the game loads) and every screen it has since closed
(`autoclose`, with how long it was up). The failure that has to be loud is
`autoclose_unarmed`: a replacement whose `include` did not land leaves the
context with no shipped code, and that does not look like a broken mod — the
announcement simply never appears, and the run reads as a quiet game rather
than a game missing a screen.

Measured on the verification run (`autoclose-verify`, 2026-07-29):
`TechCivicCompletedPopup` closed itself after 2.01s, 2.17s and 2.58s and
`BoostUnlockedPopup` after 2.20s and 2.23s, each `ended:true`, with the game
playing on through all of them. The overshoot past 2.00 is one frame at the
frame rate a loaded machine was managing, not drift.

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

## The lobby a run sets up

**The map size IS the player count.** Civilization VI derives both majors and
city-states from it — Duel 2, Tiny 4, **Small 6**, Standard 8 — and there is no
separate player-count control on the Create Game screen for the harness to set.

`docs/COMPETITIVE.md` pins the competitive lobby CIVVIS aims at, and its size line
is *"Firaxis-default map size and city-states for the player count"*. So a six-player
game is `MAPSIZE_SMALL`, and that is the default for `civ6_play.py`,
`civ6_civvis_climb.py` and `civ6_climb.py`. Duel and Tiny were measuring two- and
four-player games against rules written for six.

| setting | value | why |
|---|---|---|
| map size | `MAPSIZE_SMALL` | six majors, Firaxis-default city-states |
| speed | `GAMESPEED_ONLINE` | the competitive lobby's speed |
| start era | Ancient | the competitive lobby's start |
| game modes | none | the competitive lobby disables all of them |

⚠ **`MapScript` in the baked config is ignored** — the FrontEnd context that would
read it never loads, so every game is whatever the Create Game screen defaults to.
Selecting the map on-screen was tried and reverted; see the comment in
`configure_and_start`.

**None of this is assumed.** The `seat` event reports the difficulty, size, speed
and player count the game actually generated, read from inside it, and `configured`
is false unless they match what the run asked for — so a misclick on the setup
screen shows up as a run that says so rather than as a Small result recorded under a
Duel heading.

## The ladder

`tools/civ6_ladder.py` holds the record. A rung is claimed only by a victory
event naming the controller's own team, with the run's event log kept.

See `docs/CIV6_LADDER.md` for the current standing.
