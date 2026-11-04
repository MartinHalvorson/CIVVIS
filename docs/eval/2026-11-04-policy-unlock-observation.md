# Observe policy change costs and upkeep discounts

Native state now exports an optional `policy_change` object with the host's
`unlock_cost`, `made` flag and `currency` (`gold` or `faith`). Each field is
observed independently; unavailable APIs remain unknown. Known zero costs and
false flags are preserved. An entirely unknown observation is omitted, and a
finite raw sentinel such as -1 is retained without interpreting it.

The state also exports optional `unit_maintenance_discount`, the Treasury's
per-unit discount. Existing unit records already carry their per-formation
maintenance before that discount. This lets future policy analyses compare
recurring savings against the native bill rather than assume a current discount.

The shipped `Base/Assets/UI/Screens/GovernmentScreen.lua:1383-1422` reads
`PolicyChangeMade()`, `GetCostToUnlockPolicies()` and
`GlobalParameters.GOVERNMENT_UNLOCK_WITH_FAITH` to decide which controls to show.
Its `OnUnlockPolicies` at 1592 submits `PlayerOperations.UNLOCK_POLICIES`.
`Base/Assets/UI/Screens/ReportScreen.lua:306` reads
`pTreasury:GetMaintDiscountPerUnit()`. The companion receipt pins hashes and
line ranges of the installed source files.

The motivation is a selected native development case: at 142014 turn 342/frame 1,
the treasury held 10 Gold at about -24 per turn. An offline addition of the
existing bankruptcy treatment requests Triangular Trade instead of Medina
Quarter. The old capture lacks the unlock observations, so it cannot establish
that the host could confirm this request. A requested deck is not proof of a
paid/free opportunity or applied policy. The new object records inputs, not a
complete `can_change` verdict: slots, card availability and the game core still
matter.

This change adds observations only. Policy selection, requests, unlock payments,
action counts and Rust consumers are unchanged. The regression fails against
the old script and passes with the new fields; all 116 mod fixtures and 121 Lua
syntax checks pass under Lua 5.1.5. It covers live value changes, zero/false,
missing and failing APIs, wrong types, currency modes, partial observations and
state wiring. Actual native field readback remains a post-merge check; no
historical policy application, treasury improvement or strength is claimed.
