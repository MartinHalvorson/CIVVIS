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

⚠ **`src/mapgen.rs` was not actually fully converted, and the exception was the
default script.** `tennis_ball_seam_point` and the seam-side test around it kept
calling `f64::sin`/`cos`/`asin`/`atan2` directly — ten call sites building a 3-D
point on the globe from a longitude, which is precisely the shape of the
original bug. Routing them through `trig` (2026-08-18) changed **2 of 6**
tennis-ball globe seeds on macOS, so the native build and the wasm bundle had
been generating different worlds from the same seed on the map most games are
played on. The paragraph above had said otherwise since the fix landed.

The gate could not have caught it: see below.

⚠ **It was still not fully converted after that, and it happened the same way
twice.** `tennis_ball_seam_point` was routed through `trig` on 2026-08-18;
`tennis_ball_seam_proximity`, the function immediately below it in the same
file, kept its `atan2` and went on reading a longitude off the platform's own
library. `src/city_states.rs` was never mentioned by this page at all, and its
`direction` — which decides which city-states a seed seats, and on a true-start
map where they stand — computed a unit vector with `f64::sin`/`cos`. Both were
converted on 2026-08-23 (#2383).

The lesson is not "look harder next time". A conversion is done per *call site*
and reviewed per *function*, so the call two lines below the one being fixed is
exactly what survives. That is a test now rather than a habit: see
`a_file_that_uses_trig_uses_nothing_else` below.

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

Three tests, and they answer three different questions.

**`the_same_seed_generates_the_same_world_on_every_platform`** (`src/mapgen.rs`)
pins the digest of the world a fixed table of seeds generates. Every platform
that runs the suite must reproduce those bits, so a reintroduced platform-trig
call fails CI on the first runner that rounds differently. If mapgen changes
deliberately, the failure message prints the digests the current code produces —
paste them in from **one** platform only, so the others keep verifying rather
than being pasted over.

**`every_rolled_map_script_has_a_pinned_world`** (`src/mapgen.rs`) is the gate on
that gate. The digest table used to pin `MapScript::Continents` and nothing else
while the CLI defaulted to `tennis_ball`, so the check that exists to keep
platform trig out of mapgen could not see the map most games are played on, and
ten such calls sat in it for a year (#1950). Adding a `tennis_ball` row fixed
that instance and left the class open — this page said so, in the form of a note
asking the next author to remember. When the requirement was finally made
mechanical (#2383), **ten of the twelve rolled scripts turned out to have no
pinned digest at all**: `land_only`, `lakes`, `inland_sea`, `grand_canals`,
`grand_canals_2`, `pangaea`, `small_continents`, `fjords`, `islands` and
`water_world`. A platform-trig call in any of them could not have failed a test.
All twelve are pinned now, on both shapes, and a new `MapScript` variant does not
compile until it is classified as rolling a world or laying out a fixed board —
and if it rolls one, it does not pass until it has a digest.

**`a_file_that_uses_trig_uses_nothing_else`** (`src/sphere.rs`) enforces the rule
structurally instead of by review: **a file that calls `trig::` anywhere must not
call a platform transcendental anywhere.** Opting in is all-or-nothing. It walks
`src/` rather than consulting a list, so a new module joins the check by
existing, and the recorded exclusions below are untouched because they never opt
in — the day one of them is converted it inherits the guarantee automatically.
This is the check that catches the failure mode the two ⚠ notes above describe
and neither digest gate could: a partial conversion changes nothing about what
native generates, so the pinned digests — computed on native — still match.

⚠ **None of the three can run on `wasm32-unknown-unknown`,** and that is
structural rather than an oversight: `cargo test` needs a host that can execute
the target, and the wasm module is executed by a browser or by Node. Every digest
on this page is therefore pinned by a *native* build and verified by *other
native* builds. The only continuous check that the shipped wasm bundle agrees is
the alternating build-parity loop (`tools/simloop/`), which builds both arms and
plays the same seed on each. `tools/simloop/mapcheck.mjs` is the same question
asked of the map alone, and can be run by hand against any module.

⚠ **That loop is a process on one Mac, and it can stop.** It did: it wedged on
2026-08-07 holding its own `.running` lock, and reported nothing for the next
sixteen days while `main` advanced past a thousand commits. The lock is
deliberately cleared by hand so a crash stays visible — which only works if
somebody looks. **A ledger row is evidence about the revision it names and
nothing else.** Before treating one as a live defect, check its `sha` against
`origin/main` and check that the loop has run since: in August 2026 a row from
iteration 27, recorded at `28438c9` *before* the #1061 fix, was still being read
as a current report of the bug that commit fixed.

## What the fix cost

Making the trig explicit changed what native builds generate: **every globe
seed produces a different map than before the fix** (measured 6 of 6 in
#1061), because native moved off its platform's rounding. The 2026-08-18
tennis-ball conversion cost the same thing again on that script alone: 2 of 6
globe seeds moved, and a pre-conversion tennis-ball globe seed does not
reproduce afterwards. Continents digests were unchanged, which is the evidence
that the conversion was confined to the seam geometry. Globe seeds from
before the fix do not reproduce afterwards. Saves were never at risk — the
save format carries the generated map rather than regenerating it, and a
world written by one build loads bit-identically into another. Flat worlds
from the standard scripts are unchanged. The wasm bundle grew 0.09%; wasm
already linked these exact implementations, so its output only changed where
native's did — they now agree.

**The 2026-08-23 completion (#2383) cost nothing, and that was measured rather
than assumed.** Converting `tennis_ball_seam_proximity` and
`city_states::direction` changed **0 of 305 seeds** on each of five
configurations — globe/continents, flat/continents, globe/tennis-ball,
true-start Earth, and the city-state seating draw itself: 1,525 generated
worlds, every digest identical to the native build before the change. All thirty
pinned digests, the ten pre-existing ones included, are byte-identical across the
change. **No globe seed stops reproducing, no save is affected, and no recorded
result is invalidated by that commit.**

That is the exception rather than the rule, and it is worth saying why: these two
sites fed a comparison with a wide margin — a seam cutoff, a farthest-point
argmax — rather than a threshold tiles sit an ULP away from, so Apple's rounding
and `libm`'s never disagreed by enough to change an answer. A conversion is
normally expected to move worlds: #1061 moved 6 of 6 globe seeds and #1950 moved
2 of 6. Measure before promising anything.
