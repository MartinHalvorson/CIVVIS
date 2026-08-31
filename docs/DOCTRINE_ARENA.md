# The doctrine arena

Eleven hand-built tactical positions, each posing one decision that a famous
engagement turned on, and an instrument that says not only who won the trade
but **how each side fought**.

Companion to `src/doctrine.rs` (the positions and the ledger) and
`src/bin/doctrine_arena.rs` (the driver). Related instruments:
`src/skirmish.rs` for the matched mirror benchmark and
`tools/tactics_bench.py` for the Tactics game mode's win-rate battery.

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
cargo run --release --features developer-tools --bin doctrine_arena -- --list
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
| `lake_trasimene` | Hannibal at Lake Trasimene, 217 BC | A march column between water and hills must become a formation before it is destroyed a section at a time |
| `the_breakthrough` | Bonaparte's *masse de rupture*; Guderian at Sedan 1940 | A fist massed on one point against a line holding everywhere |
| `hammer_and_anvil` | Alexander at Gaugamela, 331 BC | Infantry fixes the front while something fast goes round it |
| `the_golden_bridge` | Sun Tzu: leave a surrounded enemy a way out | Troops cornered with nowhere to go fight at a price the arithmetic does not predict |

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
cargo run --release --features developer-tools --bin doctrine_arena -- --a advanced --b advanced --seeds 12
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
cargo run --release --features developer-tools --bin doctrine_arena -- --a advanced --b basic --seeds 60
cargo run --release --features developer-tools --bin doctrine_arena -- --position the_reserve --a advanced --b basic
cargo run --release --features developer-tools --bin doctrine_arena -- --profile advanced --seeds 20
cargo run --release --features developer-tools --bin doctrine_arena -- --correlate --a advanced --b basic --seeds 120
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
| `arrival` | spread, in turns, of when each unit first reached the enemy — low is "fight united" |
| `foot` | the same over 2-move units alone, which separates a slow line from fast cavalry |
| `absent` | share of the force that never reached the enemy at all |
| `vanguard` | share of the force in contact on the turn contact **first** occurred |

`arrival` is the measurement behind the one thing every general on this list
agreed on: *march divided, fight united*. It is the standard deviation, over a
side's units, of the turn on which each first came within two tiles of an enemy.
A unit that never reached the enemy counts as arriving on the final turn —
deliberately, because dropping those units would let an army that left half its
strength standing in the rear report a *tighter* arrival than one that brought
everything up a turn apart.

`foot` exists because `arrival` on its own cannot answer the obvious objection.
A four-move horseman reaches the enemy turns before a line of spearmen does
however well the line is handled, so a wide spread might be a fact about the
roster and how an agent uses its cavalry rather than about how it marches.
Restricting the same spread to units with two movement points or fewer removes
that explanation entirely. `absent` is the complementary measure: never
arriving is the strongest form of arriving late, and the one form of it that no
difference in movement points can explain away.

`vanguard` is the only column here that can support a **causal** claim, and it
is worth being explicit about why the others cannot. Every other column
describes an engagement that has already happened, so correlating one against
the result restates the outcome rather than explaining it — an army being
destroyed stops arriving, which inflates its own arrival spread. `vanguard` is
recorded at a single instant, the turn contact first exists, upstream of
everything the engagement decides. `vanguard_clean` reports the share of those
instants that had not yet seen a casualty so the guarantee can be checked
rather than trusted; it runs at 100% on ten of the eleven positions and 86–88%
on `double_envelopment`.

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

## What the arena found

`advanced` against `basic`, 60 seeds a position, both roles, 2026-08-08.
Two positions are clean nulls — `the_breakthrough` (31/28) and
`the_golden_bridge` (26/29) — which is the arena declining to manufacture a
result on problems the two agents answer the same way.

