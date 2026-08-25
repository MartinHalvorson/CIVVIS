# The wonder reachability gap is closing on its own

_2026-08-19 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

#2061's census said a Diplomacy-targeted agent finished a wonder in **1 of 32**
250-turn games, and that the cause was reachability rather than price: 31 of the
53 wonders name an `adjacent_district`, and a diplomatic empire lays neither a
Harbor nor a Holy Site. This round re-asks that question against current `main`.

⚠ **The construction half of it is already answered elsewhere.** `advanced_wonder_reach`
(`docs/eval/2026-08-18-the-wonder-a-city-cannot-start-pays-its-prerequisites.md`)
credits a wonder's missing prerequisites through the ordinary `Item::Wonder`
arm, is OFF in production, and reports the same diplomatic reading — byte-identical,
zero wonders both arms, because Mahabodhi is religion-gated and the Statue of
Liberty civic-gated late. Nothing here restates that; this is the measurement of
the **baseline** it was measured against.

## What it measured

Same 32 seeds (24000000), same tool, sixteen merges apart:

| tree | Statues of Liberty finished, 32 diplomatic games |
|---|---|
| #2061, 2026-08-18 | **1** — `[1:statue_of_liberty]` |
| `main`, 2026-08-19 | **5** — seats 0, 1, 2, 3 and 4, one each |

`wonder_prereq_reach` is off in production, so this is not that arm either. Main
simply got better at putting Harbors down between the two readings.

## What was decided

**The census that motivates wonder-reachability work is already stale, and
should be re-taken before any of it is funded.** A five-fold move in sixteen
merges is larger than any effect the work itself has measured, so an argument
built on the 1-in-32 figure is now arguing against a tree that no longer exists.
`docs/SIMULATOR_PERFORMANCE.md` has said this about speed since 2026-08-17 —
"re-measure against current main right before shipping, not against the tree you
started from" — and it holds for behaviour just as hard.

**And one road recorded as closed.** The cheap version of a prerequisite credit
is to price a district by the wonder behind it, gated empire-wide-once — zero
the moment any city holds the family, which is what keeps it out of the
`one_launch_pad` failure mode. Built and measured, it is inert: **29 of 29
paired games byte-identical**. The gate is right and closes far too early,
because the two conditions are nearly disjoint in time:

| district | eras its wonders unlock in |
|---|---|
| harbor | **1, 1, 1**, 3, 4, 6 |
| holy_site | 1, **2, 2, 2** |
| campus | 1, 2, 4, 6 |

The Statue of Liberty is the era-4 harbor wonder and harbor also carries three
era-1 wonders, so by the time Civil Engineering lands a coastal empire has held
a Harbor for a hundred turns. `advanced_wonder_reach` avoids this by asking the
per-city question (`wonder_missing_prerequisites`) instead of the empire-wide
one, which is the right shape; the empire-wide variant is a dead end and no code
for it ships.
