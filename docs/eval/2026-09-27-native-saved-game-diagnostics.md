# Labeled native saved-game diagnostics

The Congress replay for #3828 used a separate player launcher. The host wedge
watchdog correctly refused to signal that unfamiliar parent, so the replay's
first turn-208 freeze waited for the outer 900-second watcher. This slowed a
diagnosis that otherwise reused the production cleanup and save-selection code.

The standard climb now accepts an explicitly labeled initial save:

```sh
python3 tools/civ6_civvis_climb.py --attempts 1 \
  --load-save /absolute/path/AutoSave_0204.Civ6Save \
  --diagnostic-label congress-runner-verification \
  --difficulty DIFFICULTY_KING --leader LEADER_SIMON_BOLIVAR \
  --map Pangaea.lua --map-size MAPSIZE_TINY --speed GAMESPEED_ONLINE \
  --victory domination --refresh-seconds 0
```

The input is archived and checksummed before the player starts. The archive,
not the rotating original, reaches `civ6_play --load-save`. A save outside the
native manual directory is copied into its constant `civvis-resume` row and
checked byte-for-byte by SHA-256. An external staging failure refuses the load;
ordinary autosave recovery retains its existing filter fallback. Installed
`Base/Assets/UI/FrontEnd/LoadGameMenu.lua:108` calls
`Network.LoadGame(m_thisLoadFile, serverType);`; lines 320–323 toggle the
autosave filter. The existing menu driver still selects a verified named row.

The diagnostic requires exactly one attempt, a nonempty label, pinned code,
zero decider refresh, and no live gene screen. It uses the standard climb as
the player's parent, with its existing watchdog ownership, cleanup checks,
freeze evidence, bounded backtracking, and six-resume policy. Bridge source and
binary hashes are rechecked before a continuation. Its first observed native
turn must be read from events; the save's numeric suffix is not a readback.

Before every player launch, `native-diagnostic.json` retains the label, root
tag, bridge revision, binary SHA-256, input SHA-256, byte count, and archive
manifest. Every continuation inherits that marker. Complete and partial player
summaries carry it, and ladder backfill also reads the prelaunch marker if a
child never enriched its summary. An unreadable marker remains excluded.
Saved-board results are written to the root's `diagnostic-result.json`, not
the climb's fresh-game JSONL. The ladder excludes diagnostics from both
attempts and rung claims. Append-only published summaries retain the label
for off-seat readers. Consumers that read raw summaries directly must filter
the `diagnostic` field before estimating fresh-game strength.

This does not convert normal frozen-game continuations into diagnostics.
Those are still parts of the fresh game that originally generated them.
Neither a saved-board win nor several recovery segments establish an
independent fresh-game sample or a Domination improvement.

## Registered native verification

The selected input is the complete turn-204 archive from the second recovery
of #3828's Congress replay. That continuation previously escaped the repeated
turn-208 GDR freeze and ended in a rival Science victory at turn 221. Its
profile was four players, King, Gran Colombia, Simón Bolívar, Tiny Pangaea,
Online, Gathering Storm, with all six victories enabled and a native turn
cap of 250. The same decider image and all 21 force-on treatments are retained.

The replay will start only after the currently active fresh game finishes.
An expiring, fingerprinted lease holds only its supervisor; the active climb
and player keep running. A Terminal-owned wrapper will run the actual
standard climb from a frozen candidate checkout. It will restore the ordinary
supervisor after the diagnostic or on failure. No watchdog ownership check,
native outcome, capture-permission setting, or live-game process name changes.

External evidence lives under
`~/civvis-war-evidence-20260927/native-saved-game-diagnostics/`.

## Local regression evidence

Against the unchanged baseline, 11 selected regression tests produced five
failures and five errors. They expose absent CLI support, missing ladder
exclusion, and unstaged external archives. Candidate focused suites passed
204 climb tests, 240 player tests, and 208 ladder tests. They exercise the
actual command builder and main recovery loop, exact archived bytes before
launch, frozen refresh, inherited prelaunch markers, changed-binary refusal,
archive/staging failures, a no-turn start, direct recording and backfill, and
continued eligibility of ordinary recovery.

The fresh game `civvis-20260927T153159Z` independently ended at turn 156 in
Religious victory for rival team 3. It held no foreign cities or original
capitals. This is another native religion-defense failure, not evidence about
the diagnostic runner or the strength of the Congress fix.
