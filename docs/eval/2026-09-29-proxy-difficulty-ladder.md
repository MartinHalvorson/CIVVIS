# The ladder proxy from Prince to Immortal: where the Science seat stops winning, and three levers that do nothing there

_2026-09-29 · `claude-opus55-0926` on `martins-m4-air` · simulator rounds, no native games_

Follow-up to `docs/eval/2026-09-26-ladder-proxy-immortal.md`. Same instrument
(`examples/ladder_proxy.rs`, private, not committed) and shape: 4 majors, Tiny
60×38 Pangaea, Online, 6 city-states, full 250-turn clock, games end at the
first victory. The focal seat is the live controller on the Science lane
(`--target science`, deployed genome plus `deploy/live-force-on.txt`). The
three rival seats are `AdvancedAi::new()` + `enable_live_bridge()` and carry
the rung's full AI bonuses. Main `0684eeb`–`d0ec407`; every comparison below
is paired by seed.

New proxy fields this round: each major's end-of-game techs, Science and
space projects (`techs_end`, `sci_end`, `space_end`); city-state
suzerainties, envoys and recruited Great People at turns 60/100/150; and Great
Person patronage actions per seat and currency.

## What was asked

The live ladder holds Emperor (2026-09-09, Science at t213) and is 0 for 35
native Immortal attempts. Rivals won Technology at t175–216. Is the Science
seat losing a close race at the top rungs, and at which rung does it stop
winning?

## How it was measured

One difficulty sweep on the same 12 seeds (81000000+; Immortal 16). A second
Prince/King block of 16 seeds (83000000+) carried the new counters. Three
single-gene arms ran at King against their paired baseline: science denial on
82000000+, idle-Faith patronage and the Great Person closeness limit on
83000000+. "Rival mean" averages the three rival seats; `z` is a paired z on
the per-seed difference.

## What it measured

### The win rate falls off at King, not at Immortal

| rung (seeds 81000000+) | focal wins | focal Science t100 | rival mean Science t100 | focal cities t30 | rival mean cities t30 |
|---|---:|---:|---:|---:|---:|
| Prince (12) | **5** | 89.2 | 69.1 | 2.8 | 2.4 |
| King (12) | 1 | 69.2 | 100.7 | 2.5 | 2.7 |
| Emperor (12) | 0 | 58.2 | 139.1 | 2.3 | 3.7 |
| Immortal (16) | 0 | 60.8 | 175.1 | 2.2 | 3.7 |

At Prince the rivals have no bonus, and the live seat is the strongest seat
in the game. It wins 5 of 12 and leads the rival mean in cities, population,
Science and techs. The live seat is not merely the stock agent: with the focal
seat set to the rivals' own configuration (`--focal-mode deploy`) on the same
seeds, it won 1 of 12 and survived 8.

King's bonus is modest (+8% Science and Culture, +20% Production and Gold,
one era boost, a free Warrior and Builder), yet the focal seat's wins fall
from 5 to 1.

### At Immortal the seat is not in the race

Every Immortal game (16) ended in a rival victory: Science 13, Culture 2,
Religion 1. At the end the winner held all 77 techs and all four space
projects. The focal seat held 29–61 techs, and in 14 of 16 games it had
launched nothing. Its Science at turn 60 was 24 against the best rival's 95.
The gap starts with land: the rivals' free Settler and +60% Production take
the continent first. The focal seat's own `desired_cities` then falls with
the room left (`city_target_meets_the_map`). It averaged 8.4 at turn 30
against 8.9 at Prince.

### What the King seat loses

Second block (83000000+, 16 paired), focal seat King − Prince:

| turn-100 reading unless noted | Prince | King | paired z |
|---|---:|---:|---:|
| focal Science | 86.4 | 79.6 | −0.95 |
| focal cities | 6.88 | 6.50 | −1.19 |
| focal population | 43.7 | 43.6 | −0.03 |
| focal city-state suzerainties | 1.06 | 1.06 | 0.00 |
| focal envoys | 13.1 | 13.1 | 0.00 |
| focal Great People recruited | 6.00 | 4.38 | **−2.23** |
| focal Great People recruited, t150 | 15.5 | 11.6 | **−2.62** |
| rival mean Great People recruited, t150 | 10.2 | 13.8 | — |

The first block read a larger Science loss (−20 at t100, z −3.07 on 12 seeds).
This block reads −7 and not significant, so part of the first reading was
noise. The measurable external loss is **Great People**: the King rivals take
a quarter of the focal seat's by turn 150. Suzerainty and envoys do not move.

### Three levers that do nothing at King

| King, 16 paired | wins | share | why |
|---|---|---:|---|
| `science-threat-denial` + `science-denial-war` (82000000+) | 2 → 3 | −0.45 pp (z −1.06) | focal wars declared 5 → 5; rival space projects unchanged. The denial barely acts. |
| `idle-faith-patronage` (83000000+) | 2 → 2 | 0.00 pp | 16/16 records identical. Its gate is no religion and ≥ 600 Faith, and the focal seat banks 180–285. |
| `tally-great-people` (83000000+) | 2 → 2 | 0.00 pp | 16/16 records identical. The 0.40 closeness limit never binds: the seat buys ~5.9 Faith and 0.4 Gold Great People per game (rivals 7.7 Faith). |

The Great Person gap is therefore point generation. The rivals' +20%
Production raises the districts and buildings that earn the points sooner.
Purchase policy does not explain it.

### A measurement artifact to avoid

The proxy's `alloc60`/`alloc100` production census reports the focal seat's
cities "idle" (empty queue) for 24–27% of their Production at every rung, and
the rivals for ~0%. This is observation timing, not waste. `observe` runs as
each new turn opens, after seat 0's `begin_turn` has completed items and
before its AI refills the queues. The same seat under the rivals'
configuration reads the same 23%. Real lost Production is `lost60`, which is
≈ 0 at Prince.

## What was decided

- **Nothing shipped or forced.** The three King arms are nulls or no-ops, and
  none of them should be re-screened without a new mechanism behind it.
- **Where the rungs are lost.** The proxy rivals are this project's own agent
  with Firaxis' handicap bonuses, far stronger than the native Firaxis AI. The
  compounding of +20–60% Production is what separates the rungs, not a single
  decision the seat gets wrong. The live seat's Prince advantage (≈ +29%
  Science over the rival mean) is roughly what King's bonus takes back.
- **Open lead.** Great Person point generation under a stronger neighbour.
  The proxy now records recruitment and patronage per seat, so a candidate
  lever can be read against the 15.5 → 11.6 loss directly.
- The earlier package recommendation (`befriend-the-strongest` on,
  `peacetime-deterrence` off; see the 2026-09-26 note) is unchanged and still
  the operator's call.
