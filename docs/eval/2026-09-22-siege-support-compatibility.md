# Siege support compatibility

The siege formation previously inferred wall bypass from the presence of any
adjacent Siege Tower. This overestimated capture damage for cavalry and against
Renaissance walls, Tsikhe, and Steel defenses. It also allowed ineffective
support to authorize a melee attack into healthy walls. The support mover could
choose cavalry as its escort, and production counted an obsolete Ram against
the sole support slot while rejecting a replacement Tower as a duplicate.

Combat and AI now share the defense-immunity query. Capture estimates and melee
permission use the combat query with the attacking unit's promotion class.
Equipment selection uses the same building effects, including replacements,
and the Urban Defenses technology effect. The mover follows melee or
anti-cavalry infantry toward a compatible city whose walls still stand.
Production requires eligible fielded or queued infantry; obsolete breach
support no longer occupies the composition slot needed by effective equipment.
Existing or queued effective support still prevents redundant production.
A city's own queued support is excluded from the duplicate census when its
commitment is rescored, matching the governor's queue-excluded army counts.

The combat restrictions themselves are unchanged. Shipped Gathering Storm
`DLC/Expansion2/Data/Expansion2_UnitAbilities.xml:366-381` assigns
`ENABLE_WALL_ATTACK_MELEE`, `ENABLE_WALL_ATTACK_ANTI_CAVALRY`,
`BYPASS_WALLS_MELEE`, and `BYPASS_WALLS_ANTI_CAVALRY` to their corresponding
promotion classes. The existing engine immunity effects remain authoritative.

## Validation

- `cargo test --profile ci --locked`: 4,173 passed, zero failed, 53 ignored.
  This includes all seven new support regressions, existing siege production
  and formation tests, and the engine's support-era combat tests.
- Capture projections are checked against executed attacks for melee,
  anti-cavalry, cavalry, Renaissance walls, Tsikhe, and Urban Defenses.
  Separate formation tests verify both permitted breaches and refused attacks.
- Production tests execute the real support reservation, confirm a Tower
  replaces an obsolete Ram once, and rescore the queued commitment. Movement
  tests isolate eligible escorts on a safe staging ring.
- `cargo run --profile ci --locked --bin civvis -- soak --games 8 --players 4
  --turns 180 --start-seed 371700 --jobs 4`: all eight games completed.
- `python3 tools/rust_quality.py --base 453093af8 --head HEAD`: changed lines
  are formatted and warning-free. `git diff --check origin/main...` passes.

The initial fixtures required correction: Tower production needed its actual
Machinery unlock; the older basic production fixture declared a wall building
without wall HP; and the escort-choice regression initially mixed escort
selection with movement safety under city fire. The corrected cases exercise
live walls and legal production, and leave the movement-risk policy active.

These are correctness fixes to production and combat decisions. The smoke
games check stability; no native win-rate gain is claimed.
