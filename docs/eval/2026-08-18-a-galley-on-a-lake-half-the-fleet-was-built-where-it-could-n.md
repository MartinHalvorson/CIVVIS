# A galley on a lake: half the fleet was built where it could not sail

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`audit` reports that **21.19% of major unit-turns** are a unit standing still in
the open. That single number has been in the tree for a while and nothing could
be done with it, because it does not say *what* is standing still, and a
settler, a builder and a warrior standing still are three different defects.

So: attribute it, and fix whatever is biggest.

## How it was measured

`audit` gained a per-unit-kind breakdown of the major idle-field share —
the same accounting, split by the unit spending it, ordered by idle unit-turns
because a fix has to go where the mass is rather than where the worst rate on
four turns is.

Everything below is three 150-turn six-player games at 74x46, 9 city-states,
Online, seeds 21000000–02. The naval detail comes from a probe linking the
crate and following every major hull for its whole life; the enqueue path came
from a runtime backtrace at `do_produce`.

## What it measured

The breakdown named it immediately:

| unit | idle unit-turns | share of major idle | own idle rate |
|---|---:|---:|---:|
| **galley** | **1227** | **20.0%** | **54.3%** |
| builder | 944 | 15.4% | 20.5% |
| scout | 695 | 11.3% | 15.7% |
| archer | 560 | 9.1% | 21.7% |
| caravel | 403 | 6.6% | 37.9% |
| quadrireme | 230 | 3.8% | 35.9% |

A Galley spends **more than half its life motionless**, and the three naval
kinds together are 30.4% of all major idle time. Following the hulls:

- majors built **53 naval hulls**; only **17.2%** of their unit-turns involved
  any movement at all;
- **20 of the 53 never moved once**, and **12 of those were sitting on `lake`
  terrain**;
- 33 of 53 lived 25+ turns and moved on fewer than 10% of them.

**A lake is water.** `BasicAi::city_is_coastal` asks only whether some adjacent
tile is water, so a lakeside city is coastal, and Firaxis really does let it
build a Galley — this is a genuine trap in the real game, faithfully
reproduced. The hull is then on the lake for the rest of the game.

⚠ **Three fixes were written and reverted before the right one, and the reason
is worth recording: the naval build does not go through `production_value`.**
Its naval arm, a placement preference for open water, and the naval-recon
waterway test were each implemented and each measured **byte-identical** — the
enqueue was somewhere else the whole time. A backtrace at `do_produce` named it
in one run: `BasicAi::cities` → `best_naval_unit`, which `AdvancedAi` delegates
to and which carries its own `city_is_coastal` gate. Guessing cost three
rounds; the backtrace cost one.

## What the fix measured

`best_naval_unit` asks for **open water** — an adjacent `coast` or `ocean`
tile, which is also Firaxis' own rule for a Harbour — instead of any water.

| | flag off | flag on |
|---|---:|---:|
| naval hulls built (3 games) | 53 | **26** |
| hulls that never moved once | 20 | **3** |
| hulls alive 25+ turns moving on <10% | 33 | **1** |
| galley movement rate | 13.0% | **43.7%** |
| quadrireme movement rate | 37.5% | 41.0% |
| peak hulls alive per seat | 2.8 | 1.4 |
| `audit` major idle-field | 21.19% | **17.13%** |

Half the fleet is no longer built, and the half that is built sails. Galley
leaves the idle table's top four entirely.

**Strength: +44 Elo-equivalent, and unresolved.** `advanced_open_water_navy`
against `advanced`, 40 pairs / 80 games at 6p 74x46, 6 city-states, Online, 150
turns, seed 8100000: 56.2%, 8 sweeps to 3 with 29 neutral, betting CI
43.4%..69.7%, anytime p ≤ 0.4176, **INCONCLUSIVE**. That is the expected
outcome rather than a disappointing one: the same day's gate round measures
about 5% power at 40 maps for a true +40, so a run this length could not have
resolved an effect this size in either direction. The interval excludes nothing
worse than −46.

## What was decided

**Shipped as `advanced_open_water_navy`, defaulting off.** The mechanical
result is large and certain and the strength result is positive and
unresolved, and this repository's gate fails closed on insufficient evidence.
With the flag off the simulator report is byte-identical to `origin/main`
across three seeds.

This is the clearest promotion candidate currently sitting behind a flag. What
would settle it is map count, not a different profile: at 40 maps the gate
cannot see +44, so the next step is the same arm at several hundred maps rather
than another 40.

The `audit` breakdown ships unconditionally. It is what found this, and the
remaining rows are the next leads: **builder** at 944 idle unit-turns (19.8% of
its own turns) and **battering_ram** at 32.5% of its own turns are now the
largest unexplained blocks.
