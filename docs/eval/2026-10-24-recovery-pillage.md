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
The candidate binary SHA-256 is
`c8a3dcfa474062db47df3076a6ea4c2566ebf87767ebe432bb26e85a603b5a9c`,
built from `cdd215b97d2a8a152abf14a5cd7def935c50972a`.

The complete `cargo test --profile ci --locked` run exits zero: 6,244 tests
pass and 56 are ignored. The focused Rust-quality gate also exits zero:
`python3 tools/rust_quality.py --base ce31e0668f6ea0ad8a2e3e612d0fc4ae7069edc1 --head cdd215b97d2a8a152abf14a5cd7def935c50972a`.
Merging the subsequent setup-only `origin/main` update changes none of the
Rust, rules, deployment, Cargo, or build-script inputs tested here.

## Complete fixed-map game results

All 16 preregistered executions finish. Each row below is one same-seed,
same-arm build comparison; both builds have the listed outcome.

| Difficulty | Seed | Safest-stand arm | Winner seat | Victory | Turn |
|---|---:|---|---:|---|---:|
| Immortal | 58102400 | off | 2 | science | 149 |
| Immortal | 58102400 | on | 1 | science | 152 |
| Immortal | 58102401 | off | 1 | science | 153 |
| Immortal | 58102401 | on | 3 | science | 147 |
| Deity | 58102500 | off | 3 | science | 132 |
| Deity | 58102500 | on | 3 | science | 123 |
| Deity | 58102501 | off | 1 | culture | 122 |
| Deity | 58102501 | on | 1 | science | 126 |

- Focal wins: baseline `0/8`, candidate `0/8`; focal Domination wins:
  baseline `0/8`, candidate `0/8`. Neither build observes a foreign city held
  by the focal seat in any arm.
- All eight same-arm comparisons have identical complete reported outcomes.
  Seven complete focal journals are byte-identical; the remaining Deity
  `58102500/off` journal only exchanges the order of two turn-111 Detail
  messages about refused moves. The complete journals are retained, not
  replaced by the assertion that they match.
- The candidate logs **zero focal recovery pillages** in these games. Thus
  these probes do not demonstrate activation or stronger play. They are four
  maps and eight build comparisons, not sixteen independent maps.
- The paired evaluator's existing `safest-stand` off/on legs differ in actions
  on all four maps in each build. That is not evidence of a contrast caused
  by this fix. The recovery implementation changes every active planner in
  the candidate binary, so a whole-build comparison is not an isolated
  focal-strength treatment either.

`2026-10-24-recovery-pillage.results.json` records every comparison, source
revision, binary hash, journal hash, and activation count. The deterministic
gzip `2026-10-24-recovery-pillage.probes.json.gz` retains every complete raw
probe row and all complete focal journals, deduplicated by SHA-256.

This ships as a reproduced recovery correctness fix inside the existing
planner family, not a new gene selection or a claim of higher Domination
win rate. Highest-difficulty native Domination victories remain unproven;
the broader optimization goal is not complete.
