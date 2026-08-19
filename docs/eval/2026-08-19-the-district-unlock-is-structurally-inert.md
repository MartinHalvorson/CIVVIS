# The district unlock is structurally inert

_2026-08-19 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

`strategic_wonders` (#2061) shipped with a census that stated its own limit:
over 32 250-turn games a Diplomacy-targeted agent finished a wonder in **one**,
and not because it declined them on price. **31 of the 53 wonders name an
`adjacent_district`** — harbor 6, campus 4, holy_site 4, encampment 3 — so
`can_produce` never offered them and no valuation was consulted. Pricing a
wonder the agent cannot be offered is a no-op.

The obvious next move: **price the district by the victory-carrying wonder
standing behind it.** Does that get the wonders built?

## How it was measured

`AdvancedAi::strategic_wonder_unlock` prices, for a district family, the best
lane value among wonders that family unlocks which the agent has researched and
nobody has built, at a quarter share. Gated empire-wide-once — zero the moment
**any** city holds the family — because `one_launch_pad` records exactly what
the other shape costs (the first-Spaceport rung was empire-wide, every city
claimed it at once, and four cities started pads in a fortnight for one usable
launch site).

`victory_eval --target diplomatic --players 6 --turns 250 --speed online`, the
arm against `--without strategic-wonder-districts` on identical seeds.

## What it measured

**The term almost never fires, and the reason is structural rather than a
tuning miss.** Its two conditions are nearly disjoint in time:

| district | eras its wonders unlock in |
|---|---|
| harbor | **1, 1, 1**, 3, 4, 6 |
| holy_site | 1, **2, 2, 2** |
| campus | 1, 2, 4, 6 |
| encampment | 1, 1, 2 |

The Statue of Liberty — the four diplomatic points this was written for — is the
era-4 harbor wonder, and harbor also carries three era-1 wonders. So by the time
Civil Engineering lands, a coastal empire has held a Harbor for a hundred turns
and the empire-wide gate is already closed. The gate is right; it just closes
long before the wonder that needed it arrives.

Measured: on a fresh stream (29000000) **29 of 29 paired games are
byte-identical** — the arms play the same game, so there is nothing to have an
opinion about. On the older stream (24000000, 32 a side) they differ marginally:
6 Statues of Liberty against 5, lane 31/32 against 30/32, which is one game
either way. The fresh stream was stopped at 29 rather than run to 96; a term
that has not fired once in 29 games does not need 96 to be called rare, and
`⚠ nothing differed` is a non-measurement however many maps it is taken over.

## What was decided

**Not shipped.** The code passes 2 401 tests and two focused tests, including
one pinning the empire-wide gate against the `one_launch_pad` failure mode, and
it is still inert. Landing a behaviour that provably does not fire would add a
flag, a registry row, an evaluator arm and a branch in the hottest function in
the repository in exchange for nothing.

**What the correct version would need, so the next attempt starts here.** The
right condition is not "no city holds the family" but "**this wonder has no
legal site anywhere in my empire**" — the engine's own `wonder_sites`, which
closes exactly when the wonder becomes placeable and not before. That is a scan
of every owned tile of every city per candidate wonder per district item, and
district items are per (district, position); `production_value` is ~95% of the
main thread. It needs a per-turn memo before it can be tried, and this file's
own history says an unmemoized scan there is how you turn a −4% idea into a
−0.1% one.

**★★★ And the reason it may not be worth building: the constraint is relaxing on
its own.** Same 32 seeds (24000000), same tool, sixteen merges apart:

| tree | Statues of Liberty finished |
|---|---|
| #2061, 2026-08-18 | 1 — `[1:statue_of_liberty]` |
| `main`, 2026-08-19 | **5** — seats 0, 1, 2, 3 and 4, one each |

Nothing in this change caused that; main simply got better at putting Harbors
down. **Re-measure the reachability gap against current main before spending a
memo on it** — the census that motivated this work is already sixteen merges
stale, which is the standing lesson in `docs/SIMULATOR_PERFORMANCE.md` applied
to behaviour instead of speed.
