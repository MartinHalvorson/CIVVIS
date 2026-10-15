# Use observed native pressure in settlement loyalty forecasts

Computer: MacBook Pro

## Native failure and baseline

Native UI diagnostic `civvis-20261015T144627Z` ran the unchanged verified
`1d770be4c8e0d2b8c660a6ff648e8740c05b47da` controller with the #4060
observer. Immediately before founding Cumaná at `(10,18)` on turn 61, the
settler-lens export warned of -20 nearby-city loyalty pressure. The initial
native city rate was -20; a governor assigned by turn 63 improved it to -12.
The city revolted at turn 69 after its last export showed 8 loyalty and zero
damage. The native controller ignored the new observation.

The exact captured turn-61 prefix is 6,383,227 bytes, SHA-256
`e9838fc6fc11dc6433df609980dc662359300cffdc24a13cef94b237d33a7a72`.
A fresh one-shot with all 282 original policy flags instead founds at the
adjacent `(9,18)`, also carrying a native -20 warning. This is not a replay
of the persistent agent's exact choice. Both destination selection and
final founding need the observation; blocking only the starting plot can
leave a same-turn move-and-found bypass.

The minimized regression on baseline `267f3eb18` fails because founding is
still legal despite the native -20 warning; the legacy-input control passes.
Local result: one expected failure, one pass. GitHub run `38757994033`
independently fails the native-warning case.

## Native contract

`DLC/Expansion2/UI/Additions/SettlerInfluenceIconManager.lua:103` calls
`Map.GetContinentPlotsLoyalty()`; lines 104-112 display its values.
`DLC/Expansion2/Text/en_US/Expansion1_InGameText.xml:566` describes the
influence of nearby cities and recommends a governor or another site.
These warnings are not the final rate after all local modifiers. The
observer's native readback and limits are in
`2026-10-15-native-settlement-loyalty-warnings.md`.

## Implementation under validation

The mirror reads the latest preceding `settlement_loyalty` event at exactly
the selected state's turn and frame, using the existing cached event index.
Missing, unavailable, stale, malformed, or future-frame observations do not
revive earlier readings. Only revealed, finite values in the native pressure
range enter the model. An omitted plot remains unknown. Every native refresh
replaces or clears the reading.

A prospective settlement forecast substitutes that observed contribution
for the modeled nearby-city pressure. The same engine calculation still
prices governors, amenities, policies, starvation, and full-loyalty protection.
Normal owned-city native rate readbacks and turn processing are unchanged.
The observation carries its owner and turn, survives save/load and speculative
clones, and is removed from other players' decision views.

Both the existing immediate founding guard and the AI's destination revolt
forecast use the prospective accessor. Legacy inputs retain the previous
forecast. No registry or deployment-policy flag changes.

## Validation status

The baseline failure and legacy pass are established. The repaired code
passes `cargo check --profile ci --locked --all-targets`. Focused execution,
full local tests, final GitHub gates, exact native-input comparison, and
native execution of the repaired controller are still pending. Keep this
PR draft until the necessary checks and mechanism evidence are complete.
No repaired native counterfactual, win-rate, or strength improvement is
claimed yet.
