# the fog-honest arm re-screened and a version that lost

_2026-08-24 · `805c2a9b0e90`_

## What was asked

Two questions. First, whether the end-to-end fair-play controller
(`AdvancedAi::fog_honest()`) is still as weak as its only recorded screen said —
15.0% paired score over 20 maps, 95% Wilson 5.2%..36.0%, an interval wide
enough to contain parity — now that `ai_eval`, the instrument that produced
that number, has been deleted (#2351). Second, whether one named repair to the
fog-honest turn improves it.

## How it was measured

**Instrument.** `fog_honest` and `fog_honest_2` were added to the gene registry
(`src/ai/advanced/genes.rs`) as a versioned `Kind::OptIn` family, which is what
made the arm reachable by the surviving instrument at all. `gene_screen` draws
a family as one level — off, or exactly one version — so the three arms are
disjoint by construction and every contrast is clustered by game.

**Shape.** The standard screen and nothing else: six majors, 74×46 continents,
nine city-states, Online, 250 turns, all six lanes, shuffled civs. Command:

```sh
target/ci/gene_screen --games 240 --jobs 4 --genes fog-honest,fog-honest-2 \
  --start-seed 955000000 --out target/fog-screen.jsonl
target/ci/gene_screen --analyze target/fog-screen.jsonl \
  --json docs/gene_screens/2026-08-24-fog-honest-family-direct-6p-allseats-414-seats.json
```

⚠ **The batch is a declared partial: 414 of 1,440 pre-registered seats
(28.8%), 69 of 240 games, seeds `955000000..955000068` of a reserved
`955000000..955000239`.** It was cut deliberately: the host was carrying about
fourteen concurrent agents at load 130–150, where a 240-game batch of this arm
projects to roughly eight hours. The artifact declares the truncation on its
own first line and the ledger prints it as `PARTIAL`.

The two supporting readings came from `experiments/closed/fog_planning.rs`, at
the same shape: a matched-state decision diff (24 games, seeds
`930000..930023`, 334 decision points where the world *and* the controller are
cloned and one clone is given `fog_honest`), and a paired whole-game census
(12 games, seeds `940000..940011`).

## What it measured

At 80% power this run resolves a win Δ of ±8.1 pp and a share Δ of ±2.41 pp;
the family-wise 5% bar for two genes is |z| ≥ 2.24.

| level | seats | win | score share |
|---|---:|---:|---:|
| off (stock `advanced`) | 200 | 30.5% | 20.7% |
| `fog-honest` | 106 | **6.6%** | 14.1% |
| `fog-honest-2` | 108 | **0.9%** | 11.7% |

| contrast | win Δ | share Δ |
|---|---:|---:|
| `fog-honest` − off | **−23.9 pp ± 3.1 (z −7.83)** | −6.61 pp (z −7.31) |
| `fog-honest-2` − off | −29.6 pp ± 1.8 (z −16.87) | −9.05 pp (z −13.41) |
| `fog-honest-2` − `fog-honest` | **−5.7 pp ± 2.5 (z −2.25)** | −2.44 pp (z −3.10) |

Compute cost per enabled major seat: `fog-honest` +3.50 ± 6.08%,
`fog-honest-2` −0.15 ± 5.82%.

**The replay boundary** (12 paired games, 2,371 fog-honest turns, 75,483
planned actions): 97.0% of planned actions accepted, 3.05% refused, at least
one refusal on **37.8% of turns**. By kind: `trade` 68/101 = **67.3%**,
`produce` 619/2,490 = **24.9%**, `found_city` 10/70 = 14.3%, `move`
1,569/56,538 = 2.8%, `attack` 4/1,025 = 0.4%. Outcomes over the same pairs:
score share −0.033 ± 0.014, cities −1.33 ± 0.75, techs −11.25 ± 6.02.

**The matched-state diff** (334 decision points, 280 divergent): at identical
states the fog-honest controller's `produce` volume is −4.8%, `improve` −0.7%,
`attack` +0.6%, `move` −1.6%, while `slot_policy` is +28.8% and `unslot_policy`
+26.3%. First divergence by kind: `move` 164, `produce` 43, rest ≤ 9.

## What was decided

**`fog-honest`: not promoted, and no longer merely unresolved.** The old 15.0%
and this 6.6% are different statistics on different instruments — a paired-map
score against a seat win rate in a random-genome batch with a 16.7% chance
baseline — and must not be subtracted. What they agree on is direction, and
this one adds resolution the old one lacked: the arm is excluded from parity at
z −7.83, past the family-wise bar. Stock `advanced` keeps the strength gate for
the third time, now with a stated size.

**`fog-honest-2`: refused by its own family table.** The rule is that an
improvement improves when its contrast against the version before it is
positive on the win axis and it also beats off. It is negative on both axes.
It stays in the pool, off and unmeasured, rather than being deleted, because
the mechanism it establishes is worth more than the branch.

That mechanism: to reach a re-plan at all, version 2 must stop replaying at the
first refusal, since the tape's own trailing `EndTurn` would otherwise close
the turn. That treats an action tape as one dependent chain when it is many
actors' independent plans concatenated — a refused Settler move says nothing
about a Builder's `improve` twelve actions later. With refusals on 37.8% of
turns and one re-plan to recover with, it discards more good orders than it
replaces, and its flat compute column (−0.15%) confirms the turns are ending
early rather than planning twice.

**What is worth building next is not another reaction to the refusals but a fix
for them.** One production order in four and two trades in three are refused,
and production and trade legality is a fact about the seat's *own* empire,
which the redacted planning world has no reason to get wrong. That is an
execution defect in the economic layer — the shape this repository's ledger
says pays — and it is unexamined. The named third version (skip only the
refused actor's remaining actions, keep everyone else's, no early `EndTurn`, no
re-plan) is deliberately left unbuilt here: a version guessed from the
mechanism of a losing version needs its own row and its own screen.

★★★★ **Not entered as a ledger source, on purpose.** Recording it fails
`tools/test_genes.py::test_the_band_is_the_columns_own_scale_not_the_differences`,
and the test is right. The ranking's column is `(win_on − chance) × PER`, which
equals half the on−off difference only when the two arms are the same size — as
they are at `p = ½`. A versioned family splits its probability across versions,
so each version is on for a quarter of seats and off for three quarters, and
its off arm stops sitting at chance: here `win_on − chance` = −10.07 pp against
a half-difference of −6.77 pp, and `column / column_se` = −6.96 against a
`win_z` of −4.68. That is general to every versioned family. The artifact is
committed and the numbers are published; both genes stay `unmeasured` / off
until the ranking's column is fixed to be half the on−off difference. See
`docs/AI_GAPS.md`.

⚠ Limits. One seed stream, 28.8% of a declared batch, first reading for both
columns. The −23.9 pp is far outside this instrument's noise. The −5.7 pp
version contrast sits just past |z| 2 on the win axis and should be re-read on
disjoint seeds before the mechanism above is treated as established. A full
240-game batch resolves ±6.7 pp; about 440 games are needed for ±5 pp before
clustering.
