# First native Spaceport after a discarded queue revision

The captured Deity Korea Science request at turn 151, frame 0 has no owned, queued, or placed Spaceport. The active `first-pad-in-the-forge` policy requires a producer within 80% of the best placing producer. Daegu has 42 Production and native Spaceport offers quoting 22 turns; Seoul has 16 Production and offers quoting 56 turns.

The successful simulated action log first places a Spaceport in Daegu, then switches Daegu to district repair. `Game::do_produce` immediately creates a simulated foundation before setting the queue. The final native production filter retains only the repair in Daegu and emits a Spaceport placement in Seoul. The next exported state shows only Seoul's new pad. A discarded simulated commitment has therefore released the producer floor without a corresponding native placement.

The repair snapshots the existing producer policy from the authoritative observed board before speculative planning. Final native Spaceport placement orders must satisfy that snapshot before acquiring a same-turn production lease. A held order restores the prior ownership cache. Existing repairs, defense orders, queue coalescing, genuine observed commitments and the disabled treatment keep their existing behavior. No new treatment or default change is introduced.

## Captured witness

Archive SHA-256: `2fde249bfdb8a74b1c259c1477a3a4aa6a08626a1026b3d43841140e62252f0a`.

The neighboring `.native.tar.xz` contains the original canonical decision, observed process/genome, all actual captured source bytes and index, the frozen input, and the next native state. Extract it and run `python3 verify-witness.py`. It verifies the canonical checksum, all 18 identical recorded reads, and the emitted repair/slow-pad pair. The recorded binary is source `5f48fb96fa5e06e44c6d6f6d47dedfc8a08448c0`; the existing producer-policy source is unchanged in this branch. This is historical evidence of the violation, not a native counterfactual win or a measurement of the repair's strength.

## Independent corroborating world

The `.corroborating.native.tar.xz` archive has SHA-256 `4653d522cf16c5f19aa07c1a7f06b612755a45a9be34efcdc2090ef21f57680a`. Extract it and run `python3 verify.py`. It verifies a second ordinary native game, `civvis-20261105T072215Z`, at 138/0: a planned capital Spaceport is superseded by a Coal Power Plant, while Pihtokahanapiwiyin places the first actual pad. The native quotes are 19 turns at 49 Production in the capital versus 47 turns at 18 Production in the chosen city. All 18 captured reads match, and the next state confirms only the weaker city's foundation. This is another historical violation, not a candidate counterfactual or strength estimate.

## Candidate comparison and retained failed gate

The `.candidate.native.tar.xz` archive has SHA-256 `9dcdcf750007ccde81b33cfdfdc77bc2ccc34e785a60609d0c5d20bd082ef86d`. Extract it and run `python3 verify-continuation.py .`. It contains all raw outputs, captured reads, the fixed protocol, the initial failure and the subsequent analysis protocol. The executable is separately available from hosted run `41994358659`: source `624d0eeafe4a27e43a27ab7f828e3029dd751722`, Apple Silicon binary SHA-256 `d9546d69ed4aeec45229837b22b508690a6d357fe15d0c50d7d0e3ea4259f8a6`.

The original comparison stopped after OFF1 because it required the historical Daegu repair order. The fresh planner instead restored Daegu's Spaceport after proposing its repair, and emitted both Daegu and Seoul pads. The historical-repair qualification remains failed. Its divergence from the original controller's private history is not attributed to this repair.

Only the three remaining predeclared arms were then executed, once each; OFF1 was retained without re-query. This secondary analysis uses the same single frozen input and candidate. In both repetitions, all 17 actual reads per arm equal the original public prefix, actual parsed genomes differ only by `first-pad-in-the-forge`, and repeated full replies are stable. The planned native action sequences match between OFF and ON. ON's emitted orders equal OFF's with only Seoul's slow pad removed; Daegu's fast pad remains, and `observed_first_pad_held=1` confirms the new guard fired.

This verifies the native producer-floor mechanism in a fresh private planner. It does not reproduce the historical emitted repair, execute either proposal in Firaxis, or establish survival or victory strength. Emitted repair retention and lease ownership are covered by the focused regression tests.

## Validation

At source `624d0eeaf`, all eight PR checks pass: 6,952 Rust tests pass, 53 skip, and 124 tool regressions pass. All six new regressions pass. Hosted Apple Silicon validation on Rust 1.97.1 passes both library contract tests, four native emission/ownership tests, six same-turn Walls handoff tests, and both executable smokes. The full tracked source manifest and both binary checksums were verified against the clean pinned checkout before querying the candidate.

The latest `origin/main` was merged once after those checks completed. The first-pad policy, native bridge source and new test modules are byte-identical to the validated candidate; integrated CI is tracked in PR #4514. Scoped rustfmt and `git diff --check` pass. The engine rules and simulator producer policy are unchanged by this feature.
