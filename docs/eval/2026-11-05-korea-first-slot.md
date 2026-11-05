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

Any changed-candidate comparison remains fixed to this input and four scheduled OFF/ON/OFF/ON roles for the existing Campus-v2 flag, with no replacement input after outcomes. All four changed-candidate roles have now completed and independently verified. Both OFF roles reproduce the Plaza at(57,7); both ON roles choose the native-legal Seowon site(59,8). Every role has17actualreads matching the frozen bytes, actual flag absence/presence, otherwise equal genomes and stable full repeated replies. Native execution, earlier research or completion, and victory strength remain unproved.

## Validation checkpoint

Scoped rustfmt and `git diff --check` pass. Two new regressions exercise the actual one-citizen queue claim with a version-one control and the native-budget/scope/ordinary-picker guards. The Campus module now has nine tests on this branch. Candidate17e88ba3f12f0d5175a831ae6c71380e86146bad passed7,020 Rust tests (53 skipped), all nine focused regressions and all eight PR checks. [Mac42038590023](https://github.com/MartinHalvorson/CIVVIS/actions/runs/42038590023) passed all nine focused tests, both native binary builds/smokes and complete source-manifest verification. Main integration retained the implementation and tests; only this change's module prose was moved earlier in the header to avoid a known same-insertion conflict with #4552. Integrated-head CI follows separately.

## Changed-candidate archive

The `.candidate.native.tar.xz` archive is 260536 bytes, SHA-256 `160dd1e421d77c1ee0959237218fc0262ba6dee6816e89729db2efc657852bf4`. Extract it and run `python3.12 verify-candidate.py .`; independent verification passed on the originals and again after extraction. It includes all four raw replies, actual genome logs, sealed captured bytes, command lines, tested native source/build receipt, protocol and probe/verifier source. Native executable SHA-256 is `c8752cabdccc649ce458f132b76bd0a12c7596d14206a72cb204be1a5e4d6ae6`. The OFF role omits this default-OFF opt-in; ON adds `--with`, with actual genome readback required. No candidate decision was retried or input replaced.

This verifies a proposed first-district choice on one observed native public snapshot using fresh private state. It does not establish original private-history parity, actual host placement, earlier completion/research, or a victory-strength gain.
