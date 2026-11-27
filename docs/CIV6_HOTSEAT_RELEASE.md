# Civilization VI hotseat against CIVVIS: the download

One folder you download and run, and a Hot Seat game of the real Sid Meier's
Civilization VI in which you play the first seat and CIVVIS plays every other
human seat. No checkout, no Rust, no Python install. Inside the folder:

```
civvis-hotseat/
  civvis-hotseat            the program (civvis-hotseat.exe on Windows)
  README.md                 this document
  _internal/                its Python runtime, the control mod (civ6_control/mod/),
                            the CIVVIS decider (civvis_orders), bundle.json
```

It is the same harness the project plays with (`tools/civ6_hotseat.py`, its
decision workers `tools/civ6_brain.py` and the `civvis_orders` decider), frozen
by `tools/package_hotseat.py` with PyInstaller. Nothing was rewritten for it.

| | macOS (Apple silicon) | Windows (x86-64) |
|---|---|---|
| The harness with the game | **measured** from a checkout: the project plays on it | **unmeasured**: nobody has run it with the game |
| The bundle | built, run and self-checked on a Mac and in CI; **has not yet played a real game** | built and self-checked in CI only |
| Supported | once the bundle has played one game (section 2) | not yet: see section 2 |

Steam installs only. Epic Games installs and Linux (native or Proton) are out
of scope: nothing resolves them and nothing has been tried on them. The macOS
bundle is arm64 only.

The bundle is for `--manual-lobby`: you set the Hot Seat game up in Civ VI by
hand and nothing of CIVVIS's touches the screen. The harness's other mode, the
one that sets the game up itself by reading the screen, needs a Swift compiler
and Screen Recording and Accessibility permissions that a downloaded copy does
not have.

## 1. Installing and playing

### What you need

- Sid Meier's Civilization VI from Steam with Gathering Storm (the control mod
  is measured on 1.0.12.54 with Gathering Storm; other versions are untested).
- macOS on Apple silicon, or 64-bit Windows 10/11.
- A terminal: the program prints your turns and its own progress there.

### Download and first run

1. Download `civvis-hotseat-macos-arm64.zip` or `civvis-hotseat-windows-x86_64.zip`.
   Until there is a release, these are the artifacts of a
   [`hotseat-bundle`](../.github/workflows/hotseat-bundle.yml) workflow run
   (GitHub wraps each artifact in one more zip; unzip both).
2. Unzip it anywhere, for example your Downloads folder.
3. The program is not signed (section 3), so the system will stop it the first
   time. Expected, not yet measured with this bundle:
   - **macOS** quarantines everything a browser downloads, and the folder holds
     dozens of libraries. Clear the quarantine once, in Terminal:
     `xattr -dr com.apple.quarantine ~/Downloads/civvis-hotseat`
   - **Windows** SmartScreen says "Windows protected your PC": choose
     *More info* then *Run anyway*.
4. Check the machine and the bundle:

   ```
   ./civvis-hotseat/civvis-hotseat doctor      # macOS
   civvis-hotseat\civvis-hotseat.exe doctor    # Windows (Command Prompt)
   ```

   `doctor` prints where it found the game, your user folder
   (`AppOptions.txt`, `Logs`, `Mods`), where the control mod goes, whether the
   game is running, and what the bundle contains. On Windows it says plainly
   that game behaviour there is unverified.

   `civvis-hotseat selfcheck` then proves the bundle works on this computer
   without touching the game, all in a scratch folder: it installs the mod the
   way `--manual-lobby` does, feeds a recorded opening board through a scratch
   `Automation.log`, has a real decision worker answer it and stops it, then
   starts `--manual-lobby` itself and stops it with `--stop` (about twenty
   seconds).

5. **macOS only:** the control mod is installed inside the game's application
   bundle, so the first install shows an App Management prompt, or answers
   "Operation not permitted": allow your terminal under System Settings ->
   Privacy & Security -> App Management, then run it again.

### Playing a game

1. Start the harness and leave its window open:

   ```
   ./civvis-hotseat/civvis-hotseat --manual-lobby
   ```

   It installs the control mod and prints the steps below. If Civ VI is already
   running it says so: the game looks for mods only when it starts, so quit it
   completely (Civ VI menu -> Quit) and start it again.
