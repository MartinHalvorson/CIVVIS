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

## Implementation

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
forecast use the prospective accessor. A current native reading also reaches
the ordinary destination verdict when the optional modeled-rate treatment
is off and no rival city is visible. This closes the move-and-found path.
Persistent sync retains the mirror's existing blocked-site history while
clearing missing or stale native pressure readings. Legacy inputs retain the previous
forecast. No registry or deployment-policy flag changes.

## Validation status

The baseline failure and legacy pass are established. The first focused repair
run passed 27 tests and failed two: a governor roster fixture needed its city
list synchronized, and a new refresh test incorrectly expected the persistent
blocked-site union to reopen the site. GitHub independently confirmed only
the latter failure on the pushed governor correction. Both test corrections
are retained in the validation history. Review also found that the ordinary
destination verdict needed the native reading outside optional modeled-rate
checks; its regression now exercises that ordinary verdict.

Revision `4f6c8e8695eaaa3de7b4b2e48410678a79e2347a` passes the complete
local `cargo test --profile ci --locked` suite with its source hashes unchanged
throughout execution. GitHub run `38763032894` independently passes 6,061 Rust
tests with 51 skipped and four ignored documentation examples. Its performance,
collaboration, overwrite, build, and Lua gates also pass. The quality gate
requested line wrapping on two test assertions, corrected after the full local
run; production Rust behavior is unchanged by that formatting correction. The local
changed-line Rust formatting and Clippy gate passes after the correction.

Hosted native build `38763142565` passes on exact source `4f6c8e869`.
All 2,596 tracked source hashes, both ARM64 executable hashes, 156 selected
controls, and both downloaded-binary smokes are verified. The orders binary
SHA-256 is `55b7ceaad9d606baee61ab82dfa5f98b22b69179a388e88414791ed034d2d85f`.

On the same 6,383,227-byte Cumaná prefix and all 282 original CLI flags, the
repaired fresh decision submits no founding orders. Its journal explicitly
rejects `(9,18)` and nearby plots for native-pressure forecasts of -20 per
turn, approximately five turns to revolt. Effective forced and enabled arrays
match the original comparison exactly: 271 and 446 entries. This demonstrates
the forecast behavior on the captured request; it is not the original
persistent agent's exact decision or a native outcome counterfactual.

## Native execution

One fresh native world, `civvis-20261015T164637Z`, uses the verified `4f6c8e869`
controller and unchanged 282-flag context. Actual seat readback confirms
Emperor, four major players, Simon Bolívar/Gran Colombia, Tiny Pangaea,
Online speed, Gathering Storm, six city-states, all victories and 650 turns.
A freeze at turn 97 triggered the standard watchdog's saved-game recovery
under `civvis-20261015T164637Z-cont1`; that continuation is part of the same
world. No manual game intervention or operator retirement occurred.

Across both segments, all 406 decision reports and 7,052 input reads resolve
to 432 captured snapshots and 26,735,636 archived bytes. Every snapshot is an
exact prefix of its own native segment's event log. Runtime identities,
immutable attempt-binary hashes, actual seat settings, and both treatment
arrays match the verified candidate. The interrupted root summary is partial;
its provenance is verified independently from runtime startup records and
the immutable executable snapshot. The continuation has its normal enriched
summary as well.

There are 406 warning events, 109 available and 107 nonempty. Four native
journal decisions reject pressure-doomed prospective sites: forecasts of
-6 at turn 14, -12 at turn 22, and -23 at two sites on turn 61. These are full
local forecasts after substituting the observed pressure contribution.

The world peaked at five cities, lost five cities whose last exports retained
loyalty and showed combat damage, and ended with one city. The existing
production-rank policy retired it at 150: rank four, production 11 against
364/281/368. It reached no terminal victory or defeat. This is mechanism
evidence, with no controlled strength or win-rate improvement claimed.
Global forcing and verification-policy hashes are unchanged after the run.
