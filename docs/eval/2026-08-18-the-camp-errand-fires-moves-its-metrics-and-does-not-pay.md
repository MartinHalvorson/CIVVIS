# The camp errand fires, moves its metrics, and does not pay

_2026-08-18 · `claude-fable-barbs`_

## What was asked

The barbarian-ledger baseline showed nobody clears camps deliberately: ~3.3
incidental clears per game against 17 camps standing at the end, each worth
50 gold, 2–3 era score (Ancient–Medieval), and boost progress. Does a
bounded, deliberate clearing errand — the tactical decision this axis is
about — pay?

## How it was measured

Entrant `advanced_camp_bounty` vs `advanced`, 20 map pairs per seed on two
disjoint seeds (`210000000`, `220000000`), deployment shape: 6p 74×46, 9
city-states, online, 250 turns, all six victories. The errand: at most two
claimed hunters (one per camp), peacetime only, camps within `camp_radius()`
of home, approach capped at recall range, recon excluded, priced by the
same exchange gate as every other attack.

Getting the errand to fire at all took three pinned repairs, each a finding
about the peacetime path:

1. The consult had to live in the military step's **empty-enemies branch** —
   peacetime units exit there to the Basic fallback, and any slot below the
   enemy-list machinery is unreachable for them (the first screen was
   byte-identical, 20/20 neutral maps).
2. A defended camp cannot be handed to the tactical mover: its threat
   penalty (~8 beside an ancient guard) always outweighs its closing
   progress (~3.5), which is survivable against a raider that walks into
   reach and a **deadlock against a fortified guard that never moves** —
   the trace hovered at distance two indefinitely. The errand marches by
   engine pathing and attacks directly; the exchange gate already priced
   the fight.
3. The loop pin discriminates by **speed against the fallback's accidental
   clears** — the first version of the pin was satisfied by the wander and
   measured nothing.

## What it measured

The capability works and its goal metrics move hard:

| per seat-game | bounty s210 / s220 | stock s210 / s220 |
|---|---|---|
| camps cleared | 5.32 / 5.22 | 3.20 / 2.72 |
| camps ≤6 of home @t50 | 1.82 / 1.73 | 2.49 / 2.37 |
| units lost to barbarians | 0.44 / 0.57 | 0.52 / 0.38 |
| camps standing at end | 17.27 / 17.30 | same |

And it loses games:

- Seed 210000000: 45.0% (Elo −35, CI −369..+203), direction 5/7/8.
- Seed 220000000: 37.5% (−89, CI −494..+63), direction 0/5/15, p=0.0625.
- Pooled 40 pairs: 33/80 games (41.3%), direction 5 for / 12 against / 23
  neutral — the unfavorable direction repeats on a disjoint seed.

The economics never show up on the ledger: final gold is LOWER with the
bounty (397 vs 513 and 481 vs 534) and score/era score do not move its way.
The mechanism is visible in the world model: the camp target is per-major
and the deficit refills in fog at one-in-two per turn, so camps standing at
the end are identical — clearing near home is whack-a-mole that buys ~2.3
extra bounties (~115 gold) with two units' walking turns, against an enemy
that (in the current passive native world) was never going to attack
anyway.

## What was decided

Retained, not promoted: `camp_bounty` stays default-OFF; the entrant arm,
its `camp-bounty-errand` axis, and the mechanism pins ship so the errand
stays priceable. Two forward paths are recorded rather than guessed at:
an era-gated cheaper variant (clear only while the era-score moment pays,
one hunter), and re-measurement in a raiding world (see "The world where
the barbarians raid") where a camp beside the capital is a spawner of real
raids, not furniture — the value of clearing it is regime-dependent, and
the passive world is the unfaithful one.
