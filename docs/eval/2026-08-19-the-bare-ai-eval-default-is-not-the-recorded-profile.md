# The bare ai_eval default is not the recorded profile

_2026-08-19 · `3f5ef0e8`_

## What was asked

Does `district_building_chain` actually move the metric it exists to move — the buildings a
standing specialty district is missing? And, once that answer came back twice with opposite
signs: **what board does `ai_eval` give you when you do not name one?**

## How it was measured

`live` vs `live_without_district_building_chain`, Chieftain, 150 turns, on two boards:

- **bare default** — no `--players/--width/--height/--speed` — 30 pairs / 60 games.
- **live-calibrated** — `--speed online --players 10 --width 74 --height 46` — 10 pairs /
  20 games, chosen to match the live Civ 6 ladder (10 majors, small map, Online speed).

Both boards were then compared against two real ladder runs at the same turn, reading the
per-arm stats line (`seat-win% score cities pop tech civic dist build military gold`) rather
than the win rate — a Library/Campus change shows up in `build` and `tech` and needs no
victory to resolve.

## What it measured

At turn 150:

| profile | cities | **dist** | build | **tech** |
|---|---|---|---|---|
| bare default | 5.25 | **5.9** | 13.6 | **15.6** |
| `--speed online --players 10` only | 1.10 | **2.4** | 6.1 | **8.1** |
| `--speed online --players 10 --width 74 --height 46` | 8.70 | **19.4** | 44.2 | **34.0** |
| live `civvis-20260819T092530Z` | 5 | **19** | 28 | **38** |
| live `civvis-20260819T074452Z` | 6 | **17** | 24 | **36** |

The bare default runs **~3× short on districts and ~2× short on tech**. Note the second row:
raising `--players` *without* the map size is worse than changing neither — ten seats on a
default-sized board is far more crowded than Civ 6's small map, and every economy number
collapses.

And the treatment reads in opposite directions on the two boards:

| profile | build (ON) | build (OFF) | reading |
|---|---|---|---|
| bare default, 30 pairs | 13.6 | 14.1 | treated arm LOWER — "no measurable benefit" |
| calibrated, 10 pairs | **44.2** | 40.9 | treated arm ~8% HIGHER |

Same treatment, same binary. On the bare default the seat holds ~6 districts, so a treatment
that fires only when a standing district lacks its buildings rarely gets the chance to act;
the live seat holds ~19.

Paired verdicts, for completeness: bare default, 60 pairs / 120 games gave +29 Elo-equivalent
(CI −43..+107), INCONCLUSIVE — the gate itself notes it only promotes a true edge of about
+77, so a real smaller effect reads this way regardless. No paired verdict was run on the
calibrated board (~15 min per read there against ~2 min on the default).

## What was decided

**The null was the board, not the behaviour**, and it was retracted on #2094. Nothing is
promoted or withheld on this evidence: the calibrated row is n=10 pairs, direction only, and
`dist` moves the other way (19.4 v 21.6) unexplained.

Practice this round argues for:

- Name the board in every result, as `docs/EVAL.md` does throughout — the bare form is the
  obvious thing to reach for and it is not what recorded results used.
- Before trusting a verdict, read the per-arm stats line and check `cities/dist/build/tech`
  against a live run at the same turn. If the regime the treatment needs is absent, the
  verdict is inert whichever way it lands.
- Two sibling traps hit the same session and belong with this one: the default profile never
  produced a **science or diplomatic** victory (89 of 120 games religious), so win-rate reads
  say nothing for those lanes; and `--victories science` at 150 turns returns `no winner 10`,
  because a science win is not reachable at that horizon.
- For live-ladder questions specifically, `--players 10 --width 74 --height 46 --speed online`
  tracks the seat; the house `6p 74×46` profile remains the right one for comparison against
  everything else on record.
