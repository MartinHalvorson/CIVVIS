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

⚠ **Read that as economy, not as strength.** This fitness is
`0.8 · score share + 0.2 · win rate`, and the `settler_min_pop` result below
shows a 3.0 SE score-share gain converting to zero wins. So the ranking says
which genes move the **economy**, which is a necessary condition for mattering
and not a sufficient one — the same relationship divergence has to leverage,
one level up. Every `--sweep`, the opening-book sweep and the district
enumeration inherit the same caveat.

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

### Inside the load-bearing block: district *order* is settled

`economy` is the only block whose randomisation costs anything, and the
district priorities are its build-order half. All 24 orders were enumerated
(24 mirrored maps each; the shipped order is the degenerate control at
0.5000 ± 0.0000). Over the first fourteen, **not one order sits outside 2 SE**,
and the spread by leading district is small:

| first district | mean edge |
|---|---|
| holy | +0.0025 |
| commercial | +0.0013 |
| campus (shipped) | −0.0031 |
| theater | −0.0117 |

Leading with a Theatre Square is the only consistently bad choice, at about
1.6 SE. Nothing beats the shipped order.

**So the block's leverage is not the district ranking**, and the two probes
together say where it is. Divergence within the block:

| gene | bites |
|---|---|
| `builder_per_city` | **11/12** |
| `d_holy` | 5/12 |
| `wonder_min_bld` | 4/12 |
| `d_campus`, `d_theater` | 3/12 |
| `d_commercial` | 2/12 |
| `faith_builder` | **0/12** (dead) |

`builder_per_city` dominates its own block by a factor of two, and the
orderings are measured settled, so it is almost certainly what makes `economy`
load-bearing.

**This is the legitimate use of `gene_probe`.** It cannot rank *blocks* —
that mistake cost an iteration on the opening book — but inside a block
already shown load-bearing by ablation, divergence narrows which gene to
sweep. Ablation says where; divergence says which; a sweep says what value; a
disjoint re-measurement says whether to believe it.

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

### The one surviving lead, and why it still failed

`settler_min_pop = 5` — a city grows to pop 5 before building a Settler,
i.e. slower and taller expansion — is the only value in this whole sequence to
survive resampling. On **score share**, across four disjoint seeds:

| seed | maps | edge |
|---|---|---|
| 1200000 | 20 | +0.0283 ± 0.0187 |
| 1300000 | 20 | +0.0184 ± 0.0237 |
| 1600000 | 80 | +0.0174 ± 0.0069 |
| pooled | 120 | **+0.0187 ± 0.0062 (3.0 SE)** |
| 1700000 (below) | 120 | +0.011 |

Pre-registered before the run, decision rule fixed in advance: PASS requires
map directions FOR > AGAINST at sign p < 0.05, score reported beside it and
explicitly not part of the rule.

```
policy_eval --maps 120 --seed 1700000 --gene settler_min_pop --value 5
  decisive games   117/240 (48.8%)
  map directions   12 for / 15 against / 93 neutral
  sign test        p = 0.7011
  terminal score   51.1% of table
```

**NULL. The shipped value of 2 stands.**

And the failure mode is the informative part. The score-share effect
**replicated a fourth time** — 51.1% here against the pooled +0.0187. The
measurement was never wrong. **A 3.0 SE score-share gain converted to exactly
zero win improvement.**

### Every district order, enumerated

All 24 measured at 24 mirrored maps each. Best three:

| order | score | |
|---|---|---|
| commercial > theater > campus > holy | 0.5047 ± 0.0037 | 1.3 SE |
| holy > campus > theater > commercial | 0.5039 ± 0.0058 | 0.7 SE |
| commercial > campus > holy > theater | 0.5027 ± 0.0080 | 0.3 SE |

**Not one order is outside 2 SE.** The shipped `campus > commercial > holy >
theater` is not beatable, and leading with a Theatre Square (−0.0117) is the
only consistently bad choice.

## ★★ THE FINDING THAT MATTERS MOST: score share does not buy wins

`docs/EVAL.md` has long said wins and terminal score measure different things.
This is that claim demonstrated **prospectively**, under a rule fixed before
the data existed, on a change selected precisely because its score effect was
real and replicated.

It has a consequence for genetic search that is hard to escape:

| fitness | SE at 24 games | valid? |
|---|---|---|
| win rate | 0.102 | yes, and unaffordable — a 0.05 effect needs ~865 games *per genome* |
| score share | ~0.02 | affordable, and **provably does not convert** |

**A GA over this genome is caught between a proxy that is cheap but invalid
and a target that is valid but unaffordable.** That is a more fundamental
reason breeding has produced nothing here than either the rating bug or the
dead genes — those are fixable, and this is not, at any compute budget a
search can spend per genome.

