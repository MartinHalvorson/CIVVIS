# The doctrine arena

Seven hand-built tactical positions, each posing one decision that a famous
engagement turned on, and an instrument that says not only who won the trade
but **how each side fought**.

Companion to `src/doctrine.rs` (the positions and the ledger) and
`src/bin/doctrine_arena.rs` (the driver). Related instruments:
`docs/TACTICS.md` for the tactical search itself, `src/skirmish.rs` for the
matched mirror benchmark, `tools/tactics_bench.py` for the Tactics game mode's
win-rate battery.

## Why another instrument

`battle_bench` asks one question — two identical armies six tiles apart in open
ground — and asks it precisely. That is the right gate for "does this change
trade better", and this does not replace it.

What it cannot do is **pose a problem**. An agent can be excellent at the
stand-up fight and have no idea what to do when the enemy arrives in two
columns, when the ground funnels, or when half its army is six tiles behind the
other half. Nor can a single material number say **why** one agent beat
another, which is the number you actually need before deciding what to change.

## The positions

```bash
cargo run --release --bin doctrine_arena -- --list
```

| id | engagement | the decision |
| --- | --- | --- |
| `central_position` | Bonaparte, Montenotte 1796; Ligny–Quatre Bras 1815 | Beat one converging column before the other arrives |
| `oblique_order` | Epaminondas at Leuctra 371 BC; Frederick at Leuthen 1757 | Spend the massed wing before the refused one folds |
| `the_defile` | Leonidas at Thermopylae; Bonaparte at the Arcole causeway | Hold a two-tile front against more than twice the material |
| `the_ridge` | Wellington at Waterloo, 1815 | Missiles and high ground against shock troops that must close |
| `double_envelopment` | Hannibal at Cannae, 216 BC | Five better units round the wings of nine cheap ones |
| `the_reserve` | Bonaparte at Marengo; the Guard at Waterloo | Commit the reserve together, or feed it in one unit at a time |
| `the_river_line` | Bonaparte at Lodi 1796; Friedland 1807 | Two crossings; an army astride an obstacle is two armies |

**The board is painted, not generated.** Every tile of every position is written
down in `POSITIONS`, so the defile is a defile in every run, on every machine,
and after every change to map generation. A generated map is a sample; a
position is a fact. What varies between seeds is the **muster** — each unit is
nudged to a nearby free tile by a seeded jitter — so a position yields
independent samples without ceasing to be that position. A doctrine that only
works from an exact deployment is not a doctrine.

**The economy is set to nothing.** No city, so there is no objective but the
enemy army and nowhere to heal; no production and no gold, so no unit arrives
that the position did not deploy; no research, so both sides fight the
engagement in the era it was written for. The force deployed is the whole
experiment.

## The pairing, and the control

Positions are asymmetric on purpose, so each seed is played **twice with the
roles swapped** and the pair summed. Each agent takes both sides of every
problem in equal measure.

**Run the control first, every time.**

```bash
cargo run --release --bin doctrine_arena -- --a advanced --b advanced --seeds 12
```

