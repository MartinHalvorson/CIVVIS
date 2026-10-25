# Forecast the net upkeep of an upgrade

The continuous objective remains competitive native high-difficulty Domination.
This is an upkeep-estimation correction within the existing solvency policy.
No difficulty, victory lane, force bundle, or selection is changed.

Before implementation, register five regression tests against unchanged
production: a nine-case policy/formation table checked against actual engine
upgrades, a one-Gold-income modernizer case, a zero-income/cash-floor control,
a native-current-bill formation case, and an arena control. Preserve all
results, including fixture or compilation failures. Only valid logical failures
establish reproduction. Then correct the estimator, repeat the focused suite
and required full CI-profile tests, and run changed-line quality.

The forecast must subtract independently clamped net old and new bills. Use
the observed current gross bill if present (it already includes formation).
The unobserved successor bill remains a modeled estimate. Shared military
surcharges cancel. Arenas charge no maintenance. Actual upgrade permission,
Gold/resource prices, income and treasury floors remain enforced.

Native development context is the fixed completed game stems
`civvis-20261025T145013Z`, `civvis-20261025T155305Z`, and
`civvis-20261025T160321Z`. Retain their outcomes, first-frame economy/unit
snapshots, and source hashes. Census all their first-frame eligible upgrade
quotes and small-income windows rather than selecting only apparent successes.
These are unpaired maps at Emperor, not a trial of this correction. Record
whether this error can affect those observations; do not infer that it caused
the defeats. The currently observed new game confirms Immortal in its native
seat export.

This change does not modify engine rules. Actual-before/after bill tests cover
the forecast arithmetic and actual modernizer calls cover the guard decision.
Native higher-level Domination improvement requires subsequent game evidence;
these regressions alone cannot establish a win-rate gain.

## Native arithmetic census

All 575 first-frame states and 2,208 mapped land-unit upgrade quotes from the
three fixed games are retained in the companion gzip archive, with source
hashes and native outcomes. An initial audit omitted 130 Llanero quotes;
the final audit uses the exact `colombian_llanero -> llanero` alias already
in `src/mirror.rs`, leaves no unmapped pairs, and reports all cases.

The corrected arithmetic differs on 498 quoted unit-turns, but **zero**
funded, native-legal quotes change the income guard in these first frames.
This is therefore not an explanation of those defeats or a native activation
witness. The census does not reconstruct the full strategic plan, sequencing,
reservations, or replan frames, and is not an exact decision replay.

At turn 100 the win has 160.895 Science, 136 Production, 77 net Gold, six
Libraries and five Universities across eight cities. The first loss has
48.3398 Science, 76.5117 Production, 21.2148 net Gold, four Libraries and
zero Universities across seven cities. The second loss has 55.832 Science,
68.0078 Production and negative net Gold across five cities. These unpaired
map differences identify the economic foundation as an ongoing investigation;
they do not establish that a particular queue change causes victory.

Installed `Base/Assets/UI/Screens/ReportScreen.lua:306` reads
`pTreasury:GetMaintDiscountPerUnit()`, lines 314-335 select the formation's
gross bill, line 339 subtracts the discount, and line 341 admits only positive
net upkeep. The native-rule archive retains that exact excerpt and file digest.
This agrees with the existing engine's unit-maintenance arithmetic.
