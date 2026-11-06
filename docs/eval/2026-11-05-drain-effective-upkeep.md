# Effective upkeep when leaving the bankruptcy drain

`bankruptcy-exits-the-drain` and its unpriced version 2 ranked disband
candidates and credited savings using the ruleset's printed maintenance times
formation. They did not subtract the current policy discount or use the
host's observed per-unit bill. A unit that cost nothing could be deleted to
cover a deficit, and the loop could stop before the real savings covered it.

The candidate cost now follows the economy's existing maintenance calculation:
use the observed pre-discount formation bill when present, otherwise modeled
maintenance times formation; subtract the policy discount with a zero floor,
then add the military maintenance surcharge. Both ranking and credited savings
use that effective cost. Zero-cost units are ineligible. Existing protection
for garrisons, siege guns, air units, active sieges and version 2's front remains.

## Shipped rule

`Base/Assets/UI/Screens/ReportScreen.lua:306` reads:

```lua
local MaintenanceDiscountPerUnit:number = pTreasury:GetMaintDiscountPerUnit();
```

Lines 314–334 select `GetUnitMaintenance`, `GetUnitCorpsMaintenance` or
`GetUnitArmyMaintenance`; the chosen bill already includes formation.
Lines 338–341 subtract `MaintenanceDiscountPerUnit` and report only positive
results. `Game::unit_gold_maintenance_cost` already models that order, including
the policy surcharge. This correction applies it to the bankruptcy chooser.

## Native accounting case

Ordinary Indonesia/Gitarja Deity Online Science run
`civvis-20261105T221304Z`, source
`7d5d17cab46a176abf0d516127e4b1ba9cfa799b`, executable SHA256
`cbafaaa5fb627f806a4cd71b24b6b3824c57f750299f4d4c18e1251cb130fbc9`,
request `162/0`:

| Native unit | Exported maintenance | Per-unit discount | Effective upkeep |
|---|---:|---:|---:|
| Quadrireme 6356998 | 2 | 2 | 0 |
| Musketman 3145737 | 4 | 2 | 2 |

The controller emitted `DELETE` for both units and an Ironclad. Native execution
refused the Ironclad deletion; the two other units disappeared before frame 1.
Every surviving unit's exported maintenance remained equal. The native unit
bill fell from **45 to 43**, consistent with only the Musketman's discounted
2 Gold. The journal had credited 2 for the free Quadrireme and 4 for the
Musketman. The aggregate change is not attributed to the Quadrireme alone.

The independent terminal audit verifies all 485 canonical decisions, 8,437
captured reads, raw archive hashes, and executable/assigned-target bindings.
A separate case reconstruction checked the exact removed identities,
unchanged survivor bills, native discount, emitted orders, refusal and loss
events. Its local receipt is
`indonesia-science-221304-audit/zero-cost-quadrireme-162-native-case.json`
(SHA256 `18a47e6ef12968e5b97fd8116811dc4b347221601004cf7a9d91aed8574c0336`).

## Validation scope

The regression keeps the free observed Quadrireme and disbands a paying unit
under either version. A second test compares the actual before/after economy
bill with the amount needed to cover a deficit across policy discounts,
formations, observed bills and the Elite Forces surcharge. Existing tests cover
the gene-off, cash-covered, garrison, siege, front and per-turn-cap boundaries.

The two affected versions have no priced ledger head; no registry, verdict,
price, force file or default is changed. The correction does not establish a
native strength improvement or solve the larger infrastructure deficit. No
native game or waiting harness is launched for this work.
