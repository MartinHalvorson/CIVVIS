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
verified by SHA-256. An external staging failure refuses the load;
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

The replay ran after fresh `civvis-20260927T155010Z` exited around turn 115.
That interruption has no native victory/defeat result; the preserved crash
report records `EXC_BAD_ACCESS`, `SIGSEGV`, and invalid address `0x18`. Its
autosave was archived separately, rather than treating the exit as a loss.
An expiring, fingerprinted lease held only its supervisor while the active
climb and player continued. The Terminal-owned wrapper then ran the actual
standard climb from frozen candidate `77cfd585f82e4cc795e7a498aa4f0211b9e863af`.

Native `civvis-20260927T160634Z` loaded turn 204 and observed every turn through
221 (53 state frames, 18 turns). The 1,416,677-byte input SHA-256 was
`6e3cf4bcfdde42bed4ce9af39e30d80bac25ff6ad5986117447921038c00907b`;
the staged native manual row passed that same checksum. Native seat readback
matched the registered four-player King Gran Colombia profile, all six
victories, and the native 250-turn cap. All 21 force-on treatments were read
back in the actual genome. The decider SHA-256 stayed
`d0bd1f27f3ffb18c5e76bb6e4a4ec12f5047188e7b1f173e03fb4da74b3e9483`.
The observed player PID 47421 had standard climb PID 47161 as its parent,
satisfying the existing watchdog ownership condition. No freeze occurred;
recovery label inheritance and pin refusal remain regression-tested, not
newly measured native recoveries.

The native terminal event was rival team 1's Science victory at turn 221,
`won=false`, matching the prior trajectory's ending. It is one saved-board
diagnostic, with no Domination gain claimed. The complete summary and root
`diagnostic-result.json` retain the label and input pins. Owned cleanup
completed and the ordinary supervisor resumed at 16:11:15 UTC.

The premerge supervisor initially backfilled this labeled diagnostic using
the old `a5b6d499f` reader, which did not understand the new exclusions. The
rollout correction retained a full ledger backup and removed only this
task's nonwinning diagnostic row; the other 1,296 entries and rung wins stayed
unchanged. The new reader refused to record the same summary. Future
premerge diagnostics must keep the boundary reserved until the new reader
is integrated. This mixed-version observation and its correction are in
`mixed-version-ledger-correction.json` outside the repository.

No watchdog ownership check, native outcome, capture-permission setting,
or live-game process name changed.

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
continued eligibility of ordinary recovery. Another 77 adjacent snapshot,
conquest, ledger-publication, and watchdog tests passed. The required
`cargo test --profile ci --locked` passed 4,423 tests, with zero failed and
53 ignored. No engine or AI code changed in this process fix.

The fresh game `civvis-20260927T153159Z` independently ended at turn 156 in
Religious victory for rival team 3. Complete native ownership records show
two foreign major cities (Tver at 149 and Nizhniy Novgorod at 152), one foreign
minor city (Geneva by 156), and zero foreign major original capitals. The
last native state shows seven of 13 cities following Orthodoxy, compared with
six of 12 immediately before Geneva joined the empire. All three acquired
cities followed that faith. This is another native religion-defense failure,
with city acquisition contributing to the observed majority; it is not
evidence about the diagnostic runner or the strength of the Congress fix.