| position | mean swing | +/- se | t | sign p | better/worse |
| --- | --- | --- | --- | --- | --- |
| `central_position` | +46.5 | 31.7 | 1.47 | 0.2976 | 34/25 |
| `oblique_order` | +72.0 | 36.9 | 1.95 | 0.4188 | 31/24 |
| `the_defile` | +45.3 | 17.2 | 2.64 | 0.0213 | 33/16 |
| `the_ridge` | +118.8 | 48.0 | 2.48 | 0.5682 | 27/22 |
| `double_envelopment` | +44.0 | 33.0 | 1.33 | 0.4614 | 26/20 |
| **`the_reserve`** | **−189.2** | **58.5** | **−3.23** | **0.0005** | **15/42** |
| `the_river_line` | +89.3 | 51.4 | 1.74 | 0.0331 | 37/20 |
| `lake_trasimene` | +114.5 | 57.9 | 1.98 | 0.2203 | 32/22 |
| `the_breakthrough` | +26.0 | 39.8 | 0.65 | 0.7948 | 31/28 |
| `hammer_and_anvil` | +176.2 | 35.4 | 4.97 | 0.0005 | 42/15 |
| `the_golden_bridge` | +5.8 | 27.9 | 0.21 | 0.7877 | 26/29 |
| ALL POSITIONS | +49.9 | 12.9 | 3.86 | 0.0041 | 334/263 |

The pooled row is the caveat above, demonstrated: `+49.9` on a run containing a
`+176` and a `−189`. `the_ridge` is the other caveat: a `+118.8` mean beside a
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
forcing concentration would fix it. Any follow-up needs a narrowly scoped
treatment that prices local force ratio, run through this harness and then
through `battle_bench` and `tools/tactics_bench.py`, because a gain here is a
necessary condition for the whole-game gate and never a substitute for it.

### The arena separates two controllers a win rate calls a coin flip

`docs/closed/TACTICS_BASELINE.md` warns that `advanced_v1` — a frozen copy of the live
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

### The signature: `advanced` fights better than it manoeuvres

With `arrival` in the profile, the reserve result stops being a fact about one
position and becomes a fact about the agent. Comparing `advanced` and `basic`
**in the same role on the same position**, 60 seeds, arrival spread in turns:

| position | role 0 (adv / basic) | role 1 (adv / basic) |
| --- | --- | --- |
| `central_position` | 6.27 / 4.55 | 6.82 / 6.89 |
| `oblique_order` | 2.79 / 1.72 | 2.27 / 1.72 |
| `the_defile` | 2.12 / 0.96 | 4.24 / 7.45 |
| `the_ridge` | 3.54 / 2.25 | 4.02 / 2.88 |
| `double_envelopment` | 3.74 / 3.99 | 2.68 / 2.25 |
| `the_reserve` | 5.06 / 2.79 | 6.46 / 3.68 |
| `the_river_line` | 6.46 / 4.70 | 5.15 / 3.69 |
| `lake_trasimene` | 2.06 / 1.56 | 1.91 / 0.74 |
| `the_breakthrough` | 5.50 / 3.46 | 3.67 / 2.04 |
| `hammer_and_anvil` | 4.54 / 1.97 | 3.90 / 2.39 |

**`advanced` arrives more spread out than `basic` in 17 of 20 cells**, often by
a factor of two, and the same holds for `screen`: it leaves its ranged units
unscreened in nearly every cell (`hammer_and_anvil` role 1: 31% against 65%).

It nonetheless wins most positions, and the profile says why that is not a
contradiction — its `focus` is higher almost everywhere (100% against 94% on
`the_breakthrough` role 0, 100% against 96% on `hammer_and_anvil` role 1). It
trades better once engaged and assembles worse getting there. `the_reserve` is
simply the position built to charge for the second thing, which is why it is the
one it loses.

That is a sharper target than "improve tactics": the lever is in how the army
*closes*, not in how it fights once it has. A candidate that addresses it still
has to be built and run through the gates in order.

### Does arriving together actually cause winning? Mostly not.

```bash
cargo run --release --features developer-tools --bin doctrine_arena -- --correlate --a advanced --b basic --seeds 120
```

