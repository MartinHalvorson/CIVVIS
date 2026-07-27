# Openings: what each civilization actually plays

The scripted opening has been bounded twice and the ordering layers around it
once. `docs/GENOME.md` records all three:

| layer | bound | source |
|---|---|---|
| opening book, swept | holdout `-0.0019 ± 0.0148` | `opening_sweep` |
| opening book, deleted entirely | `-0.003` | ablation |
| technology order, randomised | below the 0.09 resolution | `order_ablate` |
| civic order, randomised | below the 0.09 resolution | `order_ablate` |

Every one of those pooled all civilizations together and asked whether a
**global** opening could be improved. None of them asked the prior question:
**does the civilization change the opening at all, and if so, is the answer any
good?** `src/bin/opening_census.rs` is the instrument for that.

```text
opening_census --swap --maps 8 --players 6 --window 40    # does the civ matter?
opening_census --maps 60 --players 6 --turns 500          # what does it play?
```

## 1. The civilization does reach the opening — 8.88 of 21 distinct

`--swap` holds the map, the seed, the rivals **and the start tile** fixed,
overwrites one seat's civilization name, and replays the opening window. The
overwrite happens after construction, so Civilization VI's start bias is
deliberately given up: anything that diverges is the decision layer reading who
the seat is, and nothing else.

8 maps, seed 300000, 6 players, 24×16, window 40, depth 6, 21 civilizations:

| map | distinct openings |
|---|---|
| 300000 | 10 |
| 300001 | 4 |
| 300002 | 2 |
| 300003 | 14 |
| 300004 | 7 |
| 300005 | 6 |
| 300006 | 15 |
| 300007 | 13 |

**0 of 8 maps had every civilization open identically. Mean 8.88 distinct
openings out of 21.** The separations are visible in the sequences: Aztec picks
up `eagle_warrior`, Russia queues `[lavra]`, and a religious branch runs
`[holy_site] > shrine > project:holy_site_prayers` where the generic line runs
`monument > settler`.

### ⚠ A hypothesis this retracts

I predicted the opposite, from a static read: `Game::leader_trait`
(`src/game.rs:15439`) exposes the shipped preference-trait vector — over the 21
seated majors, `expansionist` ×6, `low_religious_preference` ×6,
`cultural_major_civ` ×5, `science_major_civ` ×5, `aggressive_military` ×4 and
six rarer ones — and it has **zero callers**. The planner's only by-name
civilization code is three string literals (`"Greece"` twice in
`victory_focus`, `"China"` once) plus a `+55` unique-unit bonus in tech
valuation. From that I expected a civilization-blind opening.

That was wrong, and it is the `gene_probe` lesson from `docs/GENOME.md` running
backwards: **a channel being unread does not mean the signal is absent**, it
means the signal arrives some other way. Here it arrives through *mechanics* —
which unit and district the civilization can build at all — rather than through
preference. `leader_trait` is still unread, and that remains a real unexploited
axis, but it is not the reason openings differ.

## 2. Among survivors the opening is nearly a constant

60 maps × 6 players, 500 turns, `randomize_civs`, window 50, depth 6.

Seating had to be fixed before this mode meant anything: `seat_civs` hands the
stock roster out in seat order and `randomize_civs` defaults off, so every map
seated Rome at seat 0 and a per-civilization table would really have been a
per-seat table.

- 3 of 360 seats never held a capital inside the window.
- **116 distinct build sequences over 357 seats.**
- Second city: mean turn **20–34** by civilization. Cities at turn 50: **≈1.4**.

Every opening that reached six seats shares one stem:

| seats | wins | score share | opening |
|---|---|---|---|
| 8 | 1 | 0.3481 ± 0.0508 | warrior > settler > builder > monument > **[holy_site] > shrine** |
| 7 | 0 | 0.2436 ± 0.0490 | warrior > settler > builder > monument > **slinger > trader** |
| 7 | 1 | 0.2226 ± 0.0424 | warrior > settler > builder > monument > **trader > slinger** |
| 13 | 0 | 0.2006 ± 0.0370 | warrior > **builder > monument > settler** > trader > slinger |
| 7 | 3 | 0.1790 ± 0.0468 | warrior > settler > builder > monument > **galley > settler** |
| 9 | 0 | 0.1589 ± 0.0250 | warrior > settler > builder > monument > **galley > trader** |

Five of the six are `warrior > settler > builder > monument` and differ only in
slots five and six. **The 8.88-way divergence the swap probe measures is not
what the surviving population plays.** It is mostly the difference between the
generic line and branches taken by seats that go on to do badly or die.

⚠ **This table is correlational and the start is inside both columns.** It
describes the population; it does not say an opening wins. The causal
instrument is a paired `ai_eval` against a seat forced onto the candidate
opening, the way `opening_sweep`'s holdout is.

## 3. The confound that had to be removed, and the finding inside it

The first run of this table ranked `warrior` (a one-item sequence) and
`warrior > builder` at the bottom with score shares of 0.0005 and 0.0024 over
90 and 43 seats. Those are not openings. **Sequence length is a proxy for early
survival**: a seat whose capital falls on turn 12 records two builds and then
scores nothing, so pooling short sequences in ranks openings by how long their
owner lived.

Measured directly:

> seats recording all 6 builds score **0.3007 ± 0.0121** (187 seats); seats
> recording fewer score **0.0222 ± 0.0062** (170 seats).

A 13× gap, far larger than any difference *between* openings (0.16–0.35). The
outcome table is now restricted to full-depth sequences.

The confound is itself the biggest number in this document: **47.6% of major
seats are effectively out of the game before they finish six capital builds**,
and the average survivor holds **1.4 cities at turn 50** with its second city
arriving around turn 25. Whatever the opening does or does not choose well, the
population it is choosing for is half-eliminated early and expanding at roughly
a third of competent Civilization VI tempo.

## What to measure next, in order

1. **Bound the civilization-aware channels by ablation, on wins.** Strip the
   unique-unit tech bonus and unique-district preference and play it paired
   against stock. `docs/GENOME.md`'s rule is that a null on selection is
   uninterpretable without the ceiling beside it — the 8.88-way divergence says
   the channel *fires*, and reachability is not leverage.
2. **Ask why half the seats die.** This is a larger effect than anything in the
   opening tables and nothing in `docs/AI_GAPS.md` names it. Separate "died to
   a rival", "died to barbarians" and "never expanded" before proposing a fix.
3. **Only then, per-civilization openings.** If (1) returns null the way the
   opening book did, a `leader_trait`-aware opening is very likely null too,
   and the honest conclusion is that the opening is not where this agent's
   strength is.
