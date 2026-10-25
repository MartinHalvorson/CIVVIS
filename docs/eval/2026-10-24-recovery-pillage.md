# Safe healing pillages before battle-line recovery

## Scope and evidence

The active goal remains high-difficulty Domination wins, not a passing unit
suite. Recent native Emperor games still lose with substantial military
attrition; those outcomes do not establish a cause for individual losses.
This task isolates a recovery opportunity rather than claiming to solve them.

Native rules evidence: `Base/Assets/Gameplay/Data/Improvements.xml:38` defines
`IMPROVEMENT_FARM` with `PlunderType="PLUNDER_HEAL" PlunderAmount="50"`.
The current engine already applies that heal through ordinary `Action::Pillage`.
The battle planner currently reserves wounded units for recovery before the
per-unit action ladder can use it.

## Preregistered behavior

- Use only an already-occupied, legally pillageable healing improvement.
- Require an existing recovery need and no predicted incoming damage at the
  occupied tile. Do not divert units toward farms or trade an escape for loot.
- Preserve strikers, garrisons, exhausted units, reserved siege/civilian escorts,
  formations, and the no-healing arena configuration.
- Apply the ordinary engine action, then reserve the remaining turn. Do not
  change healing rules, game difficulty, opponents, the live genome, or pins.

## Validation plan, recorded before implementation

1. Run the new healing tests against unchanged production code. The two
   positive tests must fail on their heal assertions, not fixture setup.
2. Run the same tests after implementation, plus the complete battle-planner
   suite, formatting checks, and the repository CI-profile Rust suite.
3. Compare unchanged and patched simulator binaries on fixed same-seed
   Domination probes: Immortal seeds `58102400` and `58102401`; Deity seeds
   `58102500` and `58102501`. Keep both existing `safest-stand` legs in each
   build, for 16 complete game executions total, rather than selecting a
   favorable arm after observing results. The profile is the evaluator's
   four-major Gran Colombia seat, adaptive rivals, Tiny Pangaea, six minors,
   Online, all victories, natural 250-turn clock, and ordinary difficulty
   bonuses with seat zero exempt.
   Enable the evaluator's read-only `CIVVIS_PAIR_EXPLAIN` journal for every
   execution, preserving full-turn traces and counting the new recovery
   messages rather than inferring activation from an outcome difference.
4. Report all outcomes and null contrasts. These development probes are not
   native verification, a fresh holdout, or proof of higher Domination win rate.
5. Commit, push, run `ship`, and retain the broader goal unless actual
   highest-difficulty native Domination evidence proves it complete.

## Results

The unchanged-production run of
`cargo test --profile ci --locked --lib healing_tests -- --nocapture`
completed with eight passing tests and the two intended positive failures:
the wounded unit remained at 40 instead of 90 HP, and the recovering unit
remained at 75 instead of 100 HP. Both fixtures reached their heal assertions;
neither failure was a compilation error or a fixture-precondition failure.
Production source was the claim commit
`2ce92f788d797ae11ec5851709fe3a112b202e4b`, with battle-planner blob
`5937347eef95fe6e1f9cb4a7b52813eee93fcc02`.

The simulator binary requires `--features developer-tools`; an initial build
without that feature was rejected before compiling. The baseline build uses
`cargo build --profile ci --locked --features developer-tools --bin victory_eval`
before any production-code edit.

The implementation adds one current-tile check to `rotate_wounded`, applies
the existing legal pillage action, and claims the unit and any linked support
for the rest of the turn. It does not search for a farm or clone the world.
The existing battle-planner gene family remains the activation boundary; no
new gene or deployment selection is introduced.

`cargo test --profile ci --locked --lib ai::advanced::battle_planner` passes
all 85 tests, including all 13 healing tests. The nine added tests cover the
two reproduced positive cases, non-healing improvements, ownership/peace/
already-pillaged legality, healthy/exhausted units, hostile fire, a linked
support formation, an already-planned striker, and arenas without healing.
Focused rustfmt and `git diff --check` also pass.

The unchanged simulator binary SHA-256 is
`807114f554efb68e0ed1fea1b784dadbf422f843e8b44bbc48affddcce8c8461`.
Full-suite and game results remain pending. No higher win-rate or native
Domination-victory claim is made by the regression tests.