It must report a paired mean of exactly `+0.0` on every position with `0/12`
seeds diverging. It does. That is what licenses reading any treatment number out
of this harness — the same role `--grant none` plays for the oracle and
`--a advanced --b advanced` plays for `battle_bench`. Every position also
carries its own **fires-check** (the `fires` column: seeds on which the two
agents' play diverged at all), because a null from a treatment that never fired
is the harness talking, not the game.

## Reading the report

```bash
cargo run --release --bin doctrine_arena -- --a advanced --b basic --seeds 60
cargo run --release --bin doctrine_arena -- --position the_reserve --a advanced --b basic
cargo run --release --bin doctrine_arena -- --profile advanced --seeds 20
```

**Read the rows, not the pooled line.** The positions were chosen to pose
*different* problems. An agent much better at one and worse at another pools to
nothing, and the pooled row will say so while the rows say what happened.

**Cross-read the split against the p.** The mean is driven by size and the sign
test by count, so an agent that wins big and loses small shows a large positive
mean beside a losing split — see `the_ridge` below. Neither number is wrong;
they are answering different questions, which is why both are printed.

### The doctrine profile

Read off the board rather than the result. Every field is `--` rather than a
zero when the engagement gave nothing to measure it from.

| column | what it counts |
| --- | --- |
| `concentr.` | own units near the contact zone less enemy units near it, per contact turn |
| `disper.` | mean distance between own units, per turn — low is a body that moves as one |
| `envelop.` | enemy units taken from two or more sides at once, per contact turn |
| `focus` | share of damage dealt that landed on enemies that died |
| `ground` | share of own unit-turns spent on hills or in cover |
| `screen` | share of own ranged unit-turns with a friendly between them and the enemy |
| `contact` | share of turns on which the two forces were within two tiles |

The contact zone is computed **once for the board**, not once per side, so
concentration is a genuine local force ratio: within one engagement one side's
figure is exactly the negative of the other's, and
`concentration_is_a_local_force_ratio_and_sums_to_zero` pins it. Defined per
side instead, both armies can report themselves outnumbered at the same
contact, which is not a fact about anything.

**These are descriptions, not scores, and nothing adds them up.** The whole
content of a doctrine is that the right value depends on the position: an army
holding a defile *should* be dense and static, and the same numbers from an
army that was supposed to envelop mean it failed.

## What the arena found on its first run

`advanced` against `basic`, 60 seeds a position, both roles, 2026-08-08.

| position | mean swing | +/- se | t | sign p | better/worse |
| --- | --- | --- | --- | --- | --- |
| `central_position` | +2.3 | 35.9 | 0.07 | 0.4966 | 30/24 |
| `oblique_order` | +155.2 | 43.9 | 3.53 | 0.0814 | 35/21 |
| `the_defile` | +12.0 | 15.4 | 0.78 | 1.0000 | 26/25 |
| `the_ridge` | +90.0 | 44.2 | 2.04 | 0.4966 | 24/30 |
| `double_envelopment` | +30.5 | 25.9 | 1.18 | 0.1839 | 28/18 |
| **`the_reserve`** | **−270.5** | **62.0** | **−4.36** | **0.0018** | **16/40** |
| `the_river_line` | +155.8 | 60.1 | 2.59 | 0.1480 | 35/23 |
| ALL POSITIONS | +25.0 | 17.8 | 1.41 | 0.5355 | 194/181 |

The pooled row is the caveat above, demonstrated: `+25.0` on a run containing a
`+155` and a `−270`.

### `advanced` commits to contact locally outnumbered

`the_reserve` is the one position where `advanced` is beaten, and beaten badly —
511 units killed against 632 lost, replicated from a first run at 40 seeds
(−267.5, p = 0.0076) to 60 (−270.5, p = 0.0018). The profile names it:

```
                      concentr.  disper.  envelop.  focus  ground  screen  contact
advanced r0 (-89)         -0.46    +1.90     +0.18    91%      7%     28%      88%
basic     r0 (+46)        +1.11    +1.71     +0.18    95%     10%     46%      88%
advanced r1 (-46)         -1.11    +2.28     +0.15    92%      4%     25%      88%
basic     r1 (+89)        +0.46    +2.09     +0.21    97%      6%     44%      88%
```

In **both** roles `advanced` is on the negative side of the local force ratio
and `basic` on the positive side: it arrives at the point of contact outnumbered
by half a unit to a full one, and its ranged units spend 25–28% of their time
screened against `basic`'s 44–46%. This is Napoleon's actual complaint about
reserves, measured — the reserve is being fed in rather than committed.

Worth stating what this does **not** yet establish: the arena shows the
correlation between arriving outnumbered and losing the position, not that
forcing concentration would fix it. The next step is a treatment that prices
local force ratio in the joint plan, run through this harness and then through
`battle_bench` and `tools/tactics_bench.py`, because a gain here is a necessary
condition for the whole-game gate and never a substitute for it.

### `advanced` spreads its fire against a dense mass

On `double_envelopment` role 0 — nine cheap units in depth — `advanced` puts
34% of its damage on units that died. Against the same position `basic` manages
22%, and both roles lose the position heavily, so this is the position being
hard rather than one agent failing it. But an agent spending two-thirds of its
damage on units that survive to heal is a lever, and `focus` is the column that
found it.

## What this does not license

The same two limits `src/skirmish.rs` states, and one more.

- **It measures fighting, not winning.** `docs/ORACLE.md` records that military
  capability granted outright has not moved whole-game wins in this simulator.
  A gain here is a necessary condition for the whole-game gate, never a
  substitute for it.
- **The armies are placed, not earned.** Composition is an input, so nothing
  here says whether an agent would ever have built that force.
- **The positions are a curriculum, not a distribution.** They were chosen
  because each poses a decision worth being able to make, not because they occur
  in that proportion in a real game. An agent tuned to win all seven has been
  tuned on seven boards; the pooled row is not a fitness function and must not
  be used as one.

## Adding a position

Append to `POSITIONS` in `src/doctrine.rs`. Terrain is offset `(col, row)` cells
under a `Brush`; forces are `at(kind, col, row)` in role order. The existing
tests then cover it automatically — every position must seat its whole force,
keep its painted terrain, produce a fight, conserve its ledger, and net a
self-match to exactly zero. A position that cannot do all five is not measuring
anything.
