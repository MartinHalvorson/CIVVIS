# Native unit resource prices

The native Online-speed game offers a standard Bombard for 10 Niter, while
the mirror's ordinary rules charge 20. With 22 Niter on native turn 144,
the mirror could reserve only one Bombard's materials instead of two. The
same readback offers a Cuirassier for 10 Iron. This change reads the exact
city and formation price rather than guessing a speed multiplier.

The installed primary source is
`DLC/Expansion2/UI/Loaders/ToolTipLoader_Expansion2.lua:109`:

```lua
local resourceAmount = pBuildQueue:GetUnitResourceCost(unitReference.Index, formationType);
```

The control agent exports that accessor's finite, nonnegative result as
optional buildable-menu field `r`, using the unit's Index and the native
Standard, Corps, or Army enum. Missing or invalid results remain unknown;
zero is a valid price. Rebuild and incremental mirror synchronization install
the current prices by city and exact unit/formation key. Affordability and
material reservation use the complete native bill without multiplying a
formation or applying a governor discount again. Ordinary simulator boards
and older exports continue to use the existing fallback rules. Incremental
updates clear cached producibility, including when a later export omits `r`.

## Native observation and policy comparison

The standard climb's labeled saved-game diagnostic
`civvis-20260927T165747Z` restored native turn 144. Its in-game seat readback
confirmed four players, King, Gran Colombia / Simon Bolivar, Tiny Pangaea,
Online speed, Gathering Storm, no extra modes, all six victory types enabled,
and a native turn limit of 250. All 21 existing force-on treatments remained
enabled. This is excluded from the fresh ladder and is not a fresh game.

The observer exported native prices while retaining the baseline decider.
The frozen comparison fed 58 identical recorded frames, native turns 144–163,
to the baseline and corrected deciders with the same 184-treatment genome.
Eight frames changed production choices: Bombard `produce_next` hints replaced
Field Cannon or Cuirassier hints in Guayaquil or Popayán. At turn 144, both
deciders suggested a Bombard in Bogotá; the corrected decider also suggested
one in Guayaquil. Repeated frames are repeated observations, not additional
units built. The future boards were fixed observations from the baseline;
this measures policy differences, not counterfactual conquest or win rate.
An Ironclad's actual native resource price was 1, so the observation does not
justify removing resource requirements from all units with maintenance costs.

| Artifact | Pin |
| --- | --- |
| Input save SHA-256 | `f0a9089a30e32b7afa8a673738d67f08a261e75146e9e13704667ca76982e70b` |
| Frozen event-prefix SHA-256 | `16977d74aa125db72823d7a4c6cae50c39ea49c4db1f7963e2e62ff60191776e` |
| Observer/baseline source | `65ea9d21707519e9f3b34e8d8c3f3699cde178e0` |
| Baseline decider SHA-256 | `d0bd1f27f3ffb18c5e76bb6e4a4ec12f5047188e7b1f173e03fb4da74b3e9483` |
| Corrected source | `3169ee911541d1ee8f907f3b1b2ff195f182b97a` |
| Corrected decider SHA-256 | `ccf972fb17385011c85a8072f41e6f2c5840c546a93bdebcb3872462bab53818` |

Raw evidence is retained outside Git under
`~/civvis-war-evidence-20260927/native-unit-resource-prices/`, including
`native-price-readback.json`, `frozen-policy-comparison.json`, source/binary
freeze receipts, replay outputs, and validation logs. No verified native
Domination win or forward candidate-game strength gain is claimed here.

## Validation

- `cargo test --profile ci --locked`: 4,428 passed, zero failed, 53 ignored.
- Five focused Rust regressions cover parser/rebuild/sync, cache invalidation,
  city-specific formation prices, explicit zero, invalid/absent fallback,
  affordable production, exact material reservation, and idempotent resume.
- All 70 discovered control-mod `*_test.lua` files pass through Lupa Lua 5.1.
  The new suite executes the shipped agent and checks Index rather than Hash,
  both enum spellings, exact tier calls, complete prices, zero, and missing,
  throwing, negative, NaN, infinite, and nonnumeric accessor results.
- Changed Rust files pass `rustfmt --check --edition 2021`; diff checks pass.
- Release decider build succeeds; both frozen replays complete all 58 frames.
- GitHub checks on the implementation checkpoint are all green. The final
  documentation checkpoint is independently checked before integration.