Per seed, how much more of its force each agent had up at first contact than the
other, against how much material it went on to win by. 120 seeds a position, run
on two independent agent pairs:

| position | r (adv/basic) | p | r (adv/adv_v1) | p |
| --- | --- | --- | --- | --- |
| **`central_position`** | **+0.30** | **0.0007** | **+0.25** | **0.0042** |
| `oblique_order` | +0.17 | 0.0594 | +0.18 | 0.0500 |
| `the_ridge` | +0.04 | 0.7000 | +0.24 | 0.0081 |
| `the_defile` | +0.05 | 0.5506 | +0.19 | 0.0359 |
| `double_envelopment` | +0.13 | 0.1621 | +0.14 | 0.1333 |
| **`the_reserve`** | **−0.06** | **0.5424** | **+0.04** | **0.6426** |
| `the_river_line` | +0.05 | 0.5943 | +0.16 | 0.0779 |
| `lake_trasimene` | +0.10 | 0.2552 | −0.02 | 0.8086 |
| `the_breakthrough` | +0.12 | 0.1977 | −0.09 | 0.3358 |
| `hammer_and_anvil` | +0.03 | 0.7058 | +0.08 | 0.3577 |
| `the_golden_bridge` | +0.05 | 0.6146 | +0.07 | 0.4416 |
| ALL POSITIONS | +0.07 | 0.0108 | +0.14 | 0.0000 |

**Read `r`, not `p`, on the pooled row.** At n = 1320 a correlation of 0.07 is
"significant" and explains half a percent of the variance. The pooled p is a
statement about sample size, not about the game.

Two things replicate across both pairs, and they point in opposite directions.

**`central_position` is real** — r ≈ +0.25 to +0.30, p < 0.005 on both pairs,
and a slope near +1000 material per whole extra share of the force up at first
contact. It is also exactly the position where the doctrine says it should be:
the enemy arrives as two columns that have to unite, so how much of your army is
present when the first of them reaches you is the entire question. Bonaparte's
own position is the one that rewards Bonaparte's own maxim, and the arena found
that without being told.

**`the_reserve` is a null** — −0.06 and +0.04, both p > 0.5. And that is the
position the whole arrival lane was built on.

### What that costs the lane, stated plainly

`advanced` arrives over roughly twice the span `basic` does, the effect is in
the foot and not the cavalry, and `advanced` loses `the_reserve`. All of that
stands. What does **not** stand is the step from there to "so make it arrive
together and it will stop losing `the_reserve`": the between-agent difference
does not reproduce as a within-agent relationship on that position, on either
pair.

So the arrival lane is **real but narrow** — a lever on the converging-columns
problem specifically, and not on the position that motivated it. Whatever costs
`advanced` `the_reserve`, the evidence does not say it is the arrival spread.
The remaining candidate visible in the profile is `screen`, where `advanced`
runs 25–32% against `basic`'s 39–50% on that position; it is not causally clean
the way `vanguard` is, so it needs the same treatment this section gave arrival
before anyone builds against it.

This check cost one afternoon and was run **before** any treatment was written.
That ordering is the point. Historic experiments show what the alternative
costs: an adjacent-friendly-support term built on exactly this kind of reasoning
was swept and could not be distinguished from zero at any weight, and #1399
records a local-superiority brake that had to be removed because both armies
braked at the edge of enemy reach and the battle deadlocked.

### The confound, tested and dead

The arrival result had an obvious alternative explanation: `advanced` might
simply push its cavalry forward, and a horseman arriving five turns before the
infantry produces a wide spread no matter how well the infantry is handled.
`foot` — the same spread over two-move units alone — settles it. 60 seeds, same
role, same position, `advanced` / `basic`, all-units arrival then foot-only:

