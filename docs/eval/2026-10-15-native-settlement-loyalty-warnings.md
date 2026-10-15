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
The shipped UI queries it with the settler lens active. The native readback
below establishes that this API also supplies changing warnings without
selecting a Settler or changing the active lens.

## Implementation

Normal state exports also emit a separate `settlement_loyalty` event.
It contains the turn/frame, availability status, and numeric warnings
for revealed plots only, in stable coordinate order. The reader queries
the native API only for the local player while an owned Settler exists.
It changes no unit selection or lens state. Missing APIs and failed reads
produce an unavailable result; an empty native table stays empty.
An omitted plot is unknown, not a zero-pressure prediction.

The separate event leaves older deciders' state schema intact. This change
does not yet import the values into CIVVIS or alter settlement decisions.
The native readback below validates the exported signal.

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
- A synthetic diagnostic event added to the exact earlier turn-63 prefix
  leaves the verified legacy decider's entire reply identical under all
  282 policy flags. This is a schema compatibility control, not a native
  API observation or persistent-history replay.

## Native readback

Run `civvis-20261015T144627Z` uses frozen UI source
`634909c36660e335627951515fe203e6c5c4f676` with the previously verified
`1d770be4c8e0d2b8c660a6ff648e8740c05b47da` controller executable
(SHA-256 `2f9f7de0e1fb288426865962a8ce158831273e612876d1b8c0407df483e9b4e2`).
The Rust, data, and build inputs are identical between those sources. The
controller does not consume this new event. Both source identities are
recorded explicitly; this is a UI diagnostic, outside the prior three-game
aircraft cohort. The native seat reads Emperor, four players, Gran Colombia,
Tiny Pangaea, Online, Gathering Storm, six city-states, all victory conditions,
and a 650-turn cap; the same 282 policy flags are preserved.

By turn 100, the exporter had produced 205 available observations and 62
explicit `no_settler` observations, including 163 nonempty warning tables.
No malformed warning values were observed. At least 17 plots changed value
across observations: for example `(9,15)` changed from -9 at turn 39 to -12
at turn 40. Tables were initially empty and supplied negative values from
turn 39 without selecting a unit or changing a lens.

The unchanged AI founded Cumaná at `(10,18)` on turn 61 despite an exported
**-20** native warning at that site immediately before its decision. Its
initial city export also read **-20** loyalty per turn. A governor assigned
by turn 63 improved the rate to **-12**, and the city revolted at turn 69,
following a last export of 8 loyalty and zero damage. This is a concrete
case for a subsequent forecast integration; the exporter alone prevents
no settlement.

A fresh one-shot of the exact captured turn-61 input instead chooses an
adjacent founding site `(9,18)`, also carrying the native -20 warning. It
does not reproduce the persistent agent's exact site, and neither the
one-shot nor this diagnostic supplies a repaired native counterfactual.
The captured prefix is 6,383,227 bytes, SHA-256
`e9838fc6fc11dc6433df609980dc662359300cffdc24a13cef94b237d33a7a72`.

Runtime exports continue normally. The live export clocks have whole-second
resolution; zero timing rows do not establish the query's fine-grained cost.
The reader makes at most one API query per normal state export while a
Settler exists. No native strength, founding, or loyalty improvement is
claimed by this observational change.
