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
| `central_position` | +46.5 | 31.7 | 1.47 | 0.2976 | 34/25 |
| `oblique_order` | +72.0 | 36.9 | 1.95 | 0.4188 | 31/24 |
| `the_defile` | +45.3 | 17.2 | 2.64 | 0.0213 | 33/16 |
| `the_ridge` | +118.8 | 48.0 | 2.48 | 0.5682 | 27/22 |
| `double_envelopment` | +44.0 | 33.0 | 1.33 | 0.4614 | 26/20 |
| **`the_reserve`** | **−189.2** | **58.5** | **−3.23** | **0.0005** | **15/42** |
| `the_river_line` | +89.3 | 51.4 | 1.74 | 0.0331 | 37/20 |
| ALL POSITIONS | +32.4 | 16.3 | 1.99 | 0.0870 | 203/169 |

The pooled row is the caveat above, demonstrated: `+32.4` on a run containing a
`+119` and a `−189`. `the_ridge` is the other caveat: a `+118.8` mean beside a
27/22 split and a sign p of 0.57 — `advanced` wins that position big and loses
it small, and the two tests are answering different questions about it.

### `advanced` commits to contact locally outnumbered

`the_reserve` is the one position where `advanced` is beaten, and beaten badly —
536 units killed against 621 lost. It has now survived three runs and one change
to the instrument: 40 seeds (−267.5, p = 0.0076), 60 seeds (−270.5, p = 0.0018),
and 60 seeds again after the muster was fixed (−189.2, p = 0.0005). The mean
moved and the split did not, which is what a real effect looks like when the
sampling around it changes. The profile names it:

```
                      concentr.  disper.  envelop.  focus  ground  screen  contact
advanced r0 (-15)         -0.11    +1.87     +0.21    93%      9%     32%      86%
basic     r0 (+80)        +1.18    +1.67     +0.17    96%     10%     50%      85%
advanced r1 (-80)         -1.18    +2.18     +0.12    89%      4%     25%      85%
basic     r1 (+15)        +0.11    +2.02     +0.20    93%      6%     39%      86%
```

In **both** roles `advanced` is on the negative side of the local force ratio
and `basic` on the positive side, and the gap is widest in role 1 — the *far*
reserve, the one that has to march before it can help, where `advanced` fights
at −1.18 and loses 80 material a seed. Its ranged units spend 25–32% of their
time screened against `basic`'s 39–50%, and its army is the more spread out of
the two in that role (2.18 against 2.02). This is Napoleon's actual complaint
about reserves, measured — the reserve is being fed in rather than committed.

Worth stating what this does **not** yet establish: the arena shows the
correlation between arriving outnumbered and losing the position, not that
forcing concentration would fix it. The next step is a treatment that prices
local force ratio in the joint plan, run through this harness and then through
`battle_bench` and `tools/tactics_bench.py`, because a gain here is a necessary
condition for the whole-game gate and never a substitute for it.

### The arena separates two controllers a win rate calls a coin flip

`docs/TACTICS_BASELINE.md` warns that `advanced_v1` — a frozen copy of the live
controller — sits near 50% against `advanced` in the Tactics-mode win-rate
battery, because nearly everything separating them is empire machinery an arena
never exercises, and that **a near-50% result there is the expected null rather
than a finding**.

The doctrine arena resolves the same pair decisively. 40 seeds a position, both
roles:

| position | mean swing | t | sign p | better/worse |
| --- | --- | --- | --- | --- |
| `central_position` | +97.2 | 2.46 | 0.0017 | 29/9 |
| `oblique_order` | +466.0 | 7.11 | 0.0000 | 33/7 |
| `the_defile` | +5.5 | 0.26 | 1.0000 | 12/13 |
| `the_ridge` | +897.8 | 16.94 | 0.0000 | 39/1 |
| `double_envelopment` | +335.8 | 14.35 | 0.0000 | 39/0 |
| `the_reserve` | +501.2 | 6.60 | 0.0000 | 33/6 |
| `the_river_line` | +265.5 | 3.27 | 0.0807 | 26/14 |
| ALL POSITIONS | +367.0 | 13.79 | 0.0000 | 211/50 |

`advanced` is ahead on six of seven, two of them at 39 seeds to 0 or 1, and
`the_defile` is a genuine null rather than a small win (12/13). This is the
resolving-power argument `src/skirmish.rs` makes, on harder ground: a signed
material swing carries far more information per game than a win or a loss does,
and posing seven different problems finds separation that one problem asked
repeatedly does not.

Note that `the_reserve` — the position `advanced` loses to `basic` — is one it
wins hugely against `advanced_v1` (+501.2). Losing a position is not the same as
being bad at it, and this is why the arena reports opponents separately rather
than a single tactical rating.

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
