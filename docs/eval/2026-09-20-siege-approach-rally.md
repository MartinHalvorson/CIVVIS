# Stage a siege from the army's approach

Native King domination game `civvis-20260920T092905Z` targeted the Egyptian city
ỉwnw at native (16,29). At turn 96 its six-unit siege force was mustering at
(19,29), beyond the city from the army approaching from the west/northwest,
with zero readiness. Several units remained embarked west of the target. This
is evidence of poor staging, not proof that staging alone caused the campaign
to fail.

The objective board used the same `far_side` selector for Siege, Defend, and
Relieve. It ranks distance away from the nearby hostile centroid ahead of
distance to the force. An enemy on the approach side therefore sends a siege
force beyond the city it has yet to take.

For a Siege, the selector now restricts its two-to-three-tile rally ring to
passable land closer to the force than the objective, when such land exists.
It then applies the existing safety ranking within that set. Defensive rallies
keep their existing behavior. When terrain leaves no approach-side land, the
existing available-ring fallback remains; this is not a pathfinding guarantee.

The integration regression gives a siege force a visible defender on its
approach. Before the fix its rally lies three tiles beyond the city. Afterward
the force and its projected Muster group share an approach-side rally. Two
additional tests preserve defensive refuge selection and a coast where only
far-side land is available. The first fixture initially failed to expose the
defender; correcting its visibility reproduced the original defect before any
production changes.

All 19 objective-board tests pass. Before merging newer main changes, the full
`cargo test --profile ci --locked` suite passes with 3,674 library and 204 binary
tests; 49 library and four documentation tests are ignored.

The frozen replay uses the native events through turn 112 and identical
domination/Gran Colombia verification arguments for baseline and patched builds
on base `bd61b7195`. Local evidence is under `/tmp/civvis-siege-rally-replay/`.
Frozen observations cannot execute revised moves or establish a city capture
or victory. No engine mechanics change; an engine soak does not apply.
