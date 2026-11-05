# Korea's first Seowon slot at one citizen

In the completed qualified Deity Korea Science pilot `civvis-20261105T114538Z-current-writing-pilot-01`, Jinju was founded at33. At43/0 it had one citizen, one specialty-district slot and an idle queue. Native menus offered a Seowon at26production/eightturns in three legal sites, and a Government Plaza at29production/eightturns. The actual decision placed the Plaza. The existing Campus-v2 picker requires two citizens, so it did not claim the first slot.

The native game completed the Plaza at50, reached population4 at57, completed Jinju's Seowon at62 and Library at65. These are observations, not proof that a different first district would have produced earlier milestones. The full pilot lost to a rival Science victory at178 without city losses; its453canonicalrecords and7878actualcapturedreads were independently verified. It establishes no population strength estimate.

## Eligibility change

Under the existing `campus-claims-the-free-slot-2` option, let a one-citizen Korean Science city use its first slot for a Seowon when the native quote fits the option's existing Campus build-time budget. The ordinary picker still determines a legal site and checks actual producibility; its queued-district, threat, recent attack, Prophet and culture checks remain. Other claim protections and production ordering are untouched.

The new exception requires an actual host quote. Other civilizations and victory targets, disabled Science, version one, missing native quotes and over-budget Seowons retain their population floors. This changes no engine rule, native Lua, registry row, new flag or runtime force file. It is separate from the repeated-Settler scheduling change in #4552; both intentional header paragraphs must survive integration.

## Native evidence

The neighboring archive is 91352 bytes, SHA-256 `a413dacfad7270dbd4a6cbcdbe92491d5a16f5b89e532f2769f8468e34e00a05`. Extract it and run `python3.12 verify-witness.py .`; verification also passed after independent extraction. It contains the original canonical record, full actual captured-read sequence, observed source/genome context, selection protocol, preparation source, menus and raw original-source controls.

The protocol selected the first actual read of the preselected43/0 decision before any query, without assuming original read-byte or private-state parity. In this case all18originalreads are in fact byte-identical: SHA-256 `1ad0942a5b71c7b4ae310b2b2d2bd043879fdcf44249a5ee1538be477e23d87e`. The original executable is source `0ab72aa31ec43b2d127bf86ba67b7b1ee1a28c15`, SHA-256 `632d442409e028d97d4f411a6ba64eb8a88e57e6e89f61d4731307a4a9fd8c45`.

Both scheduled original-binary cold controls reproduce the Plaza at native(57,7). Each has17actualreads identical to the selected prefix, the observed genome and stable full orders/decision replies. These controls use fresh private planners; they do not establish the original controller's entire private history.

Any changed-candidate comparison remains fixed to this input and four scheduled OFF/ON/OFF/ON roles for the existing Campus-v2 flag, with no replacement input after outcomes. No changed candidate has yet been queried. Native execution, earlier research or completion, and victory strength remain unproved.

## Validation checkpoint

Scoped rustfmt and `git diff --check` pass. Two new regressions exercise the actual one-citizen queue claim with a version-one control and the native-budget/scope/ordinary-picker guards. The Campus module now has nine tests on this branch. Rust execution, CI, focused Mac validation and the predeclared candidate proposal comparison remain pending.
