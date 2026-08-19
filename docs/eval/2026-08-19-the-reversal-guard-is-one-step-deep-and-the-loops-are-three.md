# The reversal guard is one step deep and the loops are three

_2026-08-19 · `claude-opus5-loop`_

## What was asked

`self_tile_move` is named in `civvis_orders.rs` as the largest single waste
class on the Civilization VI ladder — 25,387 dropped orders across 43 live
runs. A unit told to walk to the tile it is already standing on does nothing
that turn, and on the bridge the order is not even sent. How much of our own
movement goes the same way, and where does it come from?

## How it was measured

Counted, not assumed, in three 6-player 200-turn Online games run
single-threaded.

The first metric was wrong and the shapes said so. Taking "spent movement
points and ended on the starting tile" as the definition of a round trip
counted **988 units that never walked a step** — Fortify zeroes movement too.
The engine already records the walked route (`Game::unit_move_trails`), so the
second cut read hops from that and dropped every unit with none. Two other
splits were built and discarded the same way: a recon/non-recon split that
reported **zero recon trips while `scout` was the top kind** (a broken
classifier), and an at-war split that returned 100% (everyone is permanently at
war with the barbarians). A census that reports zero is a broken census.

Attribution came from `#[track_caller]` on `Game::apply` rather than from
tagging twenty call sites by hand — two edits, and the return step names its own
source line.

## What it measured

Native `advanced` walks **2,015 round trips** in 29,390 walked unit-turns, and
three call sites produce every one of them: `ai.rs:10266` (1,111),
`ai.rs:10341` (710), `advanced.rs:12402` (194). The first is
`tactical_apply_move`'s raw exit, which neither checks the same-turn reversal
guard nor records into it — so a unit that steps out through the raw exit can
step back through the guarded one, because the guard has no record of the
outbound step. That exit is what `recorded_tactical_step` closes, and it is
deliberately off natively so recorded ladders replay move-for-move.

With it on — the live bundle — the two-hop shape collapses from 1,383 to 23.
The guard works. What it cannot see is everything longer, because
`last_path_step_from` remembers exactly one tile:

| arm | walked round trips | of walked unit-turns | movement wasted |
|---|---|---|---|
| `live_without_whole_turn_backtrack_guard` | 329 | 1.69% | 2.14% |
| `live` (guard on) | 51 | 0.28% | 0.24% |

Shapes before: `(3 hops, reach 1)` **191**, `(4,2)` 70, `(4,1)` 33, `(2,1)` 23.
Shapes after: `(2,1)` 40, `(4,1)` 8, `(4,2)` 3 — the three-hop class is gone.

⚠ The two-hop count rises, 23 → 40. Refusing the longer loops pushes a few
units into short ones through `advanced.rs:12402`, which bypasses the guard
entirely; that call site is untouched here and is the obvious next one.

## What was decided

Shipped in the live bundle as `whole_turn_backtrack_guard`, with
`live_without_whole_turn_backtrack_guard` registered so it can be priced. It also
joins the native repair bundle, because `engine_repairs_are_the_live_bridge_minus_the_firaxis_semantics`
holds every bridge repair to be a native repair unless it encodes a rule of
Firaxis' game, and a loop back to the start is a wasted turn in either engine.
Off for native tournament games and off for the frozen anchor, so no re-pin: the
storage widened from "the tile just left" to "every tile stepped from this
turn", and with the flag off only `trail.last()` is read, which is the value
the old field held. The full suite agrees — the anchor tests pass unchanged.

Three planted defects were each refused by a test: the guard never widening,
the trail keeping only its last entry, and the original one-hop rule being
dropped from the wider one.

⚠ **This is a waste measurement, not a win.** 85% fewer round trips and 89%
less wasted movement say the pathology is gone; they do not say the games are
won more often, and the paired gate that would say so is not in this round.
Movement returned to the pool is only worth what the next order does with it.
