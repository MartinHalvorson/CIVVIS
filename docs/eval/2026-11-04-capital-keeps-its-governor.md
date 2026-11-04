# Opt-in capital Governor Loyalty reserve

The native Campus-screen game `civvis-20261104T192352Z` lost its original
capital, Bogotá, to Free Cities at turn 90. It was undamaged, at peace and on
1.89062 Loyalty with a -3.95312 rate in its last owned turn. This is an observed
Loyalty loss, not a siege classification inferred only from disappearance.

At 69/0 the original planner moved Magnus from Bogotá to Caracas for emergency
Loyalty relief. Bogotá had 96.3125 Loyalty but was already losing 0.515625 a turn
with Magnus. Caracas had 66.7188 Loyalty and a -4.21875 rate. The assignment was
checked natively; at 70/0 Bogotá's rate was -9.0625. Magnus returned at 78/0,
when Bogotá had 22.875 Loyalty; that assignment was also checked. Retaining
Magnus is a plausible intervention, not proof the capital survives: nearby
population pressure continues changing, and the two cities compete for one
Governor.

The fixed first-ten Campus launch-context audit contains seven own-city
Governor-transfer proposals. This was the sole donor already losing Loyalty.
The other capital donor had a rate of +28. These are observed proposals in
mixed worlds/revisions, not randomized treatment outcomes. Their state lines
and signed decision identifiers remain in the external audit.

`capital-keeps-its-governor` is a new, independently screenable opt-in, default
OFF. It prevents emergency and economic Governor departures from our own
original capital when its estimated post-departure Loyalty rate is negative.
Other cities, foreign-founded capitals, unassigned Governors and safe capital
departures retain their old permissions. It neither adds a Governor nor spends
a title or policy unlock.

A positive current rate is insufficient: it already includes the Governor.
The helper removes the fixed observed Loyalty-rate override in an analysis
clone, measures the rules model's support difference with and without that
Governor, then subtracts that support from the actual current rate. This also
prices modeled policy and local Governor-dependent effects. The model-derived
difference is an estimate of immediate departure effects, not a native
counterfactual measurement or forecast of future pressure. No trial action or
metadata mutation reaches the actual game.

Shipped primary rule:
`DLC/Expansion1/Data/Expansion1_Governors.xml:72` gives Magnus
`IdentityPressure="8"`. Native readback at 79/0 already reflects his return
before establishment, so the guard must account for assigned Governors rather
than waiting for their economic promotions to establish.

## Qualification scope

Two original-binary controls reproduce all 225 full native replies and exact
actual read sequences through 90/2, with successful EOF exits. The raw prefix,
signed decision records, binary/argv/cwd/genome context and sealed capture
archives are preserved under
`civvis-tactics-results/2026-11-04/loyalty-donor-audit/`.

The policy-window correction (#4430) changes earlier metadata and policy
orders, independently of this new treatment. Its frozen local executable is
therefore separately queried twice on this corpus as the reference for the
candidate with this gene OFF. The second reference checks complete full
payloads from the first; both finish all 225 requests with actual exit zero.
A new gene comparison must use the same corrected executable and background
for OFF and ON, and identify its first difference separately from the earlier
policy correction.

The Campus screen remains unchanged. There is no new live arming, default
promotion, saved-capital claim or win-rate claim from this work.
