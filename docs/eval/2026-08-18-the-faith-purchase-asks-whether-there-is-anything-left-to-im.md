# The Faith purchase asks whether there is anything left to improve

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

Not a treatment this time — a defect hunt. `audit` is the repository's own
symptom detector and had not been run this session. What does it say about
current head?

## How it was measured

`audit`, 6 games at the deployment shape — 6 players, 74x46, 9 city-states,
Online, 250 turns, seeds 51000000–05. Then a deterministic counter inside
`builder_step`, and a second one on the Faith purchase, over 3 games of the same
shape.

## What it measured

**Major units are idle-field 13.2%–17.3% of their unit-turns, in every one of
the six games.** "Idle-field" is the audit's own term and it is strict: the unit
did not change tile, its work mark did not move, it is not fortified and it is
not standing in one of its owner's cities. A unit improving a tile or attacking
without moving is *not* counted — the tracker restarts and returns before the
counter.

By unit, aggregated:

| unit | idle unit-turns | share of its own turns |
|---|---:|---:|
| builder | 6,884 | 18.0% |
| scout | 6,520 | 18.5% |
| horseman | 3,786 | 23.7% |
| galley | 3,188 | 27.8% |
| knight | 2,022 | 24.4% |

**Builder is the largest block, and the reason is not the one that looks
likely.** Instrumenting `builder_step`'s three outcomes over 3 games:

| outcome | count |
|---|---:|
| stepped toward a target | 467 |
| target found, could not step | **43** |
| **no improvable tile anywhere in the empire** | **4,377** |

So it is not units stuck behind terrain — that is 43. It is builders with
nothing to do, on 90% of the decisions they reach. And the distribution by
25-turn bucket is **flat** — `[221, 608, 678, 617, 553, 440, 417, 393, 277,
173]` — peaking in the expansion phase, not late when everything is improved.

## What was decided

**One line shipped.** `has_builder_work` already gates the production path and
the gold-purchase path. The Faith purchase counted cities and nothing else.
`advanced.rs`'s `PRODUCTION_BUILDERS_PER_CITY` states in prose that "the
existing `has_builder_work` gate stops production once there is no yield to
add", which was true of two paths out of three.

Counted over 3 games, with the gate in place:

| | |
|---|---:|
| Faith Builders bought | **3** |
| Faith Builder purchases the gate blocked | **156** |

Without it the empire buys 159 and **98% of them have no tile to work**. That is
Faith spent on a unit that will stand still, and Faith buys anything.

The frozen anchor does not move, so no ledger bump.

⚠ **No unit test ships with it, and that is deliberate.** I wrote one, and the
mutation check refused it: removing the gate left the test green, because the
fixture never reaches the Faith path inside `cities()`. A test that cannot fail
on the change it guards is worse than none — the same call as #2060's second
guard. The evidence here is the counter, which is the stronger instrument
anyway, and `builders_are_only_built_when_there_is_ground_to_work` continues to
cover the predicate itself.

⚠ **This does not close the idle finding.** 156 blocked purchases do not account
for 4,377 no-work builder decisions. Most of those are Builders that were built
while work existed and outlived it: every acquisition path is gated at the
moment of purchase and nothing retires a Builder afterwards. Civilization VI
consumes a Builder on its last charge; an empire that runs out of work simply
keeps them. **What to do with a Builder that has charges and no work is the next
question**, and it is worth more than this line was — as are `galley` at 27.8%
and `horseman` at 23.7%, which have nothing to do with Builders at all.
