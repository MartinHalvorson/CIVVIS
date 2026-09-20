# Preserve queued siege work across replans and host upgrades

Native King/Gran Colombia run `civvis-20260920T104944Z`, pinned to
`c027b1c4a1cffbc5024500650ef727468142d1f9`, ordered Bogotá to build a
Trebuchet on turn 101. A defensive replan then lost its known city target and
removed the first-siege valuation. That replan appeared in reasoning but did
not export a replacement at that frame. On turn 103 the host reported a
Bombard with 67 production invested and three turns remaining. The controller
exported `produce BUILDING_UNIVERSITY` for native city 65536; turn 104 read back
a University. No siege unit was fielded until turn 132.

Two commitment failures contributed. The siege reservation required a current
city target even for an already queued first weapon against known hostile
walls. Separately, `release_foreign_production` cleared the planning queue when
the host upgraded the ordered Trebuchet to a Bombard. This made real work look
idle before the production governor saw it.

The reservation now survives a targetless replan only for an existing land
siege queue, while hostile walls remain and no other queued or fielded siege
weapon fills the need. It does not start new targetless siege reservations.
Emergency local defense retains its upstream priority. The live bridge also
recognizes forward unit upgrade chains as continuation of its own production;
unrelated replacements and backward changes still release the queue. The
installed game's `Base/Assets/Gameplay/Data/Units.xml:875` contains
`<Row Unit="UNIT_TREBUCHET" UpgradeUnit="UNIT_BOMBARD"/>`.

## Validation

The targetless Recovery and Expansion governor regressions failed before the
reservation change and passed afterward. Eleven focused siege tests passed.
A separate bridge regression failed on Trebuchet-to-Bombard before the upgrade
fix and passed afterward; its seven cases cover direct and multi-step upgrades,
another unit line, backward changes, unrelated units, buildings, and unknown IDs.
The full `cargo test --profile ci --locked` suite passed: 3,691 library tests
and 205 binary tests, with 49 library and four doc tests ignored.
`cargo fmt --check` and `git diff --check` passed.

## Frozen native replay

Both binaries used identical fast release flags (16 codegen units, no LTO,
incremental enabled). Baseline source was claim `bdec671ae2af7845d1ddc482b0060d164ba06605`
on main `0ed0054e47b1eb9f045d054c3dd79d7e25ddeb39`. Inputs were the native
run through turn 135: 12,399 events, 25,033,482 bytes, SHA-256
`3b2ca412fd1f1d0024891b34ee735a3480e4d838059a4d1d1ca40c2db47bfddf`.
Both baseline and final candidate completed 353 decision frames successfully
with the same 19 forced genes and domination target.

The reservation-only candidate changed zero exported order frames: it was
insufficient. The final candidate changed 35 frames. Both still order the
Trebuchet at turn 101; only baseline exports the University replacement at
turn 103. The earliest difference is turn 97, where another upgraded queue
holds for one frame before its wall replacement. Later differences include
other cities' choices because preserved military queues affect empire counts.

These are frozen-state controller comparisons, not new native outcomes: after
orders diverge, subsequent recorded states still describe the original game.
They establish removal of the observed replacement order, not earlier weapon
completion, a successful siege, or a victory. No engine rules changed; focused
governor/bridge regressions and the native replay are the relevant validation.
Artifacts are retained locally under `/tmp/civvis-queued-siege-replay/`.
