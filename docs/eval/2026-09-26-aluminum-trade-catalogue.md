# Aluminum availability on rival trade tables

Prince native run `civvis-20260926T222220Z`, pinned to `3d8d41e43`,
completed Radio at turn 151 and Advanced Flight at 157. Its turn-162 board
has ten cities, an Aerodrome, no Settler, and zero Aluminum stock or income.
The two charted deposits are unowned: `(6,23)` has uncharted surroundings,
and `(9,22)` forecasts negative colony Loyalty. The seat still reports zero
Aluminum income at turn 209. None of this establishes that any rival can sell
Aluminum: the exported state currently includes only tradeable luxuries.

This patch reads the strategic resource amounts available in each met,
peaceful major's existing diplomacy deal column. It neither requests nor
closes a deal, changes a working deal, reads a hidden resource tile, nor
changes AI policy. Missing API/deal data remains unknown, distinct from an
empty known catalogue. Existing snapshots remain readable.

The shipped source is `DLC/Expansion2/UI/Replacements/DiplomacyDealView.lua`:
line 1715 calls `DealManager.GetPossibleDealItems(player:GetID(),
GetOtherPlayer(player):GetID(), DealItemTypes.RESOURCES, pForDeal)`;
lines 1722–1724 filter by resource class and display `entry.MaxAmount`.
Those are the quantities exposed here. They are current trade offers, not
an inferred income rate or the rival's total inventory.

Before implementation, add a Lua regression against the real exported
function, then verify exact resource quantities, invalid entries, unknown
versus empty replies, partner eligibility, and absence of deal mutations.
Run mirror deserialization tests for absent, null, empty-array, empty-map
and populated catalogues. Run the complete Rust suite, Lua mod suite and
changed-line Rust quality checks. A simulator win comparison is inapplicable:
no game or policy behavior changes. Native data will be read only after the
owner adopts the merged revision at an ordinary completed-game boundary.
The active native tab, process and orders database remain untouched.