| position | role | arrival | foot | gap (all) | gap (foot) |
| --- | --- | --- | --- | --- | --- |
| `the_reserve` | 1 | 6.46 / 3.68 | 7.02 / 3.98 | 2.78 | **3.04** |
| `hammer_and_anvil` | 0 | 4.54 / 1.97 | 4.73 / 1.93 | 2.57 | **2.80** |
| `the_river_line` | 0 | 6.46 / 4.70 | 6.81 / 4.91 | 1.76 | **1.90** |
| `the_reserve` | 0 | 5.06 / 2.79 | 5.41 / 2.93 | 2.27 | **2.48** |
| `the_ridge` | 1 | 4.02 / 2.88 | 4.23 / 2.75 | 1.14 | **1.48** |

`hammer_and_anvil` role 0 is the decisive cell, because it is the position built
around fast units — a line that holds and two horsemen sent wide. If the cavalry
were the explanation, that is where the gap should collapse when they are
removed. It **widens**, from 2.57 to 2.80, and the same happens in every other
cell above: taking the horse out makes `advanced` look *worse*, not better.

**The line is what comes up piecemeal.** The cavalry was, if anything, masking
the effect.

### The other thing `absent` found

Two cells report a quarter of an army never reaching the enemy at all:

| position | role | `advanced` absent | `basic` absent | swing |
| --- | --- | --- | --- | --- |
| `the_defile` | 1 (the column, seven units, one way in) | **24%** | 3% | +131 / +108 |
| `the_golden_bridge` | 1 (the encircling, seven units, one mouth) | **24%** | 2% | +150 / +148 |

Both are the large force attacking into a narrow mouth, and in both `advanced`
holds roughly 1.7 of its 7 units out of the battle entirely where `basic`
commits everything. Worth being careful here: this is **not** obviously a
defect. Not feeding a column into a defile one unit at a time is the correct
answer to that position, and `advanced` wins both roles. But it is a large,
specific, previously invisible behaviour, and it is the sort of thing that is
right on a defile and badly wrong in the open — which is exactly why the column
is reported per position rather than pooled.

### `advanced` spreads its fire against a dense mass

On `double_envelopment` role 0 — nine cheap units in depth — `advanced` puts
34% of its damage on units that died. Against the same position `basic` manages
22%, and both roles lose the position heavily, so this is the position being
hard rather than one agent failing it. But an agent spending two-thirds of its
damage on units that survive to heal is a lever, and `focus` is the column that
found it.

## Captured engagements, healing, and genes in the arena (2026-08-26)

Three additions, each answering one of the limits stated below.

### The positions are a curriculum; a captured file is the distribution

```bash
cargo run --release --features developer-tools --bin doctrine_arena -- --capture --games 24 --out target/engagements.json
cargo run --release --features developer-tools --bin doctrine_arena -- --engagements target/engagements.json --list
cargo run --release --features developer-tools --bin doctrine_arena -- --engagements target/engagements.json \
    --a advanced --b advanced --seeds 12          # the control, on the file too
```

`--capture` plays whole games (`--players 4 --width 60 --height 38 --turns
150` by default, the deployed controller in every seat, the barbarian seat
included) and takes every engagement they produce: the board at the moment two
armies at war first come within two tiles of each other — the contact zone's
own threshold — as every tile within a `--radius 7` window with its hills,
cover, water, mountains and **river crossings**, and both sides' land units as
they stood, **hit points and promotions included**. One capture per pair of
players every `--cooldown 25` turns; a long war is several engagements. Each
board is read over `--window-turns 16`.

The file plays back with `--engagements` exactly like the hand-built
positions — paired, roles swapped, the same ledger and profile — and `--list`
describes it the same way. The difference is what the pooled row means. The
positions were chosen to pose eleven different problems, so their pooled row
is not a number; a captured file is a sample of the fights the controller
actually gets into, on the ground it gets into them on, with the army it
brought, and there the pooled row is the number.

What a captured board loses, stated so it is not read as the whole game:
cities (an arena founds none, and a siege is its own instrument), third
parties (only the two armies are seated), improvements and resources (an
arena has no economy), and everything outside the window. What it keeps that
a hand-built position never had: real rivers (`Engagement::rivers` — a
position fakes a river line with coast, and the +5 crossing penalty is a term
the fighting turns on), wounded units, and earned promotions.

