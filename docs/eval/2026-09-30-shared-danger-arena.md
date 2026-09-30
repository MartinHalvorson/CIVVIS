# Shared danger: the battle planner reads each enemy blow once

_2026-09-30 · `claude-opus55-loop` on `mbp-m5-max-128` · doctrine arena + fires screen_

## What was asked

Why does a King Domination siege with eight or more units staged strike a city
twice in 27 turns? On `civvis-20260930T221624Z` (unwalled Stockholm, a
crossbowman, a catapult and a defender) the battle planner rotated 89-hp
archers out "to heal" and held 100-hp spearmen "rather than strike" every turn.
Over that game's first 162 turns the planner doomed 193 unit-turns, held 114
strikes and rotated 55 units standing at 80+ hp.

## The mechanism

`DangerField::danger(tile, unit)` is the sum of every visible hostile's blow on
that unit at that tile. The rotation (`danger > hp - 20`) and the doomed-blow
veto ask it of every unit independently, so each of nine units is charged all
three enemy strikes at once. A hostile strikes once a turn.

## The treatment

Opt-in gene `shared-danger`: each hostile's blow is weighted by one over the
number of our land units standing in its reach now, and the reading never falls
under the strongest single blow (one hostile can still spend its whole strike
on the unit asked about). It changes the rotation's exposure test and the
doomed-blow veto only; the kill plan's end-tile costs and the positions plan
keep the full field.

## What it measured

Doctrine arena, the live seat's planner stack
(`battle-planner-2 + strike-reach + safest-stand + doomed-blow-veto-2 +
fire-plan + swap-rotation-2 + objective-board + siege-train + siege-commitment
+ siege-positive-damage-budget`) with and without the gene, 240 seeds × 2 role
swaps (start seed 5000):

| position | mean | ± se | t | better/worse |
|---|---:|---:|---:|---:|
| oblique_order | +97.7 | 23.2 | 4.22 | 141/85 |
| the_ridge | +111.0 | 27.8 | 3.99 | 132/96 |
| double_envelopment | +42.2 | 7.4 | 5.67 | 45/6 |
| lake_trasimene | +71.4 | 25.6 | 2.78 | 123/84 |
| the_storming | +71.5 | 30.6 | 2.34 | 107/85 |
| the_golden_bridge | +15.4 | 7.5 | 2.07 | 29/18 |
| the_breakthrough | +31.5 | 22.1 | 1.42 | 127/98 |
| the_river_line | +21.4 | 19.8 | 1.08 | 116/105 |
| the_relief | +18.0 | 25.0 | 0.72 | 96/97 |
| the_reserve | +17.8 | 23.3 | 0.76 | 112/101 |
| central_position | -0.1 | 14.0 | -0.01 | 80/103 |
| the_defile | -9.9 | 5.4 | -1.83 | 30/47 |
| hammer_and_anvil | -35.5 | 18.7 | -1.89 | 97/122 |
| **all positions** | **+34.8** | **5.8** | **5.99** | **1235/1047** |

Kills per loss 1.09 against 0.92; cities taken/lost +127/-117. The two
negative rows are under two standard errors.

Whole-game fires probe (24 games, Emperor, 6p 74x46, `docs/gene_screens/fires/shared-danger.json`):
win +2.0 ± 6.9 pp, share -0.2 ± 1.3 pp on 33 seats on — a fires check, not a
verdict; compute +5.6 ± 3.2%.

## What was decided

Registered as `Kind::OptIn` (off by default) and added to
`deploy/live-force-on.txt` for the live Domination seat.
