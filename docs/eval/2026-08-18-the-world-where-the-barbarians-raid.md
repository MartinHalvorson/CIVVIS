# The world where the barbarians raid

_2026-08-18 · `claude-fable-barbs`_

## What was asked

The barbarian-ledger baseline showed native barbarians take nothing: 0.00
civilians captured in 240 seat-games, while the live regime loses settlers to
raiders (`civvis-20260815T233405Z`, `civvis-20260816T155856Z`). How much of
that gap is the engine's own barbarians being frozen, and what would the
native game look like if they acted?

## How it was measured

A detached study worktree (never a PR) flipped the freeze and re-ran the
baseline comparison exactly: `advanced` vs `advanced_v1`, the SAME 20 map
pairs (seed `210000000..=210000019`), same deployment shape — 6p 74×46, 9
city-states, online, 250 turns, `--deployment-comparison`. The only
difference between the two rounds is the flip.

The freeze is not one gate. The barbarian seat is constructed as a minor
with no home city, and SEVEN separate paths key on `BasicAi::minor_home`,
which is `None` for it — each alone is sufficient to keep barbarians
passive:

1. the `military_step` minor gate (every barb military unit returns to
   `fortify_or_stop`);
2. the same step's enemy list (`minor_enemy_near_home` is false with no
   home, so `enemy_ids` is empty for barbarians);
3. the attack-scan tile filter (skips every tile for a homeless minor);
4. `nearest_enemy`'s city-target minor filter;
5. `nearest_enemy`'s unit-target minor filter;
6. the shared movement scorer's `within_minor_front` (no home → no legal
   neighbour → the unit cannot move at all);
7. the path mover's own minor gate (returns false with no home).

A probe (`study_probe_barbarian_units_act`) pinned the diagnosis: before
the patches 0 of 20 barbarian units had moved by t39; after, 14 of 27.
The study kept camp guards on their camps, left the recon units to the
engine's scout phase, and let barbarians capture settlers (the live
failure mode). The dead branches these gates strand — `near_home` is
unconditionally true for barbarians, barbarians are excluded from the camp
target arm, the `capture_adjacent_civilian` doc claims barb scouts capture —
show the code was written expecting barbarians to run this path; the freeze
is fallout from the city-state alignment (#10), not a design.

## What it measured

Per seat-game, same seeds, passive world → raiding world:

| metric | `advanced` | `advanced_v1` |
|---|---|---|
| civilians lost to barbarians | 0.00 → **10.41** | 0.00 → **12.19** |
| units lost to barbarians | 0.47 → 5.58 | 0.11 → 3.02 |
| barbs killed | 19.32 → 22.76 | 23.40 → 25.74 |
| camps cleared | 3.34 → 3.82 | 3.42 → 3.25 |
| camps standing at end | 17.10 → 17.23 | same |

Head-to-head moved from 77.5% (+215, CI +40..+390) to 85.0% (+301, CI
+100..+503) for `advanced`; average game length 203.2 → 207.0 turns.

Ten-plus civilians lost per seat per game is the early game the live seat
already plays. Every native defensive treatment that fires-checked
byte-identical (sea-answers) did so because the native enemy never acts —
the check measured the world, not the treatment.

## What was decided

Not shipped, deliberately. Activating barbarians re-baselines every native
measurement (champion fitness, Elo pins, every retained result), and the
study flip is not yet Civ-6-faithful (barbarians chase at unlimited range;
no camp leash). Recorded here as the enabling change for pricing ANY
defensive handling natively, with the seven gates named so the real repair
is mechanical. The recommendation: a deliberate, pre-registered engine
round — barbarian activation with a Civ-6-shaped leash — treated as an
environment migration with re-pins, not as an AI treatment.
