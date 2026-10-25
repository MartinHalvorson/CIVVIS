# Reuse derived city data in the airfield-slot scan

## Profile observation

A one-second sample of the already-running registered handover screen
(PID 33195, binary `886617e4704f310765b6143797a1c6e7d437feb3a38e1c23ae825d269180f540`)
contains 787 samples on its serial game thread. Of these, 314 are inside
`surge_field_slot_refusals`' district filter and 199 reach
`air_surge_reserves_field_slot`; repeated city-yield and amenity derivation
appears below that call. This is a short, selected profile and does not
measure whole-game cost or a speed improvement. The ongoing screen keeps
its frozen binary and original registration; do not restart it.

The scan enumerates district candidates on one immutable `&Game` board.
Each candidate can rank the same city yields again. The engine's existing
`Game::query_memo` is designed for this scope: nested guards share answers,
and the outermost guard discards them before the board can change. A new
scope after the existing disabled/unfuelled early exit should avoid those
repeated derivations without retaining data across production changes.

## Validation contract

Preserve the existing early exit, candidate order, reservation predicates
and returned refusals. The existing Bomber-wing and field-slot tests cover
the behavior. Run those tests, required full Rust validation and changed-file
quality. Check immutable-scan output equivalence against the parent on
identical representative late-game inputs before interpreting timing.

Measure baseline and candidate built with matching compiler/profile settings,
on fixed inputs with the airfield reservation actually active. Retain source,
binary, input and output digests and interleave the timings. A standard CI
cost check whose gene is off does not measure the activated scan. Report a
changed output as a behavior change rather than a pure optimization. Do not
infer a game-wide gain from the sampled fraction or from a small hot-loop
measurement. Functional validation and timing are pending.
