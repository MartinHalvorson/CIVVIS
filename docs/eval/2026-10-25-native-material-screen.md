# Native material trial: input qualification

The eight-start trial registered in #4119 is still unarmed. Its intended
comparison remains the existing `army-upgrade-materials` buyer plus the new
stock reserve against that policy being off. All original inclusion,
continuation, outcome and interpretation rules remain in force.

## Why the recorder had not activated

The supported host policy contains `CIVVIS_CAPTURE_DECIDER_INPUTS=1`, but
the actual game driver for `civvis-20261025T051724Z` lacks the capture flag
and its run has no input archive. At the beginning of this continuation the
live supervisor is still the process launched from the older first-foothold
worktree. Its source has no input-capture selector. Updating the runtime
checkout does not replace this long-lived shell or its inherited environment.
That game does not qualify the recorder and receives no trial arm.

That completed game lost to rival Culture at turn 203, with eleven cities,
six cities at turn 60, three combat captures, four city losses, 46 kills and
26 unit losses. It used frozen source `8810924ee`, before the stock reserve.
It is development evidence and does not measure the repair.

## Boundary reload and actual qualification

The previously requested safe boundary reload completes after that game.
At 05:39:53 UTC the authoritative process snapshot shows supervisor 68618
running the managed source, and game driver 69075 for the new unarmed game
`civvis-20261025T053159Z` carries `--civvis-capture-inputs`. The companion
precondition archive preserves the complete actual process commands and
policy digest. It proves capture is enabled, rather than the earlier
inference that this second game also lacked it.

A prospective per-invocation policy fallback was implemented and tested in
this branch while the reload was unresolved. Once actual capture appeared,
that extra change was removed from the final diff. The registered reload
already supplies the required recorder, so no new launcher behavior is needed
for this trial. Its intermediate pushed commit remains in branch history.

The new game's first decision is turn 1, frame 0. It reports a complete
capture containing seventeen reads of snapshot zero. Reconstruction returns
28,858 bytes with SHA-256
`b365f5ae46cc8f1b4bc6ea55d97e29b2a028ed6f8c2e0dfac21cdf3120c36a55`.
Those bytes include the seat, native state and terrain export. The
qualification archive retains the complete descriptor, first decision,
status, runtime receipt and reconstructed read. This verifies captured input
structure and bytes, not host execution or replay equivalence.

The frozen binary is still source `8810924ee`, SHA-256
`20fe763757f68a6ccf024ad8d57ab7bf6b35acdadc56842761b802d36f0b531e`.
It predates the stock reserve. The repaired native binary must still be
verified after #4119 merges before arming the previously registered trial.
A failed or missing qualification never starts that cohort, and an assigned
game's later capture failure never removes or replaces it.

## Validation

The discarded fallback passed eleven focused policy/runner tests and all
220 launcher tests. Its broader developer-tool run executes 3,431 tests:
one fails because the existing roadmap names `src/ai/advanced/tests.rs`
as a conflict hotspot although its current measured touch rate is 3%, below
the 5% floor; one test is skipped. This unrelated failure is preserved and
is not a claim of full-suite success. No Rust or launcher changes remain in
this evidence-only final diff. There is no repair-strength result yet.
