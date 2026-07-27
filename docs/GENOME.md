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

## Method rules these runs paid for

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
