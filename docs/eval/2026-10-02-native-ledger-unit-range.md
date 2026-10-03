# Read native range before diagnosing missed shots

The tactical ledger's firing envelope inferred two hexes for every unit with
positive ranged strength, including Slingers, Skirmishers, and Quadriremes.
Current native exports already carry the unit's actual `range`. In the shipped
game, `Base/Assets/UI/Panels/UnitPanel.lua:2250` reads
`kSubjectData.Range = unit:GetRange();`; the control agent exports this same
reading for the seat's units.

`_attack_range` now uses a nonnegative integer native reading. Zero still means
one-hex melee reach. Missing, null, negative, boolean, string, or fractional
readings retain the previous estimate, preserving older recordings. A native
range above two also wins, including promoted siege or aircraft. This changes
only the report, not the AI's unit catalogue, actions, native controller,
forced policies, or live-game state.

## Recorded-history readback

The before/after reports use the same complete native event streams and orders
databases, opened by the ledger's existing read-only path. No games were rerun.

| King game | Firing unit-turns / in-range unit-turns, before → after | Idle units with at least 50 HP, before → after |
| --- | --- | --- |
| `civvis-20261001T050754Z`, 172 turns | 166/335 → 161/322 | 133/335 → 125/322 |
| `civvis-20261001T024402Z`, 168 turns | 68/193 → 68/188 | 96/193 → 91/188 |

The first recording removes 13 Skirmisher unit-turns whose nearest hostile
was two hexes away despite native range one. The second removes four such
Skirmisher unit-turns and one Slinger unit-turn. No new in-range unit-turns
are added in these recordings. Five excluded Skirmisher unit-turns did issue
a strike later that turn: the numerator is conditioned on the frame-zero
position, so this is not evidence that those actual strikes were illegal.

All other report sections and engagement statistics match exactly. The
remaining idle counts are still not a missed-shot count: proximity alone
does not establish line of sight, host legality, remaining attacks, or safe
combat. "Healthy" here retains the ledger's existing threshold of at least
50 HP; it does not mean full health. The helper's definition now names these
limits explicitly. The AI's static Skirmisher catalogue already has range
one; this report correction does not demonstrate an AI range error or a
game-strength gain.

Source event SHA-256 values are
`99bb98e4535e44911cd061b160bf814dc1f9d0a33fdb86dad3049956361f47a7`
for `050754Z` and
`9cf4452826704620e881c856516feea25cb1b8e32058096f8c723f6e91edb3e5`
for `024402Z`, also retained by the preceding wounded-route replay.

## Validation

Three of five new test methods fail on the old helper (five failed assertions,
including three short-range unit subtests). They cover short native range,
extended native range, and the engagement numerator/denominator. The two
compatibility methods already pass: native melee zero and invalid/legacy
readings. After the change, all 54 discovered `test_civ6_tactics_ledger*.py`
tests pass, as do all seven CI-wiring tests and Python 3.9 compilation of the
two edited Python files. Report comparison asserts that only the firing
envelope ratios change. The full `cargo test --profile ci --locked --jobs 3`
suite also passes: 4,454 tests, zero failures, 53 existing ignores. No simulator
soak applies to a reporting-only change.

The broader discovered tooling suite runs 3,108 tests with one skip and one
unrelated failure: `test_the_live_roadmap_passes_its_own_check`. The current
200-commit history puts the roadmap's `src/ai/advanced/tests.rs` hotspot below
the guard's five-percent floor. Running `tools/conflict_hotspots.py --check`
from unchanged integrated main reproduces the same failure. The reporting
patch does not change that roadmap or bypass its guard; this is not an
all-green local tooling run.

Evidence root:
`~/civvis-tactics-results/2026-10-02/native-ledger-unit-range-pr3871/`,
including the failing-first log and complete before/after JSON and text reports.
