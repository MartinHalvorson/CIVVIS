# Deferred production keeps the selected plot

The native bridge accepted `produce_next` coordinates but stored only the item. When the current queue finished, `driveProduction` called `buildParams` without those coordinates and selected the first offered host plot. A remembered direct order also outranked the deferred instruction, allowing an old repeatable unit to run again instead of the planned district.

The lease now retains optional coordinates, takes precedence only once the queue is empty, and passes them through the existing native target validation. A successful request consumes the item and plot and clears the obsolete direct directive. A thrown request retains the lease. A newer direct choice clears an older lease. Coordinate-free hints keep the existing host-selected behavior; partial or stale coordinates cannot silently substitute another site.

The shipped source `Base/Assets/UI/StrategicView_MapPlacement.lua:47` sets `tParameters[CityOperationTypes.PARAM_X] = kPlot:GetX();`, with PARAM_Y on the next line. Its BUILD request passes those parameters, and its placement target query at line 106 uses `CityManager.GetOperationTargets`. This repair preserves that native placement contract through the deferred handoff.

## Validation

- Official Lua 5.1.5 built in an isolated test directory; no system installation or live game modification.
- Before repair, the executable regression failed: `selected x survives deferred handoff: expected 2, got 1`.
- After repair, all 124 discovered `tools/civ6_control/mod/*_test.lua` tests passed under Lua 5.1; every mod Lua file passed `luac -p`.
- All 11 Python production regressions passed.
- The new executable regression covers an occupied queue, exact non-first site, one-shot consumption, stale legal target refusal, obsolete direct choice, coordinate-free replacement, incomplete coordinates, host request exception/retry, and a newer direct directive.
- The existing control-mod CI job discovers the new regression by glob; no manual test list was added.

This is an actuator regression and repair, not a measured win-rate improvement. The running Campus OFF/ON pilot remains frozen on its original source and does not include this repair. A Lua request returning without an exception does not prove host completion; the existing request-consumption contract is preserved.
