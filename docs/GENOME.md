# The genome, and why breeding it has not worked

`docs/SUPERHUMAN.md` is about the macro search. This is about the other half:
the 48-gene `Weights` vector that `civvis evolve` and `civvis league` search,
and which about a thousand rounds of live evolution moved without producing a
measurable gain (`docs/RATING.md`).

Three causes are now on record. The first was known. The second and third are
measured here.

---

## 1. Selection had no signal

`docs/RATING.md`: the deployed Glicko-2 ratings scored **−0.025 nats/game**,
worse than guessing, so evolution selected on noise. Fixed by `civvis rating`.

## 2. About a quarter of the genome cannot change a game

`src/bin/gene_probe.rs` drives each gene to **both ends of its own bounds**
and plays the same map against the same opponents with `AdvancedAi` — the
agent that plays every evaluated game — comparing the seat turn by turn on
cities, units, techs, civics, policies, score, gold and wars. Divergence
proves causally that a gene bites.

**11 of 48 genes produced zero divergence** over 12 trials at 4p/200 turns:

| block | genes |
|---|---|
| war declaration | `war_ratio`, `war_margin`, `peace_ratio`, `war_min_turn` |
| settle site | `settle_food`, `settle_prod`, `settle_gold`, `settle_dist` |
| other | `settler_stop_turn`, `faith_builder` |

Two coherent blocks of four is the signature of a bypassed subsystem, not
eleven coincidences. Every consumer of the war block is in `impl BasicAi`,
while `AdvancedAi` has its own `DeclareWar` path and does not delegate. The
settle block is subtler and the distinction matters: those genes *are* read by
`BasicAi::settle_value`, which `AdvancedAi` can reach, but it normally uses
its own `settle_value` with hard-coded ring weights — so they are either
unreached, or reached with a site argmax insensitive to reweighting.

The loud end of the same table, for contrast: `open0`, `mv_support`,
`withdraw_hp`, `rejoin_hp`, `pol_production`, `pol_gold` and `pol_faith` all
bite **12/12**, first divergence between turns 8 and 32.

**This mechanically explains a recorded null.** `StrategicAi`'s `Doctrine`
axis perturbs a genome per doctrine. Against the zero-divergence set:

| doctrine | levers | dead |
|---|---|---|
| `Militarize` | 6 | **3** |
| `Consolidate` | 5 | 2 |
| `Expand` | 5 | 1 |

`docs/SUPERHUMAN.md` records Doctrine as 0 switches in 16 reviews and 14/14
neutral maps, with no mechanism offered. Half the war doctrine's levers move
genes the playing agent never consults.

**Silence is not proof of inertness**, and the tool never says "inert". A
gene that only acts in a game reaching a war, a city count or an era those
maps never reach reads quiet for want of an occasion. Raise `--maps` and
`--turns` before concluding anything about one gene; `--only <substring>`
makes checking a single gene a one-minute question instead of a forty-minute
one.

## 3. Whole decision layers have no genes at all

Policy cards, technology order, civic order and any notion of a timed
military buildout are chosen by hand-written code with no genome exposure.
Evolution cannot breed what it cannot reach.

---

## What was tried against this, and what happened

### Policy cards — closed, negative

The AI played an Ancient-era deck for the whole game: `POLICY_PRIORITY` names
twenty cards of a 125-card catalogue, in fixed order, tried only while a slot
stood empty, identical for every civ and every victory lane. One entry,
`meritocracy`, is not in the ruleset at all. Measured over 64 seat-games, an
average seat unlocked **42.0** cards and played **7.3**.

Replacing it with a counterfactual valuation — slot the card, read the empire
either side, so nothing names an effect key and all 125 cards plus mod cards
are covered — raised distinct cards per seat to 11.06 and occupancy to 94.3%.

It bought nothing, three independent ways:

| approach | result |
|---|---|
| valuation vs the legacy list | 18 map directions to 15, p=0.7283 |
| hand-set appetites | 0.4842, below parity |
| GA-bred appetites, 5 generations | +0.0138 ± 0.0138 (1.0 SE) |

…while the layer itself is worth a great deal: the legacy list against
**holding no cards at all** is 23 map directions to 6, **p=0.0023**.

**Conclusion: the shipped twenty already capture essentially all the value the
card layer offers.** Do not reopen without a new mechanism — card
interactions, or lane-aware decks, not another appetite vector.

### The opening book — live, and worth nothing

