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

So CIVVIS decides per turn and this mod actuates. The stale baked settle-plan path
was removed rather than retained beside this supported route. See `tools/civ6_brain.py`,
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

### Local input has no package-manager dependency

The few controls that cannot be sent through the game's own bridge use
`tools/civ6_control/macos_input.py`. It uses `cliclick` when an operator has it,
but a normal macOS Command Line Tools installation is enough: on first use it
builds a tiny cached CoreGraphics helper in the temporary directory. The helper
uses Quartz points, matching the window bounds read by System Events, rather
than Retina screenshot pixels.

`python3 tools/civ6_preflight.py` compiles that helper without clicking and
reports the selected backend. A host without either `cliclick` or `swiftc` fails
there, before a lobby-setting click can leave a run in an unknown configuration.

Verified lobby navigation also needs Pillow (`python3 -m pip install --user
Pillow`): the setup driver detects the variable-height Single Player menu from a
screenshot and will not substitute a guessed Create Game row when image support
is absent.

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

### Unit orders are sequenced per unit (2026-08-19)

`UnitManager.RequestOperation` returns before the unit has moved, so a list
like `MOVE_TO a; RANGE_ATTACK t` applied in one callback aims the shot from
where the unit *stood*. The bridge answered that for two weeks by sending a
unit's walk and deferring every later order to the next turn — which turned
every planned move-then-strike into a move (7 melee attacks against 1,546
`MOVE_TO` in 188 turns of war on run `civvis-20260803T005930Z`).

The mod now keeps a **per-unit order queue** (`CivvisQueue`). `applyOrders`
issues the first order for each unit at once and queues the rest; a queued
order is issued once the earlier one has settled — the unit stands where the
move was aimed, or has no movement left, or the host's own `UnitMoveComplete`
/ `UnitOperationDeactivated` fired for it, or a grace period ran out — and
every one still passes `CanStartOperation`. Refusals are named:
`queue_no_moves`, `queue_stalled`, `queue_prior_refused`, `unit_gone:<id>`,
`queue_turn_over`. `settleTurn` holds the turn while a queue is pending,
bounded by `OrderQueueMaxTicks`; a wedged operation costs decision quality,
never progress. ⚠ Until 2026-08-21 it did not hold the turn on the tick the
opening orders went out — that tick returned true and the caller requested
`ACTION_ENDTURN` at once, so the queue (and any replan frame) lived only
while the host refused the request; the apply tick now returns false and the
next tick drains, frames, and releases (`step_turn_actions_test.lua`). The
queue also *watches* each unit's opening walk (`CivvisQueue.watch`, a
rows-less entry that settles like any queued order and issues nothing), so
the frame decision waits for the walk to land. A
settler's refused `FOUND_CITY` is retried behind its walk, and a `FOUND_CITY`
row now carries its site: the mod founds only with the settler standing on
it, and names a miss `found_off_site` (a found on the hex one step short of
the site is legal far more often than not, and that is where the settler
stands when the found runs first).
The `seat` event advertises `order_queue`, and `civvis_orders` sends a unit's
whole sequence only when it does. `orders` gains `queued` and
`explore_guarded`; a per-turn `orders_queue` event carries `applied`,
`refused`, `refusals`, `strikes_planned`, `strikes_landed` and `waited`.
`--no-order-queue` restores the one-order-per-unit rule for an A/B.

Two related rules: an unmentioned combat unit within `ExploreGuardRadius` of
a visible hostile combat unit or an at-war city is held rather than handed to
`UNITOPERATION_AUTOMATE_EXPLORE`, and `UNITOPERATION_PILLAGE` is resolved so
`Action::Pillage` crosses. See `docs/LIVE_TACTICS.md`.

### A move is this turn's leg (2026-08-19)

`UNITOPERATION_MOVE_TO` takes a destination and the host paths to it across
as many turns as it needs — and walks the unit along the rest at the start of
the next turn, before `beginTurn` exports. The board then planned movement
the unit no longer had. The mod (`CivvisBoard`) now sends every `MOVE_TO` as
the furthest plot on the host's own path (`GetMoveToPathEx`) that the unit
reaches this turn, refuses by name a move whose first step is already next
turn (`move_no_moves_this_turn`), never caps a melee ATTACK, and cancels
combat units' queued paths at turn start (`UNITCOMMAND_CANCEL`;
`queued_paths` reports the count). Units export `queued_dest` and
`embarked`; tiles export `rt` (route type) and `rp` (pillaged). The `seat`
event advertises `moves_at_turn_start`, and only then does the mirror trust
the export's `moves`. `--no-cap-moves-to-reach` / `--no-cancel-queued-paths`
restore the old rules for an A/B. See `docs/LIVE_TACTICS.md` §6.

