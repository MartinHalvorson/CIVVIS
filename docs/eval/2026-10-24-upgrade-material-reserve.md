# Preserve a funded army upgrade's material

## Observed native case

The registered `army-upgrade-materials` on-arm game
`civvis-20261025T035310Z` uses source `8810924ee`. At turn 114, its first
frame reports 157 Gold, 15 Iron, and two healthy Heavy Chariots whose native
Knight quotes each require 80 Gold and 10 Iron. Both are outside friendly
territory. The decider emits `sell RESOURCE_IRON=10`; the host records the
sale package. Turn 115 reports seven Iron, and the decider begins requesting
three Iron back for a Knight upgrade. This is a development witness, not a
holdout or a claim about wins.

The current resource-sale protection reads missing-material wants. Those
wants correctly return empty while the stock already covers an upgrade,
which leaves the already-owned upgrade bill exposed to the ordinary sale.

## Registered correction and checks

Keep the existing default-off policy and all published selections unchanged.
Protect enough stock for one currently fundable, eligible native upgrade per
resource. Reuse the same health, role, host quote, research, and fuel guards
as the material buyer. This reserves stock and grants no permission to
upgrade outside friendly territory. Keep the buyer's existing cash limits.

Process sale packages in order: preserve a package that leaves the quoted
reserve and debit its committed quantity before judging another package.
Reject a package that would consume that reserve, including a mixed package;
do not change its price or forge a smaller package. Leave purchases alone.
Use the host's ten-Iron bill rather than guessing the model's twenty, and
retain one bill rather than reserving the whole obsolete army at once.

First extract the existing sale-protection handler without changing it and
run seven focused bridge regressions. Expect three real logical failures
(last stocked bill, double spending, and unsafe bundle) and four controls
to pass. Compilation or fixture failures are not regression RED. Only after
valid RED, add the stock reserve and repeat the bridge and existing material
tests, full required CI-profile tests, and Rust quality checks.

Preserve the complete selected native records, original and corrected test
logs, and source hashes. The four-game native pilot continues on its frozen
per-attempt binaries; this code does not change a running game or replace
the independent war-handover study in #4118. Any native trial of the repair
needs its own source/flag registration and actual readback. No strength gain
or default promotion follows from these regression tests.
