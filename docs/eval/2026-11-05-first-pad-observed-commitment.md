# First native Spaceport after a discarded queue revision

The captured Deity Korea Science request at turn 151, frame 0 has no owned, queued, or placed Spaceport. The active `first-pad-in-the-forge` policy requires a producer within 80% of the best placing producer. Daegu has 42 Production and native Spaceport offers quoting 22 turns; Seoul has 16 Production and offers quoting 56 turns.

The successful simulated action log first places a Spaceport in Daegu, then switches Daegu to district repair. `Game::do_produce` immediately creates a simulated foundation before setting the queue. The final native production filter retains only the repair in Daegu and emits a Spaceport placement in Seoul. The next exported state shows only Seoul's new pad. A discarded simulated commitment has therefore released the producer floor without a corresponding native placement.

The repair snapshots the existing producer policy from the authoritative observed board before speculative planning. Final native Spaceport placement orders must satisfy that snapshot before acquiring a same-turn production lease. A held order restores the prior ownership cache. Existing repairs, defense orders, queue coalescing, genuine observed commitments and the disabled treatment keep their existing behavior. No new treatment or default change is introduced.

## Captured witness

Archive SHA-256: `2fde249bfdb8a74b1c259c1477a3a4aa6a08626a1026b3d43841140e62252f0a`.

The neighboring `.native.tar.xz` contains the original canonical decision, observed process/genome, all actual captured source bytes and index, the frozen input, and the next native state. Extract it and run `python3 verify-witness.py`. It verifies the canonical checksum, all 18 identical recorded reads, and the emitted repair/slow-pad pair. The recorded binary is source `5f48fb96fa5e06e44c6d6f6d47dedfc8a08448c0`; the existing producer-policy source is unchanged in this branch. This is historical evidence of the violation, not a native counterfactual win or a measurement of the repair's strength.

## Validation

Scoped rustfmt and `git diff --check` pass. Two library contract tests and four native emission/ownership regressions are added; Rust execution, focused Mac validation and a fixed-input candidate comparison remain pending. The engine rules and simulator producer policy are unchanged.