Format: `Engagement` in `src/doctrine.rs`, one JSON array, written by
`Engagement::to_json` and read by `Engagement::from_json`, which refuses a
unit the ruleset lacks or a cell off the board. Every hand-built `Position`
converts to one (`Engagement::curriculum()`), and
`a_converted_position_builds_the_same_board_as_the_position` holds the
conversion to the tile and the hit point, so every arena number recorded
before this change is comparable with every one after it. The odd-row
offset layout is kept by moving a window's rows by an even count only;
`a_captured_engagement_keeps_the_contacts_geometry_and_state` asserts every
pairwise distance survives the capture.

### Healing is the board's question

`--heal` turns recovery on for every board of a run; the same switch is
`TacticsRules::heal` and `--tactics-heal` on `civvis soak`. Off is the arena's
rule since the mode existed: one engagement, permanent damage, a trade you win
stays won. On is a campaign: the neutral rate everywhere on a board with no
territory, and the recovery step runs, so "pull the wounded unit back and put
a fresh one in its place" can be measured at all. A doctrine that preserves
units cannot be priced with healing off — a unit that steps back is simply
absent — and the live seat's 0.18 kills per loss is a campaign number. Read
both. They are different questions, and a board says which it is asking.

### A seat can carry genes

```bash
cargo run --release --features developer-tools --bin doctrine_arena -- --a advanced+close-as-a-body --b advanced --seeds 60
cargo run --release --features developer-tools --bin battle_bench -- --a advanced+fire-plan --b advanced --games 200
```

A seat is a built-in name or `advanced+<gene>+<gene>` (`civvis::elo::seat_spec`,
`seat_ai`); `battle_bench` takes the same form. An unknown gene is refused by
name rather than played as the control, because an arm that silently became
the control would report a null indistinguishable from a real one. The `fires`
column says whether the gene changed the play at all.

### The gate for a tactical gene

Recorded because the alternative culled a controller that won 99.6 % of its
arena fights. The 35,148-seat whole-game screen read `joint-tactics` at
−0.104 pp and removed it, and `docs/TACTICS.md` §7 records the same shape twice
before: an 8.5× swing in measured combat quality, at 300 maps, and no movement
in the win rate. A whole game resolves about fourteen kills and a 100-cell
paired run resolves a 70/30 split. That instrument cannot see a tactical
change, so it cannot be its gate. A tactical gene is read on:

1. `battle_bench`, control then treatment — the paired swing, and `fires` > 0;
2. `doctrine_arena` on the curriculum (read the rows, not the pool) and on a
   captured file, healing off and healing on — the pooled swing on the file,
   kills per loss, and the profile columns the gene claims to move;
3. the live ledger's kills per loss and captures, once it has run there.

The whole-game screen is then a **no-harm** check: a tactical gene that reads
clearly worse there is stopped, and one that reads null is not culled for it.

### The arena can pose a siege (2026-08-27)

A `Position` may now state a **city** — whose it is, where it stands, its hit
points and its wall pool — and a captured `Engagement` carries the city the
fight was over. Until this, the arena could pose every tactical problem
except the one the live seat keeps losing: eleven declared wars, four sieges
reduced to 180-190 of 200, and no capture. An assault is a different problem
from a field fight, and a board that dropped the city could not pose it.

```rust
cities: &[city(/* role */ 1, /* col,row */ 17, 6, /* hp */ 200, /* wall */ 100)],
```

`build_engagement` places them with `Game::place_city` — the arena's own
path; the `FoundCity` *action* stays refused on a battlefield, because a
position states what is held rather than letting a settler decide — and
overrides the wall pool through `observed_city_max_wall_hp`, the same field
the live mirror uses for the host's number, so a walled position is walled
and an unwalled one cannot grow a wall from a ruleset default. The muster
will not seat a unit on a city tile. `capture_engagement` takes every city
inside the window with its hit points and walls, so a real siege replays as a
board. `DoctrineLedger` gains `cities_taken` / `cities_lost`, scored from a
change of owner between two readings and reported as a `cities` line.

