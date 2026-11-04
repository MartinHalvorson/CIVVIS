# Honor native policy-change windows

In native run `civvis-20261104T174321Z` at67/0, the UI unlock quote was115 Gold while treasury held53. The controller requested `AGOGE,URBAN_PLANNING` in place of `AGOGE,ILKUM`. At67/1 the new cards were installed and Gold remained53. Government, current civic, civic progress and completed civics were unchanged. The native host verified the changed deck at68. The actual captured last read is byte-bound to the pre-request native state. This disproves the assumption that the direct request is automatically rejected when the UI quote exceeds treasury.

The shipped `Base/Assets/UI/Screens/GovernmentScreen.lua:1383-1422` reads `PolicyChangeMade()` and `GetCostToUnlockPolicies()` and exposes Confirm only when a change has not been made and cost is zero. Its `OnUnlockPolicies` at1589-1592 explicitly requests `PlayerOperations.UNLOCK_POLICIES`. The controller called `RequestPolicyChanges` directly without that gate. The compressed native-before events and JSON preserve the observation; this is not a claim that an unlock fee was paid.

The candidate checks the same free window before a full deck, targeted repair, single-card request, or automatic fill/fallback. A positive quote defers the change; pending, missing, failing, mistyped, negative and nonfinite readings do not establish a free window. It does not buy an unlock or add payment planning. An already-observed complete deck is still acknowledged without a transaction. A deferred request can proceed when a confirmed free window arrives.

Lua5.1.5 syntax-checks122 files and runs117 fixtures successfully. The extended deck regression and new lock/filler fixture both fail against the old controller. Tests cover unchanged slots and no API calls while deferred, free-window release, existing government transitions, targeted repairs, complete-deck acknowledgement, and the actual automatic filler. Source/file hashes are in the JSON.

Native post-deployment verification is still required. Keep Campus-screen observations from before and after this controller correction separate. No strength claim or policy improvement is inferred from these fixtures.
