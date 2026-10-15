# Native aircraft targets and current visibility

Native Emperor run `civvis-20261015T090922Z` lost to Babylon's Culture victory
at turn 213. It took one city and lost seven. This was one fresh world using
source `d187571478e1c22d15714f340927296cb99810d4`, four-player Tiny Pangaea,
Online Gathering Storm, Simon Bolivar, six city-states, all victories and a
650-turn limit. Its complete enabled/forced policy sets match parent
`997f3bc5907c2347139340dada7698e6d763e173` (442/270). No improvement in win
rate or native Domination result is established.

All 599 decisions used downloaded native binary SHA-256
`fe7bb9177eee057a74a1670f7427eb75da957d64ad5dae2f27f47d3f5a6cb165`.
All reported complete captures: 10,394 reads referencing 707 archived
snapshots. Every snapshot was checked against its exact native event prefix.
This is not a reconstruction of persistent AI history.

## Observed failure

There were 18 native `AIR_ATTACK` refusals. All reported
`native_target_known=true`, `native_target_offered=false`; 15 target plots
were under native fog. Three visible improvement targets were also refused;
their cause remains unresolved and this change does not address them.

At turn 170/frame 0, Bomber `11927570` at `(36,25)` targeted the standing,
unpillaged Spaceport at `(46,24)`. That plot was revealed but not currently
visible. The nearby Spy stood in Babylon at `(45,26)`. Reconstructed sight
allowed the simulated Air Pillage; the native engine refused it. The same
Spaceport was refused again on turns 171 and 180 and never received a native
strike during the game.

A fresh one-shot decision on exact capture snapshot 534 reproduces
`AIR_ATTACK (46,24)` for the same Bomber. The input is 51,822,717 bytes,
SHA-256 `14b11467adb82a891d6dbc64aada0c3f53fc2c6057635b84aa220077a40e4765`.
The archived input output was checked byte for byte against that native
prefix. This reproduces the order, not counterfactual survival or victory.

The host exports current plot sight through
`PlayersVisibility[pid]:IsVisible(x, y)` in
`tools/civ6_control/mod/CivvisControlAgent.lua` (the `vis` plot field).
The installed game's `Base/Assets/UI/Panels/UnitPanel.lua:3817-3819` offers
only rows where `modifier == UnitOperationResults.MODIFIER_IS_TARGET`.
`Base/Assets/UI/WorldInput.lua:2077-2078` requests an air attack only after
`CanStartOperation` accepts its parameters. The refusal's native target
annotation uses these primary interfaces.

## Proposed repair and validation

Validation is in progress. Minimized regressions first exercise excessive
reconstructed sight near a Spy, a visible alternative, visibility refresh and
the fallback for older exports without current plot sight. The intended
repair uses the existing per-unit strike block mechanism for native fogged
air targets; it does not change simulated visibility rules or game settings.

Raw receipts, captures and the frozen one-shot result remain outside the
repository under
`~/civvis-tactics-results/2026-10-14/current-conquest-core/`. The separate
capture forwarding driver has SHA-256
`25d20613ef422b7066a875739561ca992d385c8a77204e3410455cde087c6783`;
it changed orchestration only, while the source and binary remained frozen.
