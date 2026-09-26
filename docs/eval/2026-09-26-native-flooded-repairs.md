# Native flooding and district repair legality

## Observed failure

In the King / Gran Colombia / four-major Tiny Pangaea native run
`civvis-20260926T194755Z` (pinned `b0b3c40`), Bogotá's Commercial Hub at
native `(47,20)` was pillaged and absent from the host production menu.
The planner nevertheless selected district repair at axial `(37,20)` on
turns 203–207. The tile became coast and lost its district on turn 209.
The Campus at native `(48,20)` was pillaged from turn 203 and disappeared
into coast on turn 215. It had been healthy on turn 201; this is not evidence
of a whole-game research deficit. No claim is made that the host accepted
the impossible repair requests.

The old tile export carried the lowland band and pillage but not exact
flood/submerge state. The mirror cleared generated weather, then inferred
flooding from global climate phase. Crucially, `can_produce(Item::Repair)`
did not reject flooding even when that inference marked it. The repair
guard and exact tile observations solve separate parts of the same defect.

Read-only source evidence is in that run's `events.jsonl` and `why.log`
under `~/civvis-civ6-runs/control/`. No live order database or game tab
was changed for this task.

## Native authority

Installed `DLC/Expansion2/UI/Replacements/PlotTooltip_Expansion2.lua:34–35`:

```lua
data.Flooded = TerrainManager.IsFlooded(pPlot);
data.Submerged = TerrainManager.IsSubmerged(pPlot);
```

Installed `DLC/Expansion2/Text/en_US/Expansion2_InGameText.xml:403–404`:
`LOC_DISTRICT_REPAIR_LOCATION_FLOODED` — “Cannot repair, location flooded.”
`Expansion2_Buildings_Text.xml` describes Flood Barriers allowing flooded
tiles to be repaired after construction, but not recovering submerged tiles.

## Change and compatibility

The mod exports optional boolean `flooded` and `submerged` plot fields and
includes each in the tile-delta signature. A false result is explicit;
missing, throwing, or non-boolean API reads remain unknown. Unrevealed
plots remain unexported. Both rebuild and same-turn live sync apply exact
flags after phase inference and before yield calibration, so a host-observed
dry tile cannot be reflooded by the model. Unknown fields retain the old
phase-derived fallback, including archived exports.

District and building repairs require neither flooding nor submergence.
Dry pillaged districts remain repairable. No production priorities, research
weights, Flood Barrier policy, or native order translation are changed.

## Validation and limits

Regression tests cover district and building repair enumeration, drying,
submergence, host-state precedence, rebuild, same-turn delta sync, omitted
climate, and legacy fields. The Lua test executes the shipped tile sweep,
including deltas caused solely by the new flags and unknown API results.
Local validation on `207b16d33` passed: `cargo test --profile ci --locked`
(4,299 passed, 53 existing ignored), all 65 discovered control-mod Lua 5.1
suites, full agent Lua 5.1 parsing, changed-Rust formatting/warning checks,
and two King/four-major/Pangaea/Online simulation smoke games with a 250-turn
cap (seeds 926776–926777). Both simulations ended normally with non-domination
victories; they are execution checks, not strength evidence.

This is a legality/fidelity correction, not a measured domination improvement.
After normal integration and a completed-game boundary, check actual native
flags, production choices, and accepted next-snapshot queues separately.
