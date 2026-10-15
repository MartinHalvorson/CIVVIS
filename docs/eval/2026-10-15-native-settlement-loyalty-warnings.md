# Native settler lens loyalty warnings

Computer: MacBook Pro

## Native evidence motivating the diagnostic

Valid Emperor attempt `civvis-20261015T113100Z` founded Caracas at native
`(20,20)` on turn 27. Its loyalty rate was -20.6562 on turn 28; it revolted
on turn 35, in peacetime, after the last export showed 3.1875 loyalty and
-9.92188 per turn. Before founding, the observed rival player 2 had no
revealed city or border in the captured input. This is not evidence that
the existing known-border guard failed.

The same game lost Maracaibo at turn 58 while at war, after an export with
100 loyalty, +14 loyalty per turn, and 32 damage. That loss must not be
classified as loyalty. The eventual loss was a rival Religious victory at
turn 135; this diagnostic does not claim to explain or prevent that outcome.

## Shipped UI contract

`DLC/Expansion2/UI/Additions/SettlerInfluenceIconManager.lua:103-112` reads
`Map.GetContinentPlotsLoyalty()`, maps each index through `Map.GetPlotByIndex`,
and displays its value as the settler lens warning. The tooltip in
`DLC/Expansion2/Text/en_US/Expansion1_InGameText.xml:565-566` describes the
influence of nearby cities on a new settlement and recommends a governor
or another location. This is a native UI observation; it does not require
inventing the locations of cities hidden from the mirror.

The value describes nearby-city influence, not the final rate after all
local modifiers. It must not replace a complete loyalty forecast directly.
The shipped UI queries it with the settler lens active. Availability and
refresh without selecting that lens remain native readback questions.

## Implementation under evaluation

Normal state exports also emit a separate `settlement_loyalty` event.
It contains the turn/frame, availability status, and numeric warnings
for revealed plots only, in stable coordinate order. The reader queries
the native API only for the local player while an owned Settler exists.
It changes no unit selection or lens state. Missing APIs and failed reads
produce an unavailable result; an empty native table stays empty.
An omitted plot is unknown, not a zero-pressure prediction.

The separate event leaves older deciders' state schema intact. This change
does not yet import the values into CIVVIS or alter settlement decisions.
Native readback is required before marking the PR ready.

## Validation

- Baseline: the new production-helper test fails on the missing reader.
- Lua 5.1 compiles all **108** mod scripts and passes all **101** discovered
  test scripts, including the register-budget and existing export checks.
- The new test executes the production reader and call site. It covers
  numeric/string warnings, zero, hidden plots, deterministic order, empty
  responses, unsupported APIs, malformed values and plots, missing visibility,
  other players, no Settler, and action-transition suppression.
- An initial local suite run exposed two fixture sections that included a
  new table-member definition without defining its table. The reader now
  follows the existing bare-global function pattern; the full rerun passes.
- Native API data, refresh, and runtime cost remain **unverified**. No
  native strength, founding, or loyalty improvement is claimed.