It also indicts the fitness this session's own breeder used
(`0.8 · score + 0.2 · wins`): it is 80% weighted on a quantity now shown not
to convert. That breeder's null is therefore uninformative about the policy
appetites — it was selecting on the wrong thing, which is a fourth distinct
way to breed on noise.

## ★★★ Score is a CORRELATE of victory, not a cause — measured

The `settler_min_pop` null suggested score share does not convert to wins. Two
further experiments pinned down why, and the second refuted my own hypothesis.

**First, score is an excellent classifier.** Over 240 seat-games with a random
genome per seat, ranking seats by end-of-game share:

| proxy | AUC |
|---|---|
| **score share** | **0.949** |
| civic share | 0.890 |
| population share | 0.887 |
| tech share | 0.860 |
| city share | 0.854 |
| military power | 0.755 |
| gold | 0.714 |
| faith | 0.611 |

Score share is very nearly a perfect predictor of who won, and the best of the
eight. So "score is a bad proxy" was never the right description.

**Second, the convexity hypothesis — and its refutation.** I proposed that the
defect was the *mean*: winning is a threshold (be first), and the mean of a
threshold quantity rewards padding games already won or lost. The prediction
was that an increasingly convex statistic would report parity for
`settler_min_pop = 5`, whose wins answer is known to be parity. Measured over
60 mirrored maps:

| statistic | edge |
|---|---|
| mean share | +0.0107 ± 0.0086 |
| share² | +0.0155 ± 0.0134 |
| share⁴ | +0.0164 ± 0.0188 |
| **top of table** | **+0.0167 ± 0.0290** |

**Refuted.** The edge does not shrink; it grows slightly. Even top-of-table —
the strictest be-first statistic there is — reports +0.0167 where wins report
parity. So score is invalid as a selection signal at **every** convexity, and
the functional was never the problem.

**What actually reconciles AUC 0.949 with parity on wins.** The AUC is
**observational**: strong empires both score well and win, so score ranks them
correctly. The A/B is an **intervention**: it moves score without moving wins.
The gap between the two is exactly the correlation-versus-causation
distinction, measured on this engine.

That is `docs/SUPERHUMAN.md` §0 one level up. A state-value net encodes
correlation, and an argmax over actions optimises whichever correlate is
cheapest to move — `policy_wide` found the contact terms and went to −313 Elo.
Here a **selection statistic** encodes correlation, and a GA optimises
whichever correlate is cheapest to move. Score share is that correlate. The
mechanism is identical; only the layer differs.

**Consequence.** No functional of terminal score is a valid fitness. Selection
has to read wins, or something causally upstream of a victory condition —
lane progress, not economy — and wins cost about 865 games per genome. That is
why breeding does not work here, stated as a mechanism rather than as a
tally of nulls.

## ★★★ A fitness that survives both tests: lane progress

The causation result predicted that a statistic causally **upstream of a
victory condition** would survive the intervention test score fails. It does.
`victory_threat` — the empire's progress along its best enabled victory lane —
is the only candidate that passes both halves:

| statistic | AUC (observational) | edge on a change whose wins answer is parity |
|---|---|---|
| **lane progress** | **0.997** | **+0.0031 (0.2 SE)** ✓ |
| score share | 0.949 | +0.0107 (1.2 SE) ✗ |
| share² | — | +0.0155 ✗ |
| share⁴ | — | +0.0164 ✗ |
| top of table | — | +0.0167 ✗ |

Both halves are needed and neither alone would do. **Parity under intervention
is necessary but not sufficient** — a statistic that measures nothing also
reports parity, which is why the AUC census had to run before any of this was
believed. And **AUC alone is not sufficient either** — score has 0.949 and
still fails the intervention.

### The honest caveat on AUC 0.997

That number is **partly tautological**. The game ends when a victory condition
completes, so at game end the winner has, by construction, the highest
victory-condition progress. As a classifier it is close to circular.

What is *not* tautological is that it is **continuous**. A seat that reached
90% of a religious victory outranks one at 10%, though neither won. That is
information about every game which a binary win indicator discards, and it is
the entire reason this is cheaper than measuring wins:

| statistic | SE | games |
|---|---|---|
| lane progress | 0.0146 | 60 map-pairs |
| win rate | 0.0456 | 120 games |

Roughly a **10× variance advantage**, i.e. about ten times fewer games for the
same resolution.

### What this means for breeding

`docs/RATING.md` and every result above say the genome cannot be bred because
selection either has no signal or reads a correlate. Lane progress is the
first candidate that is **cheap, discriminating, and causally aligned** at
once. It does not make the genome worth breeding — the leverage ranking says
only `economy` carries anything, and every value tested is already at or near
its optimum — but it removes the *methodological* obstacle, and it is the
right fitness for any future search on this engine, including searches over
things that are not the genome.

