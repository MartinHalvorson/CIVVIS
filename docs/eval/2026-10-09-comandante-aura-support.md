# Idle Comandante aura support

An unavailable retirement leaves the current Great Person driver holding its
position. That can leave a Comandante's passive combat bonus away from troops.
The new fallback moves an otherwise idle Comandante toward a friendly military
recipient when no eligible unit is already within two tiles. Activation and
movement toward a usable retirement plot retain their existing priority.

## Native evidence and its limits

The frozen `civvis-20261008T221617Z` trace used controller revision
`fb5ac25063ab293d25f8446f1ebc19fa9433318c`, binary SHA-256
`10f8dda1519989d8dc41a6b771048605c7ba920d72f4f61d57afa853ff41d1df`.
The current and frozen Rust Great Person selectors are byte-identical, with
function SHA-256 `2e88f488b99e461ae842a0f04ab04f358330e63cf8e9af1b7e3a8194764731ad`.

Páez, unit `3604503`, stood at native offset `(16,13)` from turns 77 through
153. Across 77 distinct turns without an activation highlight, 67 last-frame
observations included eligible troops within two tiles and ten did not. During
that interval, 36 recorded friendly combat-side observations were outside two
tiles of his preceding exported position. These observations motivate a
positioning fallback; they do not establish a safe alternative route, an
applied native bonus, or a changed combat outcome. Repeated exports and combat
events are not independent games or trials.

His retirement highlights first appeared on turn 154. Shipped
`DLC/GranColombia_Maya/Data/GranColombia_Maya_GreatPeople.xml:29` requires nearby
light or heavy cavalry for Páez's retirement. Waiting before then is not
evidence of ignored activation eligibility.

The read-only audit receipts are `comandante-proximity-inventory.json` and
`comandante-combat-proximity-inventory.json` under the retained native baseline
artifacts. Original event hashes remained unchanged. The live rules cache
refreshed externally after the first audit; the follow-up uses an owned SQLite
backup with SHA-256
`54db1ceac56442d385c9f2933c59cecb403467594a2da57f8d49078f118c86bb`.
Its quoted aura rows match the first audit. This is a snapshot of the current
rules profile, not an archived rules database from the original game.

## Shipped rule and movement authority

`GranColombia_Maya_GreatPeople.xml:41` attaches
`GREATPERSON_COMANDANTE_STRENGTH_AOE_LAND` to Páez at birth. The shipped modifier
grants `ABILITY_COMANDANTE_AOE_STRENGTH`; its `COMANDANTE_AOE_STRENGTH` argument
is `Amount=5`. `AOE_LAND_REQUIREMENTS` combines land domain with
`AOE_REQUIRES_OWNER_ADJACENCY`, whose arguments are `MinDistance=0` and
`MaxDistance=2`. The ability's `TypeTags` determine recipients. The driver
discovers those tags rather than maintaining a unit roster.

`Base/Assets/UI/WorldInput.lua:961` reads
`UnitManager.GetMoveToPathEx(kUnit, endPlotId)`. The fallback uses that native
path and the existing parameterized `CanStartOperation`/`RequestOperation`
wrapper. It requires a complete path from the observed origin to the requested
destination, with every path turn in the current-turn allowance (`0..1`). No
multi-turn path is queued.

## Admission and bounds

Only a Comandante with a remaining charge and movement is considered. Existing
aura coverage holds its position. A candidate recipient must be a discovered
eligible land military unit, on-map and not embarked. A civilian occupying its
tile, or another Comandante already covering it, excludes that destination.
A troop ordered toward another tile, or still carrying a queued order, cannot
serve as a destination: its observed position may already be stale.
Every path tile must have confirmed ownership by the player, be land and
passable, and be outside two tiles of the existing hostile-unit inventory.
Unreadable rules, unit fields, path, ownership, or movement allowance cause a
hold. These guards reduce exposure; they do not forecast every enemy attack.

Candidates are ordered by distance and stable unit ID. At most eight distinct
destinations reach the native pathfinder per attempt. Support is considered
once per Comandante per turn. An explicit controller unit order suppresses
support for that turn, including later frames. Existing Urdaneta retirement
value checks still reserve an unhelpful reset; a reserved General can now
position its aura without spending the charge.

The `gp` event's `aura_support` action records an accepted movement request and
its recipient. It is not a native movement-completion or combat-bonus witness.

## Validation

- Corrected final fixtures against unchanged controller source reproduce nine
  expected failures among 74 checks: ordinary Páez support, discovered eligible
  cavalry support, reserved Urdaneta support, stationary Fortify recipient support, and asynchronous request count.
  The preservation controls pass. Baseline source is retained from test-only
  checkpoint `f26e0cd0d`.
- The initial support candidate reproduced two additional stale-recipient
  failures: a troop received a move request, but the General followed its old
  position, or still carried a queued order. The corrected driver excludes
  those recipients and preserves the
  troop's explicit controller request.
- Candidate: all 74 checks pass under the installed Lua 5.1 runtime.
- All 84 discovered `tools/civ6_control/mod/*_test.lua` suites pass under Lua 5.1.
  The existing CI job discovers this new suite through the same glob.
- The CI type-name gate rejected two synthetic fixture names before running
  Lua. Final fixtures use shipped Horseman and Giant Death Robot names; tag
  removal still proves that eligibility is read from host rule data. The
  unchanged-source behavior failures above are local regression evidence.
- Final source `5294cbfc6` passes every CI check. Control-mod run
  `37879169778` confirms the 74 new checks; tooling run `37879169783` passes
  3,270 tests with 215 skipped. Rust compilation and cost measurement are
  explicitly skipped for this Lua/documentation-only task diff.
- The required full local Rust regression is recorded separately in the PR
  before ship. Its result is distinct from the mod tests and native outcomes.

No Rust game mechanic, mirror schema, native pin, policy, installed mod, or
verification lane was changed. Native adoption and completed matched outcomes
remain necessary to assess this candidate. No win-rate improvement is claimed.
