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

Holding stock can defer profitable sales. Judge any native trial using
successful upgrades, treasury and trade income, expansion, capital captures,
and terminal outcomes. Refused sale packages alone do not establish a
performance improvement, and the observed Iron income may cover a temporary
shortfall without a purchase.

## Regression result and implementation

The no-op extraction at `6a306a3f5` produces four passes and exactly the
three registered logical failures. Each unsafe-sale assertion reports zero
removed packages where one is required; all fixture and control assertions
pass. The validation receipt retains the complete output and all four source
hashes before the behavioral change.

The implementation shares eligible native upgrade quotes between the buyer
and a stock reserve. The buyer retains its original shortfall calculation,
cash floor, ordering, and one-upgrade limit. The reserve keeps the largest
single fundable bill for each resource, rather than summing the army. The
sale handler tracks quantities committed by earlier retained packages and
rejects whole packages that cross a reserve. Missing-material and siege-gun
sale/barter protection remains in place.

Two additional package checks cover repeated resource entries and a rejected
package leaving the budget available to a later safe sale. Two core checks
cover a ten-unit army retaining one native bill and fractional native costs
rounding only the missing purchase. GREEN and full-suite validation are
pending. These changes have not been deployed into a native game.

The first post-implementation build at `1d0bbcd25` fails because the new
bridge helper refers to `BTreeMap` without importing it. This is a compile
failure, not GREEN or a policy result. Preserve that log and correct those
two references before repeating validation.

## Follow-up limit on the native witness

The companion follow-up archive retains fourteen first-frame states from
turns 113 through 126, all structured notes and the full journal in that
interval. These observations were selected after the sale was noticed and
remain development evidence. Iron replenishes to eleven by turn 117; both
observed units are still Heavy Chariots then, with native eighty-Gold Knight
quotes blocked by a treasury of only sixty-three Gold. At turn 118 both
remain Heavy Chariots with friendly-territory refusals despite thirteen Iron.

The existing return-to-upgrade mechanism is already present and logs actions
in this native game. At turn 114 the second chariot is four tiles from a
visible enemy Bombard and Field Cannon, while the first is three tiles from
its own nearest city. The structured plan is Recovery. These facts do not
reconstruct all planner reservations or prove a safe counterfactual upgrade.
Preserving the stock alone is not evidence that either upgrade completes or
that the trade-income cost is worthwhile. A native trial must retain these
economic and territory constraints in its interpretation.

The additional upgrade audit pairs every issued native upgrade in the
original four-game pilot with its exact input frame and following state.
It requires the host-quoted successor, a unique new unit at the same tile,
preserved XP, level, formation and promotions, and complete, unambiguous
observations. The off/on/on/off games show 13, 32, 15 and 19 such transitions,
including recon and naval upgrades outside the material buyer's scope. Two
of these successful transitions carry later `unit_gone` failure receipts;
the archive preserves the actual before/after states and both receipts
rather than equating a receipt count with completed upgrades.

In the sale-witness game, its one observed Heavy Chariot-to-Knight upgrade
occurs at turn 110, before the first material-buy marker at turn 115. The
five subsequent upgrade transitions are at turns 145, 159, 163, 188 and 196.
This is descriptive evidence from the original unpaired maps, with no
attribution to purchased material and no native observation of this repair.