### Validated against a known positive, not just a known null

One agreement on one null is not evidence — a statistic that measures nothing
passes it for free. The discriminating case is `legacy` deck against **no cards
at all**, whose wins answer is strongly positive (23 map directions to 6,
p=0.0023). A valid fitness must report parity on the first *and* a clear
positive on the second.

| statistic | `settler_min_pop=5` (wins: **parity**) | `legacy vs empty` (wins: **positive**) |
|---|---|---|
| mean share | +0.0107 (1.2 SE) | +0.0262 (3.1 SE) |
| share² | +0.0155 (1.2 SE) | +0.0480 (3.6 SE) |
| share⁴ | +0.0164 (0.9 SE) | +0.0747 (4.0 SE) |
| top of table | +0.0167 (0.6 SE) | +0.0917 (2.8 SE) |
| **lane progress** | **+0.0031 (0.2 SE)** | **+0.0280 (1.7 SE)** |

**Lane progress is the only statistic whose reading on the known null is
statistically indistinguishable from zero while still reporting a positive on
the known positive.** Its separation between the two cases is 9.0×, against
2.4× for mean score share.

The score functionals are not useless — `share⁴` and `top of table` do point
the right way on both — but every one of them reports a false positive of
+0.011 to +0.017 on a change that wins nothing, which is exactly the error
that would drive a search toward padding.

**Still only two test cases.** Both come from this loop's own runs, and a third
independently-known change would strengthen it. But the two chosen are the
strongest available: one null and one significant positive, both decided on
wins at 120 mirrored maps.

## ★★★ The ranking redone on the fitness that tracks wins

The leverage ranking above used score share, which is a correlate. Redone with
`--lane` on victory-lane progress, same seed, same draws:

| block | genes | cost (wins-tracking) | |
|---|---|---|---|
| combat_value | 3 | +0.0318 ± 0.0268 | 1.2 SE |
| economy | 7 | +0.0057 ± 0.0135 | 0.4 SE |
| war_decl | 5 | +0.0023 ± 0.0023 | 1.0 SE |
| opening | 4 | +0.0008 ± 0.0118 | 0.1 SE |
| policy | 8 | +0.0006 ± 0.0229 | 0.0 SE |
| movement | 2 | −0.0076 ± 0.0160 | scrambling helped |
| doctrine | 11 | −0.0116 ± 0.0221 | scrambling helped |
| expansion | 4 | −0.0319 ± 0.0228 | 1.4 SE, scrambling helped |

**Not one block of the forty-eight genes is outside its interval.** Five costs
positive, three negative, all scattered around zero — which is what *nothing
matters* looks like.

Two things this retracts or sharpens:

- **`economy` is not load-bearing.** It fell from 3.2 SE on score to 0.4 SE on
  wins. That headline was an economy finding, exactly as its caveat said, and
  it does not survive on the statistic that tracks winning.
- **The genome conclusion no longer rests on a proxy.** The earlier version had
  to be read as "nothing moves the economy". This version reads "nothing moves
  winning", which is the claim that was wanted all along.

> **If getting a block wrong is free, getting it righter cannot pay.** A search
> over these genes — genetic or otherwise — is not the route to a stronger
> agent on this engine.

### A note on variance, against a likely misreading

Lane SEs here are 0.0135–0.0268 against score's 0.0060–0.0233, so **lane is not
tighter than score**. Its ~10× advantage is over a **binary win rate**
(SE 0.0456 over 120 games). The ordering is: score tightest but invalid, lane
in the middle and valid, wins loosest and valid. Lane is the cheapest *valid*
statistic, not the cheapest one.

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

## Which measurements here are strength evidence, and which are not

This distinction is load-bearing for reading anything above, so it is stated
plainly rather than left implicit.

**Decided on WINS — valid strength evidence:**

| comparison | result |
|---|---|
| live deck vs legacy | 18–15, p=0.7283 — null |
| legacy deck vs **empty** | 23–6, **p=0.0023** — the card layer matters |
| legacy vs legacy | exact parity, zero variance — harness self-check |
| `settler_min_pop` 5 vs 2 | 12–15, p=0.7011 — null |

**Decided on score share — economy evidence only:** the block leverage
ranking, every `--sweep`, the opening-book sweep and ablation, and the district
enumeration. These say what moves the economy. Given the `settler_min_pop`
result, they do **not** license a claim about strength, and the honest reading
of every one of them is "no effect on the economy either", which is a weaker
statement than it first appears.

The asymmetry is not a flaw in those runs; it is what they cost. A win-based
measurement of one gene value took 240 full games to reach p=0.70. Pricing all
48 genes that way is not affordable, which is the same wall the GA hits.

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
