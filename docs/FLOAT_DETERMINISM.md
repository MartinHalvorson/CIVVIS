# Float determinism across builds

The same seed must generate the same world in every build of this engine —
the desktop binary, the Linux CI runner, and the `wasm32-unknown-unknown`
module the site serves. This page records why that was ever false, what
enforces it now, and the rule new code has to follow.

## What went wrong (#1061)

A globe game on the site was not the game the desktop build played from the
same seed. Flat maps were bit-identical; globe maps were not — 4 of the first
8 seeds tried generated a different map before a turn was played.

The cause was never a structural bug. IEEE-754 pins `sqrt` and the arithmetic
operators to correct rounding, but only *recommends* it for the
transcendentals — `sin`, `cos`, `asin`, `acos`, `atan`, `atan2`. Rust's `f64`
methods call whatever math library the platform links: Apple's on macOS,
glibc's on Linux, the UCRT's on Windows, and Rust's own `libm` on wasm32.
Those implementations legitimately disagree by an ULP on some inputs.

The globe's latitude is `asin(z)` of a cell centre that is itself built from
`sin`/`cos` ([`src/sphere.rs`]), and mapgen feeds that latitude straight into
the climate bands. One ULP of latitude moves a tile across a
`grassland`/`plains` threshold, and everything downstream — features,
resources, continents, city-state ownership — reshuffles from that tile
outward. Flat maps never showed the problem because flat mapgen computes
coordinates by row-index arithmetic and calls no trig at all.

Whether a given seed diverged depended on whether some tile happened to sit
within a rounding step of a climate boundary. That is a property of the seed,
not of the build — which is why any single-seed check has roughly even odds
of concluding nothing is wrong.

## The rule

**Anything that turns a seed into game state calls [`crate::sphere::trig`],
never the `f64` transcendental methods.** That module routes through the
[`libm`](https://crates.io/crates/libm) crate — the same pure-Rust
implementations on every target, so every build computes the same bits by
construction. `sqrt` is exempt: the standard pins it, and wasm's `f64.sqrt`
instruction honours that.

As of the fix, the geometry path is fully converted: `src/sphere.rs` (cell
centres, latitude, longitude, neighbour winding), `src/mapgen.rs` (climate
sampling, the Earth scripts, the canal scripts, block frames) and
`src/world.rs` (`WorldMap::direction`).

Deliberately **not** converted, so their platform behaviour is a recorded
decision rather than an accident:

- `src/fractal.rs` — plate drift calls `sin`/`cos`/`exp` on the flat path
  too, and roughly forty wasm-versus-native pairs across six flat boards came
  out bit-identical, so the platform implementations demonstrably agree on
  the inputs it produces. Converting it would risk churning flat worlds for a
  divergence nobody has ever observed. If a flat board ever diverges, this is
  the first place to route through `trig`.
- `src/game.rs` line-of-sight slerp (`acos`/`sin` along a globe sight line) —
  game-state-affecting, but every globe pair whose generated maps matched
  also played identical full games, so these calls agree in practice. If the
  simulation loop ever reports "map identical, game diverged" on a globe,
  convert this next.
- AI evaluation `exp`/`ln` in `src/ai.rs` and `src/ai/advanced.rs` — same
  empirical status. This used to carry a second objection: converting those
  files re-pinned the `advanced_v1` byte-hash source contract. That hash is
  gone (#1841); the anchor is now pinned by its decision stream, so a
  conversion that changes no float result changes nothing to re-pin — and one
  that DOES change a result will fail `advanced_v1_plays_the_same_game_it_always_did`,
  which is the honest signal and was always the real question.

## What enforces it

`the_same_seed_generates_the_same_world_on_every_platform` in `src/mapgen.rs`
pins the digest of the world several fixed seeds generate — globe seeds that
demonstrably diverged before the fix, plus flat controls. The values were
computed once, on one platform; every platform that runs the suite must
reproduce them bit for bit, so a reintroduced platform-trig call fails CI on
the first runner that rounds differently. If mapgen changes deliberately, the
test's failure message prints the new digests to pin — from one platform
only, so the others keep verifying rather than being pasted over.

## What the fix cost

Making the trig explicit changed what native builds generate: **every globe
seed produces a different map than before the fix** (measured 6 of 6 in
#1061), because native moved off its platform's rounding. Globe seeds from
before the fix do not reproduce afterwards. Saves were never at risk — the
save format carries the generated map rather than regenerating it, and a
world written by one build loads bit-identically into another. Flat worlds
from the standard scripts are unchanged. The wasm bundle grew 0.09%; wasm
already linked these exact implementations, so its output only changed where
native's did — they now agree.
