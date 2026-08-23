# The arms that were pre-registered and never run

_2026-08-23 · `1510b482`_

## What was asked

`docs/ROADMAP.md` objective 3 asks the shipped bundle to be priced by
withholding *before the next effect hides inside a composite the way
`city_target_floor` did* — that one hid −41 Elo. Several arms were registered,
pre-priced, and never run. This round runs the one that is load-bearing and
records the disposition of the rest, so the open ends stop being invisible.

The load-bearing one is **`advanced_governor_victory_lanes`, seed 29000000**.
`docs/eval/2026-08-18-pricing-the-governor-s-routing-and-the-settling-asymmetry.md`
closed with it named as "the open end": `advanced_every_lane` measured −95 at
the deployment profile, the Expansion half measured −20 compact / −16
deployment and was RETAINED, so **by subtraction the four victory lanes should
carry roughly −70 to −80**. Two later lines agree with that prediction from
completely different instruments, which is what makes the third leg worth
buying:

| line | reading |
|---|---|
| `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md` | `governor-victory-lanes` **−4.73 pp, win z −15.37** — worst of 99 genes, while the gene ships default ON |
| `docs/gene_ledger.json` | `verdict: unresolved`, **`conflict: true`**, `helps * · share HURTS **`: win z **+2.46** against share z **−15.918** |
| the sibling half | `governor-every-lane` reads share z −16.933 and is correctly OFF |

## PRE-REGISTRATION (written and committed before the run)

**Arm.** `advanced_governor_victory_lanes` vs `advanced`.

⚠ **It is an *enable* arm, not a withhold arm**, and the round that registered
it is explicit about why. `AdvancedAi::new()` has `governor_victory_lanes:
false`; the arm calls `enable_governor_victory_lanes()`. Its sibling
`advanced_governor_expansion_lane` has the same shape, and that is what makes
the subtraction legal: all three readings (`advanced_every_lane` −95, the
Expansion half −16, this arm) are the same *kind* of delta measured against the
same `advanced` control. A withhold arm would not subtract against them.

**Design.** `ai_eval advanced_governor_victory_lanes advanced --matrix --pairs
100 --jobs 4 --seed 29000000`. Fixed N, no `--stop-when-decisive`: the betting
interval is anytime-valid, but an early-stopped point estimate is selected on
having crossed, and `docs/EVAL_INTEGRITY.md` R3 is precisely the rule against
quoting a decision procedure as an estimator.

**Seed streams.** `--matrix` strides its profiles by `MATRIX_PROFILE_SEED_STRIDE`
= 1,000,000, so the registered seed 29000000 resolves to compact
[29000000, 29000099], deployment-online [30000000, 30000099],
deployment-contested [31000000, 31000099].

**Pairs: 100 intended, against the 400 the sibling arms ran — and this is a
declared deviation, not a stopping point discovered later.** Measured cost on
this machine at 2026-08-23 00:54–01:24, load average 72, twelve sibling agents
resident: a 3-profile matrix at 8 pairs took **23m21s wall and 2200 CPU-s**
(≈46 CPU-s/game) and returned only **157% CPU against 4 requested jobs**. That
is 2.92 wall-minutes per matrix pair, so the registered 400 pairs is ~19.4
hours of wall clock here and would not land. 100 pairs is ~4.9 hours and does.

**What 100 pairs can and cannot see.** The sibling arm resolved −20 compact at
p=0.0089 on 400 pairs, which implies SE ≈ 7.6 Elo there and therefore
SE ≈ 15 Elo at 100. Against a true −75 that is z ≈ −5, so this round can
comfortably separate "the victory lanes are the carrier" from "near zero" and
from the Expansion half's −16. It **cannot** put a tight interval around
−70 vs −80; a 95% interval at this N is roughly ±30 Elo wide on each side. The
run reports its own smallest-resolvable-edge line and that line, not this
paragraph, is the record.

**Third profile declared.** The 2026-08-18 round ran two profiles because
`deployment-contested` was added to `PROMOTION_PROFILES` the same day
(`ba5515d0`, #2042). Running `--matrix` today therefore produces a profile the
sibling arms never had. The verdict below is called on **compact and
deployment-online**, the round's own two columns; the contested profile is
reported beside them and is not comparable to either.

## Results

<!-- filled in after the run -->

## What was decided

<!-- filled in after the run -->
