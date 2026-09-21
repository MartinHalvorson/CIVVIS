# Preserve original capitals when the native Palace moves

The native city export used `IsCapital()` alone. The mirror wrote that current
Palace flag into `City::is_capital`, which the engine and Domination planner use
for original capitals. After a capture, this erased the original objective and
promoted the replacement capital. A stable captured original capital could no
longer complete its front; the planner kept pursuing the former owner's towns.
The same flag protects original capitals against a Raze decision.

## Primary contract and observed trigger

The installed `Base/Assets/UI/PartialScreens/WorldRankings.lua:1842–1848` reads
`city:GetOriginalOwner()` and counts a major's `city:IsOriginalCapital()` toward
Domination. Current capital status is a different fact.

The native trace `civvis-20260920T132244Z-cont1` illustrates the distinction:

| Turn | London (30,19) | Plymouth (28,13) |
| --- | --- | --- |
| 180 | England; current capital | England; not current capital |
| 187 | Georgia; not current capital | England; current capital |
| 189 and 197 | England again; not current capital | England; current capital |

London was first observed as England's capital at turn 66 in the base run.
Its original-capital status is an inference from that older trace, not a field
that trace exported. This change obtains the explicit native fact rather than
inferring it from names or possession history.

## Change

Own, visible rival, and visible minor city exports now include optional
`original_capital` and founder identity. The bridge uses the original flag and
mapped founder for the engine's original-capital facts. The independently observed
flag remains authoritative even if the founder cannot be mapped (an original
capital still cannot be razed); no replacement founder is guessed. Unknown
founders retain the existing degraded association rather than adding a new seat.

The current Palace has a separate observation map. Palace yields, loyalty
pressure, home-continent lookup, capital-adjacent placement, spy home placement,
pantheon unit placement, and declaration location retain the current capital's
location. Without observations, each retains its previous simulated behavior.
Ownership transfers and liberations invalidate Palace observations for affected
seats so hypothetical captures can move the Palace. Other seats keep their facts.

Missing `original_capital` preserves the legacy snapshot interpretation. The
new Game map has a serialization default for old saves and is rebuilt on every
host metrics pass. No game runtime is modified mid-run.

## Validation

- `cargo test --profile ci --locked`: 3,775 library and 205 binary/integration
  tests passed (49 library and four documentation tests ignored).
- Eight new Rust regressions cover captured originals, replacement capitals,
  rival occupation/recapture, nonzero host seats, unknown founders, old saves
  and snapshots, hypothetical liberation, current Palace yield/loyalty parity,
  and the real AI's front handoff after a mirrored original-capital capture.
- The capital-focused run passed 61 tests before the final unknown-founder
  refinement; the full suite above includes the final refinement.
- All 53 control-mod `*_test.lua` files passed under `lupa.lua51.LuaRuntime`;
  the full controller compiled with Lua 5.1. The new test executes the actual
  own/rival/minor export expressions, including explicit false and API failure.
- `cargo fmt --all -- --check`, `git diff --check origin/main...`, and all 14
  treatment append-point tests passed.
- `target/ci/civvis soak --players 4 --games 8 --start-seed 364000 --turns 180 --jobs 4`:
  8/8 completed. This checks simulation stability, not native victory.

No native capital capture or Domination victory is claimed from fixtures or
simulation. New native observations are needed to validate the full campaign.