**`the_storming`** is the first assault position: a 200-hit-point city behind
**100 points of wall** with three defenders, against two catapults, three
melee and an archer — 520 material against 165. Walls are an outer pool a
melee unit cannot capture through, so the assault has an order: break the
wall, then take the city, and a body that storms an unbreached wall is spent
for nothing.

**What it found, immediately.** The control is exact — `advanced` against
itself is `+0.0` with 0 of 12 seeds diverging and captures symmetric at
`+1/-1` each way. Against `basic`, `advanced` **loses the position**:
**−200.8 ± 102.5 a seed (t −1.96, sign p 0.024, 12/27)**, at 0.77 kills per
loss against 1.31. It is the second position `advanced` loses, after
`the_reserve`. And the captures column reproduces the live record in
thirty-four turns instead of two hundred and fifty: over forty assaults
`advanced` took the city **three times**, `basic` none.

The profile names the failure, and it is not the walls. On `the_storming` the
besieger's **arrival spread is 10.91 turns** — the worst figure on any board
in the arena, against 1.2–8.1 everywhere else — with `contact` at 70 %, the
lowest anywhere. A 520-material siege train is not beaten by a 165-material
garrison; it is fed to it over eleven turns.

That is a testable claim, and the two genes that might have answered it do
not, yet: `close-as-a-body` reads **+37.5 ± 30.3 (4/1)** — the right sign,
but it fires on only 6 of 40 seeds, because it acts on an `Advance` posture
out of contact and this board reaches contact quickly — and `fire-plan`
reads **−53.5 ± 36.1**. Neither is significant. The assault is a problem the
current controller does not solve and the current genes do not address, and
now it is a thirty-four-turn board rather than a two-hundred-and-fifty-turn
game.

### The instrument says how units are being lost (2026-08-27)

`salvag.` is the share of a side's losses that were **already at or below 30
hit points the turn before they died** — losses the controller had a turn's
warning of and could have answered by rotating, withdrawing or healing. The
rest were killed from a health it had no reason to act on. `DoctrineLedger`
counts them (`losses_when_salvageable`), the profile reports the share, and
`tools/civ6_tactics_ledger.py` computes the same figure on a live run
(`lost_when_salvageable` / `salvageable_share`, threshold shared as
`SALVAGEABLE_HP`), so an arena number and a live number are the same number.

Thirty rather than forty-five: below `withdraw_hp`, the line the controller's
own recovery already uses, so the column counts units it had *already been
told* to pull out and did not — not every unit that happened to be scratched.

The distinction only became worth making when a board could heal (`--heal`).
With nothing to recover to, "should have pulled it out" was not a real
alternative — which is why `swap-rotation` measured the same with healing on
and off.

**What it says about the deployed controller** (`--profile advanced`, 14
seeds): on three of four positions the **winning** side has the *higher*
share — `central_position` 50 % against the loser's 26 %, `the_defile` 71 %
against 26 %, `the_reserve` 39 % against 28 %. That is the expected shape: a
side that is winning loses units it has worn down, and a side that is losing
has them killed outright.

`the_storming` inverts it, and that is the finding. The besieger **loses the
position by 291 a seed and its losses are 52 % salvageable** — it is worn
down and then killed wounded, rather than overwhelmed. Beside an arrival
spread of 7–11 turns, that is one story: the siege train arrives piecemeal,
is ground down at the wall, and dies there instead of being pulled out.

**The obvious answer does not work, and the column is how we know.**
`swap-rotation` is the gene for pulling a wounded unit out of a line, and on
`the_storming` it reads **−9.8 ± 33.8**, fires on only 9 of 40 seeds, and
moves the salvageable share not at all (50 % against 48 %). The reason is in
its own predicate: the relief must stand *further from the enemy* than the
unit it replaces, and on a ring around a city every besieger is about
equally far from it. A rotation measured against **the city** rather than the
nearest enemy is a different gene, and this board can now price it.

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
