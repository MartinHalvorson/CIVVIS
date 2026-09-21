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
observation of what Firaxis granted. The planning pass records the resulting
breakthrough in persistent air-surge memory. Subsequent observed frames lack
the technology, but the old planner retains that deadline and times it out.

The diagnostic replay finishes all 423 frames and reproduces the turn-118
stand-down. Diagnostic source and binaries are local scratch artifacts;
they are not installed into the running verifier.

## Intended correction

Clear the remembered breakthrough turn whenever the current board lacks
Advanced Flight. A later confirmed breakthrough then starts its own full
Aluminum grace period. Keep the existing timeout for genuinely researched
technology without Bombers or Aluminum. This corrects observation-dependent
plan memory without changing the random reward mechanic or granting resources.

## Validation

Regression validation and matched replay are in progress. Frozen orders do
not establish executed builds, captures, or a native Domination victory.
