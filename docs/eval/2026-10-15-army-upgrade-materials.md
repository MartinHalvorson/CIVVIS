# Materials for a fundable army upgrade

The completed Emperor attempt `civvis-20261014T230322Z` lost at turn 109.
At turn 89 it still fielded ten Heavy Chariots. Their native Knight upgrade
quote was 160 Gold plus ten Iron. The seat held no Iron and earned none;
peaceful Germany and Sumeria each exported 25 tradeable Iron. There were
no recorded Iron purchase proposals. Four Olives were held, so surplus
barter is an opportunity to investigate. Exported offers do not establish
an acceptable price, accepted deal, completed upgrade or successful defense.
The raw evidence and observations remain in
`~/civvis-tactics-results/2026-10-14/capital-recon-priority/native-collapse-context.json`.

The shipped resource buyer asks for siege-gun resources and the air wing's
Aluminum. This change adds the opt-in `army-upgrade-materials` policy for
one useful, affordable land upgrade. It reads the existing exact host
successor, Gold and material bill when present. The shipped UI uses
`DLC/Expansion2/UI/Replacements/UnitPanel_Expansion2.lua:94`:
`local upgradeResource, upgradeResourceCost = pUnit:GetUpgradeResourceCost();`.
The existing `Game::unit_upgrade_resource_price` applies that bill rather
than guessing the model's twenty Iron in the posed ten-Iron native case.

The request retains the upgrade's Gold plus the existing 30-Gold upgrade
floor, increased for negative income. It chooses one healthy, unlinked,
unreserved land unit with at least a 15-strength improvement. Existing
stock or the next turn's income retires the request. An unknown host
successor is not replaced with a guessed unit; an upgrade into a fuel-using
unit requires income that covers its upkeep. The existing upgrade pass and
native permission still determine actual execution.

The trade transport retains the siege buyer's existing budget and cadence.
The army request has its own cash reservation, includes planned spending,
uses no future-income credit, and can barter only the existing bounded
surplus. Native luxury counts protect the last copy. Already committed
sales, another purchase, hostile sellers and incomplete material lots
prevent an ask. Required army materials and the existing siege/air resources
are protected from surplus sale or barter.

The new library and transport controls cover the host bill, one fundable
upgrade from a large obsolete army, disabled and other-victory cases,
reservations, existing stock, invalid successors, cash limits, surplus
barter, missing or hostile sellers, busy deals and last-copy protection.
CI ran all 5,929 Rust tests successfully, including all ten new library and
transport regressions, with 51 existing tests skipped. Rust documentation
examples retained four existing ignores. Rust quality and paired cost checks
passed. The initial overall gate failed on stale evaluation metadata and a
missing fires waiver; the metadata has been regenerated with all 141
deployment tags and the operator selection note preserved. The 21 evaluation
manifest, 17 fires-gate and 186 gene-tooling tests passed locally, as did
`cargo check --locked --all-targets` and all 14 append-point controls.
The initial local library run had 5,593 passes and one server-viewer failure:
its two-second observation window saw only turns 1, 2 and 3, below its required
four turns. The unchanged isolated test passed. No assertion was weakened.

Review found the original transport caller read the untouched authoritative
mirror after planning had spent Gold on a separate copy. A complete decision
fixture buys a Builder and Monument, then the old caller still bids 386 Gold
for Iron against its original 600-Gold treasury while reporting 190 reserved
for a Knight. The corrected caller reads the planning copy. A separate
control rejects even fully bartered material when earlier spending used the
upgrade reservation. Both corrected regressions and all seven transport
controls pass. The complete integrated local suite is being rerun with four
test threads to reduce contention.

The fires waiver records that simulator `gene_screen` does not execute this
CLI/native trade channel. No simulator null result is entered as proof that
the policy fires. No native adoption, execution reach, win-rate improvement
or highest-difficulty victory is established for this policy.