### A mid-turn combat frame, off by default (2026-08-19)

With `CombatFrames ≥ 1` (`--combat-frames`), once the opening orders and
their per-unit queue have settled on a turn that issued a strike, the mod
exports the board again stamped `frame: 1` and waits for the brain to answer
the same turn on it — its own short poll budget (`CombatFramePolls`), no
stale answer, no fallback: past the budget `combat_frame_timeout` and the
turn ends as before. The order channel gained a `frame` column (rows of frame
N sit at seq 10000·N; one `ready` row per turn names the newest frame; a
database from before the column is migrated in place); the mod's readers
select by frame and read a column-less channel as frame 0. On a frame no
unit is handed to explore automation and no `turn` record is written. Units
export `attacks_remaining`. Default **off** until one live run has been read
(`docs/LIVE_TACTICS.md` §8).

### The tactical ledger (2026-08-19)

The mod writes the combat record the host already knows (`CivvisLedger`):
`strike` before every ATTACK / RANGE_ATTACK with the host's own preview
(`CombatManager.SimulateAttackInto`, the shipped UnitPanel's combat preview);
`combat` at `CombatVisEnd` with attacker, defender, hit points read back at
Begin and End, damage both ways, kills, the `UnitDamageChanged` deltas seen
while the combat was open, and the strike's preview joined on; `unit_lost`
for our units leaving the map (last known kind, treasury); `city_occupation`
when a city changes hands. Hostile and rival units carry the host's unit id.
`tools/civ6_tactics_ledger.py <run-dir>` turns a run into the arrival,
combat, roster and hover ledger; see `docs/LIVE_TACTICS.md` §5.

### Production and envoy handoffs are host-timed

The Rust bridge still sends ordinary `produce` orders immediately. When the
exported queue is expected to finish during that turn, it also sends a
non-mutating `produce_next` lease. The InGame agent records that lease without
touching the queue, emits `build_hint`, and consumes it only when the host
raises its production blocker. The bridge carries the lease across a fresh
planning board, so a slow host frame cannot release the still-running item as
foreign production. `production_next_hints`, `production_next_consumed`, and
`production_next_expired` in the order note make the handoff auditable; leases
are kept out of the host applied-rate denominator until an actual `build` event.

Envoy orders remain one-token CIVVIS orders and are confirmed by the next host
frame rather than by the issuing callback. When the optional envoy lane is
enabled, the mod emits `envoy` for the request and `envoy_reconcile` with the
fresh `GetTokensToGive()` readback on the following turn. The lower-bound field
accounts for envoys earned between frames, so a changed purse is not silently
called a refusal.

This is smaller than enumerating decisions ourselves and it cannot silently
skip a decision type this build has and the code does not know about: an
unrecognised blocker is reported by name. A blocker that survives
`MaxBlockedAttempts` is dismissed through `NotificationManager.Dismiss` and
logged as forfeited — a far better trade than a run that stops dead, because a
stopped run reports nothing at all.

⚠⚠ **That 40-attempt forfeit is not reachable on a wedged turn, and step 3 is
refused on one.** Both halves were measured by run `civvis-20260807T190903Z`
turn 39 (issue #1374), which sat 900 s on `ENDTURN_BLOCKING_UNITS` and died to
the outside watchdog:

- A board waiting on input publishes almost no game-core events, and
  `onGameCoreTick` keeps 1 in 16 of the ones it does. `attempts` never passed
  **1** in fifteen minutes against a bound of 40. Any escalation that counts to
  a large number is wall-clock unreachable exactly when it is needed.
- The shipped `ActionPanel.DoEndTurn` never requests `ACTION_ENDTURN` while
  `ENDTURN_BLOCKING_UNITS`, `ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS` or
  `ENDTURN_BLOCKING_STACKED_UNITS` is active — it calls
  `UI.SelectNextReadyUnit()` and waits for a human. Step 3 above is therefore
  issued into a wall on those three. The only form that gets past them is the
  one Firaxis reserves for SHIFT+ENTER and calls "Unsupported":
  `UI.RequestAction(ActionTypes.ACTION_ENDTURN, { REASON = "UserForced" })`.

So a **soft** blocker still current on the sighting after its own answer is
forfeited immediately rather than at 40: the still-ready units are parked with
`orderIdle` (skip/fortify/alert/sleep — position-preserving, never the legacy
`orderFor` pass), the notification is dismissed, and for those three the turn is
forced. Under `CivvisDecides` the trigger is the second sighting, because the
`civvis_complete` answer changes nothing by construction and waiting longer buys
no information. The retry is bounded at `MaxSoftBlockerForfeits` per blocker per
turn; once it is spent, a `wedged` event names the prompt, so a killed attempt
says what stopped it instead of reading as a slow machine.

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
blocking!"). One context renders every generic dialog in the game, so
`InGamePopup` alone arms per *dialog* rather than per context: the open is
wrapped, the options are read, and the stopwatch runs only for a dialog that
may be ended. The wrapper has to be re-registered with `LuaEvents` rather than
merely assigned, because the shipped `Initialize` already handed the event its
own function; if that swap cannot be made, the dialog is never armed and the
screen behaves exactly as it ships. Closing goes through the shipped Escape
handler rather than a bare close, so the button's own callback runs.

**Which dialogs may be ended, and how that is known.** Two shapes, and the
answer is read off the data the dialog carries rather than guessed:

- **One button.** An acknowledgement — **UNIT CAPTURED**. Nothing is asked, so
  nothing is answered by ending it.
- **Any button tagged `_CMD_CANCEL`.** `PopupDialogInGame:AddCancelButton`
  writes `PopupDialog.COMMAND_CANCEL` into its option, and the shipped
  `InGamePopup.InputHandler` maps Escape to
  `ActivateCommand(COMMAND_CANCEL)` first and `COMMAND_DEFAULT` second. A
  dialog carrying a CANCEL has an explicit decline path written by its own
  author, and Escape runs that author's cancel callback — `nil` in every
  shipped caller, which is what dismissing without consequence means.

A dialog with two buttons and no CANCEL is a forced choice and is left exactly
as it ships. `tools/civ6_control/mod/dialog_escape_test.lua` pins all three
cases against option lists in the shape the shipped callers build.

⚠⚠ **The rule used to be the button count alone, and the reason it gave named a
screen that lives somewhere else.** It cited **raze or keep this city** as the
two-button danger. Raze/keep is `RazeCity.lua` with its own `RazeCity.xml`,
queued through `UIManager:QueuePopup` and holding its own input handler; it
never reaches `PopupDialogInGame`, so nothing here could touch it either way.
Meanwhile every shipped Ok/Cancel and Yes/No dialog — the ones written to be
declined — was refused by the count and sat over the map until the desktop
backstop or the 900-second watchdog. Every `PopupDialogInGame:new` caller with
a CANCEL is a confirmation of an action a person just took in a panel
(`UnitPanel`'s delete, `WorldInput`'s WMD launch, `GovernmentScreen`'s anarchy
switch, `GovernorAssignmentChooser`'s replacement,
`StrategicView_MapPlacement`'s pin), and this controller takes none of them
through a panel — it issues the operations directly. A confirmation reaching an
unattended seat is a dialog nothing was waiting on.

⚠ **And Escape is not a universal exit.** A leader conversation that asks a
question ignores every in-Lua rung and ignores Escape too — measured,
`cliclick kp:esc` left Gorgo's embassy request exactly where it was. A question
needs an answer, and there is nothing for a dismiss to do; that is what
`popup_clear.py`'s held click on the dialogue button is for. The rule above is
about generic dialogs, not about every screen.

⚠ **`screen:"InGamePopup"` names nothing on its own**, because one context
renders them all. The `autoclose` event carries a `buttons` field with the
dialog's command strings joined by `+`, so which generic dialogs actually reach
an unattended run is a census rather than an argument from shipped source —
which is how the old rule came to protect against a screen in another file.

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

**The map size IS the player count.** There is no separate player-count control on
the Create Game screen for the harness to set, and Civilization VI derives both the
majors and the city-states from the size. Straight from the shipped `Maps` table
(`Cache/DebugGameplay.sqlite`):

| `MapSizeType` | `DefaultPlayers` | grid |
|---|---|---|
| `MAPSIZE_DUEL` | 2 | 44×26 |
| `MAPSIZE_TINY` | 4 | 60×38 |
| **`MAPSIZE_SMALL`** | **6** | **74×46** |
| `MAPSIZE_STANDARD` | 8 | 84×54 |

74×46 is also the board CIVVIS' own exhibition and league games run on, so a Small
Civilization VI game and a CIVVIS game are the same size.

`docs/COMPETITIVE.md` pins the competitive lobby CIVVIS aims at, and its size line
is *"Firaxis-default map size and city-states for the player count"*. So a six-player
game is `MAPSIZE_SMALL`, and that is the default for `civ6_play.py` and
`civ6_civvis_climb.py`. Duel and Tiny were measuring two- and
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

The record has two homes. The **live ledger** sits beside the runs
(`<runs>/ladder.json`) and `civ6_play.py` records every summary into it the
moment the summary exists — recording was once a by-hand step, and between
July 31 and August 16, 2026 that step simply stopped happening while 211
summaries piled up unrecorded. The **published snapshot** is
`docs/civ6_ladder.json` + `docs/CIV6_LADDER.md`, refreshed with
`civ6_ladder.py publish` and landed like any other change. `civ6_ladder.py
check --stale-hours 12 --min-applied 90` fails when summaries are unrecorded,
when the snapshot trails the ledger, when no run has finished recently — the
last one is how a silently halted supervisor becomes a visible failure — or
when the newest measured run applied under the floored percentage of its
orders. Every summary carries `orders_seen`/`orders_applied` summed from its
own turn events, so bridge health rides the ledger instead of living only in
a status tool somebody has to think to run; it sat at 79.9% once for days
that way.

### An order is applied when the next frame shows it (2026-08-25)

`orders_applied` used to count arms whose request did not throw, and `pcall`
success is not acceptance: a Settler was requested on 83 consecutive turns
with `applied = true` and nothing built, a purchase bought nothing, and a
`MOVE_TO (14,11)` from (13,11) ended at (12,9) with no refusal recorded.
`civvis_orders` now keeps every order it issued on turn T and checks it
against the `state` frame of turn T+1 — the unit stands at or closer to the
ordered tile, the target is damaged or gone (or a `combat` record names the
attacker), the city's `producing` is the item, the bought unit or building
exists or the treasury fell, the plot is ours, the city-state's envoy count
rose, a deal was answered (`deal_closed` / `deal_declined` / `deal_session`
answered), a `peace_response` arrived and `at_war` agrees with it, `at_war`
is set after a declaration, the governor holds the seat, the research, civic,
government, pantheon and policy deck read back, and so on. Kinds the frame
cannot answer are listed with a reason in `UNVERIFIABLE_ORDER_KINDS` and
`UNVERIFIABLE_UNIT_VERBS` (`produce_next`, `delegation`, `aid_gift`, `levy`;
`SKIP_TURN`, `SLEEP`, `ALERT`, `HEAL`, `AUTOMATE_EXPLORE`, `SPY_*`), and a
test refuses any order kind that is neither checked nor listed.

The verdicts ride back through the orders channel as rows of kind
`order_verified` / `order_failed` / `turn_verified` — the mod is the ledger's
only writer — and the mod re-emits them as events: `order_verified {turn,
order_kind, verb, subject, checked_on}`, `order_failed {…, reason}`, and one
`turn_verified {turn, orders_issued, orders_applied, orders_failed,
orders_unverifiable, orders_seen, orders_reported}` per turn, where
`orders_applied` is the verified count and `orders_seen` / `orders_reported`
are the mod's own counts for that turn. The `turn` record keeps its
return-code count under both `orders_applied` (its old name, for the readers
that clock on it) and `orders_reported`. `civ6_ladder.orders_ledger` sums the
three: the summary's `orders_applied` is the VERIFIED count wherever a turn
carries a verdict (a turn without one — the last turn, the turn before a
decider restart, any run older than the check — keeps its reported count, and
`orders_unverified_turns` says how many did), `orders_reported` is the return
codes' count, and the attempt row carries both `applied_pct` and
`reported_pct`. `civvis_orders --mirror <run> --audit-orders <orders.jsonl>`
replays a recorded run's orders (exported from its `orders.sqlite`) through
the same checks.

Measured offline on 256 local runs (654k checkable orders): the arms reported
91.4% ok while the next frame verified 75.7%. The largest gaps by kind:
`MOVE_TO` 82% (68k `did_not_move`), `ATTACK` 25%, `RANGE_ATTACK` 40%,
`city_strike` 29%, `PROMOTE` 39%, `purchase_faith` 37%, `peace` 1% (no
response), `UPGRADE` and `DELETE` 0% (both refused by the mod itself:
prophets are retired, never deleted).

See `docs/CIV6_LADDER.md` for the current standing.

### A lost game is stopped, not played out (2026-08-19)

The ladder loses its games early and then plays them to turn 250 anyway: of
the 48 live games that had reached a terminal result by 2026-08-19 (7 wins),
34 sat under three quarters of the best rival's score for five consecutive
turns at or after turn 120, and none of those 34 won — 3,700 game-turns, about
nine host-hours per batch of twelve, spent on results already written. The
wins' own low-water marks after turn 120 were 0.87 and 0.88 of the rival's
score, so the line sits a tenth under the worst comeback the seat has staged.

`civ6_play.py --abandon-below-win-rate 0.05` stops a run once the **measured
expected win rate** — the Laplace rate `(wins+1)/(games+2)` of the
best-evidenced matching cell in `civ6_play.ABANDON_CELLS` (`(100, 0.60)`
0/25, `(120, 0.75)` 0/34) — has sat under the floor for `ABANDON_PATIENCE`
(5) consecutive agent turns. A readable standing back over the floor resets
the count; a turn without a standing neither counts nor resets. Off by
default; `civ6_civvis_climb.py` forwards the same flag and the supervisor
reads `CIVVIS_ABANDON_BELOW_WIN_RATE` from the login shell (the operator's
request on 2026-08-19 was "ok to abandon games early if expected win rate
<5%"). An abandoned run is filed with `reason: "abandoned"` and the verdict
(turn, standing, estimate, floor) in its summary and ledger row — its own
ending, never a stall, a wedge or a defeat. The table is measured, not
chosen: re-fit it when the ladder climbs (the fit is written out in
`tools/test_civ6_play.py`).

### The run always tests the latest code

Two mechanisms, one guarantee. Between games, the supervisor pulls and
rebuilds head before every attempt. Mid-game, the brain's
`GitHubRuntimeUpdater` fetches origin/main every 30 seconds, builds it in a
dedicated worktree, and hands the running game a fresh decider at the next
turn boundary — re-execing the brain itself from the new revision's tree, so
the Python harness follows too. Only the Lua mod waits for the next game
(Civilization VI loads it once, at game start).

The guarantee is *provable*, not assumed:

- `runtime_updates.jsonl` in the run directory opens with a `start` row
  naming the revision the run began on and adds a `handoff` row (with UTC)
  for every mid-game advance; the summary carries the whole list as
  `decider_revisions` and the ladder entry records it as `revisions`.
- The updater writes `~/.cache/civvis/live-game-runtime/heartbeat.json`
  every refresh cycle, success or failure. `civ6_ladder.py watch
  --minutes 10` fails when the heartbeat is missing, stale, or reporting a
  refresh error; `civvis_sync.sh` runs it every cycle, gated on a live
  brain process so build gaps between games cannot cry wolf.

## Reading the residual census

`residual` counts what happened when the hand-written ladder was consulted on
a turn CIVVIS was supposed to be deciding. **It is three different things and
they must never be added together**, because the sum reads as the worst of
them:

| bucket | meaning | what to do |
|---|---|---|
| `unasked` | CIVVIS issues orders that answer this prompt, and the ladder answered it first. **A second AI decided under CIVVIS's name.** | add the prompt to `CIVVIS_OWNED_BLOCKERS` |
| `after_civvis` | CIVVIS answered, the prompt came back anyway, and the bounded escape at the forfeit ladder asked for one real answer instead of wedging the turn | nothing — this is the design, and without it runs sat 900 s on a standing prompt |
| `declined` | the ladder had no answer either; **nothing decided anything** | nothing, unless the prompt should be CIVVIS's |

⚠ On 2026-08-17 a review of fourteen runs read the flat total of 1,577 as
"1,577 decisions taken by the Lua fallback instead of CIVVIS" and had to
withdraw it. The split was **937 `after_civvis`, ~350 `declined`, and 3
`unasked`** — the leak was `ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE`,
which is now owned. The reader had the source open. A number that misleads a
careful reader is a broken instrument, so the agent now writes the buckets and
`civ6_civvis_status.py` prints the leak first, alone, with its prompt names.

Runs from before that change carry no buckets and are reported as
`unclassified`: a flat total cannot be split after the fact, and guessing
which way it went is the same error again.

The join that keeps `unasked` at zero is `CivvisAnswersPrompt` in
`CivvisControlAgent.lua` — prompt to the CIVVIS order kind that answers it —
enforced by `residual_census_test.lua`, which fails when a mapped prompt is not
owned or names an order kind `civvis_orders.rs` never emits. Before it, the
owned list was maintained by somebody eventually reading a log.

## The host itself is part of the bridge (macOS 26, measured 2026-08-07)

A Steam reinstall on macOS 26.5.1 established four host facts that sit UNDER
everything above, and `tools/computer_control.py` is the systematic layer that
reports and manages them (repo tooling only — nothing under `web/` may import
it, so none of it ships to civvis.ai):

- **Gatekeeper can refuse the game while its process runs.** The refusal is a
  `CoreServicesUIAgent` modal — *"Civilization VI" is damaged and can't be
  opened* — and behind it the child initialises nothing: no `Logs/` directory,
  an events file frozen at zero bytes, a harness reading "slow machine". A
  poisoned trust record survives a VALID signature (measured: mod removed,
  `codesign` clean, still refused, on both a direct child exec and a
  `steam://rungameid` launch). The recovery that works is a Steam
  **reinstall**; the modal's default button — Move to Trash — is the one thing
  that must never be pressed, which is why `dismiss` works from a per-owner
  button allowlist instead of clicking defaults.
- **Installing the mod invalidates the bundle's seal.** Every file written
  under `Contents/Assets/DLC/CivvisControl` lands in `codesign -v` as "a
  sealed resource is missing or invalid"; uninstalling restores "valid on
  disk". A fresh trust record tolerates the broken seal — the game launches
  and plays — so `bundle` reports signature and writability as two independent
  facts rather than one health bit.
- **The loop cannot run as a bare LaunchAgent, and the way it fails is silent.**
  macOS attributes the permission to write inside `Civ6.app` to the RESPONSIBLE
  process. Terminal holds that grant on the fleet host; `launchd` does not, and
  a LaunchAgent's children inherit launchd's empty set. A KeepAlive job running
  `civvis-game-supervisor.sh` therefore builds head, launches the climb, and
  watches every attempt die at `NO GAME — PermissionError: cannot install
  .../Assets/DLC/CivvisControl` while launchd faithfully restarts a loop that
  can never play. The Finder fallback below does not rescue it: driving Finder
  is an Apple Event and sending one needs an Automation grant launchd also
  lacks. Measured three ways on 2026-08-17 with the same three lines of Python
  — Terminal child: writes; bare LaunchAgent: `Operation not permitted`;
  LaunchAgent that ran `open -a Terminal <script>`: writes. So supervision
  belongs to launchd, which survives a closed session and a reboot, but it must
  START the loop through Terminal. `tools/ops/ladder_watchdog.py` is what the
  interval job runs, and `test_ops_portability.py` fails if any managed plist
  goes back to naming the supervisor directly.
- **The install tree is TCC-protected against Terminal, not against Finder.**
  Writes into the bundle from Terminal's children fail with "Operation not
  permitted" even with the game closed, while Finder performs the same
  operations — which is exactly the fallback `civ6_control/install.py` uses.
  Symlinking the mod out of the bundle is a DEAD END, not an open idea:
  Finder's `move (POSIX file … as alias)` dereferences the link and moves its
  target; moving the link as a folder item is refused (-5000) from a temp
  directory and times out (-1712) from the home directory. Directory replace
  through Finder is the mechanism that works.
- **`Civ6_Exe_Child` bypasses the stub's single-copy refusal.** A leftover
  child from a torn-down run plus one fresh launch gave two live games, and
  every `System Events` call addressed to "the" Civ6 process then drives an
  arbitrary one. `games --ensure-single` enforces the stub's rule from
  outside: oldest child kept, newer ones culled.

Two host settings belong with these: `PlayIntroVideo 0` in the NESTED
`AppOptions.txt` (a fresh install resets it to 1, and the first-run cinematic
then covers the main menu — sixteen straight "no submenu (0 rows)" failures
while the movie played), and the operator's standing window layout, which
`layout` applies: terminal lower-left, CIVVIS upper-left, the game upper-right,
lower-right left free for the operator.
