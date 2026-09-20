# Stage toward a city without visible defending units

Native King/Gran Colombia run `civvis-20260920T114721Z` reached turn 204
without capturing a city and lost to Ottoman diplomatic victory. It kept all
11 cities but made only four attacks on enemy city centers. At turn 193 its
Urfa siege force mustered at axial `(9,20)`, while Urfa was at `(27,12)`.
The force had enough nominal strength for the siege bill, but no staged units.
This is an observed staging failure, not proof that staging was the only
reason the campaign failed.

`far_side` returned the force medoid immediately whenever no hostile military
unit was observed near the objective. With `SiegeStage::Stage`, that medoid
became the muster anchor, so distant forces could gather far from the city
instead of reaching its siege ring. The absence of a visible field defender
does not remove the city, its walls, or the need to approach it.

Siege rallies now consider the existing passable-land ring two to three hexes
from the objective even without visible defenders. They prefer the army's
approach side when available, then proximity to the force center and stable
coordinate tie-breaking. With defenders, the existing safety ranking remains.
Defend/Relieve keep their no-contact fallback; an empty usable ring still
falls back to the force center. This is geometric staging, not a new path
reachability guarantee. No game rules, visibility, war authorization, or
combat safety rules change.

## Validation

The new staged-force regression failed before the production change: its
anchor stayed twelve hexes from the city. The defensive and unusable-ring
controls passed before and after. All 22 objective-board tests passed after
the fix, including existing defended-rally and approach-side coverage.

After merging main `fa7be98b4` (hidden-rival elimination),
`cargo test --profile ci --locked` passed 3,708 library and 205 binary tests;
49 library and four doc tests were ignored. `cargo fmt --check` and
`git diff --check` passed. No engine soak applies to this AI-only change.

## Frozen native replay

Baseline was clean claim `6f5a5eb6f4b2f8eb6de118f541cecce7ecf37109` on main
`d9ee52e8754e5204ccbfda74a6194b4cd7b92776`. The candidate isolates the staging
change on that same base; the later main merge was covered by the full suite.
Both binaries used 16 release codegen units, LTO disabled, and incremental
compilation. Both received the same 19 forced genes, Gran Colombia, and the
domination target.

The input freezes native observations through turn 200: 24,654 events,
64,201,318 bytes, SHA-256
`7252eac53c8b99cdb5815ac115370de5008210202aefe322143e0a7bab3024a4`.
Both runs completed all 585 decision frames with exit 0; 39 exported order
frames changed. The first change is turn 121: Archer 1900557 receives
`MOVE_TO (20,30)` instead of `(25,29)`, while the Ngaruawahia rally changes
from axial `(8,22)` to `(27,29)`, three hexes from its target `(27,32)`.
That individual movement is not claimed to prove path progress.

At turn 193 the Urfa land rally changes from `(9,20)` to `(24,14)`, three
hexes from Urfa. The sea force rally changes from `(46,23)` to `(30,12)`.
The exported batch gains a Destroyer 9306128 move from its observed native
`(16,25)` toward `(18,23)`, three hexes closer to Urfa at native `(33,12)`.
The baseline issues no movement order for that unit in this frame.

Baseline elapsed 234.43 seconds and candidate 278.69 seconds on a busy host
with concurrent native play and compilation; this is not a controlled speed
benchmark. Subsequent frozen observations still describe the original game
once the orders diverge. The replay establishes changed rally assignments and
exported movement, not executed arrivals, wall damage, capture, or victory.
Artifacts, binaries, logs, and comparison JSON remain in
`/tmp/civvis-unopposed-replay/`.
