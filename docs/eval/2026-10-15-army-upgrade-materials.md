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
Validation is in progress. No native adoption, execution reach, win-rate
improvement or highest-difficulty victory is established for this policy.
