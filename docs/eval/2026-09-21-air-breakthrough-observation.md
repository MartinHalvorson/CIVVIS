# Confirm air breakthroughs before retaining their deadline

Native run `civvis-20260921T075909Z` appointed an air surge at turn 96,
abandoned it at turn 118 for “no Aluminum for the wing”, and appointed
another at turn 128. The host did not own Advanced Flight until turn 153.
A frozen replay on `fc0a59fcb` reproduces the premature stand-down across
423 decision frames. This source run ended without a recorded outcome.

A diagnostic copy of the order binary compares the observed board with the
throwaway board after planning. At turn 104, the observed board lacks
Advanced Flight while the planning board owns it. That frame's actions
include the first Mahavihara improvement. Its simulated random technology
reward is implemented in `src/game/actions.rs`; the reward is not an
observation of what Firaxis granted. The host confirms the Mahavihara at
(18, 25) on turn 104 and reports Square Rigging newly owned at frame 104/1,
not Advanced Flight. The planning pass records the resulting
breakthrough in persistent air-surge memory. Subsequent observed frames lack
the technology, but the old planner retains that deadline and times it out.

The diagnostic replay finishes all 423 frames and reproduces the turn-118
stand-down. Diagnostic source and binaries are local scratch artifacts;
they are not installed into the running verifier.

## Correction

Clear the remembered breakthrough turn whenever the current board lacks
Advanced Flight. A later confirmed breakthrough then starts its own full
Aluminum grace period. Keep the existing timeout for genuinely researched
technology without Bombers or Aluminum. This corrects observation-dependent
plan memory without changing the random reward mechanic or granting resources.

## Validation

Two regression tests fail on the old planner: an unconfirmed breakthrough
expires research, and a later real breakthrough inherits the expired clock.
The genuine-shortage control already passes. With the correction, all 25
air-surge tests pass. After merging `125af83bf`, the full Rust suite passes:
3,789 library tests and 206 other tests, with 49 library and four doc tests
ignored. Formatting, whitespace, and all 14 treatment append-point checks
pass. Eight simulator stability games finish (four players, seeds
364800–364807, 180 turns, four workers).

Both frozen replay variants include the Temple fix from #3645 and the Great
Person loop fix from #3644. Both finish all 423 decision frames: baseline
130.95 seconds, candidate 102.62 seconds. These single timings are not a
paired performance result. The candidate retains the turn-96 appointment
through turn 118, eliminating the premature Aluminum stand-down and the
turn-128 reappointment. Both still stand down at turn 147 because urgent
victory denial supersedes the surge.

Five decision frames change actionable orders: at turns 118, 119, 120, 123,
and 126, research stays on Steam Power toward the appointed air breakthrough
instead of detouring through Military Engineering, Castles, Gunpowder, Metal
Casting, or Economics. Five internal action frames differ; eight exported
frames differ when synthetic receipt checks are included. The candidate's
receipt failures reflect comparison against the unchanged historical host
stream, which executed the old research choices. They are not newly executed
native failures. Frozen orders do not establish an earlier actual technology,
executed builds, captures, or a native Domination victory.