2. Start Civilization VI.
3. Main menu -> **Multiplayer -> Hot Seat**. Choose the map, speed and anything
   else you like, then **Confirm Settings**.
4. In the staging room the **first row is you**. Add a player for each seat
   CIVVIS should play and set it to **Human** (up to five). AI rows are Civ VI's
   own AI.
5. **Start Game.** The harness reads the game you made back from the game
   itself (one line in its window) and starts one CIVVIS worker per Human row
   after yours, named "CIVVIS jack 1", "CIVVIS jack 2", ...

What to expect:

- **Your turn plays like single player.** When you end it, a cover ("CIVVIS is
  taking its turn", with the seat and turn and nothing about that empire) hides
  the screen until your next turn begins. Then your hand-over "Start Turn" is
  pressed for you, the cover lifts, and you are back on your own map where you
  left it, with no hand-over dialog. `--no-cover` shows CIVVIS's turns as they
  play; `--keep-handover` leaves the hand-over screen for you to press.
- **Esc on the cover** opens the game menu (save, load, quit).
- **Sound plays through the cover.** It hides pixels only: CIVVIS's leader
  voice lines, diplomacy music, combat and unit sounds, fanfares and
  notification chimes are all audible.
- **A CIVVIS turn that stalls is not hidden for good.** The cover stands aside
  for the rest of that turn after 90 s of a quiet CIVVIS seat (180 s with no
  seat active), and the harness window says what you can do: close a screen
  that is waiting for a click, or press NEXT TURN. Nothing of CIVVIS's clicks
  for you in this mode.
- The terminal prints a line when it is your turn and one per CIVVIS turn.
- **Hot Seat only.** With no harness running, the installed mod does nothing
  at all, in a Hot Seat game or any other.
- Diplomacy with a CIVVIS seat works the way Hot Seat diplomacy does: a proposal
  waits for the other seat's turn.

Stopping: Ctrl-C in the harness window, or `civvis-hotseat --stop` from another
one. Civ VI and your game stay up, and within seconds every seat is a person's
again (the mod acts only while it hears the harness). `civvis-hotseat --status`
says what is running. Every line of a game is kept in
`~/civvis-civ6-runs/hotseat/<tag>/` (`hotseat.log`, `turns.jsonl`, and one
folder per CIVVIS seat).

### What the mod changes while it is installed

Tell anyone who plays with it:

- **Your own mod setup is rebuilt from defaults.** Installing and uninstalling
  both delete `Mods.sqlite`, or the game would not notice the mod (it indexes
  mod folders by time). Civ VI rebuilds it at its next start, and it holds your
  mod groups (`ModGroups`, `ModGroupItems`): those come back as the defaults.
- **Your own interface mods may be pushed aside.** The mod replaces about 30 of
  the game's screens (popups, the hand-over, the top panel) with the shipped
  script plus its own hooks. Civ VI loads one replacement per screen, so a UI
  mod of yours for one of those screens does not load while this one is
  installed.
- **macOS asks for App Management**, because the mod goes inside the game's
  application bundle (`Civ6.app/Contents/Assets/DLC/CivvisControl`, the only
  folder this build of the game scans). Installing also breaks the bundle's
  code signature, and removing the mod restores it.
- Five CIVVIS seat contexts load in every game, single player included. They
  stay dormant outside Hot Seat, but they are parsed.

### Uninstalling

1. Quit Civilization VI. (The harness removes the mod by itself when it ends
   with Civ VI closed; with Civ VI running it leaves the mod installed and says
   so.)
2. `civvis-hotseat uninstall` removes the control mod from the game. On macOS
   that restores the game bundle's code signature. Like installing, it deletes
   `Mods.sqlite`.
3. Delete the `civvis-hotseat` folder, and if you like the game records in
   `~/civvis-civ6-runs/hotseat`.

### Building the bundle yourself

From a checkout with Python 3.12 and a Rust toolchain:

```
python3 tools/package_hotseat.py --build-orders --archive
```

This builds `civvis_orders`, installs the pinned PyInstaller into a private
virtual environment under `target/hotseat-bundle/`, and writes
`target/hotseat-bundle/dist/civvis-hotseat/` and
`target/hotseat-bundle/civvis-hotseat-<os>-<arch>.zip`. `--orders-bin PATH`
ships an existing `civvis_orders` instead of building one.

## 2. What is verified where

Everything the project knows about how Civilization VI treats the control mod
was measured on macOS (`docs/CIV6_COMPUTER_CONTROL.md`). The live rows below
are the runs of 2026-11-27 on a MacBook Air (M4), Prince, Online speed,
Pangaea, from a checkout: the person at seat 0 was a test stand-in
(`--auto-human`) and a script set the lobby up by hand. Evidence (screenshots,
logs, verdicts) is in `~/civvis-civ6-runs/hotseat/<run>/evidence/` on that
Mac, and the runs are written up in #5943 and #5944. Both manual runs stopped
at turn 5 when the stand-in's own Scout (seat 0's unit, not CIVVIS's) could
not be given a move the game accepted. On Windows the only evidence is the
`windows-x86_64` job of
[`.github/workflows/hotseat-bundle.yml`](../.github/workflows/hotseat-bundle.yml),
on a runner with no game.

| | macOS | Windows |
|---|---|---|
| bundle builds (PyInstaller 6.22.3, Python 3.12) | measured, locally and in CI | measured, in CI |
| `--help`, `--status`, `--stop`, `doctor` from the unpacked archive | measured | measured, in CI |
| `selfcheck`: screen-free from the first import; the `--manual-lobby` install (five seat agents, the cover, `RequireHarness` in every prelude, no `FrontEndActions`) from the bundle into a scratch install tree; the `Automation.log` relay; a frozen worker answers a recorded board into SQLite with a `ready` row; the worker is recognised and stopped; a stale game lock is taken over; a child of the program inherits screen-free mode; the frozen `--manual-lobby` harness installs, `--status` sees it, `--stop` ends it and its beat and mod go | measured | measured, in CI |
| Steam install and user folder resolution | measured | unit-tested over fake trees (`tools/test_civ6_env.py`); on the runner, with no game, `doctor` reports the install not found and the user folder as `C:\Users\runneradmin\Documents\My Games\Sid Meier's Civilization VI` |
| reading the process table and a process's command line | measured (`ps`) | measured, in CI: the Toolhelp table names the program, and PowerShell's command line lets `--status` recognise it |
| the game discovers the mod | measured: install `DLC` only, never the user `Mods` | **unmeasured** |
| `Automation.Log` writes, and where | measured: `<user dir>/Logs/Automation.log`, newline-terminated | **unmeasured** |
| `DB.Query("ATTACH DATABASE ...")` reaches our SQLite file | measured | **unmeasured** |
| paths in the mod's SQL | measured for macOS paths | **unmeasured** (see below) |
| Hot Seat menus, hand-over screen | measured | **unmeasured** |
| the cover over CIVVIS's turns, the person's own map uncovered with no hand-over dialog | **seen live** (`cover-live-7`: six game turns, 12 Start Turn presses with no crash, 0 of 16 screenshots wrong) | **unmeasured** |
| a hand-set lobby with a screen-free harness | **seen live** (`manual-live-1`: CIVVIS played turns 1-4 on its worker's orders; the harness logged 0 screen modules loaded for real, and neither it nor its worker had Pillow loaded) | **unmeasured** |
| the two together | **seen live** for 4 turns (`manual-cover-1`) | **unmeasured** |
| the mod silent once the harness stops | **seen live** (both manual runs: the game's CIVVIS lines did not grow in the 60 s after the stop) | **unmeasured** |
| Esc on the cover; the cover lifting from up when the harness stops or dies; the stall stand-aside timers | tests only (`hotseat_cover_test.lua`, `harness_gate_test.lua`) | tests only |
| the game's process names | measured (`Civ6_Exe_Child`) | **unmeasured** (`CivilizationVI.exe`, `CivilizationVI_DX12.exe` from the Steam depot) |
| a whole game played by the **frozen bundle** (not a checkout) | **unmeasured**: no game was launched while it was built | **unmeasured** |
| a browser-downloaded copy past Gatekeeper / SmartScreen | **unmeasured** | **unmeasured** |

### Found on the way, fixed for Windows in this bundle

These were defects on the harness's non-screen path, found by reading it for
Windows; each is fixed behind a platform check, so macOS runs exactly as before.

- **`os.kill(pid, 0)` is not a probe on Windows.** Signal 0 there is
  `CTRL_C_EVENT` (a console control event, not "does it exist") and every other
  signal is TerminateProcess. The game lock's stale-holder check
  (`tools/civ6_control/gamelock.py`) and the harness's stale-worker check used
  it; a lock left by a crashed run would never have been taken over. Windows
  now asks the system (`civ6_env.pid_alive`), and `selfcheck` takes a stale lock
  over on the Windows runner.
- **There is no `ps`.** `civ6_env.game_pids()` would have read "no game" every
  poll, and the harness ends the moment it does. Windows reads the process table
  in-process; command lines (to tell our workers from a reused pid) come from
  PowerShell's `Get-CimInstance Win32_Process`.
- **`--stop` sent SIGTERM**, which on Windows is TerminateProcess: no teardown,
  the lock and the mod left behind. On Windows `--stop` writes the stop request
  it always wrote and the harness polls for it.
- **The mod strips every backslash from the path it ATTACHes**
  (`sqlSafe` in `CivvisControlAgent.lua`), so a Windows path would have reached
  SQLite as `C:Usersname...` and the mod would have opened some other file. The
  harness now hands the mod that path with forward slashes on Windows, which
  Windows and SQLite both accept; `selfcheck` checks the mod would attach the
  worker's file. Apostrophes are stripped too, on every system: a runs folder
  whose path holds one cannot work (`doctor` says so; set
  `CIVVIS_HOTSEAT_RUNS`). `civ6_play.py` (the unattended lane, not in the
  bundle) still passes the plain path.
- **The mod's sources do not decode under the Windows code page.** Python on
  Windows reads files as cp1252 unless told otherwise, and
  `CivvisControlAgent.lua` holds byte 0x90, which cp1252 cannot decode, so
  `install.py` would fail on its first read. The bundle runs in Python's UTF-8
  mode; a run from a checkout on Windows needs `set PYTHONUTF8=1`. The Windows
  job records this as a finding step.
- **Every decision worker died on its first board** (measured: the first
  Windows run of the bundle job). `civ6_decision_trace.py` checks that the
  decider binary did not change while it hashed it by comparing `os.stat` of
  the path with `os.fstat` of the open handle. On the Windows runner the two
  disagreed for the decider copied into the run directory, so it raised
  "decision binary changed while recording provenance" on the first decision
  and the worker died. Windows now asks `os.stat` both times. The job's
  findings step compares the two: on a checkout file every field agrees, and
  on the decider copied with `shutil.copy2` (as `freeze_binary` copies it),
  idle or running, only `st_ctime_ns` differs, by 24 s (`os.stat`
  1795762985606911500 against `os.fstat` 1795762961649839800, run
  45954989582).
- **`--manual-lobby` refused every Windows path.** It stops when the orders
  path holds a quote or a backslash, which every Windows path does; it now
  checks the path as the mod is handed it (forward slashes). Its loop polls the
  Windows stop request too, and an abbreviated `--manual` re-executes the
  frozen program rather than handing it a script path it cannot run.
- **A frozen copy of the decider named `civvis_orders` would not start**:
  CreateProcess appends `.exe` to a name without one. The pinned copy keeps
  `civvis_orders.exe`.
- `civ6_env.quit_game()` refuses on Windows rather than hard-kill the game.

Seen, and left as they are:

- `install.py` writes the configured scripts in text mode, so on Windows they
  land with CRLF line endings: the seat agent is 779,549 bytes there against
  755,639 on macOS (measured in the bundle job). Lua reads either. The size
  guard against the game's silent drop of a script near 1 MiB
  (`LUA_SIZE_KNOWN_GOOD`) counts the bytes before that translation, and the
  drop itself was measured on macOS only.
- Off the bundle's path: `watch.GameLiveness` (used by `civ6_play.py`'s relay)
  still probes with `os.kill(pid, 0)`, and `--resume` looks for saves in the
  macOS folder only.

### What must be measured on a Mac before the bundle is called supported there

One whole short game played by the unpacked bundle, not a checkout, from a
browser download: clear the quarantine as in section 1, `doctor`, `selfcheck`,
then `civvis-hotseat --manual-lobby` with one Human row after yours for ten
turns. The seat's `turn` events must read `"orders_source": "civvis"`, and
`civvis-hotseat --stop` then `uninstall` must leave `codesign -v` on `Civ6.app`
reporting the bundle valid again.

### What must be measured on a Windows machine with Civilization VI

Before Windows is called supported, someone with the game (Steam, Gathering
Storm) runs each of these and writes the result into this section and
`docs/CIV6_COMPUTER_CONTROL.md`. Commands are for the bundle unless they say
"from a checkout" (Python 3.12, `set PYTHONUTF8=1`).

1. **Where things are.** `civvis-hotseat doctor` with the game installed: is the
   install found (also from a second Steam library), is the user folder
   `Documents\My Games\Sid Meier's Civilization VI` (or its OneDrive-redirected
   copy) with `AppOptions.txt`, `Logs` and `Mods` in it, and does
   `Mods.sqlite` live there (`install.py` deletes it so the next launch rescans)?
2. **Can the mod be written there.** `civvis-hotseat --manual-lobby` with the
   game closed: does the install into `<install>\DLC\CivvisControl` succeed
   without administrator rights? Steam's default library is under
   `Program Files (x86)`.
3. **Does the game discover it, and does `Automation.Log` write.** Start the
   game, enter a Hot Seat game with one Human row after yours, and look for the
   mod's first lines:
   `findstr CIVVISJSON "%USERPROFILE%\Documents\My Games\Sid Meier's Civilization VI\Logs\Automation.log"`
   (or wherever `doctor` says the log is). A `"kind":"loaded"` line means the
   DLC folder was scanned and the log works. If nothing appears: copy the
   installed `CivvisControl` folder into the user `Mods` folder, delete
   `Mods.sqlite`, restart the game and look again, and record which location
   was scanned. Also record whether the log's lines end in CRLF or LF (the
   relay was measured on LF only).
4. **Does ATTACH reach our file.** In the seat's `events.jsonl`
   (`~\civvis-civ6-runs\hotseat\<tag>\<tag>-p1\`), the `orders_channel` event
   must read `"attached": true` with the forward-slash path, and the `turn`
   events must read `"orders_source": "civvis"`, not `"fallback"`. The deeper
   probe, from a checkout: run the harness with
   `--manual-lobby --play-flags=--probe-channels`, run
   `python tools/civ6_control/probe_channel.py --seconds 240` beside it, then
   `python tools/civ6_control/probe_channel.py --report <seat run dir>`; the
   `db_attach` and `db_orders` fields must name no error.
5. **Path quoting.** Repeat 4 with `set CIVVIS_HOTSEAT_RUNS=C:\Temp\civvis runs`
   (a space) to confirm a quoted path with spaces attaches; a path with an
   apostrophe is known not to.
6. **Process names.** With the game at its main menu, `civvis-hotseat doctor`
   must say "game running: yes" under both the DirectX 11 and DirectX 12
   executables; otherwise the harness believes the game closed and stops.
7. **Hot Seat menus and turns.** A whole game of a few turns: the staging-room
   wording the printed instructions use, the hand-over screen pressed for you,
   the cover over CIVVIS's turns, CIVVIS seats ending their own turns, and
   `hotseat_seat` events reading `"applied": true` (the seat names).
8. **Stopping.** `civvis-hotseat --stop` from a second window while a game runs
   (the Windows stop request), then `civvis-hotseat --status`; and Ctrl-C in
   the harness window. The game must stay up, the workers must be gone, and a
   new `--manual-lobby` must start without complaining about the game lock.

## 3. What a public release still needs

- **Apple notarization**: a Developer ID Application certificate and notary
  credentials (`notarytool`) to sign every binary in the folder and notarize
  it. This repository has neither, so the macOS download is unsigned beyond
  PyInstaller's ad-hoc signature and Gatekeeper quarantines it.
- **Windows code signing**: an Authenticode certificate (OV or EV) to sign the
  program and the decider. Without it SmartScreen warns on every new download.
- **The Windows measurements above.**
- **A publishing workflow.** `hotseat-bundle.yml` only keeps workflow artifacts
  (14 days). A release needs fixed asset names on a GitHub Release, like
  `release.yml` does for the `civvis` engine, and the decider built with the
  full release profile (the bundle check builds without LTO to stay fast).
- An Intel macOS build, if Intel Macs are to be served (`macos-15-intel`).
