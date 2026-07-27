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
outcome table is now restricted to full-depth sequences, and §4 checks the
proxy against measured survival rather than leaving it to stand.

## 4. ⚠ The elimination rate is my map, not the agent — retracted

The first version of this document led with "47.6% of major seats are out
before six capital builds", measured at 6 players on 24×16. Survival is now
tracked directly rather than inferred, and the death rate turns out to be a
function of how crowded I made the map:

| players × map | land tiles per seat | lost every city | mean death turn | cities @50 | peak cities |
|---|---|---|---|---|---|
| 6 × 24×16 | 64 | **50.1%** | 29 | 1.38 | 2.43 |
| 4 × 24×16 (the `ai_eval` convention) | 96 | 30.7% | 72 | 1.78 | 3.17 |
| 4 × 32×22 | 176 | 12.9% | 205 | 1.95 | 4.42 |

**Barbarians are not the cause**: with `--barbarians 0` at 6 players the rate is
51.8% at the same mean death turn 29. It is rivals, and it is density. Every
seat founds its capital — `0 never founded at all` in all six runs — so nothing
is losing its opening settler either.

**Read the 6-player row as a property of the configuration.** The repository's
own evaluation geometry is 4 players on 24×16, and any claim about elimination
should be made there or roomier.

## 5. What survives every configuration: the opening does not expand

`cities at turn 50` is **1.38 / 1.78 / 1.95** across the three rows above. Even
with 176 land tiles per seat and 87% of seats alive at the end, the agent holds
**fewer than two cities at turn 50** and peaks at **4.42 ever**.

That is not an accident of tuning, it is written down. `AdvancedAi::plan`
(`src/ai/advanced.rs:1431`):

```rust
let map_capacity = (2 + land / 55).clamp(3, 9);
let city_cadence = g.standard_duration(90).max(1) as usize;
let desired_cities = (3 + g.turn as usize / city_cadence)
    .min(map_capacity)
    .min(6);
```

So the target is **3 cities for the first 90 turns** and **6 for the rest of
the game, forever, on any map**. On 32×22 `map_capacity` is not even binding —
the `.min(6)` and the 90-turn cadence are. Two things follow, and they are
different problems:

1. **The target is low.** Competent Civilization VI play on a standard map is
   4–6 cities by turn 50 and well past 6 by the midgame. The `.min(6)` is a
   hard ceiling nothing can lift.
2. **The agent does not even reach its own target.** 1.95 cities at turn 50
   against a `desired_cities` of 3. That is a failure to execute the opening,
   not a policy choice, and it is the more interesting of the two.

### The mechanism for (2): one settler

Three cities takes two settlers out of the capital. Over 112 full-depth seats
at depth 8 on 32×22, the capital queues **1.72 ± 0.05** settlers in its first
eight builds, and the split matters:

| settlers in first 8 builds | seats | score share | wins | peak cities |
|---|---|---|---|---|
| 1 | 36 | 0.2320 ± 0.0223 | 5 (13.9%) | 4.31 |
| 2 | 71 | 0.2661 ± 0.0162 | **22 (31.0%)** | 4.70 |

The same signal appears in the six-slot table in §2: of the six openings that
reached six seats, exactly one contains a second settler
(`… > galley > settler`), and it holds 3 wins in 7 seats where the other five
hold 0–1 in 7–13.

⚠ **Correlational, and the obvious confound is the strong one.** A seat that
builds two settlers is a seat with room and safety, so the start is inside both
columns. This is a hypothesis with a mechanism attached, not a result.

## 6. Why the second settler is missing: expansion is serialized empire-wide

The production gate for a settler (`src/ai/advanced.rs:6699`):

```rust
if city_count + counts.settlers < plan.desired_cities
    && counts.settlers == 0
    && city.pop >= 2
    && expansion_open
    && site.is_some()
```

`counts` is an `EmpireCounts`, so `counts.settlers == 0` means **at most one
settler may exist in the whole empire at a time**. The first clause already
caps cities-plus-settlers at the target, so this one adds no cap — it is purely
a serialization constraint. A four-city empire expands no faster than a
one-city empire: build a settler, walk it, found, and only then may the next
one start.

**Fires-check — the founding cadence, 60 maps, 4 players, 32×22:**

| N cities | seats | mean turn first held | gap |
|---|---|---|---|
| 2 | 232 | 37.0 ± 1.3 | |
| 3 | 195 | 71.0 ± 2.2 | +34.0 |
| 4 | 157 | 89.5 ± 2.5 | +18.5 |
| 5 | 87 | 118.7 ± 4.5 | +29.2 |
| 6 | 50 | 150.2 ± 7.3 | +31.5 |

**The gaps do not shrink.** A five-city empire adds its sixth as slowly as a
two-city empire added its third, which is what serialization looks like and is
not what a compounding expansion looks like. Expansion is supposed to be the
one thing in the game that accelerates.

And the blocked time is measured directly: a seat spends **60.8 ± 3.8 turns**
short of its city target *with a settler already walking* — time in which
`counts.settlers == 0` forbids starting another — against **68.5 ± 4.2 turns**
short with no settler anywhere, which that clause does not explain (site
availability, `pop >= 2`, the expansion window, or simply losing the production
argmax to something else).

So roughly **half** of all time-below-target is attributable to this one
clause, and the other half needs its own diagnosis.

## What to measure next, in order

1. **Relax `counts.settlers == 0`, causally.** Allow settlers in flight up to
   the shortfall (the outer clause already caps the total), and run a paired,
   seat-mirrored `ai_eval` on wins at the promotion gate's population. This is
   the first hypothesis in this line of work with a measured gap (1.95 cities
   against a target of 3), a named mechanism (§6), a passing fires-check (flat
   founding gaps) and a quantified ceiling (60.8 turns per seat). It touches
   `src/ai/advanced.rs`, which the version-control policy names a conflict
   hotspot — extend the ownership claim and check open PRs before editing.

   **What would refute it:** settlers cost population, so two in flight from a
   young empire may stall the cities producing them. If terminal score falls
   while city count rises, the extra cities are not paying for themselves and
   the clause was load-bearing.
2. **Lift `.min(6)`, separately.** Do not bundle it with (1): they are
   independent, and `docs/GENOME.md`'s rule about candidate-set changes moving
   effective thresholds applies to expansion targets too.
3. **Bound the civilization-aware channels by ablation, on wins.** Strip the
   unique-unit tech bonus and the unique-district preference and play it paired
   against stock. The 8.88-way divergence in §1 says the channel *fires*;
   reachability is not leverage.
4. **Only then, per-civilization openings.** If (3) returns null the way the
   opening book did, a `leader_trait`-aware opening is very likely null too.

Note the ordering has changed since the first draft: (1) and (2) are about
tempo and outranked the per-civilization work once the expansion numbers came
in. Openings do set the tempo here — just not by *which* six things get built.
