# Preserve a replacement Builder's opening reservation

The production goal remains active. This candidate is under validation and has
no measured early-production gain or Firaxis verification yet.

The frozen public targeted Domination player with `enable_live_bridge()` and
stock adaptive rivals completed consumed Deity diagnostics 61007901 and
61007903. Both reproduce every action and final-state byte of the #3932
candidate. The full journal windows have contiguous saved IDs and report no
ring eviction or turn truncation. Source, binary, dependency and raw hashes are
in the adjacent manifest and diagnostic-results files. Raw files and the
executable are preserved outside the worktree under
`civvis-production-evidence/2026-10-04/opening`.

## Observed failure

On 61007901 the opening Builder completes a Mine at turn 14 and Farms at turns
15 and 18, spending all three charges. There are no Builders at the turn 25,
50 or 75 checkpoints. This is a replacement-workforce problem, not evidence
that the opening never builds one or that the opening Builder is captured.

At turn 22 the higher-level pass accepts `Produce Builder` for Quito, city
176, after finding three eligible local improvement jobs. The normal scorer
then accepts `Produce Scout` for that same city in the same frame. Both orders
are in the actual action log, not merely a planning projection. The journal
prices the Scout at 19 against Builder 15. At turn 50 Quito has population 4,
production 3.6 and two worked, unlocked productive improvement opportunities,
but the reserved Builder never completes. Initial-round views precede troop
movement and combat; opportunity counts do not establish safe Builder routes.

## Candidate mechanism

Record only an accepted higher-level Builder reservation and its starting
turn/city. Preserve its legal queue through ordinary reviews and replanning
on that turn, before production can be invested. A receipt does not protect
another item, an illegal continuation or an uninvested queue on a later turn.
Existing invested-work commitments then control continuation. Siege handling
still runs before this protection, and the existing insolvency preemption
keeps its priority. Builder admission, workforce quotas, travel scoring and
civilian safety are unchanged.

Before editing, inspect all 23 open PR file lists and all five patches touching
`advanced.rs`; none rewrites the reservation field/default or commitment gate.
The claimed child and new test file have no open competing patches.

## Validation status

The standalone diagnostic probe compiles successfully and both consumed games
exit zero with exact reference equality. The initial test fixture used an off-map rival-city coordinate, so CI failed
before exercising the reservation. After correcting that fixture, all four
focused regressions pass in a fast opt-level-zero build, and incremental Rust
quality passes. The normal optimized full suite and prospective paired
production evaluation remain outstanding. No validation or strength claim is inferred from an
accepted reservation alone.
