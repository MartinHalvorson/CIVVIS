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

A descriptive census of 36 completed native attempts between `040148Z` and
`090922Z` found 66 refused air attacks in five games: 59 against fogged plots
and seven against visible plots. All 471 air sorties that passed the native
start check, across five of those games, targeted currently visible plots.
These are different worlds and sources, so the census supports an execution
guard rather than a win-rate comparison.

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

## Repair and validation

The mirror's shared final import step now adds known native fog targets
within each aircraft's attack range to the existing per-unit strike blocks.
This runs after host sightings are recorded, on both rebuild and refresh.
It covers Air Strike and Air Pillage before selection and simulated
execution. Positive exported foreign-unit sightings remain valid, and
explicit strike refusals remain blocked even when their plot is visible.

Exports without any positive current plot-visibility flag retain their
previous behavior. Ground attacks and the simulation's sight derivation are
unchanged. Native refresh replaces the blocks, reopening newly visible
targets. Sight gained by a simulated move waits for that native refresh;
the guard does not predict a new native target offer from reconstructed sight.

Before the repair, checkpoint `f9d054e70561b8517c70d3af6717ce2ff3748dc1`
failed both minimized fog regressions for the expected assertions; its legacy
control passed. The local command was
`cargo test --profile ci --locked --lib mirror::air_fog_targets`
(10m38s compile, exit 101, one passed and two failed).

All seven repaired controls pass: the excessive sight case and a usable
visible alternative, refresh to visibility, legacy fallback, fresh visible
target, exported foreign-unit sight, explicit refusal, and a ground-shot
scope check.

| Validation | Source | Result |
| --- | --- | --- |
| Full local `cargo test --profile ci --locked` | `0d4ad940e4e211d0d21c03157a5901b74c85a889` | 5,960 passed; 51 test ignores and four doc ignores |
| GitHub full Rust suite, run `38716713454` | `0d4ad940e4e211d0d21c03157a5901b74c85a889` | 6,047 passed; 51 skips and four doc ignores; all PR checks green |
| Clean integration with main `8f508ffaa8e7c086290a41fdc0b148c128bbbbed` | `3c7337bf86abe9b22e324d587547ea52f4374bef` | No conflict; `cargo check --profile ci --locked` passed in 27.10s |
| Native selected controls, run `38717214429` | `d5b72a3f4e8e471be021162156edfac1556601e0` | 112 passed, including all seven new controls; 2,519 tracked source files verified |

The native candidate changes only this mirror guard and its controls from
the captured pilot's `d187571478e1c22d15714f340927296cb99810d4` source.
Its guard module and test file are byte-identical to the public repair.
It preserves the original AI, rules, deployment and native control files.
Both downloaded arm64 executables passed local smoke checks after receipt,
compiler, complete source manifest and binary checksum verification:

- `civvis`: `6704e08468d9d6d54058d8bd412a76692bc53086b67f148347515d1db83d0a86`
- `civvis_orders`: `d46182ebcca636ff93e2cd34b36c9b8a8b0f7cd28e3f3939892c154f9fed33af`

## Recorded native turn comparison

Both verified native binaries were run afresh on the identical captured
turn-170/frame-0 prefix above, with all 279 original CLI forcing choices.
Both complete effective policy sets equal the original native run's
442 enabled and 270 forced treatments. Each binary's archived input was
checked byte for byte against that prefix.

| Binary | Bomber `11927570` order | Native plot sight |
| --- | --- | --- |
| Baseline `d187571478e1c22d15714f340927296cb99810d4` | `AIR_ATTACK (46,24)`, the Spaceport refused in native play | Fog |
| Candidate `d5b72a3f4e8e471be021162156edfac1556601e0` | `AIR_ATTACK (45,26)`, Babylon | Visible |

The candidate assigns both available bombers to visible Babylon, instead of
ordering one toward the known rejected Spaceport. This proves the intended
selection change on the recorded input. These are fresh one-shot decisions;
they do not reconstruct persistent AI history, execute the alternative in
the original native game, or establish survival, damage, capture or win-rate
improvement. Fresh native play is a separate check.

Raw receipts, captures and the frozen one-shot result remain outside the
repository under
`~/civvis-tactics-results/2026-10-14/current-conquest-core/` and
`~/civvis-tactics-results/2026-10-14/native-air-fog-targets/`. The separate
capture forwarding driver has SHA-256
`25d20613ef422b7066a875739561ca992d385c8a77204e3410455cde087c6783`;
it changed orchestration only, while the source and binary remained frozen.
