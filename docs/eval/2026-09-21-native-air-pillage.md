# Export strategic bombing to the native game

Native run `civvis-20260921T161428Z` built its first Bomber by turn 148.
It executed air strikes on turns 148–152, then selected an infrastructure
bombing mission after its turn-153 promotion. On frames 153/1 through 156/2,
Bomber 8323094 repeatedly had an internal `AirPillage` action against axial
(8,20), native offset (18,20), but no exported unit command. The translator
handled `AirStrike`, `AirRebase`, and `AirPatrol`, and silently dropped
`AirPillage`. The skipped-action classifier omitted it too.

The shipped game explicitly uses the existing air-attack operation for this:

- `Base/Assets/UI/Panels/UnitPanel.lua:3629`: “all plots they can attack
  (including air-pillage)”.
- `Base/Assets/UI/WorldInput.lua:2077–2078` checks and requests
  `UnitOperationTypes.AIR_ATTACK` with the target's positional parameters.
- `Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:1041` describes strategic
  bombing of district buildings and improvements, without ground-pillage yield.

`AirPillage` now exports through the same `AIR_ATTACK` mapping as `AirStrike`.
The existing native operation checks legality and refuses undeclared wars.
A missing native aircraft ID still produces no order and is classified with
other unmapped unit actions. No combat rule or mission valuation changes.

The regression fails before the mapping change and passes afterwards. Native
execution of the newly exported mission remains a separate verification step;
a recorded-state replay cannot establish that the host accepted a new order,
that the siege succeeded, or that the game was won.

## Validation

The isolated patch at 2ba67337c was replayed against 2450248dd on all 497
decision frames through turn 180/2. The replay's initial genome metadata
exactly matches the native run. It adds 27 AIR_ATTACK orders on 16 frames,
including Bomber 8323094's missing mission on 153/1. Internal action choices
are unchanged across every frame.

Eighteen exported frames differ when verification receipts are included.
The old recorded future did not execute the newly proposed commands: its
22 failed and three successful added receipts are counterfactual comparisons
against that old history, not evidence of native execution.

Local validation on the isolated patch: 4,067 Rust tests passed, 53 ignored;
all 174 order-bridge tests passed, 14 append-point checks passed, eight soak
games completed, and scoped formatting and incremental Rust quality passed.
Replay artifacts are retained under /tmp/civvis-native-air-pillage-replay.

The unchanged native continuation reproduces the gap at turn 184: all four
Bombers (8323094, 9764894, 9044003, and 11206697) select air-pillage missions
on frames 0, 1, and 2. This identifies missing wing commands; it does not
establish the cause of the native game's turn stall.