The book is four capital builds indexed into a six-entry menu. `gene_probe`
put it at the top of the bite table: `open0` diverges **12/12** with mean first
divergence at **turn 8**, harder and earlier than any other gene. So it looked
like the best place in the genome for a search to work.

`src/bin/opening_sweep.rs` swept it — coordinate descent over 4 slots x 7
options, every cell with an interval. Each slot's winner looked ahead on the
maps that chose it:

| slot | winner | edge on its own maps |
|---|---|---|
| 0 | slinger | +0.0303 |
| 1 | monument | +0.0034 |
| 2 | builder (shipped) | +0.0435 |
| 3 | scout | +0.0410 |

Assembled, that predicts roughly +0.05 to +0.10. On **48 disjoint maps** the
assembled book `slinger -> monument -> builder -> scout` scored
**-0.0019 +/- 0.0148 (-0.1 SE)**. Pure selection bias; four maxima drawn from
28 cells whose SE is 0.03.

Then the ablation that should have run *first*: setting all four slots past the
menu, so nothing is scripted and every build is evaluated normally, scores
**0.4972 +/- 0.0164, -0.2 SE**. **Deleting the opening book entirely costs
nothing measurable.**

### ⚠ Reachability is not leverage

That result corrects the reasoning that chose the book in the first place.
`gene_probe` measures whether a gene **changes the game**. It does not measure
whether the gene changes the **outcome**. The opening book is the most
reachable block in the genome and is worth zero, so:

> A gene can diverge 12/12 at turn 8 and still be worthless. Divergence is a
> necessary condition for a gene to matter, never a sufficient one.

Use `gene_probe` to *exclude* — a gene that cannot act certainly cannot help.
Never use it to rank what to work on. Rank by an **ablation**: what does this
subsystem cost when removed?

### The leverage ranking — one block of eight is load-bearing

`src/bin/gene_leverage.rs` replaces each block of related genes with uniform
draws from its own bounds and plays it against the shipped agent, paired and
seat-mirrored, over three draws. Cost is `0.5 − scrambled score`:

| block | genes | cost | verdict |
|---|---|---|---|
| **economy** | 7 | **+0.0193 ± 0.0060** | **3.2 SE — load-bearing** |
| combat_value | 3 | +0.0158 ± 0.0233 | settled |
| opening | 4 | +0.0091 ± 0.0078 | settled |
| war_decl | 5 | +0.0040 ± 0.0040 | settled |
| policy | 8 | +0.0037 ± 0.0145 | settled |
| doctrine | 11 | +0.0019 ± 0.0165 | settled |
| movement | 2 | +0.0015 ± 0.0140 | settled |
| **expansion** | 4 | **−0.0305 ± 0.0166** | 1.8 SE — scrambling *helped* |

**Only `economy` carries anything.** Everything else can be replaced with
uniform noise for free.

The eleven combat-**doctrine** genes deserve their own line. That is the
largest block in the genome and the one this repository has spent the most
design effort on, and scrambling all eleven costs **−0.0019**. It agrees with
the standing finding that this agent's wars resolve nothing: the doctrine is
well-built machinery bolted to a subsystem that never converts.

### `city_target` saturates above six

The expansion block was the only one where randomising *beat* the shipped
values, so the obvious suspect was `city_target = 4` against a 2..12 range
whose random mean is 7 — the AI under-expanding. Swept at 20 mirrored maps a
point:

| value | score | edge |
|---|---|---|
| 2 | 0.4899 ± 0.0152 | −0.0101 |
| 4 (shipped) | 0.5000 ± 0.0000 | — |
| 6 | 0.5034 ± 0.0043 | +0.0034 |
| 8 | 0.5035 ± 0.0043 | +0.0035 |
| 10 | 0.5035 ± 0.0043 | +0.0035 |
| 12 | 0.5035 ± 0.0043 | +0.0035 |

Six, eight, ten and twelve are **identical to four decimal places**, which
says the cap stops binding at about six: the agent never gets that many cities
anyway, so raising the ceiling changes nothing. Something other than the
target limits expansion.

And the magnitudes do not work. The block effect is +0.0305; the largest
single-gene effect here is +0.0035, a tenth of it, at 0.8 SE. **So
`city_target` is not what makes randomised expansion beat the shipped values**
— `settler_min_pop` or `min_city_dist` carries it. Stopping at the block
ablation would have produced a confident and wrong write-up.

### …and neither expansion gene carries the block either

