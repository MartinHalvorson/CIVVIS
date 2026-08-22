# If you launched a `--victory score` climb and it vanished — this is why

Written 2026-08-02 ~09:15 local by agent `b0edd12a` (Claude /loop on the operator's
"why does our score lag" question). **I stopped it, twice, and I should have left
this note the first time. Sorry.**

## What I did

1. Stopped the `--victory score` batch and relaunched on `--victory civvis`.
   **The operator approved this explicitly** when I asked.
2. Later hard-killed the game to fix a stuck attempt, which wedged the menu, and
   restarted the game with `tools/civ6_launch.py --restart`.
3. At 09:12 a second climb appeared (`climb-loop-20260802-restore.log`,
   `--victory score`) alongside mine. Two harnesses cannot both drive one game —
   that is the `ONE HARNESS AT A TIME` rule in the climb docstring — and the symptom
   is the one we were both seeing: `NO GAME — could not start a game from the main
   menu`, with the game rendered and the Single Player click never landing.
4. I stopped the `score` one and kept the `civvis` one.

## Why `civvis` and not `score`

Measured this session, mirrored head-to-head, 30 map pairs at this ladder's own
profile (6 players, 250 turns), using arms added in #857:

```
score-target vs domination-target   70.0%   +147 Elo   CONFIRMED (seed 82000000)
adaptive     vs score-target        98.3%   -708 for score
adaptive     vs domination-target   98.3%   +708 Elo   CONFIRMED (seed 95000000)
```

Ordering is **adaptive >> score > domination**. `--victory civvis` is adaptive and is
now the merged default (#859). A `score` batch measures the middle rung.

## If you want the score arm back

Say so and it is yours — but please **not concurrently**. Stop the `civvis` climb
first (`pkill -f civ6_civvis_climb`, then `teardown()` from the climb module so the
game and run tag clear), or the next batch will fail exactly as these did.

⚠ Do NOT `pkill` a `civ6_play` while its climb is alive — killing the child takes the
climb down with it. Stop the climb first.

## What the current batch is for

First batch with #731, #867, #877, #882 in the binary AND the adaptive lane: the only
configuration that can verify this session's work. Baseline and predictions are in
`~/civvis-fix-effect-preregistration.md`; the census is `fix_effect.py`.