`min_city_dist` swept at 20 mirrored maps a point: 3 → −0.0119, **4 (shipped)
→ best**, 5 → −0.0042, 6 → −0.0160, 7 → −0.0176. Every alternative is worse.
The shipped value is already the optimum of its own range.

`settler_min_pop`: 1 → −0.0030, **2 (shipped)**, 3 → +0.0051, 4 → **−0.0256**,
5 → **+0.0283**. That swings 0.054 between two *adjacent* values of what is
essentially a monotone threshold. A real response surface does not do that;
noise does.

So the expansion block's +0.0305 is carried by no single gene: `city_target`
saturates and is worth a tenth of it, `min_city_dist` says shipped is optimal,
and `settler_min_pop` is erratic. The block reading was itself 1.8 SE on three
draws. **The most economical explanation is that the block-level effect was
noise**, which the decomposition is what revealed.

That is the third time in this sequence that a block- or sweep-level signal
dissolved under decomposition or resampling. The pattern is stable enough to
state as a property of this evaluator: **at 16–20 mirrored maps, effects below
about 0.03 cannot be distinguished from noise, and the only defence is
replication on disjoint maps.**

### War timing — hypothesis refuted

`src/bin/war_census.rs`, 53 wars over 24 maps at 6p/500 turns: **98% of wars
open with the army already in position**, peak only 8.1 turns later. The AI
does not declare first and build after.

What it does do is open wars at a mean **11.5× military advantage** — it
fights only walkovers. Making that threshold genetic did **not** work
(`adv_war_ratio` diverged 1/16, `adv_war_margin` 0/16) and was reverted, for a
reason visible in the motivating number: the gate is
`close_enough && ready && staged`, and at 11.5× the `ready` test is satisfied
**8.7× over**, so it was never the binding conjunct. **A subsystem's stated
threshold is not necessarily its binding one.** The real target is
`campaign_staged_for_war`.

---

## The conclusion this all points at

Every measured attempt to make this agent stronger by **tuning parameters** has
returned null: the policy appetites three ways, the opening book two ways, the
war-declaration threshold, and about a thousand rounds of whole-genome
evolution. Meanwhile every promoted gain in the repository has come from
**giving the search more counterfactual rollout** — `strategic_deep` at +45
Elo, warm branches at +37.

Taken with `docs/SUPERHUMAN.md` §0, which reaches the same verdict about
learned state-value components, the pattern is hard to miss:

> **Rollouts win. Regression on outcomes does not, and neither does parameter
> tuning.** The `Weights` genome is not where superhuman strength is going to
> come from, and a search over it should not be the main line of work.

The remaining headroom identified but not taken: `campaign_staged_for_war`,
the binding conjunct on war declaration, which is force coordination and the
reason this agent fights only 11.5x walkovers.

## Method rules these runs paid for

0. **Bound a subsystem by ablation before optimising inside it, and rank work
   by ablation rather than by reachability.** Both closed threads followed the
   same arc — a promising-looking gap, a mechanism, a null — and in both cases
   the ablation was the number that made the null interpretable. For policy
   cards it said the layer is worth a great deal and the incumbent already
   captures it; for the opening book it said the block is worth nothing at all.
1. **Compute the standard error of a fitness before spending compute on it.**
   A win rate over 24 games has SE 0.102, while the largest effects this
   repository has measured are +0.053 and +0.065. A breeder built on it
   produced 0.500, 0.542, 0.500 — a random walk that looked like a search.
   Selection now reads `0.8 * score share + 0.2 * win rate`, and score
   *selects* while a win-based run *decides*.
2. **Bound a subsystem before optimising inside it.** A null on selection is
   uninterpretable without the ceiling beside it; the card-layer ablation is
   what turned "cards don't matter" into "the incumbent list is already good".
3. **Never write the interpretation into the instrument.** `war_census`
   originally closed with a conclusion composed before any data existed; the
   first run refuted it and the canned text would have reported the opposite
   of the measurement. It now branches on what was measured.
4. **Prefer a sweep to a search where the space is small and discrete.** The
   opening book is 7⁴ books; coordinate descent over 28 cells returns a table
   rather than a champion, and every cell carries an interval.
5. **A degenerate control is a real check.** Identical arms play the same game
   in both mirrored directions, so they must return exactly parity with zero
   variance. Both `policy_eval` and `opening_sweep` reproduce that, which
   proves determinism and zero harness noise — and proves nothing about the
   null distribution for arms that genuinely differ.
